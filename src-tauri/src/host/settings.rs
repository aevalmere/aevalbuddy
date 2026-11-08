//! `LlmSettings` over split (a)'s store (ARCHITECTURE.md section 12).

use crate::llm::catalogue::Kind;
use crate::llm::provider::Provider;
use crate::llm::settings::LlmSettings;
use crate::store;

/// The settings store, read live on every call.
pub struct StoreSettings;

impl LlmSettings for StoreSettings {
    fn api_key(&self, provider: Provider) -> Option<String> {
        let k = store::get().get_provider_key(provider.id());
        (!k.trim().is_empty()).then_some(k)
    }

    /// `provider` in aevalbuddy.json, when the user picked one.
    fn provider_override(&self) -> Option<Provider> {
        store::get()
            .raw("provider")
            .and_then(|v| v.as_str().and_then(Provider::from_id))
    }

    fn model(&self, kind: Kind) -> String {
        let s = store::get();
        match kind {
            Kind::Vision => s.get_model(),
            Kind::Text => s.get_brain(),
            Kind::Find => s.get_find_model(),
            Kind::Agent => s.get_agent_model(),
        }
    }

    fn heal_model(&self, kind: Kind, id: &str) {
        let s = store::get();
        match kind {
            Kind::Vision => {
                s.heal_model(id);
            }
            Kind::Text => {
                s.heal_brain(id);
            }
            Kind::Find => s.heal_find_model(id),
            Kind::Agent => s.heal_agent_model(id),
        }
    }

    /// `nativeModels.<provider>.<kind>` in aevalbuddy.json, when set.
    fn native_model(&self, provider: Provider, kind: Kind) -> Option<String> {
        let v = store::get().raw("nativeModels")?;
        let id = v.get(provider.id())?.get(kind.as_str())?.as_str()?.trim();
        (!id.is_empty()).then(|| id.to_string())
    }

    fn show_paid(&self) -> bool {
        store::get().get_show_paid_models()
    }
}
