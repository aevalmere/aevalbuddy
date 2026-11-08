//! The seam between split (b) and the settings store that split (a) owns
//! (ARCHITECTURE.md section 10). The store implements [`LlmSettings`]; this
//! side only reads keys and model choices through it, and writes back only
//! the healed model ids that `liveModels` wrote in Electron.

use parking_lot::Mutex;
use std::collections::HashMap;

use super::catalogue::Kind;
use super::provider::Provider;

/// What the model layer needs from the settings store.
pub trait LlmSettings: Send + Sync {
    /// The saved key for one provider, decrypted, or `None`.
    fn api_key(&self, provider: Provider) -> Option<String>;
    /// The provider the user picked in Settings, if they overrode the one
    /// detected from their keys.
    fn provider_override(&self) -> Option<Provider>;
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
}

/// The provider and key a request goes out with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Active {
    pub provider: Provider,
    pub key: String,
}

fn key_of(s: &dyn LlmSettings, p: Provider) -> Option<String> {
    s.api_key(p)
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

/// Which provider answers: the override when it has a key; else OpenRouter,
/// whose free models stay the default path; else the first native provider
/// with a key. `None` is the Electron "needs-key" state.
pub fn active(s: &dyn LlmSettings) -> Option<Active> {
    let mut order: Vec<Provider> = s.provider_override().into_iter().collect();
    order.extend(Provider::ALL);
    for provider in order {
        if let Some(key) = key_of(s, provider) {
            return Some(Active { provider, key });
        }
    }
    None
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
    pub models: Mutex<HashMap<Kind, String>>,
    pub native: Mutex<HashMap<(Provider, Kind), String>>,
    pub healed: Mutex<Vec<(Kind, String)>>,
    pub paid: Mutex<bool>,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_key_is_needs_key() {
        assert_eq!(active(&MemorySettings::default()), None);
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
}
