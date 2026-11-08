//! The shapes Rust sends to the pages for providers and answers (contract
//! K-19). The renderer lanes build against these field names; the mocks in
//! `tools/ui-shots/lanes/llm.mjs` repeat them. Field names are camelCase on
//! the wire. Optional fields are left out when empty, so older pages that
//! read only the old fields keep working.

use serde::{Deserialize, Serialize};

/// How a `query:error` row looks: a failure, or a neutral note (busy free
/// models counting down, the question before switching to a paid provider).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    #[default]
    Error,
    Note,
}

/// One button under an error or a note, in the order shown. `id` is what
/// the page acts on:
///
/// - `retry`: `query:retry` for this window.
/// - `try-now`: retry now instead of waiting out the countdown.
/// - `hop`: `query:retry {provider, model}` on the named provider.
/// - `wait`: keep waiting (the countdown continues).
/// - `never-ask`: `settings:set-failover {askBeforePaid: false}`, then `hop`.
/// - `fix-key`: open Settings at `section` (`prov-<id>`).
/// - `add-credit`, `privacy`: open `url` in the browser.
/// - `test-again`: `settings:test-key {provider}`.
/// - `free-model`: `settings:save-model {model}` with `model`, then retry.
/// - `choose-model`: open Settings at `section` (`models-<kind>`).
/// - `add-provider`: open Settings at `section` (`providers`).
/// - `copy-details`: copy the error's `detail` text.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
}

impl Action {
    pub fn new(id: &str, label: impl Into<String>) -> Action {
        Action {
            id: id.to_string(),
            label: label.into(),
            ..Action::default()
        }
    }
}

/// listen `query:error`. The output window's `send` adds `gen`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryError {
    /// The sentence shown (design-ux copy deck, Errors).
    pub message: String,
    /// The question this answer was for, for Retry.
    pub question: String,
    /// One of `llm::errors::codes`.
    pub code: String,
    /// The provider id that failed (`openrouter`, `anthropic`, ...).
    pub provider: String,
    /// The model id, for the tooltip and Copy details.
    pub model: String,
    /// The model's display name, for the text.
    pub model_name: String,
    /// A countdown before the next automatic retry, for `free-busy` and
    /// `rate-limited` notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_in_ms: Option<u64>,
    pub tone: Tone,
    pub actions: Vec<Action>,
    /// Raw status and provider message, for Copy details. Never a key.
    pub detail: String,
}

/// `query:done.sent.windowText`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowText {
    /// The window's title.
    pub app: String,
    /// Characters of its text that were sent.
    pub chars: usize,
}

/// `query:done.sent.reader`: the model that read the screen for the answer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderSent {
    pub model: String,
    pub model_name: String,
}

/// `query:done.sent`: exactly what this answer sent, and to whom
/// (design-ux U16). `image` is `"screen"`, `"region"` or absent;
/// `output:sent-image {msgId}` returns the picture while it is one of the
/// window's last three.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sent {
    pub provider: String,
    pub provider_name: String,
    pub model: String,
    pub model_name: String,
    /// A free model: its provider may keep and train on what it received.
    pub free: bool,
    pub image: Option<String>,
    /// `[width, height]` of the picture sent, in pixels.
    pub image_size: Option<[u32; 2]>,
    pub window_text: Option<WindowText>,
    /// Earlier messages of the conversation that went with the question.
    pub history: usize,
    pub reader: Option<ReaderSent>,
    pub msg_id: String,
}

/// `query:done.usage`. The first seven fields are the ones the page already
/// reads.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// The short model id (kept for older pages).
    pub model: String,
    pub ttft_ms: u64,
    pub ms: u64,
    pub prompt: u64,
    pub completion: u64,
    pub cost: Option<f64>,
    pub text_only: bool,
    /// The full id of the model that answered.
    pub model_id: String,
    pub model_name: String,
    pub provider: String,
    pub provider_name: String,
    /// The provider name when the answer came from a provider other than
    /// the first one tried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
    /// Why it moved: `"OpenRouter was busy"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hop_reason: Option<String>,
}

/// One row of `settings:get.providers`, in try order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRow {
    pub id: String,
    pub name: String,
    /// 1-based position in the try order.
    pub order: usize,
    pub has_key: bool,
    /// The masked key (`sk-ant-a…9Q2c`), or empty.
    pub key_hint: String,
    /// `not-added`, `checking`, `ready`, `rejected`, `no-credit`, `busy`,
    /// `blocked`, `unchecked`.
    pub status: String,
    pub status_text: String,
    /// ms since the epoch.
    pub checked_at: Option<u64>,
    /// ms since the epoch.
    pub busy_until: Option<u64>,
    /// OpenRouter credit left, in dollars.
    pub credit: Option<f64>,
}

/// `settings:get.failover` and `settings:set-failover`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailoverPrefs {
    /// When one provider is busy or fails, try the next.
    pub across_providers: bool,
    /// Ask before switching from free models to a paid provider.
    pub ask_before_paid: bool,
}

impl Default for FailoverPrefs {
    fn default() -> Self {
        FailoverPrefs {
            across_providers: true,
            ask_before_paid: true,
        }
    }
}

/// listen `provider:status`, to Settings and the overlay.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatusEvent {
    pub id: String,
    pub status: String,
    pub status_text: String,
    pub busy_until: Option<u64>,
    /// One bar notice when an answer finds a key newly rejected: `Anthropic
    /// rejected your key, so answers come from OpenRouter for now.`, shown
    /// with `Fix key` (settings section `prov-<id>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

/// One entry of `models:list.providers`: a ready provider's models.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModels {
    pub id: String,
    pub name: String,
    /// The same `{tier, label, models}` groups `models:list` returns.
    pub groups: Vec<serde_json::Value>,
}

/// invoke `settings:save-key {provider, key}`: the key is tested before it
/// is stored. On failure `error` says why and `detected` names the provider
/// the key's prefix belongs to, when that is another one.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveKeyReply {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected: Option<String>,
    /// `{openrouter: bool, anthropic: bool, ...}`, for older pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<serde_json::Value>,
}

/// invoke `settings:test-key {provider}`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestKeyReply {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub credit: Option<f64>,
    /// Buttons for a failed test (`privacy`, `add-credit`).
    pub actions: Vec<Action>,
}

/// invoke `settings:clip-key`: a key on the clipboard for a provider that
/// has none. The key itself stays in Rust.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipKeyReply {
    pub found: bool,
    pub masked: String,
    pub provider: Option<String>,
}

/// The optional pin on `query:submit` and `query:retry`: this question goes
/// to this model and never fails over (design-ux U20).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    pub provider: String,
    pub model: String,
}

impl Pin {
    /// `{provider, model}` from an invoke payload, when both are there.
    pub fn from_payload(p: &serde_json::Value) -> Option<Pin> {
        let text = |k: &str| {
            p.get(k)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        Some(Pin {
            provider: text("provider")?,
            model: text("model")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wire_names_are_camel_case() {
        let e = QueryError {
            message: "m".into(),
            code: "free-busy".into(),
            model_name: "Gemma 4 31B".into(),
            retry_in_ms: Some(20_000),
            tone: Tone::Note,
            actions: vec![Action::new("try-now", "Try now")],
            ..QueryError::default()
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["modelName"], json!("Gemma 4 31B"));
        assert_eq!(v["retryInMs"], json!(20_000));
        assert_eq!(v["tone"], json!("note"));
        let first = json!({ "id": "try-now", "label": "Try now" });
        assert_eq!(v["actions"][0], first);
        let row = serde_json::to_value(ProviderRow::default()).unwrap();
        for k in ["hasKey", "keyHint", "statusText", "checkedAt", "busyUntil"] {
            assert!(row.get(k).is_some(), "{k}");
        }
        let f = serde_json::to_value(FailoverPrefs::default()).unwrap();
        assert_eq!(f, json!({ "acrossProviders": true, "askBeforePaid": true }));
        let sent = serde_json::to_value(Sent::default()).unwrap();
        for k in ["providerName", "imageSize", "windowText", "msgId"] {
            assert!(sent.get(k).is_some(), "{k}");
        }
        let usage = serde_json::to_value(Usage::default()).unwrap();
        assert!(usage.get("via").is_none());
        assert!(usage.get("ttftMs").is_some());
    }

    #[test]
    fn a_pin_needs_both_fields() {
        let p = json!({ "provider": "anthropic", "model": "claude-opus-5-5" });
        let pin = Pin::from_payload(&p).unwrap();
        assert_eq!(pin.provider, "anthropic");
        assert_eq!(Pin::from_payload(&json!({ "provider": "anthropic" })), None);
        assert_eq!(Pin::from_payload(&json!({ "model": " " })), None);
    }
}
