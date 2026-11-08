//! The model client: every request the app makes to a model goes through
//! [`Llm`], so `AEVAL_FAKE_LLM` can answer all of them from fixtures and
//! `AEVAL_OFFLINE` can refuse all of them (PARITY MODEL-01, MODEL-05,
//! MODEL-08, PROMPT-01, PROMPT-04, PROMPT-05). Port of the request half of
//! lib/openrouter.js, with the native providers behind the same calls.

use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::catalogue::{self, Kind};
use super::errors::{self, ApiError};
use super::failover::BadModels;
use super::fake::Fake;
use super::prompt::{self, ExactText, SkillText, SystemPromptOpts};
use super::provider::Provider;
use super::settings::Active;
use super::sse::{data_payload, LineBuffer, Meter};
use super::wire::{self, HttpRequest, Method, StreamDecoder, Turn};

pub type BodyStream = Pin<Box<dyn Stream<Item = Result<Vec<u8>, ApiError>> + Send>>;

/// An HTTP reply whose body arrives in chunks.
pub struct Reply {
    pub status: u16,
    /// The `Retry-After` header, as sent.
    pub retry_after: Option<String>,
    pub body: BodyStream,
}

impl Reply {
    /// A reply whose whole body is already known (fixtures, tests).
    pub fn full(status: u16, body: impl Into<Vec<u8>>) -> Reply {
        let bytes: Vec<u8> = body.into();
        Reply {
            status,
            retry_after: None,
            body: Box::pin(futures_util::stream::iter(vec![Ok(bytes)])),
        }
    }

    pub fn with_retry_after(mut self, value: Option<String>) -> Reply {
        self.retry_after = value;
        self
    }

    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The whole body, or an abort if `cancel` fires first.
    pub async fn bytes(mut self, cancel: &CancellationToken) -> Result<Vec<u8>, ApiError> {
        let mut out = Vec::new();
        while let Some(chunk) = next_chunk(&mut self.body, cancel, None).await? {
            out.extend(chunk);
        }
        Ok(out)
    }
}

/// The next chunk of a body. With `idle`, a body that sends nothing at all
/// for that long (keep-alive comments count) is a 408, so failover moves
/// on instead of spinning forever (B-041).
async fn next_chunk(
    body: &mut BodyStream,
    cancel: &CancellationToken,
    idle: Option<Duration>,
) -> Result<Option<Vec<u8>>, ApiError> {
    let Some(wait) = idle else {
        return tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(ApiError::abort()),
            chunk = body.next() => chunk.transpose(),
        };
    };
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(ApiError::abort()),
        chunk = tokio::time::timeout(wait, body.next()) => match chunk {
            Ok(chunk) => chunk.transpose(),
            Err(_) => Err(stalled(wait)),
        },
    }
}

fn stalled(wait: Duration) -> ApiError {
    ApiError::http(408, format!("no data for {} s", wait.as_secs()))
}

/// Sends one HTTP request. The real one is reqwest; tests and the fake LLM
/// hand back canned replies.
#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&self, req: HttpRequest, cancel: &CancellationToken) -> Result<Reply, ApiError>;
}

pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    pub fn new() -> Self {
        // A dead route fails in 10 s instead of the OS's minutes, and the
        // keep-alive notices a connection that vanished mid-answer.
        let client = reqwest::Client::builder()
            .user_agent(concat!("AevalBuddy/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        ReqwestTransport { client }
    }
}

impl Default for ReqwestTransport {
    fn default() -> Self {
        Self::new()
    }
}

fn net_error(e: &reqwest::Error) -> ApiError {
    if e.is_timeout() {
        ApiError::other(format!("timeout: {e}"))
    } else {
        // fetch's TypeError: the connection, not the server, failed.
        ApiError::network(format!("fetch failed: {e}"))
    }
}

#[async_trait]
impl Transport for ReqwestTransport {
    async fn send(&self, req: HttpRequest, cancel: &CancellationToken) -> Result<Reply, ApiError> {
        let mut rb = match req.method {
            Method::Get => self.client.get(&req.url),
            Method::Post => self.client.post(&req.url),
        };
        for (k, v) in &req.headers {
            rb = rb.header(k, v);
        }
        if let Some(body) = &req.body {
            rb = rb.body(body.to_string());
        }
        if let Some(t) = req.timeout {
            rb = rb.timeout(t);
        }
        let resp = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(ApiError::abort()),
            r = rb.send() => r.map_err(|e| net_error(&e))?,
        };
        let status = resp.status().as_u16();
        let retry_after = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = resp
            .bytes_stream()
            .map(|r| r.map(|b| b.to_vec()).map_err(|e| net_error(&e)));
        Ok(Reply {
            status,
            retry_after,
            body: Box::pin(body),
        })
    }
}

/// What a request is for, read only by the fixtures (`meta` in JS).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub purpose: String,
    pub query: Option<String>,
    pub task: Option<String>,
}

impl Meta {
    pub fn purpose(purpose: &str) -> Meta {
        Meta {
            purpose: purpose.to_string(),
            ..Meta::default()
        }
    }
}

/// One earlier turn of the conversation, text only.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatTurn {
    pub role: String,
    pub text: String,
    /// `trail` for an agent run's collapsed rows.
    pub kind: Option<String>,
}

/// The newest message and what travels with it (`streamQuery` options).
#[derive(Clone, Copy, Debug, Default)]
pub struct QueryOpts<'a> {
    pub model: &'a str,
    pub question: &'a str,
    pub image_base64: Option<&'a str>,
    pub image_width: u32,
    pub image_height: u32,
    pub history: &'a [ChatTurn],
    pub skill: Option<&'a SkillText>,
    pub screen_report: Option<&'a str>,
    pub via_region: bool,
    pub exact_text: Option<&'a ExactText>,
}

/// The reader stage's inputs (`readScreen` options).
#[derive(Clone, Copy, Debug, Default)]
pub struct ReaderOpts<'a> {
    pub model: &'a str,
    pub image_base64: &'a str,
    pub question: &'a str,
    pub region: bool,
    pub exact_text: Option<&'a ExactText>,
}

/// `settings:test-key`'s answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyCheck {
    pub ok: bool,
    pub message: String,
}

impl KeyCheck {
    fn new(ok: bool, message: impl Into<String>) -> KeyCheck {
        KeyCheck {
            ok,
            message: message.into(),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({ "ok": self.ok, "message": self.message })
    }
}

const CATALOGUE_TTL: Duration = Duration::from_secs(5 * 60);

/// A stream that sends nothing for this long has stalled (B-041).
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(45);

/// The longest a non-streamed call may take, headers to last byte.
pub const OVERALL_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Default)]
struct CatalogueCache {
    at: Option<Instant>,
    data: Option<Arc<Vec<Value>>>,
}

/// The model client.
pub struct Llm {
    transport: Arc<dyn Transport>,
    fake: Option<Arc<Fake>>,
    offline: Option<bool>,
    catalogue: Mutex<CatalogueCache>,
    bad: BadModels,
    idle: Duration,
    overall: Duration,
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn data_url(base64: &str) -> String {
    format!("data:image/jpeg;base64,{base64}")
}

impl Llm {
    /// The client the app runs with: reqwest, plus the fixtures when
    /// `AEVAL_FAKE_LLM` is set.
    pub fn from_env() -> Llm {
        let fake = std::env::var_os("AEVAL_FAKE_LLM")
            .filter(|v| !v.is_empty())
            .map(|_| Arc::new(Fake::load()));
        let mut llm = Llm::new(Arc::new(ReqwestTransport::new()), fake);
        // The self-test shortens the stall timeout so a stalled fixture
        // fails over in seconds.
        let idle = std::env::var("AEVAL_LLM_IDLE_MS").ok();
        if let Some(ms) = idle.and_then(|v| v.trim().parse::<u64>().ok()) {
            llm.set_timeouts(Duration::from_millis(ms), OVERALL_TIMEOUT);
        }
        llm
    }

    pub fn new(transport: Arc<dyn Transport>, fake: Option<Arc<Fake>>) -> Llm {
        Llm {
            transport,
            fake,
            offline: None,
            catalogue: Mutex::new(CatalogueCache::default()),
            bad: BadModels::default(),
            idle: IDLE_TIMEOUT,
            overall: OVERALL_TIMEOUT,
        }
    }

    /// The stall timeout for streams and the cap for whole calls.
    pub fn set_timeouts(&mut self, idle: Duration, overall: Duration) {
        self.idle = idle;
        self.overall = overall;
    }

    /// Forces the offline state regardless of `AEVAL_OFFLINE` (tests).
    // Test seam.
    #[allow(dead_code)]
    pub fn set_offline(&mut self, offline: bool) {
        self.offline = Some(offline);
    }

    // Test seam.
    #[allow(dead_code)]
    pub fn fake(&self) -> Option<&Arc<Fake>> {
        self.fake.as_ref()
    }

    /// `AEVAL_OFFLINE=1` makes every network call fail the way a dropped
    /// connection does.
    pub fn forced_offline(&self) -> bool {
        self.offline
            .unwrap_or_else(|| std::env::var_os("AEVAL_OFFLINE").is_some_and(|v| !v.is_empty()))
    }

    /// Every chat request goes through here. Returns a 2xx reply and the
    /// dialect to read it in (fixtures always answer in OpenRouter's); any
    /// other status comes back as its error, with `Retry-After` read and a
    /// retired model remembered.
    async fn request(
        &self,
        active: &Active,
        body: &Value,
        cancel: &CancellationToken,
        meta: &Meta,
    ) -> Result<(Reply, Provider), ApiError> {
        if self.forced_offline() {
            return Err(ApiError::network("fetch failed (offline)"));
        }
        // A stream sends its headers at once; a whole reply only when done.
        let stream = body["stream"].as_bool().unwrap_or(false);
        let wait = if stream { self.idle } else { self.overall };
        let sent = async {
            match &self.fake {
                Some(fake) => {
                    let reply = fake.respond(active.provider, body, cancel, meta).await;
                    reply.map(|r| (r, Provider::OpenRouter))
                }
                None => {
                    let req = wire::chat_request(active.provider, &active.key, body);
                    let reply = self.transport.send(req, cancel).await;
                    reply.map(|r| (r, active.provider))
                }
            }
        };
        let (reply, dialect) = match tokio::time::timeout(wait, sent).await {
            Ok(r) => r?,
            Err(_) => return Err(stalled(wait)),
        };
        if reply.ok() {
            return Ok((reply, dialect));
        }
        let err = Self::fail_from(reply, cancel).await;
        let model = body["model"].as_str().unwrap_or_default();
        let policy = errors::code_of(active.provider, &err, model) == errors::codes::DATA_POLICY;
        if err.status == Some(404) && !policy && !model.is_empty() {
            self.bad.mark(model);
        }
        Err(err)
    }

    async fn fail_from(reply: Reply, cancel: &CancellationToken) -> ApiError {
        let status = reply.status;
        let header = reply.retry_after.clone().unwrap_or_default();
        let wait = errors::parse_retry_after(&header, super::status::now_ms());
        let err = match reply.bytes(cancel).await {
            Ok(body) => wire::error_from_body(status, &body),
            Err(e) if e.is_abort() => return e,
            Err(_) => ApiError::http(status, format!("HTTP {status}")),
        };
        err.with_retry_after(wait)
    }

    /// A non-streamed call's JSON reply, within the overall cap. A 200 whose
    /// body is an `error` object is that error (B-125).
    async fn request_json(
        &self,
        active: &Active,
        body: &Value,
        cancel: &CancellationToken,
        meta: &Meta,
    ) -> Result<(Value, Provider), ApiError> {
        let whole = self.read_json(active, body, cancel, meta);
        match tokio::time::timeout(self.overall, whole).await {
            Ok(r) => r,
            Err(_) => Err(stalled(self.overall)),
        }
    }

    async fn read_json(
        &self,
        active: &Active,
        body: &Value,
        cancel: &CancellationToken,
        meta: &Meta,
    ) -> Result<(Value, Provider), ApiError> {
        let (reply, dialect) = self.request(active, body, cancel, meta).await?;
        let bytes = reply.bytes(cancel).await?;
        let data: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        if data.get("error").is_some_and(|e| !e.is_null()) {
            return Err(wire::stream_error(&data["error"]));
        }
        if let Some(e) = wire::refusal(dialect, &data) {
            return Err(e);
        }
        Ok((data, dialect))
    }

    /// Reads a chat stream, handing each content delta on. An empty stream
    /// is a 204 unless the caller allows it.
    #[allow(clippy::too_many_arguments)]
    async fn read_sse(
        reply: Reply,
        dialect: Provider,
        on_delta: &mut (dyn FnMut(&str) + Send),
        allow_empty: bool,
        meter: &mut Meter,
        cancel: &CancellationToken,
        idle: Duration,
    ) -> Result<(), ApiError> {
        if !reply.ok() {
            return Err(Self::fail_from(reply, cancel).await);
        }
        let mut body = reply.body;
        let mut lines = LineBuffer::default();
        let mut decoder = StreamDecoder::new(dialect);
        let mut got = false;
        'read: while let Some(chunk) = next_chunk(&mut body, cancel, Some(idle)).await? {
            for line in lines.push(&chunk)? {
                let Some(payload) = data_payload(&line) else {
                    continue;
                };
                let out = decoder.data(payload)?;
                if let Some(u) = &out.usage {
                    meter.take(u);
                }
                if let Some(text) = &out.text {
                    meter.first_token();
                    got = true;
                    on_delta(text);
                }
                if out.done {
                    break 'read;
                }
            }
        }
        if let Some(u) = decoder.finish() {
            meter.take(&u);
        }
        if !got && !allow_empty {
            return Err(ApiError::http(204, "empty response"));
        }
        Ok(())
    }

    /// The reader stage: one non-streamed call that returns a structured
    /// report of the screen, or a `reader`-tagged error.
    pub async fn read_screen(
        &self,
        active: &Active,
        o: ReaderOpts<'_>,
        cancel: &CancellationToken,
    ) -> Result<String, ApiError> {
        let exact = prompt::exact_text_block(o.exact_text);
        let system = if o.region {
            format!("{}{}", prompt::reader_prompt(), prompt::REGION_NOTE)
        } else {
            prompt::reader_prompt()
        };
        let lead = if exact.is_empty() {
            String::new()
        } else {
            format!("{exact}\n\n")
        };
        let text = format!("{lead}The user asked: {}\n\nReport the screen.", o.question);
        let body = json!({
            "model": o.model,
            "stream": false,
            "max_tokens": 1600,
            "temperature": 0,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": [
                    { "type": "image_url", "image_url": { "url": data_url(o.image_base64) } },
                    { "type": "text", "text": text },
                ] },
            ],
        });
        let meta = Meta::purpose("reader");
        let stage = |e: ApiError| e.with_stage("reader");
        let sent = self.request_json(active, &body, cancel, &meta).await;
        let (data, dialect) = sent.map_err(stage)?;
        let turn = wire::parse_turn(dialect, &data, now_ms());
        let report = prompt::unfence(turn.content.trim());
        if !prompt::valid_report(&report) {
            let msg = "the reader did not describe the screen";
            return Err(stage(ApiError::http(204, msg)));
        }
        Ok(report)
    }

    /// The `messages` array of one answer turn: system prompt, up to the
    /// caller's history as text, then the newest message with the screen.
    pub fn query_messages(o: &QueryOpts<'_>) -> Vec<Value> {
        let exact = prompt::exact_text_block(o.exact_text);
        let mut content = Vec::new();
        if !exact.is_empty() {
            content.push(json!({ "type": "text", "text": exact }));
        }
        if let Some(report) = o.screen_report {
            let text = format!("What is on the user's screen right now:\n\n{report}");
            content.push(json!({ "type": "text", "text": text }));
        } else if let Some(img) = o.image_base64 {
            content.push(json!({ "type": "image_url", "image_url": { "url": data_url(img) } }));
        }
        content.push(json!({ "type": "text", "text": o.question }));
        let sees = o.image_base64.is_some() || o.screen_report.is_some();
        let opts = SystemPromptOpts {
            has_image: sees && o.image_width > 0 && o.image_height > 0,
            via_reader: o.screen_report.is_some(),
            via_region: o.via_region,
            exact_text: !exact.is_empty(),
        };
        let system = prompt::make_system_prompt(opts, o.skill);
        let mut messages = vec![json!({ "role": "system", "content": system })];
        for m in crate::ask::sendable_history(o.history) {
            messages.push(json!({ "role": m.role, "content": m.text }));
        }
        messages.push(json!({ "role": "user", "content": content }));
        messages
    }

    /// Streams one conversation turn (temperature 0.2: the pointer digits are
    /// a reading task).
    pub async fn stream_query(
        &self,
        active: &Active,
        o: QueryOpts<'_>,
        cancel: &CancellationToken,
        on_delta: &mut (dyn FnMut(&str) + Send),
        meter: &mut Meter,
    ) -> Result<(), ApiError> {
        let body = json!({
            "model": o.model,
            "stream": true,
            "temperature": 0.2,
            "messages": Self::query_messages(&o),
        });
        let meta = Meta::purpose("answer");
        let (reply, dialect) = self.request(active, &body, cancel, &meta).await?;
        let idle = self.idle;
        Self::read_sse(reply, dialect, on_delta, false, meter, cancel, idle).await
    }

    /// A streamed text-only completion, for Find's two stages. Returns the
    /// whole text; `on_delta` sees it as it arrives.
    #[allow(clippy::too_many_arguments)]
    pub async fn stream_text(
        &self,
        active: &Active,
        model: &str,
        system: &str,
        user: &str,
        meta: &Meta,
        max_tokens: Option<u32>,
        cancel: &CancellationToken,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> Result<String, ApiError> {
        let body = json!({
            "model": model,
            "stream": true,
            "temperature": 0,
            "max_tokens": max_tokens.unwrap_or(800),
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
        });
        let (reply, dialect) = self.request(active, &body, cancel, meta).await?;
        let mut text = String::new();
        let mut meter = Meter::default();
        let mut collect = |d: &str| {
            text.push_str(d);
            on_delta(d);
        };
        let idle = self.idle;
        let read = Self::read_sse(reply, dialect, &mut collect, true, &mut meter, cancel, idle);
        read.await?;
        Ok(text)
    }

    /// One non-streamed agent turn. With `tools` the model may answer with
    /// native tool calls (one per turn); without them it answers in text.
    pub async fn chat_turn(
        &self,
        active: &Active,
        model: &str,
        messages: &[Value],
        tools: Option<&Value>,
        cancel: &CancellationToken,
        meta: &Meta,
    ) -> Result<Turn, ApiError> {
        let mut body = json!({
            "model": model,
            "stream": false,
            "temperature": 0,
            "messages": messages,
        });
        if let Some(t) = tools.filter(|t| t.as_array().is_some_and(|a| !a.is_empty())) {
            body["tools"] = t.clone();
            body["tool_choice"] = json!("auto");
            // One action per turn: the screen changes after each.
            body["parallel_tool_calls"] = json!(false);
        }
        let (data, dialect) = self.request_json(active, &body, cancel, meta).await?;
        Ok(wire::parse_turn(dialect, &data, now_ms()))
    }

    /// Checks a key and says what it can see, before the first question.
    pub async fn test_key(&self, provider: Provider, key: &str) -> KeyCheck {
        let key = key.trim();
        if key.is_empty() {
            return KeyCheck::new(false, "No key saved yet.");
        }
        let p = provider.name();
        let cancel = CancellationToken::new();
        let req = wire::key_request(provider, key);
        let reply = match self.transport.send(req, &cancel).await {
            Ok(r) => r,
            Err(_) => {
                let msg = format!("Could not reach {p}. Check your connection.");
                return KeyCheck::new(false, msg);
            }
        };
        let rejected = reply.status == 401
            || (provider.is_native() && reply.status == 403)
            || (provider == Provider::Gemini && reply.status == 400);
        if rejected {
            let msg = format!("{p} rejected this key. Check it and save again.");
            return KeyCheck::new(false, msg);
        }
        if !reply.ok() {
            return KeyCheck::new(false, format!("{p} replied HTTP {}.", reply.status));
        }
        let body = match reply.bytes(&cancel).await {
            Ok(b) => b,
            Err(_) => {
                let msg = format!("Could not reach {p}. Check your connection.");
                return KeyCheck::new(false, msg);
            }
        };
        if provider.is_native() {
            return KeyCheck::new(true, "Key works.");
        }
        let Ok(parsed) = serde_json::from_slice::<Value>(&body) else {
            return KeyCheck::new(false, "Could not reach OpenRouter. Check your connection.");
        };
        openrouter_credit(&parsed)
    }

    /// The live OpenRouter catalogue, or `None` when it cannot be fetched.
    /// Cached for five minutes so the pickers and the per-question check
    /// share one request.
    pub async fn fetch_catalogue(&self) -> Option<Arc<Vec<Value>>> {
        if self.forced_offline() {
            return None;
        }
        {
            let c = self.catalogue.lock();
            if let (Some(at), Some(data)) = (c.at, &c.data) {
                if at.elapsed() < CATALOGUE_TTL {
                    return Some(data.clone());
                }
            }
        }
        let req = HttpRequest {
            method: Method::Get,
            url: wire::OPENROUTER_MODELS.to_string(),
            headers: Vec::new(),
            body: None,
            timeout: Some(Duration::from_secs(8)),
        };
        let cancel = CancellationToken::new();
        let fetched = async {
            let reply = self.transport.send(req, &cancel).await.ok()?;
            if !reply.ok() {
                return None;
            }
            let bytes = reply.bytes(&cancel).await.ok()?;
            let body: Value = serde_json::from_slice(&bytes).ok()?;
            let all = body["data"].as_array().cloned().unwrap_or_default();
            (!all.is_empty()).then_some(all)
        };
        let all = tokio::time::timeout(Duration::from_secs(8), fetched)
            .await
            .ok()
            .flatten()?;
        let data = Arc::new(all);
        let mut c = self.catalogue.lock();
        c.at = Some(Instant::now());
        c.data = Some(data.clone());
        Some(data)
    }

    /// Whatever catalogue is cached; never waits.
    pub fn known_catalogue(&self) -> Option<Arc<Vec<Value>>> {
        self.catalogue.lock().data.clone()
    }

    /// `pickLive` against the live catalogue, fetching it if the cache is
    /// cold.
    pub async fn resolve_live(&self, kind: Kind, id: &str) -> String {
        let all = self.fetch_catalogue().await;
        catalogue::pick_live(kind, id, all.as_deref().map(Vec::as_slice))
    }

    /// `pickLive` against whatever is cached.
    pub fn resolve_known(&self, kind: Kind, id: &str) -> String {
        let all = self.known_catalogue();
        catalogue::pick_live(kind, id, all.as_deref().map(Vec::as_slice))
    }

    /// True when the model can take native tool calls. Every curated native
    /// model can; OpenRouter answers from the catalogue.
    pub fn supports_tools(&self, provider: Provider, id: &str) -> bool {
        // OpenAI's GPT-6 family takes tool calls only through the Responses
        // API (Chat Completions refuses them; checked 2026-11-07), so the
        // agent drives it with the one-JSON-object fallback instead.
        if provider == Provider::OpenAi {
            return false;
        }
        if provider.is_native() {
            return true;
        }
        let all = self.known_catalogue();
        catalogue::supports_tools(all.as_deref().map(Vec::as_slice), id)
    }

    /// True when the model takes images.
    pub fn sees_images(&self, provider: Provider, id: &str) -> bool {
        if provider.is_native() {
            return true;
        }
        let all = self.known_catalogue();
        catalogue::sees_images(all.as_deref().map(Vec::as_slice), id)
    }

    /// The next free stand-in: untried, not retired this session, and (once
    /// the catalogue is known) listed and able to do the job (B-042).
    pub fn next_free_model(&self, kind: Kind, tried: &[String]) -> Option<String> {
        let all = self.known_catalogue();
        let skip = self.bad.skip(tried);
        catalogue::next_free_model(kind, &skip, all.as_deref().map(Vec::as_slice))
    }

    /// The model to ask first: `id`, unless it is a free model that already
    /// answered 404 this session.
    pub fn first_model(&self, kind: Kind, id: &str) -> String {
        self.bad.first(id, |skip| self.next_free_model(kind, skip))
    }

    /// The picker for one provider and job (`listModels`).
    pub async fn list_models(&self, provider: Provider, kind: Kind, include_paid: bool) -> Value {
        if provider.is_native() {
            let models: Vec<Value> = provider
                .models(kind)
                .iter()
                .map(|c| {
                    let mut m = json!({
                        "id": c.id,
                        "name": c.id,
                        "note": c.note,
                        "tier": "paid",
                        "detail": "",
                    });
                    if kind == Kind::Agent {
                        m["tag"] = json!("Tools");
                        m["tools"] = json!(true);
                    }
                    m
                })
                .collect();
            let label = format!("{} (your key)", provider.name());
            let group = json!({ "tier": "paid", "label": label, "models": models });
            return json!({ "groups": [group], "live": false, "provider": provider.id() });
        }
        let all = self.fetch_catalogue().await;
        catalogue::list_models(kind, include_paid, all.as_deref().map(Vec::as_slice))
    }
}

/// The OpenRouter `/key` reply as one sentence about the credit left.
pub fn openrouter_credit(body: &Value) -> KeyCheck {
    let d = &body["data"];
    let limit = catalogue::js_number(d.get("limit").unwrap_or(&Value::Bool(true)));
    let used = catalogue::js_number(d.get("usage").unwrap_or(&Value::Bool(true)));
    let has = |k: &str| d.get(k).is_some();
    if has("limit") && has("usage") && limit.is_finite() && used.is_finite() {
        let left = (limit - used).max(0.0);
        if left > 0.0 {
            return KeyCheck::new(true, format!("Key works. About ${left:.2} of credit left."));
        }
        return KeyCheck::new(
            true,
            "Key works, but the credit is used up. Add credit, or pick a Free model.",
        );
    }
    // No limit set means a pay-as-you-go account whose balance OpenRouter
    // does not report here; the key itself is confirmed good.
    KeyCheck::new(true, "Key works.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// Canned replies, and every request that was sent.
    #[derive(Default)]
    pub struct Canned {
        pub replies: Mutex<VecDeque<Result<(u16, String), ApiError>>>,
        pub sent: Mutex<Vec<HttpRequest>>,
    }

    impl Canned {
        pub fn with(replies: Vec<Result<(u16, String), ApiError>>) -> Arc<Canned> {
            Arc::new(Canned {
                replies: Mutex::new(replies.into()),
                sent: Mutex::new(Vec::new()),
            })
        }
    }

    #[async_trait]
    impl Transport for Canned {
        async fn send(&self, req: HttpRequest, _: &CancellationToken) -> Result<Reply, ApiError> {
            self.sent.lock().push(req);
            let next = self.replies.lock().pop_front();
            let (status, body) = next.unwrap_or_else(|| Ok((500, String::new())))?;
            Ok(Reply::full(status, body))
        }
    }

    fn block<T>(f: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(f)
    }

    fn client(replies: Vec<Result<(u16, String), ApiError>>) -> (Llm, Arc<Canned>) {
        let canned = Canned::with(replies);
        let mut llm = Llm::new(canned.clone(), None);
        llm.set_offline(false);
        (llm, canned)
    }

    fn or_key() -> Active {
        Active {
            provider: Provider::OpenRouter,
            key: "sk-or-k".into(),
        }
    }

    fn sse(chunks: &[&str]) -> String {
        let mut s = String::new();
        for c in chunks {
            let line = json!({ "choices": [{ "delta": { "content": c } }] });
            s.push_str(&format!(": OPENROUTER PROCESSING\n\ndata: {line}\n\n"));
        }
        s.push_str("data: [DONE]\n\n");
        s
    }

    #[test]
    fn test_key_messages() {
        let cases = [
            (
                Ok((401, String::new())),
                "OpenRouter rejected this key. Check it and save again.",
            ),
            (Ok((500, String::new())), "OpenRouter replied HTTP 500."),
            (
                Ok((200, r#"{"data":{"limit":10,"usage":2.5}}"#.into())),
                "Key works. About $7.50 of credit left.",
            ),
            (
                Ok((200, r#"{"data":{"limit":1,"usage":3}}"#.into())),
                "Key works, but the credit is used up. Add credit, or pick a Free model.",
            ),
            (Ok((200, r#"{"data":{"limit":null}}"#.into())), "Key works."),
            (
                Err(ApiError::network("down")),
                "Could not reach OpenRouter. Check your connection.",
            ),
        ];
        for (reply, want) in cases {
            let (llm, canned) = client(vec![reply]);
            let got = block(llm.test_key(Provider::OpenRouter, " sk-or-k "));
            assert_eq!(got.message, want);
            assert_eq!(canned.sent.lock()[0].url, wire::OPENROUTER_KEY);
        }
        let (llm, _) = client(vec![]);
        let got = block(llm.test_key(Provider::OpenRouter, "  "));
        assert_eq!(got, KeyCheck::new(false, "No key saved yet."));
        let (llm, canned) = client(vec![Ok((200, "{}".into()))]);
        let got = block(llm.test_key(Provider::Anthropic, "sk-ant-k"));
        assert_eq!(got, KeyCheck::new(true, "Key works."));
        assert_eq!(canned.sent.lock()[0].url, wire::ANTHROPIC_MODELS);
        let (llm, _) = client(vec![Ok((400, "{}".into()))]);
        let got = block(llm.test_key(Provider::Gemini, "AIzaX"));
        let want = "Google Gemini rejected this key. Check it and save again.";
        assert_eq!(got.message, want);
    }

    #[test]
    fn stream_query_sends_the_screen_and_streams_text() {
        let (llm, canned) = client(vec![Ok((200, sse(&["Hel", "lo"])))]);
        let history = vec![
            ChatTurn {
                role: "user".into(),
                text: "before".into(),
                kind: None,
            },
            ChatTurn {
                role: "assistant".into(),
                text: String::new(),
                kind: None,
            },
        ];
        let o = QueryOpts {
            model: "m:free",
            question: "what now?",
            image_base64: Some("QUJD"),
            image_width: 1600,
            image_height: 900,
            history: &history,
            ..QueryOpts::default()
        };
        let mut out = String::new();
        let mut meter = Meter::default();
        let cancel = CancellationToken::new();
        let mut push = |d: &str| out.push_str(d);
        let (k, c) = (or_key(), &cancel);
        block(llm.stream_query(&k, o, c, &mut push, &mut meter)).unwrap();
        assert_eq!(out, "Hello");
        let sent = canned.sent.lock()[0].body.clone().unwrap();
        assert_eq!(sent["temperature"], json!(0.2));
        assert_eq!(sent["usage"], json!({ "include": true }));
        let msgs = sent["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[1]["content"], json!("before"));
        let url = &msgs[2]["content"][0]["image_url"]["url"];
        assert_eq!(url, &json!("data:image/jpeg;base64,QUJD"));
        assert_eq!(msgs[2]["content"][1]["text"], json!("what now?"));
    }

    #[test]
    fn empty_streams_and_http_errors() {
        let (llm, _) = client(vec![
            Ok((200, "data: [DONE]\n\n".into())),
            Ok((429, r#"{"error":{"message":"Rate limit"}}"#.into())),
        ]);
        let o = QueryOpts {
            model: "m",
            question: "q",
            ..QueryOpts::default()
        };
        let cancel = CancellationToken::new();
        let mut meter = Meter::default();
        let mut sink = |_: &str| {};
        let (k, c) = (or_key(), &cancel);
        let e = block(llm.stream_query(&k, o, c, &mut sink, &mut meter));
        assert_eq!(e.unwrap_err(), ApiError::http(204, "empty response"));
        let e = block(llm.stream_query(&k, o, c, &mut sink, &mut meter));
        assert_eq!(e.unwrap_err(), ApiError::http(429, "Rate limit"));
    }

    #[test]
    fn reader_rejects_prose_and_tags_its_errors() {
        let good = json!({ "choices": [{ "message": { "content":
            "```\nSCREEN: Notepad - notes.txt\nSTATE: idle\nTASK: assist\nTEXT: NONE\n```" } }] });
        let prose = json!({ "choices": [{ "message": { "content": "It is Notepad." } }] });
        let (llm, canned) = client(vec![
            Ok((200, good.to_string())),
            Ok((200, prose.to_string())),
            Ok((404, r#"{"error":{"message":"gone"}}"#.into())),
        ]);
        let o = ReaderOpts {
            model: "v:free",
            image_base64: "QUJD",
            question: "q",
            region: true,
            exact_text: None,
        };
        let cancel = CancellationToken::new();
        let r = block(llm.read_screen(&or_key(), o, &cancel)).unwrap();
        assert!(r.starts_with("SCREEN: Notepad"));
        let sent = canned.sent.lock()[0].body.clone().unwrap();
        assert_eq!(sent["max_tokens"], json!(1600));
        let system = sent["messages"][0]["content"].as_str().unwrap();
        let tail = "so rather than assuming what the rest of it said.";
        assert!(system.ends_with(tail));
        let e = block(llm.read_screen(&or_key(), o, &cancel)).unwrap_err();
        assert_eq!((e.status, e.stage), (Some(204), Some("reader")));
        let e = block(llm.read_screen(&or_key(), o, &cancel)).unwrap_err();
        assert_eq!((e.status, e.stage), (Some(404), Some("reader")));
    }

    /// Sends the headers, then nothing.
    struct Stalled;

    #[async_trait]
    impl Transport for Stalled {
        async fn send(&self, _: HttpRequest, _: &CancellationToken) -> Result<Reply, ApiError> {
            let body: BodyStream = Box::pin(futures_util::stream::pending());
            Ok(Reply {
                status: 200,
                retry_after: None,
                body,
            })
        }
    }

    #[test]
    fn idle_stream_times_out_as_408() {
        let mut llm = Llm::new(Arc::new(Stalled), None);
        llm.set_offline(false);
        llm.set_timeouts(Duration::from_millis(50), Duration::from_millis(80));
        let o = QueryOpts {
            model: "m:free",
            question: "q",
            ..QueryOpts::default()
        };
        let cancel = CancellationToken::new();
        let mut meter = Meter::default();
        let mut sink = |_: &str| {};
        let (k, c) = (or_key(), &cancel);
        let e = block(llm.stream_query(&k, o, c, &mut sink, &mut meter)).unwrap_err();
        assert_eq!(e.status, Some(408));
        assert!(crate::llm::errors::should_failover(&e));
        // A whole reply that never ends is capped too.
        let meta = Meta::default();
        let e = block(llm.chat_turn(&k, "m", &[], None, c, &meta)).unwrap_err();
        assert_eq!(e.status, Some(408));
    }

    #[test]
    fn error_in_200_body_is_an_error() {
        let body = r#"{"error":{"code":502,"message":"upstream failed"}}"#;
        let (llm, _) = client(vec![Ok((200, body.into()))]);
        let cancel = CancellationToken::new();
        let meta = Meta::purpose("agent");
        let turn = block(llm.chat_turn(&or_key(), "m", &[], None, &cancel, &meta));
        assert_eq!(turn.unwrap_err(), ApiError::http(502, "upstream failed"));
    }

    #[test]
    fn retry_after_and_retired_models_are_read() {
        let busy = Ok((429, r#"{"error":{"message":"busy"}}"#.to_string()));
        let gone = r#"{"error":{"message":"No endpoints found"}}"#;
        let gone = Ok((404, gone.to_string()));
        let canned = Canned::with(vec![busy, gone]);
        let mut llm = Llm::new(Arc::new(Headers(canned)), None);
        llm.set_offline(false);
        let cancel = CancellationToken::new();
        let meta = Meta::purpose("agent");
        let k = or_key();
        let first = "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free";
        let e = block(llm.chat_turn(&k, first, &[], None, &cancel, &meta)).unwrap_err();
        assert_eq!(e.retry_after_ms, Some(7_000));
        assert_eq!(llm.first_model(Kind::Vision, first), first);
        let e = block(llm.chat_turn(&k, first, &[], None, &cancel, &meta)).unwrap_err();
        assert_eq!(e.status, Some(404));
        // Retired this session: skipped as the first attempt and as a
        // stand-in.
        let stand_in = "google/gemma-4-26b-a4b-it:free";
        assert_eq!(llm.first_model(Kind::Vision, first), stand_in);
        let tried = [stand_in.to_string()];
        let next = llm.next_free_model(Kind::Vision, &tried);
        let want = "dots-studio/dots-3-note-preview:free";
        assert_eq!(next.as_deref(), Some(want));
    }

    /// Canned replies with `Retry-After: 7` on each.
    struct Headers(Arc<Canned>);

    #[async_trait]
    impl Transport for Headers {
        async fn send(&self, req: HttpRequest, c: &CancellationToken) -> Result<Reply, ApiError> {
            let reply = self.0.send(req, c).await?;
            Ok(reply.with_retry_after(Some("7".into())))
        }
    }

    #[test]
    fn offline_refuses_everything() {
        let (mut llm, canned) = client(vec![]);
        llm.set_offline(true);
        let cancel = CancellationToken::new();
        let (k, meta) = (or_key(), Meta::default());
        let e = block(llm.chat_turn(&k, "m", &[], None, &cancel, &meta));
        assert!(e.unwrap_err().is_network());
        assert!(block(llm.fetch_catalogue()).is_none());
        assert!(canned.sent.lock().is_empty());
    }

    #[test]
    fn catalogue_is_cached() {
        let body = r#"{"data":[{"id":"a:free"}]}"#.to_string();
        let (llm, canned) = client(vec![Ok((200, body))]);
        assert_eq!(block(llm.fetch_catalogue()).unwrap().len(), 1);
        assert_eq!(block(llm.fetch_catalogue()).unwrap().len(), 1);
        assert_eq!(canned.sent.lock().len(), 1);
        let (llm, _) = client(vec![Ok((200, r#"{"data":[]}"#.into()))]);
        assert!(block(llm.fetch_catalogue()).is_none());
    }

    #[test]
    fn native_turns_go_to_the_native_api() {
        let reply = json!({ "content": [{ "type": "tool_use", "id": "toolu_1", "name": "wait",
            "input": { "duration": 1 } }], "usage": { "input_tokens": 3, "output_tokens": 4 } });
        let (llm, canned) = client(vec![Ok((200, reply.to_string()))]);
        let active = Active {
            provider: Provider::Anthropic,
            key: "sk-ant-k".into(),
        };
        let tools = json!([{ "type": "function", "function": { "name": "wait",
            "description": "Wait.", "parameters": { "type": "object", "properties": {} } } }]);
        let msgs = vec![json!({ "role": "user", "content": "go" })];
        let cancel = CancellationToken::new();
        let meta = Meta::purpose("agent");
        let model = "claude-opus-5-5";
        let fut = llm.chat_turn(&active, model, &msgs, Some(&tools), &cancel, &meta);
        let turn = block(fut).unwrap();
        assert_eq!(turn.tool_calls[0].name, "wait");
        let sent = canned.sent.lock()[0].clone();
        assert_eq!(sent.url, wire::ANTHROPIC_MESSAGES);
        let body = sent.body.unwrap();
        let choice = &body["tool_choice"];
        assert_eq!(choice["disable_parallel_tool_use"], json!(true));
    }
}
