//! Talking to models: the curated free-model table, the four providers
//! (OpenRouter, Anthropic, OpenAI, Gemini) behind one request shape,
//! failover, streaming, prompts and error text. Port of lib/openrouter.js
//! and lib/fake-llm.js plus the model half of main.js (PARITY MODEL-*,
//! FAIL-*, PROMPT-*, POINT-*, ERR-*). ARCHITECTURE.md section 10 documents
//! the seams.

pub mod auth;
pub mod catalogue;
pub mod client;
pub mod elements;
pub mod errors;
pub mod failover;
pub mod fake;
// Contract K-19: the renderer payloads, constructed as B-038, B-119, B-121
// and B-122 land.
#[allow(dead_code)]
pub mod payloads;
pub mod points;
pub mod prompt;
pub mod provider;
pub mod settings;
pub mod sse;
pub mod status;
pub mod wire;

use serde::Serialize;
use serde_json::Value;

use self::catalogue::Kind;
use self::provider::Provider;
use self::settings::{Candidate, LlmSettings};

/// Who receives the screen for a question (design-ux U6, contract K-18): a
/// provider that can answer now, with the model it would use for a screen
/// question.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipient {
    pub id: &'static str,
    pub name: &'static str,
    pub model: String,
    pub model_name: String,
    /// A free OpenRouter model: its provider may keep and train on what it
    /// receives.
    pub free: bool,
}

/// The ready providers in try order, each with the screen model it would
/// use (contract K-18). Disclosures name these, never a fixed company.
// Contract K-18 for agent and bar; host uses recipients_with.
#[allow(dead_code)]
pub fn recipients(s: &dyn LlmSettings) -> Vec<Recipient> {
    recipients_with(s, None)
}

/// [`recipients`] with the live catalogue for model names and for healing
/// a retired OpenRouter pick.
pub fn recipients_with(s: &dyn LlmSettings, all: Option<&[Value]>) -> Vec<Recipient> {
    recipients_of(settings::candidates(s), all)
}

fn recipients_of(list: Vec<Candidate>, all: Option<&[Value]>) -> Vec<Recipient> {
    list.into_iter()
        .map(|c| {
            let stored = c.model(Kind::Vision);
            let model = if c.provider == Provider::OpenRouter {
                catalogue::pick_live(Kind::Vision, stored, all)
            } else {
                stored.to_string()
            };
            Recipient {
                id: c.provider.id(),
                name: c.provider.name(),
                model_name: catalogue::model_name(&model, all),
                free: c.provider == Provider::OpenRouter && model.ends_with(":free"),
                model,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::settings::{candidates_with, MemorySettings};
    use crate::llm::status::Table;

    #[test]
    fn recipients_follow_order_and_skip_unready() {
        let s = MemorySettings::with_key("sk-or-v1-x");
        s.keys.lock().insert(Provider::Anthropic, "sk-ant-y".into());
        *s.order.lock() = vec![Provider::Anthropic, Provider::OpenRouter];
        let t = Table::default();
        let list = recipients_of(candidates_with(&s, Kind::Vision, &t, 0), None);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "anthropic");
        assert_eq!(list[0].name, "Anthropic");
        assert_eq!(list[0].model, "claude-sonnet-5-5");
        assert_eq!(list[0].model_name, "Claude Sonnet 5.5");
        assert!(!list[0].free);
        assert_eq!(list[1].id, "openrouter");
        assert!(list[1].free);
        // A rejected key is not a recipient.
        let mut t = Table::default();
        t.mark_rejected(Provider::Anthropic, "sk-ant-y", "rejected");
        let list = recipients_of(candidates_with(&s, Kind::Vision, &t, 0), None);
        let ids: Vec<&str> = list.iter().map(|r| r.id).collect();
        assert_eq!(ids, ["openrouter"]);
        let v = serde_json::to_value(&list[0]).unwrap();
        assert!(v.get("modelName").is_some());
    }
}
