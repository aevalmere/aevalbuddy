//! The seam between split (b) and the settings store that split (a) owns
//! (ARCHITECTURE.md section 10). The store implements [`LlmSettings`]; this
//! side reads keys, the provider order and model choices through it.
//!
//! Providers are tried in the person's order (`providerOrder`, Decision
//! D-13): the first ready one answers, and failover moves down the list. A
//! key is only ever sent to the provider it belongs to.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::fmt;

use super::catalogue::Kind;
use super::payloads::FailoverPrefs;
use super::provider::Provider;
use super::status::{self, Table};

/// What the model layer needs from the settings store.
pub trait LlmSettings: Send + Sync {
    /// The saved key for one provider, decrypted, or `None`.
    fn api_key(&self, provider: Provider) -> Option<String>;
    /// The legacy `provider` override from before `providerOrder`; it moves
    /// to the top of the order when no order is stored.
    fn provider_override(&self) -> Option<Provider>;
    /// `providerOrder`: the order providers are tried in, empty when the
    /// person never changed it.
    fn provider_order(&self) -> Vec<Provider> {
        Vec::new()
    }
    /// The stored OpenRouter model id for a job (`model`, `brain`,
    /// `findModel`, `agentModel` in aevalbuddy.json), default included.
    fn model(&self, kind: Kind) -> String;
    /// Rewrites a chosen OpenRouter id that left the catalogue
    /// (`healModel`, `healBrain`, `healAgentModel`; a no-op for an id the
    /// user never chose).
    fn heal_model(&self, kind: Kind, id: &str);
    /// The user's model for a job on a native provider, if they picked one.
    fn native_model(&self, provider: Provider, kind: Kind) -> Option<String>;
    /// `showPaidModels`.
    fn show_paid(&self) -> bool;
    /// `failover` in aevalbuddy.json.
    fn failover(&self) -> FailoverPrefs {
        FailoverPrefs::default()
    }
}

/// The provider and key a request goes out with.
#[derive(Clone, PartialEq, Eq)]
pub struct Active {
    pub provider: Provider,
    pub key: String,
}

// Keys never reach a log, even through `{:?}`.
impl fmt::Debug for Active {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Active")
            .field("provider", &self.provider)
            .field("key", &"<redacted>")
            .finish()
    }
}

const KINDS: [Kind; 4] = [Kind::Vision, Kind::Text, Kind::Find, Kind::Agent];

/// One provider failover may send a request to, with its key and the model
/// each job would use on it (contract K-16).
#[derive(Clone, PartialEq, Eq)]
pub struct Candidate {
    pub provider: Provider,
    pub key: String,
    models: [String; 4],
}

impl fmt::Debug for Candidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Candidate")
            .field("provider", &self.provider)
            .field("key", &"<redacted>")
            .field("models", &self.models)
            .finish()
    }
}

impl Candidate {
    fn new(s: &dyn LlmSettings, provider: Provider, key: String) -> Candidate {
        Candidate {
            provider,
            key,
            models: KINDS.map(|k| model_for(s, provider, k)),
        }
    }

    /// The provider and key, for a request.
    pub fn active(&self) -> Active {
        Active {
            provider: self.provider,
            key: self.key.clone(),
        }
    }

    /// `model_for(settings, provider, kind)`, as it was when the list was
    /// made.
    pub fn model(&self, kind: Kind) -> &str {
        let i = KINDS.iter().position(|k| *k == kind).unwrap_or(0);
        &self.models[i]
    }
}

fn key_of(s: &dyn LlmSettings, p: Provider) -> Option<String> {
    s.api_key(p)
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

/// The key saved for `p`, when `p` can use it: a blank key, or one whose
/// prefix names another provider (an `sk-or-` key saved under Anthropic),
/// is never sent (review-llm-agent P2-12).
pub fn usable_key(s: &dyn LlmSettings, p: Provider) -> Option<String> {
    let key = key_of(s, p)?;
    match Provider::detect(&key) {
        Some(owner) if owner != p => None,
        _ => Some(key),
    }
}

/// The order providers are tried in: the stored `providerOrder` (unknown
/// and repeated ids dropped, missing providers appended in the default
/// order); without one, the legacy `provider` override first. The default
/// is OpenRouter, Anthropic, OpenAI, Gemini: free models stay the default
/// path.
pub fn order(s: &dyn LlmSettings) -> Vec<Provider> {
    let stored = s.provider_order();
    let mut out: Vec<Provider> = Vec::new();
    let lead: Vec<Provider> = if stored.is_empty() {
        s.provider_override().into_iter().collect()
    } else {
        stored
    };
    for p in lead.into_iter().chain(Provider::ALL) {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// Providers in order with a key each can use, cooldowns ignored. The last
/// resort when every candidate is cooling down: a question still goes
/// somewhere and comes back with that provider's own error.
pub fn keyed(s: &dyn LlmSettings) -> Vec<Candidate> {
    order(s)
        .into_iter()
        .filter_map(|p| usable_key(s, p).map(|k| Candidate::new(s, p, k)))
        .collect()
}

/// The providers a screen question can go to now, in the person's order
/// (contract K-16). Skips providers with no key, a key that belongs to
/// another provider, a rejected key, a native account with no credit, a
/// provider busy until its cooldown ends, and OpenRouter with free models
/// refused unless a paid OpenRouter model is picked.
pub fn candidates(s: &dyn LlmSettings) -> Vec<Candidate> {
    candidates_for(s, Kind::Vision)
}

/// [`candidates`] for one job: the model the job uses decides whether
/// OpenRouter's "free models off" and "no credit" marks apply.
pub fn candidates_for(s: &dyn LlmSettings, kind: Kind) -> Vec<Candidate> {
    candidates_with(s, kind, &status::snapshot(), status::now_ms())
}

/// [`candidates_for`] against a given status table and time.
pub fn candidates_with(s: &dyn LlmSettings, kind: Kind, table: &Table, now: u64) -> Vec<Candidate> {
    keyed(s)
        .into_iter()
        .filter(|c| {
            let paid = c.provider == Provider::OpenRouter && !c.model(kind).ends_with(":free");
            table.usable(c.provider, &c.key, now, paid)
        })
        .collect()
}

/// The provider and key the next question goes out with: the first
/// candidate, else the first provider with a usable key even if it is
/// cooling down. `None` is the Electron "needs-key" state.
pub fn active(s: &dyn LlmSettings) -> Option<Active> {
    candidates(s)
        .into_iter()
        .next()
        .or_else(|| keyed(s).into_iter().next())
        .map(|c| c.active())
}

/// The model a job uses on a provider. OpenRouter reads the stored id (the
/// live-catalogue healing happens in client.rs); a native provider reads the
/// user's pick for it, else its default.
pub fn model_for(s: &dyn LlmSettings, provider: Provider, kind: Kind) -> String {
    if provider == Provider::OpenRouter {
        return s.model(kind);
    }
    s.native_model(provider, kind)
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| provider.default_model(kind).to_string())
}

/// An in-memory store for tests and for the self-test harness.
#[derive(Default)]
// Test seam.
#[allow(dead_code)]
pub struct MemorySettings {
    pub keys: Mutex<HashMap<Provider, String>>,
    pub provider: Mutex<Option<Provider>>,
    pub order: Mutex<Vec<Provider>>,
    pub models: Mutex<HashMap<Kind, String>>,
    pub native: Mutex<HashMap<(Provider, Kind), String>>,
    pub healed: Mutex<Vec<(Kind, String)>>,
    pub paid: Mutex<bool>,
    pub prefs: Mutex<FailoverPrefs>,
}

impl MemorySettings {
    // Test seam.
    #[allow(dead_code)]
    pub fn with_key(key: &str) -> Self {
        let s = MemorySettings::default();
        if let Some(p) = Provider::detect(key) {
            s.keys.lock().insert(p, key.to_string());
        }
        s
    }
}

impl LlmSettings for MemorySettings {
    fn api_key(&self, provider: Provider) -> Option<String> {
        self.keys.lock().get(&provider).cloned()
    }

    fn provider_override(&self) -> Option<Provider> {
        *self.provider.lock()
    }

    fn provider_order(&self) -> Vec<Provider> {
        self.order.lock().clone()
    }

    fn model(&self, kind: Kind) -> String {
        let stored = self.models.lock().get(&kind).cloned();
        stored.unwrap_or_else(|| super::catalogue::default_for(kind).to_string())
    }

    fn heal_model(&self, kind: Kind, id: &str) {
        self.healed.lock().push((kind, id.to_string()));
        self.models.lock().insert(kind, id.to_string());
    }

    fn native_model(&self, provider: Provider, kind: Kind) -> Option<String> {
        self.native.lock().get(&(provider, kind)).cloned()
    }

    fn show_paid(&self) -> bool {
        *self.paid.lock()
    }

    fn failover(&self) -> FailoverPrefs {
        *self.prefs.lock()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::status::{Status, BUSY_DEFAULT_MS};

    fn all_keys() -> MemorySettings {
        let s = MemorySettings::with_key("sk-or-v1-x");
        s.keys.lock().insert(Provider::Anthropic, "sk-ant-y".into());
        s.keys.lock().insert(Provider::OpenAi, "sk-proj-z".into());
        s.keys.lock().insert(Provider::Gemini, "AIzaW".into());
        s
    }

    fn ids(list: &[Candidate]) -> Vec<Provider> {
        list.iter().map(|c| c.provider).collect()
    }

    #[test]
    fn no_key_is_needs_key() {
        assert_eq!(active(&MemorySettings::default()), None);
        assert!(candidates(&MemorySettings::default()).is_empty());
    }

    #[test]
    fn openrouter_stays_the_default_path() {
        let s = MemorySettings::with_key("sk-or-v1-x");
        s.keys.lock().insert(Provider::Anthropic, "sk-ant-y".into());
        assert_eq!(active(&s).unwrap().provider, Provider::OpenRouter);
        *s.provider.lock() = Some(Provider::Anthropic);
        let a = active(&s).unwrap();
        assert_eq!(a.provider, Provider::Anthropic);
        assert_eq!(a.key, "sk-ant-y");
        // An override without a key falls back to a provider that has one.
        *s.provider.lock() = Some(Provider::Gemini);
        assert_eq!(active(&s).unwrap().provider, Provider::OpenRouter);
    }

    #[test]
    fn a_native_key_alone_is_used() {
        let s = MemorySettings::with_key("sk-proj-z");
        assert_eq!(active(&s).unwrap().provider, Provider::OpenAi);
        assert_eq!(model_for(&s, Provider::OpenAi, Kind::Find), "gpt-6-luna");
        let pick = (Provider::OpenAi, Kind::Find);
        s.native.lock().insert(pick, "gpt-6.1-terra".into());
        assert_eq!(model_for(&s, Provider::OpenAi, Kind::Find), "gpt-6.1-terra");
    }

    #[test]
    fn order_and_skips() {
        let s = all_keys();
        let t = Table::default();
        let default = candidates_with(&s, Kind::Vision, &t, 0);
        let want = [
            Provider::OpenRouter,
            Provider::Anthropic,
            Provider::OpenAi,
            Provider::Gemini,
        ];
        assert_eq!(ids(&default), want);
        // The stored order wins; providers it leaves out follow in the
        // default order, and unknown or repeated entries are ignored.
        *s.order.lock() = vec![Provider::Gemini, Provider::Anthropic, Provider::Gemini];
        let got = ids(&candidates_with(&s, Kind::Vision, &t, 0));
        let want = [
            Provider::Gemini,
            Provider::Anthropic,
            Provider::OpenRouter,
            Provider::OpenAi,
        ];
        assert_eq!(got, want);
        // No key, no candidate.
        s.keys.lock().remove(&Provider::Anthropic);
        s.keys.lock().insert(Provider::OpenAi, "   ".into());
        let got = ids(&candidates_with(&s, Kind::Vision, &t, 0));
        assert_eq!(got, [Provider::Gemini, Provider::OpenRouter]);
        // Each candidate carries the model each job would use on it.
        let c = &candidates_with(&s, Kind::Vision, &t, 0)[0];
        assert_eq!(c.model(Kind::Vision), "gemini-3.8-flash");
        assert_eq!(c.model(Kind::Find), "gemini-3.5-flash-lite");
        assert_eq!(c.active().key, "AIzaW");
        assert!(!format!("{c:?}").contains("AIzaW"));
    }

    #[test]
    fn legacy_provider_migrates_to_top() {
        let s = all_keys();
        *s.provider.lock() = Some(Provider::OpenAi);
        let got = ids(&candidates_with(&s, Kind::Vision, &Table::default(), 0));
        let want = [
            Provider::OpenAi,
            Provider::OpenRouter,
            Provider::Anthropic,
            Provider::Gemini,
        ];
        assert_eq!(got, want);
        // Once an order is stored the override no longer counts.
        *s.order.lock() = vec![Provider::Anthropic];
        let got = ids(&candidates_with(&s, Kind::Vision, &Table::default(), 0));
        assert_eq!(got[0], Provider::Anthropic);
        assert_eq!(got[1], Provider::OpenRouter);
    }

    #[test]
    fn mismatched_key_is_skipped() {
        let s = MemorySettings::default();
        // An OpenRouter key filed under Anthropic never goes to Anthropic.
        s.keys
            .lock()
            .insert(Provider::Anthropic, "sk-or-v1-x".into());
        assert!(candidates(&s).is_empty());
        assert_eq!(active(&s), None);
        assert_eq!(usable_key(&s, Provider::Anthropic), None);
        // A key with no known prefix is trusted to the slot it was saved in.
        s.keys.lock().insert(Provider::Gemini, "custom-key".into());
        assert_eq!(active(&s).unwrap().provider, Provider::Gemini);
    }

    #[test]
    fn cooldowns() {
        let s = all_keys();
        let mut t = Table::default();
        let or = "sk-or-v1-x";
        // 429: busy for Retry-After, or a minute without it.
        t.mark_busy(Provider::OpenRouter, or, Some(5_000), 1_000, "busy");
        let got = ids(&candidates_with(&s, Kind::Vision, &t, 1_000));
        assert_eq!(got[0], Provider::Anthropic);
        let got = ids(&candidates_with(&s, Kind::Vision, &t, 6_000));
        assert_eq!(got[0], Provider::OpenRouter);
        t.mark_busy(Provider::OpenRouter, "sk-or-v1-x", None, 0, "busy");
        let busy = t.get(Provider::OpenRouter, "sk-or-v1-x");
        assert_eq!(busy.busy_until, Some(BUSY_DEFAULT_MS));
        // 401: skipped until the key changes or a test passes.
        t.mark_rejected(Provider::Anthropic, "sk-ant-y", "rejected");
        let got = ids(&candidates_with(&s, Kind::Vision, &t, 0));
        assert!(!got.contains(&Provider::Anthropic));
        s.keys
            .lock()
            .insert(Provider::Anthropic, "sk-ant-new".into());
        let got = ids(&candidates_with(&s, Kind::Vision, &t, 0));
        assert!(got.contains(&Provider::Anthropic));
        t.mark_rejected(Provider::Anthropic, "sk-ant-new", "rejected");
        t.mark_ready(Provider::Anthropic, "sk-ant-new", 0, "Works.", None);
        let got = ids(&candidates_with(&s, Kind::Vision, &t, 0));
        assert!(got.contains(&Provider::Anthropic));
        // 402 on a native account: skipped. On OpenRouter it only rules out
        // paid models.
        t.mark_no_credit(Provider::OpenAi, "sk-proj-z", 0, "empty");
        let got = ids(&candidates_with(&s, Kind::Vision, &t, 0));
        assert!(!got.contains(&Provider::OpenAi));
        t.mark_no_credit(Provider::OpenRouter, "sk-or-v1-x", 0, "empty");
        let got = ids(&candidates_with(&s, Kind::Vision, &t, 1_000_000));
        assert_eq!(got[0], Provider::OpenRouter);
        s.models
            .lock()
            .insert(Kind::Vision, "qwen/qwen3.7-flash".into());
        let got = ids(&candidates_with(&s, Kind::Vision, &t, 1_000_000));
        assert!(!got.contains(&Provider::OpenRouter));
        // Free models off: OpenRouter is skipped unless a paid model is
        // picked for the job.
        let mut t = Table::default();
        let blocked = status::Entry {
            status: Status::Blocked,
            ..status::Entry::default()
        };
        t.set(Provider::OpenRouter, "sk-or-v1-x", blocked);
        let got = ids(&candidates_with(&s, Kind::Vision, &t, 0));
        assert_eq!(got[0], Provider::OpenRouter);
        let got = ids(&candidates_with(&s, Kind::Find, &t, 0));
        assert!(!got.contains(&Provider::OpenRouter));
        // Everything cooling down: the question still goes to the first
        // provider with a key.
        let only = MemorySettings::with_key("sk-or-v1-x");
        let mut t = Table::default();
        t.mark_busy(Provider::OpenRouter, "sk-or-v1-x", None, 0, "busy");
        assert!(candidates_with(&only, Kind::Vision, &t, 0).is_empty());
        assert_eq!(keyed(&only)[0].provider, Provider::OpenRouter);
    }
}
