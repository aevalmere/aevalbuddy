//! `AEVAL_FAKE_LLM=1`: every chat request is answered from fixtures in
//! tools/fixtures/llm/*.jsonl instead of the network, so Find and the agent
//! behave the same way on every run of the smoke test (PARITY MODEL-08).
//! The catalogue fetch is not faked. Port of lib/fake-llm.js; see that file
//! for the fixture format.
//!
//! The fixtures ship inside the binary, so the installed exe on a CI runner
//! answers the same way a dev build does. `AEVAL_FAKE_LLM_DIR` points at a
//! folder of extra .jsonl files read at startup.

use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::Mutex;
use regex::Regex;
use serde_json::{json, Map, Value};
use tokio_util::sync::CancellationToken;

use super::client::{Meta, Reply};
use super::errors::ApiError;

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
}

/// The fixtures, the `{{name}}` variables and the request log.
#[derive(Default)]
pub struct Fake {
    fixtures: Vec<Value>,
    vars: Mutex<HashMap<String, Value>>,
    requests: Mutex<VecDeque<FakeRequest>>,
    call_seq: Mutex<u64>,
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
    RE.get_or_init(|| Regex::new(r"\{\{([A-Za-z0-9_]+)\}\}").expect("var pattern"))
}

fn span_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\[([0-9]+)\]\s?([^\r  ]*)$").expect("span pattern"))
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
    v["delayMs"].as_f64().filter(|d| *d > 0.0).map_or(0, |d| d as u64)
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
            fixtures,
            ..Fake::default()
        }
    }

    /// Fills `{{name}}` in later answers (a window handle the test only
    /// learns once Notepad is open).
    pub fn set_var(&self, name: &str, value: Value) {
        self.vars.lock().insert(name.to_string(), value);
    }

    /// Every request body so far, newest last. The smoke run reads it to
    /// prove a password never reached a model.
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

    fn first(&self, purpose: &str, pred: impl Fn(&Value) -> bool) -> Option<&Value> {
        self.fixtures
            .iter()
            .find(|x| x["purpose"].as_str() == Some(purpose) && pred(x))
    }

    /// Answers one chat request as OpenRouter would.
    pub async fn respond(
        &self,
        body: &Value,
        cancel: &CancellationToken,
        meta: &Meta,
    ) -> Result<Reply, ApiError> {
        let purpose = if meta.purpose.is_empty() {
            "answer"
        } else {
            meta.purpose.as_str()
        };
        {
            let mut log = self.requests.lock();
            log.push_back(FakeRequest {
                purpose: purpose.to_string(),
                model: body["model"].as_str().map(str::to_string),
                body: body.to_string(),
            });
            if log.len() > REQUEST_LOG {
                log.pop_front();
            }
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
                let q = lower(meta.query.as_deref());
                let f = self.first(purpose, |x| lower(x["query"].as_str()) == q);
                if let Some(f) = f {
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
                let q = lower(meta.query.as_deref());
                let mut out = Vec::new();
                let matching = self.fixtures.iter().filter(|x| {
                    x["purpose"] == "find-correlate" && lower(x["query"].as_str()) == q
                });
                for f in matching {
                    let want = lower(f["match"].as_str());
                    for line in user_text.split('\n') {
                        let Some(m) = span_re().captures(line) else {
                            continue;
                        };
                        if !lower(Some(&m[2])).contains(&want) {
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
            "agent" => self.agent_turn(body, msgs, cancel, meta).await,
            _ => {
                let f = self.first(purpose, |_| true);
                let text = match f {
                    Some(f) => self.fill(&f["text"]),
                    None if purpose == "reader" => json!(READER_DEFAULT),
                    None => json!(ANSWER_DEFAULT),
                };
                if let Some(f) = f {
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
        body: &Value,
        msgs: &[Value],
        cancel: &CancellationToken,
        meta: &Meta,
    ) -> Result<Reply, ApiError> {
        let task = lower(meta.task.as_deref());
        let f = self.first("agent", |x| task.contains(&lower(x["task"].as_str())));
        let turn = msgs.iter().filter(|m| m["role"] == "assistant").count();
        let step = match f {
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
        let text = step["text"].as_str().filter(|t| !t.is_empty()).unwrap_or("Done.");
        if tools {
            return Ok(message(body, json!({ "role": "assistant", "content": text })));
        }
        let done = json!({ "done": text });
        let content = format!("```json\n{done}\n```");
        Ok(message(body, json!({ "role": "assistant", "content": content })))
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
        text(block(fake.respond(&body, &cancel, meta)).unwrap())
    }

    #[test]
    fn embedded_fixtures_load() {
        let fake = Fake::load();
        assert!(fake.fixtures.len() >= 10);
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
        assert_eq!(content, "```json\n{\"tool\":\"focus_window\",\"handle\":\"4242\"}\n```");
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
        assert_eq!((status, out.as_str()), (429, r#"{"error":{"message":"fake failure"}}"#));
        let cancel = CancellationToken::new();
        cancel.cancel();
        let e = block(fake.respond(&json!({}), &cancel, &agent_meta("slow")));
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
        assert!(out.contains(r#"{\"id\":3,\"score\":0.9,\"why\":\"the balance\"}\n"#));
        assert!(!out.contains(r#"\"id\":0"#));
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
