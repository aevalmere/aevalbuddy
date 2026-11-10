//! Request and response mapping for each provider (REQUESTS.md 2).
//!
//! Every caller builds its request in the OpenAI chat shape, exactly as the
//! Electron code built it for OpenRouter, so the OpenRouter body stays byte
//! for byte what lib/openrouter.js sent (PARITY MODEL-01). The native clients
//! translate that one shape:
//!
//! - Anthropic Messages API: `system` lifted out, image parts as base64
//!   `image` blocks, tool calls as `tool_use` / `tool_result` blocks,
//!   `disable_parallel_tool_use` for one action per turn.
//! - OpenAI Chat Completions: the same shape, with `max_completion_tokens`
//!   and `stream_options.include_usage`.
//! - Gemini `generateContent` / `streamGenerateContent?alt=sse`: `contents`
//!   with `inlineData` images, `functionCall` / `functionResponse` parts and
//!   `parametersJsonSchema` tool declarations.
//!
//! What a native model said that must be replayed unchanged on the next turn
//! (Anthropic thinking blocks, Gemini thought signatures) rides on the
//! assistant message under `_native`, which the OpenRouter body never carries.

use std::time::Duration;

use serde_json::{json, Map, Value};

use super::errors::{codes, ApiError};
use super::provider::Provider;
use super::sse::Usage;

pub const OPENROUTER_CHAT: &str = "https://openrouter.ai/api/v1/chat/completions";
pub const OPENROUTER_MODELS: &str = "https://openrouter.ai/api/v1/models";
pub const OPENROUTER_KEY: &str = "https://openrouter.ai/api/v1/key";
pub const ANTHROPIC_MESSAGES: &str = "https://api.anthropic.com/v1/messages";
pub const ANTHROPIC_MODELS: &str = "https://api.anthropic.com/v1/models";
pub const ANTHROPIC_VERSION: &str = "2023-06-01";
pub const OPENAI_CHAT: &str = "https://api.openai.com/v1/chat/completions";
pub const OPENAI_MODELS: &str = "https://api.openai.com/v1/models";
pub const GEMINI_MODELS: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// The field on an assistant message that holds a native reply for replay.
pub const NATIVE_FIELD: &str = "_native";

/// Native models think before they answer and count that against the output
/// limit, so the small limits the free-model flows use (800 for Find, 1600
/// for the reader) are raised to this floor for them.
pub const NATIVE_MIN_TOKENS: u64 = 16000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Value>,
    pub timeout: Option<Duration>,
}

fn header(k: &str, v: &str) -> (String, String) {
    (k.to_string(), v.to_string())
}

fn is_stream(body: &Value) -> bool {
    body["stream"].as_bool().unwrap_or(false)
}

/// The chat request for one provider, from the OpenAI-shaped body.
pub fn chat_request(provider: Provider, key: &str, body: &Value) -> HttpRequest {
    match provider {
        Provider::OpenRouter => HttpRequest {
            method: Method::Post,
            url: OPENROUTER_CHAT.to_string(),
            headers: vec![
                header("Authorization", &format!("Bearer {key}")),
                header("Content-Type", "application/json"),
                header("X-Title", "Aevaleron"),
            ],
            body: Some(openrouter_body(body)),
            timeout: None,
        },
        Provider::OpenAi => HttpRequest {
            method: Method::Post,
            url: OPENAI_CHAT.to_string(),
            headers: vec![
                header("Authorization", &format!("Bearer {key}")),
                header("Content-Type", "application/json"),
            ],
            body: Some(openai_body(body)),
            timeout: None,
        },
        Provider::Anthropic => HttpRequest {
            method: Method::Post,
            url: ANTHROPIC_MESSAGES.to_string(),
            headers: vec![
                header("x-api-key", key),
                header("anthropic-version", ANTHROPIC_VERSION),
                header("Content-Type", "application/json"),
            ],
            body: Some(anthropic_body(body)),
            timeout: None,
        },
        Provider::Gemini => {
            let model = body["model"].as_str().unwrap_or_default();
            let url = if is_stream(body) {
                format!("{GEMINI_MODELS}/{model}:streamGenerateContent?alt=sse")
            } else {
                format!("{GEMINI_MODELS}/{model}:generateContent")
            };
            HttpRequest {
                method: Method::Post,
                url,
                headers: vec![
                    header("x-goog-api-key", key),
                    header("Content-Type", "application/json"),
                ],
                body: Some(gemini_body(body)),
                timeout: None,
            }
        }
    }
}

/// The request that checks a key without spending anything.
pub fn key_request(provider: Provider, key: &str) -> HttpRequest {
    let (url, headers) = match provider {
        Provider::OpenRouter => (
            OPENROUTER_KEY.to_string(),
            vec![header("Authorization", &format!("Bearer {key}"))],
        ),
        Provider::OpenAi => (
            OPENAI_MODELS.to_string(),
            vec![header("Authorization", &format!("Bearer {key}"))],
        ),
        Provider::Anthropic => (
            ANTHROPIC_MODELS.to_string(),
            vec![
                header("x-api-key", key),
                header("anthropic-version", ANTHROPIC_VERSION),
            ],
        ),
        Provider::Gemini => (
            format!("{GEMINI_MODELS}?pageSize=1"),
            vec![header("x-goog-api-key", key)],
        ),
    };
    HttpRequest {
        method: Method::Get,
        url,
        headers,
        body: None,
        timeout: Some(Duration::from_secs(8)),
    }
}

/// A native provider's whole model list (B-123): the key check's request,
/// with Gemini's page opened up to every model.
pub fn native_list_request(provider: Provider, key: &str) -> HttpRequest {
    let mut req = key_request(provider, key);
    match provider {
        Provider::Gemini => req.url = format!("{GEMINI_MODELS}?pageSize=1000"),
        Provider::Anthropic => req.url = format!("{ANTHROPIC_MODELS}?limit=1000"),
        _ => {}
    }
    req
}

fn without_native(body: &Value) -> Value {
    let mut b = body.clone();
    if let Some(list) = b["messages"].as_array_mut() {
        for m in list {
            if let Some(o) = m.as_object_mut() {
                o.remove(NATIVE_FIELD);
            }
        }
    }
    b
}

/// `{...body, usage: {include: true}}`: OpenRouter reports tokens and cost
/// on the reply, which the answer's meta line and the trail's footer show.
pub fn openrouter_body(body: &Value) -> Value {
    let mut b = without_native(body);
    if let Some(o) = b.as_object_mut() {
        o.insert("usage".into(), json!({ "include": true }));
    }
    b
}

fn raised_limit(body: &Value) -> Option<u64> {
    body["max_tokens"]
        .as_f64()
        .map(|n| (n.max(0.0) as u64).max(NATIVE_MIN_TOKENS))
}

/// OpenAI's reasoning models take no sampling temperature and count output
/// as `max_completion_tokens`.
pub fn openai_body(body: &Value) -> Value {
    let mut b = without_native(body);
    let limit = raised_limit(body);
    if let Some(o) = b.as_object_mut() {
        o.remove("temperature");
        o.remove("max_tokens");
        if let Some(n) = limit {
            o.insert("max_completion_tokens".into(), json!(n));
        }
        if is_stream(body) {
            o.insert("stream_options".into(), json!({ "include_usage": true }));
        }
    }
    b
}

/// `data:<mime>;base64,<data>` into its two halves.
fn data_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let (mime, data) = rest.split_once(";base64,")?;
    Some((mime.to_string(), data.to_string()))
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter(|p| p["type"] == "text")
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// OpenAI content parts, as a list (a plain string is one text part).
fn parts_of(content: &Value) -> Vec<Value> {
    match content {
        Value::String(s) => vec![json!({ "type": "text", "text": s })],
        Value::Array(parts) => parts.clone(),
        _ => Vec::new(),
    }
}

fn tool_args(call: &Value) -> Value {
    let raw = &call["function"]["arguments"];
    match raw {
        Value::String(s) if s.trim().is_empty() => json!({}),
        Value::String(s) => serde_json::from_str(s).unwrap_or_else(|_| json!({})),
        Value::Object(_) => raw.clone(),
        _ => json!({}),
    }
}

/// Merges neighbouring messages with the same role, the way the providers
/// read them anyway, so a tool result is always in the turn right after
/// its call.
fn merge_roles(list: Vec<(String, Vec<Value>)>) -> Vec<(String, Vec<Value>)> {
    let mut out: Vec<(String, Vec<Value>)> = Vec::new();
    for (role, parts) in list {
        if parts.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some((last, prev)) if *last == role => prev.extend(parts),
            _ => out.push((role, parts)),
        }
    }
    out
}

fn system_text(body: &Value) -> String {
    let empty = Vec::new();
    let list = body["messages"].as_array().unwrap_or(&empty);
    list.iter()
        .filter(|m| m["role"] == "system")
        .map(|m| text_of(&m["content"]))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn anthropic_part(part: &Value) -> Option<Value> {
    match part["type"].as_str() {
        Some("text") => {
            let t = part["text"].as_str().unwrap_or_default();
            (!t.is_empty()).then(|| json!({ "type": "text", "text": t }))
        }
        Some("image_url") => {
            let url = part["image_url"]["url"].as_str().unwrap_or_default();
            let source = match data_url(url) {
                Some((mime, data)) => {
                    json!({ "type": "base64", "media_type": mime, "data": data })
                }
                None => json!({ "type": "url", "url": url }),
            };
            Some(json!({ "type": "image", "source": source }))
        }
        _ => None,
    }
}

/// The Anthropic Messages body. Opus 5.5 and the other current models take
/// no sampling temperature, so it is left out; `max_tokens` is required.
pub fn anthropic_body(body: &Value) -> Value {
    let empty = Vec::new();
    let mut turns: Vec<(String, Vec<Value>)> = Vec::new();
    for m in body["messages"].as_array().unwrap_or(&empty) {
        let role = m["role"].as_str().unwrap_or_default();
        match role {
            "user" => {
                let parts = parts_of(&m["content"]);
                let content = parts.iter().filter_map(anthropic_part).collect();
                turns.push(("user".into(), content));
            }
            "assistant" => {
                if let Some(native) = m[NATIVE_FIELD]["anthropic"].as_array() {
                    turns.push(("assistant".into(), native.clone()));
                    continue;
                }
                let mut blocks = Vec::new();
                let said = text_of(&m["content"]);
                if !said.trim().is_empty() {
                    blocks.push(json!({ "type": "text", "text": said }));
                }
                for c in m["tool_calls"].as_array().unwrap_or(&empty) {
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": c["id"],
                        "name": c["function"]["name"],
                        "input": tool_args(c),
                    }));
                }
                turns.push(("assistant".into(), blocks));
            }
            "tool" => {
                let block = json!({
                    "type": "tool_result",
                    "tool_use_id": m["tool_call_id"],
                    "content": text_of(&m["content"]),
                });
                turns.push(("user".into(), vec![block]));
            }
            _ => {}
        }
    }
    let messages: Vec<Value> = merge_roles(turns)
        .into_iter()
        .map(|(role, content)| json!({ "role": role, "content": content }))
        .collect();
    let mut out = Map::new();
    out.insert("model".into(), body["model"].clone());
    let limit = raised_limit(body).unwrap_or(NATIVE_MIN_TOKENS);
    out.insert("max_tokens".into(), json!(limit));
    let system = system_text(body);
    if !system.is_empty() {
        out.insert("system".into(), json!(system));
    }
    out.insert("messages".into(), json!(messages));
    if let Some(tools) = body["tools"].as_array().filter(|t| !t.is_empty()) {
        let list: Vec<Value> = tools
            .iter()
            .map(|t| {
                let f = &t["function"];
                json!({
                    "name": f["name"],
                    "description": f["description"],
                    "input_schema": f["parameters"],
                })
            })
            .collect();
        out.insert("tools".into(), json!(list));
        let one = body["parallel_tool_calls"] == json!(false);
        let choice = json!({ "type": "auto", "disable_parallel_tool_use": one });
        out.insert("tool_choice".into(), choice);
    }
    if is_stream(body) {
        out.insert("stream".into(), json!(true));
    }
    Value::Object(out)
}

fn gemini_part(part: &Value) -> Option<Value> {
    match part["type"].as_str() {
        Some("text") => {
            let t = part["text"].as_str().unwrap_or_default();
            (!t.is_empty()).then(|| json!({ "text": t }))
        }
        Some("image_url") => {
            let url = part["image_url"]["url"].as_str().unwrap_or_default();
            let (mime, data) = data_url(url)?;
            Some(json!({ "inlineData": { "mimeType": mime, "data": data } }))
        }
        _ => None,
    }
}

/// The Gemini `generateContent` body.
pub fn gemini_body(body: &Value) -> Value {
    let empty = Vec::new();
    let mut names: Vec<(String, String)> = Vec::new();
    let mut turns: Vec<(String, Vec<Value>)> = Vec::new();
    for m in body["messages"].as_array().unwrap_or(&empty) {
        match m["role"].as_str().unwrap_or_default() {
            "user" => {
                let parts = parts_of(&m["content"]);
                let content = parts.iter().filter_map(gemini_part).collect();
                turns.push(("user".into(), content));
            }
            "assistant" => {
                for c in m["tool_calls"].as_array().unwrap_or(&empty) {
                    let id = c["id"].as_str().unwrap_or_default().to_string();
                    let name = c["function"]["name"].as_str().unwrap_or_default();
                    names.push((id, name.to_string()));
                }
                if let Some(native) = m[NATIVE_FIELD]["gemini"].as_array() {
                    turns.push(("model".into(), native.clone()));
                    continue;
                }
                let mut parts = Vec::new();
                let said = text_of(&m["content"]);
                if !said.trim().is_empty() {
                    parts.push(json!({ "text": said }));
                }
                for c in m["tool_calls"].as_array().unwrap_or(&empty) {
                    let call = json!({ "name": c["function"]["name"], "args": tool_args(c) });
                    parts.push(json!({ "functionCall": call }));
                }
                turns.push(("model".into(), parts));
            }
            "tool" => {
                let id = m["tool_call_id"].as_str().unwrap_or_default();
                let name = names
                    .iter()
                    .rev()
                    .find(|(i, _)| i == id)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_default();
                let response = json!({ "content": text_of(&m["content"]) });
                let part = json!({ "functionResponse": { "name": name, "response": response } });
                turns.push(("user".into(), vec![part]));
            }
            _ => {}
        }
    }
    let contents: Vec<Value> = merge_roles(turns)
        .into_iter()
        .map(|(role, parts)| json!({ "role": role, "parts": parts }))
        .collect();
    let mut out = Map::new();
    let system = system_text(body);
    if !system.is_empty() {
        let instruction = json!({ "parts": [{ "text": system }] });
        out.insert("systemInstruction".into(), instruction);
    }
    out.insert("contents".into(), json!(contents));
    // No sampling temperature, as for OpenAI and Anthropic: Gemini's thinking
    // models are tuned for their default (Decision D-18).
    let mut config = Map::new();
    let limit = raised_limit(body).unwrap_or(NATIVE_MIN_TOKENS);
    config.insert("maxOutputTokens".into(), json!(limit));
    out.insert("generationConfig".into(), Value::Object(config));
    if let Some(tools) = body["tools"].as_array().filter(|t| !t.is_empty()) {
        let list: Vec<Value> = tools
            .iter()
            .map(|t| {
                let f = &t["function"];
                json!({
                    "name": f["name"],
                    "description": f["description"],
                    "parametersJsonSchema": f["parameters"],
                })
            })
            .collect();
        out.insert("tools".into(), json!([{ "functionDeclarations": list }]));
        let mode = json!({ "functionCallingConfig": { "mode": "AUTO" } });
        out.insert("toolConfig".into(), mode);
    }
    Value::Object(out)
}

// ------------------------------------------------------------------ replies

/// JS `Number(v) || 0` for token counts.
fn count(v: &Value) -> u64 {
    let n = super::catalogue::js_number(v);
    if n.is_finite() && n > 0.0 {
        n as u64
    } else {
        0
    }
}

/// JS `Number(usage.cost)`, kept only when finite (a missing cost is NaN, a
/// null one is 0).
fn cost(usage: &Value) -> Option<f64> {
    let n = match usage.get("cost") {
        Some(v) => super::catalogue::js_number(v),
        None => f64::NAN,
    };
    n.is_finite().then_some(n)
}

fn usage_object(u: &Value) -> Option<Usage> {
    u.is_object().then(|| openai_usage(u))
}

pub fn openai_usage(u: &Value) -> Usage {
    Usage {
        prompt: count(&u["prompt_tokens"]),
        completion: count(&u["completion_tokens"]),
        cost: cost(u),
    }
}

/// An HTTP error body's message, or `HTTP <status>` (`failFrom`).
pub fn error_from_body(status: u16, body: &[u8]) -> ApiError {
    let parsed: Option<Value> = serde_json::from_slice(body).ok();
    let message = parsed
        .as_ref()
        .and_then(|v| v["error"]["message"].as_str())
        .filter(|m| !m.is_empty())
        .map_or_else(|| format!("HTTP {status}"), str::to_string);
    ApiError::http(status, message)
}

/// `obj.error.code || 500`: a number is the status, a missing or falsy code
/// is 500, and any other code (a string) is no status at all.
pub fn stream_error(err: &Value) -> ApiError {
    let status = match &err["code"] {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f != 0.0 => Some(f as u16),
            _ => Some(500),
        },
        Value::Null | Value::Bool(false) => Some(500),
        Value::String(s) if s.is_empty() => Some(500),
        _ => None,
    };
    let message = err["message"]
        .as_str()
        .filter(|m| !m.is_empty())
        .unwrap_or("stream error");
    ApiError::coded(status, message)
}

const GEMINI_BLOCKS: [&str; 5] = [
    "SAFETY",
    "PROHIBITED_CONTENT",
    "BLOCKLIST",
    "SPII",
    "IMAGE_SAFETY",
];

/// A model that declined: Anthropic's `refusal` stop reason, or a Gemini
/// safety block (B-119). Coded, so it never fails over: another model is
/// likely to decline too, and a hop would send the screen elsewhere.
pub fn refusal(provider: Provider, obj: &Value) -> Option<ApiError> {
    let declined = match provider {
        Provider::Anthropic => {
            obj["stop_reason"] == "refusal" || obj["delta"]["stop_reason"] == "refusal"
        }
        Provider::Gemini => {
            let blocked = obj["promptFeedback"]["blockReason"].is_string();
            let finish = &obj["candidates"][0]["finishReason"];
            blocked || GEMINI_BLOCKS.iter().any(|b| finish == b)
        }
        Provider::OpenRouter | Provider::OpenAi => false,
    };
    if !declined {
        return None;
    }
    let err = ApiError::coded(None, "the model declined to answer");
    Some(err.with_code(codes::REFUSED))
}

fn anthropic_status(kind: &str) -> u16 {
    match kind {
        "invalid_request_error" => 400,
        "authentication_error" => 401,
        "billing_error" => 402,
        "permission_error" => 403,
        "not_found_error" => 404,
        "request_too_large" => 413,
        "rate_limit_error" => 429,
        "timeout_error" => 504,
        "overloaded_error" => 529,
        _ => 500,
    }
}

/// One decoded `data:` line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decoded {
    pub text: Option<String>,
    pub usage: Option<Usage>,
    pub done: bool,
}

/// Reads one provider's stream events. OpenRouter and OpenAI report usage
/// once, inline; Anthropic and Gemini report running totals, which are
/// handed over once at the end.
pub struct StreamDecoder {
    provider: Provider,
    totals: Usage,
    saw_totals: bool,
}

impl StreamDecoder {
    pub fn new(provider: Provider) -> Self {
        StreamDecoder {
            provider,
            totals: Usage::default(),
            saw_totals: false,
        }
    }

    pub fn data(&mut self, payload: &str) -> Result<Decoded, ApiError> {
        let mut out = Decoded::default();
        if payload == "[DONE]" && !self.provider.is_native_stream() {
            out.done = true;
            return Ok(out);
        }
        let Ok(obj) = serde_json::from_str::<Value>(payload) else {
            return Ok(out);
        };
        if let Some(e) = refusal(self.provider, &obj) {
            return Err(e);
        }
        match self.provider {
            Provider::OpenRouter | Provider::OpenAi => {
                if obj.get("error").is_some_and(|e| !e.is_null()) {
                    return Err(stream_error(&obj["error"]));
                }
                if obj.get("usage").is_some_and(|u| u.is_object()) {
                    out.usage = Some(openai_usage(&obj["usage"]));
                }
                let delta = obj["choices"][0]["delta"]["content"].as_str();
                out.text = delta.filter(|d| !d.is_empty()).map(str::to_string);
            }
            Provider::Anthropic => match obj["type"].as_str().unwrap_or_default() {
                "error" => {
                    let e = &obj["error"];
                    let kind = e["type"].as_str().unwrap_or_default();
                    let message = e["message"].as_str().unwrap_or("stream error");
                    return Err(ApiError::http(anthropic_status(kind), message));
                }
                "message_start" => {
                    let u = &obj["message"]["usage"];
                    self.totals.prompt = count(&u["input_tokens"])
                        + count(&u["cache_creation_input_tokens"])
                        + count(&u["cache_read_input_tokens"]);
                    self.totals.completion = count(&u["output_tokens"]);
                    self.saw_totals = true;
                }
                "content_block_delta" if obj["delta"]["type"] == "text_delta" => {
                    let t = obj["delta"]["text"].as_str().unwrap_or_default();
                    out.text = (!t.is_empty()).then(|| t.to_string());
                }
                "message_delta" => {
                    let u = &obj["usage"];
                    if u.get("output_tokens").is_some() {
                        self.totals.completion = count(&u["output_tokens"]);
                        self.saw_totals = true;
                    }
                }
                "message_stop" => out.done = true,
                _ => {}
            },
            Provider::Gemini => {
                if obj.get("error").is_some_and(|e| !e.is_null()) {
                    return Err(stream_error(&obj["error"]));
                }
                let text = gemini_text(&obj);
                out.text = (!text.is_empty()).then_some(text);
                if let Some(u) = obj.get("usageMetadata") {
                    self.totals = gemini_usage(u);
                    self.saw_totals = true;
                }
            }
        }
        Ok(out)
    }

    /// The running totals, for the providers that report them that way.
    pub fn finish(&mut self) -> Option<Usage> {
        if !self.saw_totals {
            return None;
        }
        self.saw_totals = false;
        Some(self.totals)
    }
}

impl Provider {
    /// Streams that end without a `[DONE]` sentinel.
    fn is_native_stream(self) -> bool {
        matches!(self, Provider::Anthropic | Provider::Gemini)
    }
}

fn gemini_parts(obj: &Value) -> Vec<Value> {
    obj["candidates"][0]["content"]["parts"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// The answer text of a Gemini reply, thought summaries left out.
fn gemini_text(obj: &Value) -> String {
    gemini_parts(obj)
        .iter()
        .filter(|p| p["thought"] != json!(true))
        .filter_map(|p| p["text"].as_str())
        .collect()
}

fn gemini_usage(u: &Value) -> Usage {
    Usage {
        prompt: count(&u["promptTokenCount"]),
        completion: count(&u["candidatesTokenCount"]) + count(&u["thoughtsTokenCount"]),
        cost: None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
    pub raw: String,
}

/// One non-streamed reply (`chatTurn` in lib/openrouter.js).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Turn {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Usage>,
    /// What to replay under `_native` on the next request.
    pub native: Option<Value>,
}

/// Arguments the way chatTurn read them: a JSON string (blank is `{}`), an
/// object as is, and anything unreadable as `{__unparsed: raw}`.
fn parse_args(raw: &Value) -> (Value, String) {
    match raw {
        Value::String(s) if s.trim().is_empty() => (json!({}), s.clone()),
        Value::String(s) => match serde_json::from_str::<Value>(s) {
            Ok(v) => (v, s.clone()),
            Err(_) => (json!({ "__unparsed": s }), s.clone()),
        },
        Value::Null => (json!({}), String::new()),
        other => (other.clone(), other.to_string()),
    }
}

/// A non-streamed reply in any provider's shape. `stamp` stands in for
/// `Date.now()` in the generated ids of calls that arrive without one.
pub fn parse_turn(provider: Provider, data: &Value, stamp: u128) -> Turn {
    match provider {
        Provider::OpenRouter | Provider::OpenAi => {
            let msg = &data["choices"][0]["message"];
            let empty = Vec::new();
            let calls = msg["tool_calls"].as_array().unwrap_or(&empty);
            let tool_calls = calls
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let (args, raw) = parse_args(&c["function"]["arguments"]);
                    let id = match c["id"].as_str() {
                        Some(id) if !id.is_empty() => id.to_string(),
                        _ => format!("call_{stamp}_{i}"),
                    };
                    let name = c["function"]["name"].as_str().unwrap_or_default();
                    ToolCall {
                        id,
                        name: name.to_string(),
                        args,
                        raw,
                    }
                })
                .collect();
            Turn {
                content: msg["content"].as_str().unwrap_or_default().to_string(),
                tool_calls,
                usage: data.get("usage").and_then(usage_object),
                native: None,
            }
        }
        Provider::Anthropic => {
            let empty = Vec::new();
            let blocks = data["content"].as_array().unwrap_or(&empty);
            let mut content = String::new();
            let mut tool_calls = Vec::new();
            for b in blocks {
                match b["type"].as_str() {
                    Some("text") => content.push_str(b["text"].as_str().unwrap_or_default()),
                    Some("tool_use") => tool_calls.push(ToolCall {
                        id: b["id"].as_str().unwrap_or_default().to_string(),
                        name: b["name"].as_str().unwrap_or_default().to_string(),
                        args: b["input"].clone(),
                        raw: b["input"].to_string(),
                    }),
                    _ => {}
                }
            }
            let u = &data["usage"];
            let usage = u.is_object().then(|| Usage {
                prompt: count(&u["input_tokens"])
                    + count(&u["cache_creation_input_tokens"])
                    + count(&u["cache_read_input_tokens"]),
                completion: count(&u["output_tokens"]),
                cost: None,
            });
            Turn {
                content,
                tool_calls,
                usage,
                native: Some(json!({ "anthropic": blocks })),
            }
        }
        Provider::Gemini => {
            let parts = gemini_parts(data);
            let mut tool_calls = Vec::new();
            for (i, p) in parts.iter().enumerate() {
                let Some(fc) = p.get("functionCall") else {
                    continue;
                };
                let id = match fc["id"].as_str() {
                    Some(id) if !id.is_empty() => id.to_string(),
                    _ => format!("call_{stamp}_{i}"),
                };
                let args = if fc["args"].is_object() {
                    fc["args"].clone()
                } else {
                    json!({})
                };
                tool_calls.push(ToolCall {
                    id,
                    name: fc["name"].as_str().unwrap_or_default().to_string(),
                    raw: args.to_string(),
                    args,
                });
            }
            let usage = data.get("usageMetadata").map(gemini_usage);
            Turn {
                content: gemini_text(data),
                tool_calls,
                usage,
                native: Some(json!({ "gemini": parts })),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMG: &str = "data:image/jpeg;base64,QUJD";

    fn agent_body() -> Value {
        json!({
            "model": "m",
            "stream": false,
            "temperature": 0,
            "messages": [
                { "role": "system", "content": "rules" },
                { "role": "user", "content": "Task: x" },
                {
                    "role": "assistant",
                    "content": "Plan: click",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "left_click", "arguments": "{\"ref\":\"ref_2\"}" }
                    }],
                    "_native": { "gemini": [{ "functionCall": { "name": "left_click" },
                        "thoughtSignature": "sig" }] }
                },
                { "role": "tool", "tool_call_id": "call_1", "content": "Clicked." },
                { "role": "user", "content": [
                    { "type": "text", "text": "Screenshot:" },
                    { "type": "image_url", "image_url": { "url": IMG } }
                ] }
            ],
            "tools": [{ "type": "function", "function": {
                "name": "left_click", "description": "Click.",
                "parameters": { "type": "object", "properties": {}, "required": [] } } }],
            "tool_choice": "auto",
            "parallel_tool_calls": false
        })
    }

    #[test]
    fn openrouter_body_is_the_electron_body_plus_usage() {
        let body = agent_body();
        let r = chat_request(Provider::OpenRouter, "sk-or-k", &body);
        assert_eq!(r.url, OPENROUTER_CHAT);
        assert_eq!(r.headers[0], header("Authorization", "Bearer sk-or-k"));
        assert_eq!(r.headers[2], header("X-Title", "Aevaleron"));
        let sent = r.body.unwrap();
        let keys: Vec<&String> = sent.as_object().unwrap().keys().collect();
        let want = [
            "model",
            "stream",
            "temperature",
            "messages",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
            "usage",
        ];
        assert_eq!(keys, want);
        assert_eq!(sent["usage"], json!({ "include": true }));
        assert!(sent["messages"][2].get(NATIVE_FIELD).is_none());
    }

    #[test]
    fn openai_drops_temperature_and_raises_the_limit() {
        let body = json!({ "model": "gpt", "stream": true, "temperature": 0, "max_tokens": 800,
            "messages": [{ "role": "user", "content": "hi" }] });
        let req = chat_request(Provider::OpenAi, "sk-proj-k", &body);
        let sent = req.body.unwrap();
        assert!(sent.get("temperature").is_none());
        assert!(sent.get("max_tokens").is_none());
        assert_eq!(sent["max_completion_tokens"], json!(16000));
        assert_eq!(sent["stream_options"], json!({ "include_usage": true }));
        assert!(sent.get("usage").is_none());
    }

    #[test]
    fn anthropic_body_maps_system_images_and_tools() {
        let r = chat_request(Provider::Anthropic, "sk-ant-k", &agent_body());
        assert_eq!(r.url, ANTHROPIC_MESSAGES);
        assert_eq!(r.headers[0], header("x-api-key", "sk-ant-k"));
        assert_eq!(r.headers[1], header("anthropic-version", "2023-06-01"));
        let b = r.body.unwrap();
        assert_eq!(b["system"], json!("rules"));
        assert_eq!(b["max_tokens"], json!(16000));
        assert!(b.get("temperature").is_none());
        let m = b["messages"].as_array().unwrap();
        assert_eq!(m.len(), 3);
        assert_eq!(m[1]["content"][1]["type"], json!("tool_use"));
        assert_eq!(m[1]["content"][1]["input"], json!({ "ref": "ref_2" }));
        let user = m[2]["content"].as_array().unwrap();
        assert_eq!(user[0]["type"], json!("tool_result"));
        assert_eq!(user[0]["tool_use_id"], json!("call_1"));
        assert_eq!(user[2]["source"]["media_type"], json!("image/jpeg"));
        assert_eq!(user[2]["source"]["data"], json!("QUJD"));
        assert_eq!(b["tools"][0]["input_schema"]["type"], json!("object"));
        let choice = json!({ "type": "auto", "disable_parallel_tool_use": true });
        assert_eq!(b["tool_choice"], choice);
    }

    #[test]
    fn anthropic_replays_its_own_blocks() {
        let mut body = agent_body();
        let blocks = json!([{ "type": "thinking", "thinking": "", "signature": "s" },
            { "type": "tool_use", "id": "call_1", "name": "left_click", "input": {} }]);
        body["messages"][2][NATIVE_FIELD] = json!({ "anthropic": blocks });
        let b = anthropic_body(&body);
        assert_eq!(b["messages"][1]["content"], blocks);
    }

    #[test]
    fn gemini_body_maps_parts_and_function_responses() {
        let r = chat_request(Provider::Gemini, "AIzaK", &agent_body());
        assert!(r.url.ends_with("/models/m:generateContent"));
        assert_eq!(r.headers[0], header("x-goog-api-key", "AIzaK"));
        let b = r.body.unwrap();
        assert_eq!(b["systemInstruction"]["parts"][0]["text"], json!("rules"));
        let c = b["contents"].as_array().unwrap();
        assert_eq!(c[1]["role"], json!("model"));
        assert_eq!(c[1]["parts"][0]["thoughtSignature"], json!("sig"));
        let fr = &c[2]["parts"][0]["functionResponse"];
        assert_eq!(fr["name"], json!("left_click"));
        assert_eq!(fr["response"]["content"], json!("Clicked."));
        assert_eq!(c[2]["parts"][2]["inlineData"]["data"], json!("QUJD"));
        assert!(b["generationConfig"].get("temperature").is_none());
        let decl = &b["tools"][0]["functionDeclarations"][0];
        assert_eq!(decl["parametersJsonSchema"]["type"], json!("object"));
        let stream = json!({ "model": "g", "stream": true, "messages": [] });
        let r = chat_request(Provider::Gemini, "AIzaK", &stream);
        assert!(r.url.ends_with("/models/g:streamGenerateContent?alt=sse"));
    }

    fn tokens(prompt: u64, completion: u64) -> Usage {
        Usage {
            prompt,
            completion,
            cost: None,
        }
    }

    fn decode(p: Provider, lines: &[&str]) -> Result<(String, Vec<Usage>, bool), ApiError> {
        let mut d = StreamDecoder::new(p);
        let mut text = String::new();
        let mut usage = Vec::new();
        let mut done = false;
        for l in lines {
            let out = d.data(l)?;
            text.push_str(out.text.as_deref().unwrap_or_default());
            usage.extend(out.usage);
            if out.done {
                done = true;
                break;
            }
        }
        usage.extend(d.finish());
        Ok((text, usage, done))
    }

    #[test]
    fn openrouter_stream() {
        let lines = [
            r#"{"choices":[{"delta":{"content":"Hel"}}]}"#,
            "not json",
            r#"{"choices":[{"delta":{"content":"lo"}}],"usage":{"prompt_tokens":5,"completion_tokens":2,"cost":0.001}}"#,
            "[DONE]",
            r#"{"choices":[{"delta":{"content":"never"}}]}"#,
        ];
        let (text, usage, done) = decode(Provider::OpenRouter, &lines).unwrap();
        assert_eq!(text, "Hello");
        assert!(done);
        assert_eq!(usage[0].prompt, 5);
        assert_eq!(usage[0].cost, Some(0.001));
        let busy = r#"{"error":{"code":429,"message":"busy"}}"#;
        let err = decode(Provider::OpenRouter, &[busy]);
        assert_eq!(err.unwrap_err(), ApiError::http(429, "busy"));
        let err = decode(Provider::OpenRouter, &[r#"{"error":{"message":"x"}}"#]);
        assert_eq!(err.unwrap_err().status, Some(500));
        let bad = r#"{"error":{"code":"bad","message":"x"}}"#;
        let err = decode(Provider::OpenRouter, &[bad]);
        assert_eq!(err.unwrap_err().status, None);
    }

    #[test]
    fn anthropic_stream() {
        let lines = [
            r#"{"type":"message_start","message":{"usage":{"input_tokens":7,"output_tokens":1}}}"#,
            r#"{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"hm"}}"#,
            r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hi"}}"#,
            r#"{"type":"ping"}"#,
            r#"{"type":"message_delta","usage":{"output_tokens":9}}"#,
            r#"{"type":"message_stop"}"#,
        ];
        let (text, usage, done) = decode(Provider::Anthropic, &lines).unwrap();
        assert_eq!(text, "Hi");
        assert!(done);
        assert_eq!(usage, [tokens(7, 9)]);
        let over = r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;
        let err = decode(Provider::Anthropic, &[over]).unwrap_err();
        assert_eq!(err, ApiError::http(529, "Overloaded"));
    }

    #[test]
    fn gemini_body_has_no_temperature() {
        let body = json!({ "model": "g", "temperature": 0.2, "messages": [
            { "role": "user", "content": "hi" }] });
        let b = gemini_body(&body);
        assert!(b["generationConfig"].get("temperature").is_none());
        let max = &b["generationConfig"]["maxOutputTokens"];
        assert_eq!(max, &json!(NATIVE_MIN_TOKENS));
    }

    #[test]
    fn refusals_are_coded() {
        let declined = r#"{"type":"message_delta","delta":{"stop_reason":"refusal"}}"#;
        let err = decode(Provider::Anthropic, &[declined]).unwrap_err();
        assert_eq!(err.code, Some(codes::REFUSED));
        let blocked = r#"{"candidates":[{"finishReason":"SAFETY"}]}"#;
        let err = decode(Provider::Gemini, &[blocked]).unwrap_err();
        assert_eq!(err.code, Some(codes::REFUSED));
        let prompt = json!({ "promptFeedback": { "blockReason": "OTHER" } });
        assert!(refusal(Provider::Gemini, &prompt).is_some());
        let done = r#"{"candidates":[{"finishReason":"STOP"}]}"#;
        assert!(decode(Provider::Gemini, &[done]).is_ok());
    }

    #[test]
    fn gemini_stream() {
        let lines = [
            r#"{"candidates":[{"content":{"parts":[{"text":"why","thought":true},{"text":"An"}]}}],"usageMetadata":{"promptTokenCount":3,"candidatesTokenCount":1}}"#,
            r#"{"candidates":[{"content":{"parts":[{"text":"swer"}]}}],"usageMetadata":{"promptTokenCount":3,"candidatesTokenCount":4}}"#,
        ];
        let (text, usage, _) = decode(Provider::Gemini, &lines).unwrap();
        assert_eq!(text, "Answer");
        assert_eq!(usage, [tokens(3, 4)]);
    }

    #[test]
    fn turns_in_every_shape() {
        let or = json!({ "choices": [{ "message": { "content": "ok", "tool_calls": [
            { "id": "", "function": { "name": "type", "arguments": "{bad" } },
            { "id": "c2", "function": { "name": "wait", "arguments": "" } }
        ] } }], "usage": { "prompt_tokens": 1, "completion_tokens": 2 } });
        let t = parse_turn(Provider::OpenRouter, &or, 42);
        assert_eq!(t.content, "ok");
        assert_eq!(t.tool_calls[0].id, "call_42_0");
        assert_eq!(t.tool_calls[0].args, json!({ "__unparsed": "{bad" }));
        assert_eq!(t.tool_calls[1].args, json!({}));
        assert_eq!(t.usage.unwrap().cost, None);

        let an = json!({ "content": [
            { "type": "thinking", "thinking": "", "signature": "s" },
            { "type": "text", "text": "Plan" },
            { "type": "tool_use", "id": "toolu_1", "name": "find", "input": { "query": "save" } }
        ], "usage": { "input_tokens": 10, "output_tokens": 3 } });
        let t = parse_turn(Provider::Anthropic, &an, 0);
        assert_eq!(t.content, "Plan");
        assert_eq!(t.tool_calls[0].id, "toolu_1");
        assert_eq!(t.tool_calls[0].args["query"], json!("save"));
        assert_eq!(t.native.unwrap()["anthropic"][0]["type"], json!("thinking"));

        let ge = json!({ "candidates": [{ "content": { "parts": [
            { "functionCall": { "name": "wait", "args": { "duration": 1 } },
              "thoughtSignature": "sig" }
        ] } }] });
        let t = parse_turn(Provider::Gemini, &ge, 7);
        assert_eq!(t.tool_calls[0].id, "call_7_0");
        assert_eq!(t.tool_calls[0].args["duration"], json!(1));
        let native = t.native.unwrap();
        assert_eq!(native["gemini"][0]["thoughtSignature"], json!("sig"));
    }

    #[test]
    fn http_errors_carry_the_body_message() {
        let e = error_from_body(429, br#"{"error":{"message":"Rate limit exceeded"}}"#);
        assert_eq!(e, ApiError::http(429, "Rate limit exceeded"));
        assert_eq!(error_from_body(502, b"<html>").message, "HTTP 502");
    }

    #[test]
    fn key_checks_spend_nothing() {
        for p in Provider::ALL {
            let r = key_request(p, "k");
            assert_eq!(r.method, Method::Get);
            assert_eq!(r.timeout, Some(Duration::from_secs(8)));
        }
        assert_eq!(key_request(Provider::OpenRouter, "k").url, OPENROUTER_KEY);
    }
}
