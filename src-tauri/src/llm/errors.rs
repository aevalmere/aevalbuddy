//! Request failures and the sentences the user reads about them (PARITY
//! FAIL-01, ERR-01..13). Port of `ApiError`, `shouldFailover` and
//! `friendlyError` in lib/openrouter.js.

use std::fmt;

use super::provider::Provider;

/// What went wrong, in the terms the Electron code branched on: an HTTP
/// status (`ApiError`), an abort (`AbortError`), a dropped connection
/// (`TypeError` from fetch), or anything else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Http,
    Abort,
    Network,
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApiError {
    pub kind: ErrorKind,
    /// The HTTP status, or the numeric `error.code` of a stream error.
    pub status: Option<u16>,
    pub message: String,
    /// `Some("reader")` when the reader stage failed, so the error names the
    /// reader model rather than the brain that never ran.
    pub stage: Option<&'static str>,
}

impl ApiError {
    pub fn http(status: u16, message: impl Into<String>) -> Self {
        Self::coded(Some(status), message)
    }

    /// A stream or body error; `status` is `None` when its code is not a
    /// number.
    pub fn coded(status: Option<u16>, message: impl Into<String>) -> Self {
        ApiError {
            kind: ErrorKind::Http,
            status,
            message: message.into(),
            stage: None,
        }
    }

    pub fn abort() -> Self {
        Self::of(ErrorKind::Abort, "aborted")
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self::of(ErrorKind::Network, message)
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self::of(ErrorKind::Other, message)
    }

    fn of(kind: ErrorKind, message: impl Into<String>) -> Self {
        ApiError {
            kind,
            status: None,
            message: message.into(),
            stage: None,
        }
    }

    pub fn with_stage(mut self, stage: &'static str) -> Self {
        self.stage = Some(stage);
        self
    }

    pub fn is_abort(&self) -> bool {
        self.kind == ErrorKind::Abort
    }

    pub fn is_network(&self) -> bool {
        self.kind == ErrorKind::Network
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

/// Failure a different model can answer. Free models are queued and rate
/// limited by design, and a retired id 404s until someone edits the list, so
/// these are the everyday states rather than the exceptional ones.
pub const FAILOVER_STATUS: [u16; 7] = [404, 408, 429, 500, 502, 503, 504];

pub fn should_failover(err: &ApiError) -> bool {
    if err.is_abort() {
        return false;
    }
    // A rejected key, no credit, or a malformed request fails the same way on
    // every model; retrying is just a second wait before the same message.
    err.status.is_some_and(|s| FAILOVER_STATUS.contains(&s))
}

const DATA_POLICY: &str = "Your OpenRouter account blocks free models. Free models are offered \
on the condition that their providers may keep and train on what they receive, which here \
includes your screen. To accept that, turn on \"Free model publication\" at \
openrouter.ai/settings/privacy. To avoid it, add credit and pick a paid model in Settings.";

const UNKNOWN: &str = "Request failed for an unknown reason. Retry, or ask again.";

const FREE_BUSY: &str = "Every free model is busy. Free models take a few requests a minute; \
retry in a moment, or add credit and pick a paid model in Settings.";

const RATE_LIMITED: &str = "Rate limited by the provider. Wait a moment, then retry.";

const OR_SERVER: &str = "OpenRouter returned a server error, on this model and on the \
fallback. Retry in a moment.";

const OFFLINE: &str = "No connection. Finding words on your screen still works; answers need \
the internet.";

fn mentions(detail: &str, words: &[&str]) -> bool {
    let lower = detail.to_lowercase();
    words.iter().any(|w| lower.contains(w))
}

/// Maps an error to a message that names the failure and one next action,
/// exactly as lib/openrouter.js words it. `None` for aborts, which need no
/// message. `err == None` is the JS `friendlyError(undefined)` case.
pub fn friendly_error(err: Option<&ApiError>, model: &str) -> Option<String> {
    friendly_error_for(Provider::OpenRouter, err, model)
}

/// The same mapping for any provider. OpenRouter keeps the Electron wording
/// byte for byte; the native providers get the same sentences with their own
/// name, and without the advice that only makes sense on OpenRouter (free
/// models, openrouter.ai links, a fallback model).
pub fn friendly_error_for(
    provider: Provider,
    err: Option<&ApiError>,
    model: &str,
) -> Option<String> {
    let err = match err {
        Some(e) => e,
        None => return Some(UNKNOWN.to_string()),
    };
    if err.is_abort() {
        return None;
    }
    Some(message_for(provider, err, model))
}

fn message_for(provider: Provider, err: &ApiError, model: &str) -> String {
    let p = provider.name();
    let or = provider == Provider::OpenRouter;
    let detail = err.message.as_str();
    // OpenRouter refuses :free models until the account opts in, and reports
    // it with a data-policy message on various status codes.
    if or && mentions(detail, &["data policy", "free model publication"]) {
        return DATA_POLICY.to_string();
    }
    let status = err.status.unwrap_or(0);
    if status == 401 {
        return format!(
            "{p} rejected the API key. Open Settings, check the key, and save it again."
        );
    }
    if status == 402 && or {
        return format!(
            "{model} bills per token and this account has no credit. Add a dollar at \
openrouter.ai/credits (it lasts thousands of questions at this rate), or open Settings and pick \
one from the Free group."
        );
    }
    if status == 402 {
        return format!(
            "{model} bills per token and this {p} account has no credit. Add credit to the \
{p} account, or open Settings and use another key."
        );
    }
    if status == 403 {
        return format!("{p} refused the request: {detail}");
    }
    if status == 404 && or {
        return format!(
            "{model} is not available right now, and the free model tried after it was not \
either. OpenRouter says: {detail}. Pick another model in Settings."
        );
    }
    if status == 404 {
        return format!(
            "{model} is not available right now. {p} says: {detail}. Pick another model in \
Settings."
        );
    }
    if status == 429 {
        let free = or && model.ends_with(":free");
        return if free { FREE_BUSY } else { RATE_LIMITED }.to_string();
    }
    if status == 400 {
        if mentions(detail, &["image", "vision", "multimodal", "modality"]) {
            return format!(
                "The model {model} does not accept images. Pick another Screen model in \
Settings, or turn on the Answer model so a screen model reads for it."
            );
        }
        return format!("Request rejected: {detail}");
    }
    if status == 204 {
        return format!(
            "The model {model} streamed no answer text. Pick a different model in Settings."
        );
    }
    if status >= 500 {
        if or {
            return OR_SERVER.to_string();
        }
        return format!("{p} returned a server error. Retry in a moment.");
    }
    if err.is_network() {
        return OFFLINE.to_string();
    }
    let shown = if detail.is_empty() { "Error" } else { detail };
    format!("Request failed: {shown}. Retry, or ask again.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const TABLE: &str = include_str!("../../../tools/fixtures/llm/errors.json");

    #[test]
    fn failover_statuses_are_the_everyday_ones() {
        for s in [404, 408, 429, 500, 502, 503, 504] {
            assert!(should_failover(&ApiError::http(s, "x")), "{s}");
        }
        for s in [400, 401, 402, 403, 204] {
            assert!(!should_failover(&ApiError::http(s, "x")), "{s}");
        }
        assert!(!should_failover(&ApiError::abort()));
        assert!(!should_failover(&ApiError::network("fetch failed")));
    }

    #[test]
    fn error_table_is_verbatim() {
        let rows: Vec<Value> = serde_json::from_str(TABLE).unwrap();
        assert!(rows.len() >= 20);
        for row in rows {
            let provider = Provider::from_id(row["provider"].as_str().unwrap()).unwrap();
            let detail = row["detail"].as_str().unwrap();
            let model = row["model"].as_str().unwrap();
            let err = match row["kind"].as_str() {
                Some("network") => Some(ApiError::network(detail)),
                Some("other") => Some(ApiError::other(detail)),
                Some("abort") => Some(ApiError::abort()),
                Some(_) => None,
                None => {
                    let status = row["status"].as_u64().unwrap() as u16;
                    Some(ApiError::http(status, detail))
                }
            };
            let got = friendly_error_for(provider, err.as_ref(), model);
            assert_eq!(got.as_deref(), row["text"].as_str(), "{row}");
        }
    }
}
