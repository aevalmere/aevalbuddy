//! `AEVAL_FAKE_LLM=1`: every chat request is answered from fixtures in
//! tools/fixtures/llm/*.jsonl instead of the network, so Find and the agent
//! behave the same way on every run of the smoke test (PARITY MODEL-08).
//! The catalogue fetch is not faked. Port of lib/fake-llm.js; see that file
//! for the fixture format.
//!
//! The fixtures ship inside the binary, so the installed exe on a CI runner
//! answers the same way a dev build does. `AEVAL_FAKE_LLM_DIR` points at a
//! folder of extra .jsonl files read at startup.
//!
//! Other lanes' probes script it at run time with [`Fake::push_fixture`]
//! (contract K-26). Besides the lib/fake-llm.js fields, a fixture may carry:
//!
//! - `provider`: only requests to this provider id match (`openrouter`,
//!   `anthropic`, `openai`, `gemini`); absent matches every provider.
//! - `model`: only requests for this model id match.
//! - `status`: answer with this HTTP status and
//!   `{"error":{"message": <message or "fake failure">}}`.
//! - `retryAfter`: the `Retry-After` header on that reply (seconds).
//! - `stallAfterHeadersMs`: send the headers, then nothing for this long
//!   before the body (a stalled connection).
//! - `times`: match this many requests, then drop out.
//!
//! Purpose `key` scripts a key check ([`Fake::key_reply`]) and `key-probe`
//! the free-model probe that follows it.

use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;
use std::time::Duration;

use futures_util::StreamExt;
use parking_lot::Mutex;
use regex::Regex;
use serde_json::{json, Map, Value};
use tokio_util::sync::CancellationToken;

use super::client::{Meta, Reply};
use super::errors::ApiError;
use super::provider::Provider;
use super::wire;

const EMBEDDED: [&str; 2] = [
    include_str!("../../../tools/fixtures/llm/agent.jsonl"),
    include_str!("../../../tools/fixtures/llm/find.jsonl"),
];

/// The request log keeps this many bodies (newest last).
pub const REQUEST_LOG: usize = 400;

const READER_DEFAULT: &str = "SCREEN: Notepad\nSTATE: a text file is open\nFOCUS: NONE\n\
                              TASK: assist\nTEXT: NONE\nELEMENTS:\n";
const ANSWER_DEFAULT: &str = "This is the fake model answering.";

/// One request the fake answered.
#[derive(Clone, Debug, PartialEq)]
pub struct FakeRequest {
    pub purpose: String,
    pub model: Option<String>,
    pub body: String,
    /// The provider id the request was for.
    pub provider: String,
    /// The URL the real request would have gone to.
    pub url: String,
}

/// The fixtures, the `{{name}}` variables and the request log.
#[derive(Default)]
pub struct Fake {
    fixtures: Mutex<Vec<Value>>,
    vars: Mutex<HashMap<String, Value>>,
    requests: Mutex<VecDeque<FakeRequest>>,
    call_seq: Mutex<u64>,
}

/// What a fixture is matched against besides its own predicate.
struct Want<'a> {
    purpose: &'a str,
    provider: &'a str,
    model: &'a str,
}

impl Want<'_> {
    fn admits(&self, x: &Value) -> bool {
        let field = |k: &str, have: &str| match x.get(k).and_then(Value::as_str) {
            Some(want) => want == have,
            None => true,
        };
        x["purpose"].as_str() == Some(self.purpose)
            && field("provider", self.provider)
            && field("model", self.model)
    }
}

/// A fixture that scripts the transport (a status or a stall) rather than
/// the model's words.
fn scripted(x: &Value) -> bool {
    x["status"].as_u64().is_some_and(|s| s != 0 && s != 200)
        || x["stallAfterHeadersMs"].as_u64().is_some_and(|ms| ms > 0)
}

/// `retryAfter` as a header value: a number of seconds, or a string.
fn retry_after_of(x: &Value) -> Option<String> {
    match &x["retryAfter"] {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    }
}

/// The reply's body arrives only after `ms` of silence.
fn stall(reply: Reply, ms: u64) -> Reply {
    // The timer starts when the body is first read, on whichever runtime
    // reads it.
    let quiet = futures_util::stream::once(async move {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    })
    .filter_map(|_| async { None::<Result<Vec<u8>, ApiError>> });
    Reply {
        status: reply.status,
        retry_after: reply.retry_after,
        body: Box::pin(quiet.chain(reply.body)),
    }
}

fn parse_lines(text: &str, out: &mut Vec<Value>) {
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // A broken fixture line is skipped; the assert that needed it fails.
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            out.push(v);
        }
    }
}

fn var_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    const PATTERN: &str = r"\{\{([A-Za-z0-9_]+)\}\}";
    RE.get_or_init(|| Regex::new(PATTERN).expect("var pattern"))
}

fn span_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    const PATTERN: &str = r"^\[([0-9]+)\]\s?([^\r\x{2028}\x{2029}]*)$";
    RE.get_or_init(|| Regex::new(PATTERN).expect("span pattern"))
}

/// `String(s || '').toLowerCase().trim()`.
fn lower(v: Option<&str>) -> String {
    v.unwrap_or_default().to_lowercase().trim().to_string()
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .map(|c| match (c["type"].as_str(), c["text"].as_str()) {
                (Some("text"), Some(t)) => t,
                _ => "",
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// JavaScript's `String(value)` for a variable.
fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        other => other.to_string(),
    }
}

async fn sleep(ms: u64, cancel: &CancellationToken) -> Result<(), ApiError> {
    if ms == 0 {
        return Ok(());
    }
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(ApiError::abort()),
        _ = tokio::time::sleep(Duration::from_millis(ms)) => Ok(()),
    }
}

fn delay_of(v: &Value) -> u64 {
    let ms = v["delayMs"].as_f64().unwrap_or(0.0);
    ms.max(0.0) as u64
}

fn sse(chunks: &[String]) -> Reply {
    let mut s = String::new();
    for c in chunks {
        let line = json!({ "choices": [{ "delta": { "content": c } }] });
        s.push_str(&format!("data: {line}\n\n"));
    }
    s.push_str("data: [DONE]\n\n");
    Reply::full(200, s)
}

fn message(body: &Value, msg: Value) -> Reply {
    if body["stream"].as_bool().unwrap_or(false) {
        let content = msg["content"].as_str().unwrap_or_default().to_string();
        return sse(&[content]);
    }
    let finish = if msg.get("tool_calls").is_some() {
        "tool_calls"
    } else {
        "stop"
    };
    let reply = json!({ "choices": [{ "message": msg, "finish_reason": finish }] });
    Reply::full(200, reply.to_string())
}

/// The JSON fallback path's shape: tool name plus its arguments, flat.
fn as_json_turn(call: &Value) -> String {
    let mut flat = Map::new();
    flat.insert("tool".into(), call["name"].clone());
    if let Some(args) = call["args"].as_object() {
        for (k, v) in args {
            flat.insert(k.clone(), v.clone());
        }
    }
    format!("```json\n{}\n```", Value::Object(flat))
}

fn has_tools(body: &Value) -> bool {
    body["tools"].as_array().is_some_and(|t| !t.is_empty())
}

impl Fake {
    /// The embedded fixtures plus any in `AEVAL_FAKE_LLM_DIR`.
    pub fn load() -> Fake {
        let mut fixtures = Vec::new();
        for text in EMBEDDED {
            parse_lines(text, &mut fixtures);
        }
        if let Some(dir) = std::env::var_os("AEVAL_FAKE_LLM_DIR") {
            let mut files: Vec<_> = std::fs::read_dir(dir)
                .map(|d| d.flatten().map(|e| e.path()).collect())
                .unwrap_or_default();
            files.retain(|p| p.extension().is_some_and(|x| x == "jsonl"));
            files.sort();
            for f in files {
                if let Ok(text) = std::fs::read_to_string(f) {
                    parse_lines(&text, &mut fixtures);
                }
            }
        }
        Fake::from_fixtures(fixtures)
    }

    pub fn from_fixtures(fixtures: Vec<Value>) -> Fake {
        Fake {
            fixtures: Mutex::new(fixtures),
            ..Fake::default()
        }
    }

    /// Adds a fixture ahead of every other, so a probe can script the next
    /// requests without touching this module (contract K-26).
    // Contract K-26 for the windows, agent and platform probes.
    #[allow(dead_code)]
    pub fn push_fixture(&self, fixture: Value) {
        self.fixtures.lock().insert(0, fixture);
    }

    /// Back to the embedded fixtures alone, so the next probe starts clean.
    /// Fixtures from `AEVAL_FAKE_LLM_DIR` are dropped too.
    // Contract K-26.
    #[allow(dead_code)]
    pub fn clear_pushed(&self) {
        let mut base = Vec::new();
        for text in EMBEDDED {
            parse_lines(text, &mut base);
        }
        *self.fixtures.lock() = base;
    }

    /// Fills `{{name}}` in later answers (a window handle the test only
    /// learns once Notepad is open).
    // Test seam.
    #[allow(dead_code)]
    pub fn set_var(&self, name: &str, value: Value) {
        self.vars.lock().insert(name.to_string(), value);
    }

    /// Every request body so far, newest last. The smoke run reads it to
    /// prove a password never reached a model.
    // Test seam.
    #[allow(dead_code)]
    pub fn requests(&self) -> Vec<FakeRequest> {
        self.requests.lock().iter().cloned().collect()
    }

    fn fill(&self, v: &Value) -> Value {
        match v {
            Value::String(s) => {
                let vars = self.vars.lock();
                let out = var_re().replace_all(s, |c: &regex::Captures| match vars.get(&c[1]) {
                    Some(x) => js_string(x),
                    None => c[0].to_string(),
                });
                Value::String(out.into_owned())
            }
            Value::Array(a) => Value::Array(a.iter().map(|x| self.fill(x)).collect()),
            Value::Object(o) => {
                let filled = o.iter().map(|(k, x)| (k.clone(), self.fill(x)));
                Value::Object(filled.collect())
            }
            other => other.clone(),
        }
    }

    /// The first fixture `want` admits that `pred` accepts, counting one
    /// use against its `times`.
    fn take(&self, want: &Want<'_>, pred: impl Fn(&Value) -> bool) -> Option<Value> {
        let mut list = self.fixtures.lock();
        let at = list.iter().position(|x| want.admits(x) && pred(x))?;
        let found = list[at].clone();
        if let Some(n) = found["times"].as_u64() {
            if n <= 1 {
                list.remove(at);
            } else {
                list[at]["times"] = json!(n - 1);
            }
        }
        Some(found)
    }

    /// Every fixture `want` admits that `pred` accepts.
    fn all(&self, want: &Want<'_>, pred: impl Fn(&Value) -> bool) -> Vec<Value> {
        let list = self.fixtures.lock();
        list.iter()
            .filter(|x| want.admits(x) && pred(x))
            .cloned()
            .collect()
    }

    /// A status or a stall a probe scripted for this request.
    async fn scripted_reply(
        &self,
        f: &Value,
        body: &Value,
        cancel: &CancellationToken,
    ) -> Result<Reply, ApiError> {
        sleep(delay_of(f), cancel).await?;
        if let Some(status) = f["status"].as_u64().filter(|s| *s != 0 && *s != 200) {
            let message = f["message"].as_str().filter(|m| !m.is_empty());
            let err = json!({ "error": { "message": message.unwrap_or("fake failure") } });
            let reply = Reply::full(status as u16, err.to_string());
            return Ok(reply.with_retry_after(retry_after_of(f)));
        }
        let text = match f["text"].as_str() {
            Some(t) => self.fill(&json!(t)),
            None => json!(ANSWER_DEFAULT),
        };
        let reply = message(body, json!({ "role": "assistant", "content": text }));
        let ms = f["stallAfterHeadersMs"].as_u64().unwrap_or(0);
        Ok(stall(reply, ms))
    }

    /// A key check under `AEVAL_FAKE_LLM`: a fixture with purpose `key`
    /// scripts it (a `status`, or the body in `json`); without one, any key
    /// is good. The free-model probe is purpose `key-probe`.
    pub async fn key_reply(
        &self,
        provider: Provider,
        cancel: &CancellationToken,
    ) -> Result<Reply, ApiError> {
        let want = Want {
            purpose: "key",
            provider: provider.id(),
            model: "",
        };
        match self.take(&want, |_| true) {
            Some(f) if scripted(&f) => self.scripted_reply(&f, &Value::Null, cancel).await,
            Some(f) => Ok(Reply::full(200, f["json"].to_string())),
            None => Ok(Reply::full(200, r#"{"data":{"limit":null,"usage":0}}"#)),
        }
    }

    /// Answers one chat request as OpenRouter would.
    pub async fn respond(
        &self,
        provider: Provider,
        body: &Value,
        cancel: &CancellationToken,
        meta: &Meta,
    ) -> Result<Reply, ApiError> {
        let purpose = if meta.purpose.is_empty() {
            "answer"
        } else {
            meta.purpose.as_str()
        };
        let model = body["model"].as_str().unwrap_or_default().to_string();
        {
            let mut log = self.requests.lock();
            log.push_back(FakeRequest {
                purpose: purpose.to_string(),
                model: body["model"].as_str().map(str::to_string),
                body: body.to_string(),
                provider: provider.id().to_string(),
                url: wire::chat_request(provider, "", body).url,
            });
            if log.len() > REQUEST_LOG {
                log.pop_front();
            }
        }
        let want = Want {
            purpose,
            provider: provider.id(),
            model: &model,
        };
        let q = lower(meta.query.as_deref());
        let query_ok = |x: &Value| x["query"].is_null() || lower(x["query"].as_str()) == q;
        let script = self.take(&want, |x| {
            scripted(x) && x["turns"].is_null() && query_ok(x)
        });
        if let Some(f) = script {
            return self.scripted_reply(&f, body, cancel).await;
        }
        let empty = Vec::new();
        let msgs = body["messages"].as_array().unwrap_or(&empty);
        let user_text = msgs
            .iter()
            .filter(|m| m["role"] == "user")
            .map(|m| text_of(&m["content"]))
            .collect::<Vec<_>>()
            .join("\n");

        match purpose {
            "find-expand" => {
                let f = self.take(&want, |x| lower(x["query"].as_str()) == q);
                if let Some(f) = &f {
                    sleep(delay_of(f), cancel).await?;
                }
                let lines = f.map(|f| self.fill(&f["lines"])).unwrap_or(Value::Null);
                let chunks: Vec<String> = lines
                    .as_array()
                    .map(|a| a.iter().map(|l| format!("{l}\n")).collect())
                    .unwrap_or_default();
                Ok(sse(&chunks))
            }
            "find-correlate" => {
                let mut out = Vec::new();
                let matching = self.all(&want, |x| lower(x["query"].as_str()) == q);
                for f in &matching {
                    let needle = lower(f["match"].as_str());
                    for line in user_text.split('\n') {
                        let Some(m) = span_re().captures(line) else {
                            continue;
                        };
                        if !lower(Some(&m[2])).contains(&needle) {
                            continue;
                        }
                        let id = m[1].parse::<u64>().map(Value::from);
                        let id = id.unwrap_or_else(|_| json!(m[1].parse::<f64>().unwrap_or(0.0)));
                        let score = f["score"].as_f64().filter(|s| *s != 0.0).unwrap_or(0.8);
                        let why = self.fill(&f["why"]);
                        let why = if why.is_string() { why } else { json!("") };
                        let row = json!({ "id": id, "score": score, "why": why });
                        out.push(format!("{row}\n"));
                    }
                }
                Ok(sse(&out))
            }
            "agent" => self.agent_turn(&want, body, msgs, cancel, meta).await,
            _ => {
                let f = self.take(&want, |x| !scripted(x));
                let text = match &f {
                    Some(f) => self.fill(&f["text"]),
                    None if purpose == "reader" => json!(READER_DEFAULT),
                    None => json!(ANSWER_DEFAULT),
                };
                if let Some(f) = &f {
                    sleep(delay_of(f), cancel).await?;
                }
                let mut msg = json!({ "role": "assistant" });
                if !text.is_null() {
                    msg["content"] = text;
                }
                Ok(message(body, msg))
            }
        }
    }

    async fn agent_turn(
        &self,
        want: &Want<'_>,
        body: &Value,
        msgs: &[Value],
        cancel: &CancellationToken,
        meta: &Meta,
    ) -> Result<Reply, ApiError> {
        let task = lower(meta.task.as_deref());
        let f = self.take(want, |x| task.contains(&lower(x["task"].as_str())));
        let turn = msgs.iter().filter(|m| m["role"] == "assistant").count();
        let step = match &f {
            Some(f) => {
                let picked = f["turns"].get(turn).filter(|t| truthy(t));
                let picked = picked.or_else(|| Some(&f["after"]).filter(|t| truthy(t)));
                match picked {
                    Some(p) => self.fill(p),
                    None => json!({ "text": "Done." }),
                }
            }
            None => json!({ "text": "No fixture for this task." }),
        };
        sleep(delay_of(&step), cancel).await?;
        if let Some(status) = step["status"].as_u64().filter(|s| *s != 0) {
            let message = step["message"].as_str().filter(|m| !m.is_empty());
            let message = message.unwrap_or("fake failure");
            let err = json!({ "error": { "message": message } });
            return Ok(Reply::full(status as u16, err.to_string()));
        }
        let tools = has_tools(body);
        if truthy(&step["call"]) {
            let call = &step["call"];
            if tools {
                let seq = {
                    let mut n = self.call_seq.lock();
                    *n += 1;
                    *n
                };
                let args = if call["args"].is_null() {
                    json!({})
                } else {
                    call["args"].clone()
                };
                let say = step["say"].as_str().unwrap_or_default();
                let msg = json!({
                    "role": "assistant",
                    "content": say,
                    "tool_calls": [{
                        "id": format!("call_{seq}"),
                        "type": "function",
                        "function": { "name": call["name"], "arguments": args.to_string() },
                    }],
                });
                return Ok(message(body, msg));
            }
            let msg = json!({ "role": "assistant", "content": as_json_turn(call) });
            return Ok(message(body, msg));
        }
        let text = match step["text"].as_str() {
            Some(t) if !t.is_empty() => t,
            _ => "Done.",
        };
        if tools {
            let msg = json!({ "role": "assistant", "content": text });
            return Ok(message(body, msg));
        }
        let done = json!({ "done": text });
        let content = format!("```json\n{done}\n```");
        let msg = json!({ "role": "assistant", "content": content });
        Ok(message(body, msg))
    }
}

/// JavaScript truthiness for a fixture value.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    fn block<T>(f: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(f)
    }

    fn text(reply: Reply) -> (u16, String) {
        let status = reply.status;
        let chunks: Vec<_> = block(reply.body.collect::<Vec<_>>());
        let bytes: Vec<u8> = chunks.into_iter().flat_map(|c| c.unwrap()).collect();
        (status, String::from_utf8(bytes).unwrap())
    }

    fn agent_meta(task: &str) -> Meta {
        Meta {
            purpose: "agent".into(),
            task: Some(task.into()),
            ..Meta::default()
        }
    }

    fn ask(fake: &Fake, body: Value, meta: &Meta) -> (u16, String) {
        let cancel = CancellationToken::new();
        let reply = block(fake.respond(Provider::OpenRouter, &body, &cancel, meta));
        text(reply.unwrap())
    }

    #[test]
    fn embedded_fixtures_load() {
        let fake = Fake::load();
        assert!(fake.fixtures.lock().len() >= 10);
    }

    #[test]
    fn agent_turns_follow_the_assistant_count_and_fill_vars() {
        let fake = Fake::load();
        fake.set_var("notepad", json!(4242));
        let tools = json!([{ "type": "function", "function": { "name": "wait" } }]);
        let meta = agent_meta("Smoke Notepad please");
        let body = json!({ "model": "m", "tools": tools, "messages": [
            { "role": "user", "content": "go" },
            { "role": "assistant", "content": "" },
        ] });
        let (_, out) = ask(&fake, body, &meta);
        let v: Value = serde_json::from_str(&out).unwrap();
        let call = &v["choices"][0]["message"]["tool_calls"][0];
        assert_eq!(call["id"], json!("call_1"));
        assert_eq!(call["function"]["name"], json!("focus_window"));
        assert_eq!(call["function"]["arguments"], json!(r#"{"handle":"4242"}"#));
        assert_eq!(v["choices"][0]["finish_reason"], json!("tool_calls"));
        // Without tools the same turn is the fenced JSON fallback.
        let body = json!({ "model": "m", "messages": [{ "role": "assistant", "content": "" }] });
        let (_, out) = ask(&fake, body, &meta);
        let v: Value = serde_json::from_str(&out).unwrap();
        let content = v["choices"][0]["message"]["content"].as_str().unwrap();
        let want = "```json\n{\"tool\":\"focus_window\",\"handle\":\"4242\"}\n```";
        assert_eq!(content, want);
        let (_, out) = ask(&fake, json!({ "messages": [] }), &agent_meta("unknown"));
        assert!(out.contains(r#"{\"done\":\"No fixture for this task.\"}"#));
        assert_eq!(fake.requests().len(), 3);
    }

    #[test]
    fn statuses_delays_and_streams() {
        let fixtures = vec![
            json!({ "purpose": "agent", "task": "boom", "turns": [{ "status": 429 }] }),
            json!({ "purpose": "agent", "task": "slow", "turns": [{ "delayMs": 60000 }] }),
            json!({ "purpose": "answer", "text": "Hi {{who}}" }),
        ];
        let fake = Fake::from_fixtures(fixtures);
        let (status, out) = ask(&fake, json!({}), &agent_meta("boom"));
        assert_eq!(status, 429);
        assert_eq!(out, r#"{"error":{"message":"fake failure"}}"#);
        let cancel = CancellationToken::new();
        cancel.cancel();
        let slow = agent_meta("slow");
        let empty = json!({});
        let e = block(fake.respond(Provider::OpenRouter, &empty, &cancel, &slow));
        assert!(e.err().unwrap().is_abort());
        let (_, out) = ask(&fake, json!({ "stream": true }), &Meta::purpose("answer"));
        let want = "data: {\"choices\":[{\"delta\":{\"content\":\"Hi {{who}}\"}}]}\n\n\
                    data: [DONE]\n\n";
        assert_eq!(out, want);
        let (_, out) = ask(&fake, json!({ "stream": true }), &Meta::purpose("reader"));
        assert!(out.contains("SCREEN: Notepad"));
    }

    #[test]
    fn correlate_answers_with_the_span_holding_the_match() {
        let fixtures = vec![json!({ "purpose": "find-correlate", "query": "owe",
            "match": "Total Due", "score": 0.9, "why": "the balance" })];
        let fake = Fake::from_fixtures(fixtures);
        let meta = Meta {
            purpose: "find-correlate".into(),
            query: Some(" OWE ".into()),
            task: None,
        };
        let user = "Spans:\n[0] Invoice\n[3] total due: $5\n[4]nothing";
        let body = json!({ "messages": [{ "role": "user", "content": user }] });
        let (_, out) = ask(&fake, body, &meta);
        let want = r#"{\"id\":3,\"score\":0.9,\"why\":\"the balance\"}\n"#;
        assert!(out.contains(want));
        assert!(!out.contains(r#"\"id\":0"#));
    }

    #[test]
    fn pushed_fixtures_script_status_provider_and_times() {
        let fake = Fake::from_fixtures(Vec::new());
        let busy = json!({ "purpose": "answer", "provider": "openrouter",
            "status": 429, "retryAfter": 7, "times": 2 });
        fake.push_fixture(busy);
        let body = json!({ "model": "a:free", "stream": true });
        let cancel = CancellationToken::new();
        let meta = Meta::purpose("answer");
        let or = |f: &Fake| block(f.respond(Provider::OpenRouter, &body, &cancel, &meta)).unwrap();
        let first = or(&fake);
        assert_eq!(first.status, 429);
        assert_eq!(first.retry_after.as_deref(), Some("7"));
        assert_eq!(or(&fake).status, 429);
        // Spent after two uses: the next one answers.
        let (status, out) = text(or(&fake));
        assert_eq!(status, 200);
        assert!(out.contains(ANSWER_DEFAULT));
        // A provider filter leaves the other providers alone.
        let down = json!({ "purpose": "answer", "provider": "openrouter", "status": 503 });
        fake.push_fixture(down);
        let native = block(fake.respond(Provider::Anthropic, &body, &cancel, &meta)).unwrap();
        assert_eq!(native.status, 200);
        assert_eq!(or(&fake).status, 503);
        let log = fake.requests();
        let last = log.last().unwrap();
        assert_eq!(last.provider, "openrouter");
        assert_eq!(last.url, wire::OPENROUTER_CHAT);
        assert_eq!(log[log.len() - 2].url, wire::ANTHROPIC_MESSAGES);
        fake.clear_pushed();
        assert_eq!(or(&fake).status, 200);
    }

    #[test]
    fn a_stall_holds_the_body_back() {
        let fake = Fake::from_fixtures(Vec::new());
        let stalled = json!({ "purpose": "answer", "stallAfterHeadersMs": 60000,
            "text": "late" });
        fake.push_fixture(stalled);
        let body = json!({ "model": "m", "stream": true });
        let cancel = CancellationToken::new();
        let meta = Meta::purpose("answer");
        // One runtime for the reply and the read: the stall's timer belongs
        // to the runtime that polls it.
        let (status, first) = block(async {
            let reply = fake
                .respond(Provider::OpenRouter, &body, &cancel, &meta)
                .await;
            let reply = reply.unwrap();
            let mut stream = reply.body;
            let wait = Duration::from_millis(50);
            (
                reply.status,
                tokio::time::timeout(wait, stream.next()).await,
            )
        });
        assert_eq!(status, 200);
        assert!(first.is_err(), "the body arrived during the stall");
    }

    #[test]
    fn the_log_is_capped() {
        let fake = Fake::from_fixtures(Vec::new());
        for _ in 0..REQUEST_LOG + 5 {
            ask(&fake, json!({ "model": "x" }), &Meta::default());
        }
        let log = fake.requests();
        assert_eq!(log.len(), REQUEST_LOG);
        assert_eq!(log[0].purpose, "answer");
    }
}
