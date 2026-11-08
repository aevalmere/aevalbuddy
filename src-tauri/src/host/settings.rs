//! `LlmSettings` over split (a)'s store (ARCHITECTURE.md section 12), and
//! the Settings channels about providers, keys and models (contract K-15):
//! `route` runs first from `host::ask::route`, so a channel listed here is
//! answered here and never by the core rows in ipc.rs. The list of channels
//! it owns is kept in `.claude/rust-rewrite/entries/llm.md`.

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::llm::catalogue::Kind;
use crate::llm::provider::Provider;
use crate::llm::settings::LlmSettings;
use crate::store;
use crate::windows;

/// The settings store, read live on every call.
pub struct StoreSettings;

impl LlmSettings for StoreSettings {
    fn api_key(&self, provider: Provider) -> Option<String> {
        let k = store::get().get_provider_key(provider.id());
        (!k.trim().is_empty()).then_some(k)
    }

    /// `provider` in aevalbuddy.json, the override from before
    /// `providerOrder`.
    fn provider_override(&self) -> Option<Provider> {
        store::get()
            .raw("provider")
            .and_then(|v| v.as_str().and_then(Provider::from_id))
    }

    /// `providerOrder` in aevalbuddy.json: provider ids, first tried first.
    fn provider_order(&self) -> Vec<Provider> {
        let Some(Value::Array(ids)) = store::get().raw("providerOrder") else {
            return Vec::new();
        };
        ids.iter()
            .filter_map(|v| v.as_str().and_then(Provider::from_id))
            .collect()
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

fn p_str(p: &Value, key: &str) -> String {
    match p.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(v) => v.to_string(),
    }
}

/// The provider a payload names, `openrouter` when it names none.
fn provider_of(p: &Value) -> String {
    match p_str(p, "provider").trim() {
        "" => "openrouter".into(),
        named => named.to_string(),
    }
}

fn clipboard_text(app: &AppHandle) -> String {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().read_text().unwrap_or_default()
}

/// `settings:save-key`: one key field takes any of the four providers'
/// keys; the prefix says whose it is unless the page names the provider.
fn save_key(app: &AppHandle, p: &Value) -> Value {
    let s = store::get();
    let value = p_str(p, "key").trim().to_string();
    if value.is_empty() {
        return json!({ "ok": false, "error": "Enter a key before saving." });
    }
    let provider = match p_str(p, "provider").trim() {
        "" => Provider::detect(&value)
            .map(|v| v.id().to_string())
            .unwrap_or_else(|| "openrouter".into()),
        named => named.to_string(),
    };
    if !store::PROVIDERS.contains(&provider.as_str()) {
        return json!({ "ok": false, "error": "Unknown provider." });
    }
    s.set_provider_key(&provider, &value);
    windows::send_overlay_state(app);
    super::key_saved(app, &provider);
    json!({ "ok": true, "keys": s.provider_keys_present() })
}

/// `settings:clear-key {provider}`.
fn clear_key(app: &AppHandle, p: &Value) -> Value {
    let s = store::get();
    let provider = provider_of(p);
    if !store::PROVIDERS.contains(&provider.as_str()) {
        return json!({ "ok": false, "error": "Unknown provider." });
    }
    s.clear_provider_key(&provider);
    windows::send_overlay_state(app);
    json!({ "ok": true, "keys": s.provider_keys_present() })
}

/// The four model pickers.
fn save_model(channel: &str, p: &Value) -> Value {
    let s = store::get();
    let model = p_str(p, "model");
    match channel {
        "settings:save-brain" => json!({ "ok": true, "brain": s.set_brain(&model) }),
        "settings:save-find-model" => json!({ "ok": true, "model": s.set_find_model(&model) }),
        "settings:save-agent-model" => json!({ "ok": true, "model": s.set_agent_model(&model) }),
        _ => json!({ "ok": true, "model": s.set_model(&model) }),
    }
}

/// `settings:clip-key`: an OpenRouter key on the clipboard, masked, while
/// no key is saved. The key stays in Rust.
fn clip_key(app: &AppHandle) -> Value {
    let k = if store::get().has_api_key() {
        String::new()
    } else {
        crate::ipc::clip_key_of(&clipboard_text(app))
    };
    json!({ "found": !k.is_empty(), "masked": crate::ipc::mask_key(&k) })
}

/// `settings:use-clip-key`: saves the clipboard's key.
fn use_clip_key(app: &AppHandle) -> Value {
    let k = crate::ipc::clip_key_of(&clipboard_text(app));
    if k.is_empty() {
        return json!({
            "ok": false,
            "error": "The clipboard no longer holds an OpenRouter key."
        });
    }
    store::get().set_api_key(&k);
    windows::send_overlay_state(app);
    super::key_saved(app, "openrouter");
    json!({ "ok": true })
}

/// `settings:test-key`: the named provider's key, else the one in use.
pub async fn test_key(p: &Value) -> Value {
    let sv = super::services();
    let named = p
        .get("provider")
        .and_then(Value::as_str)
        .and_then(Provider::from_id);
    let provider = named
        .or_else(|| crate::llm::settings::active(&*sv.settings).map(|a| a.provider))
        .unwrap_or(Provider::OpenRouter);
    let key = store::get().get_provider_key(provider.id());
    sv.llm.test_key(provider, &key).await.to_json()
}

/// The Settings channels about providers, keys and models (contract K-15).
/// `None` for any other channel, which falls through to the next router.
pub async fn route(
    app: &AppHandle,
    _label: &str,
    channel: &str,
    p: &Value,
) -> Option<Result<Value, String>> {
    let out = match channel {
        "settings:save-key" => save_key(app, p),
        "settings:clear-key" => clear_key(app, p),
        "settings:save-model"
        | "settings:save-brain"
        | "settings:save-find-model"
        | "settings:save-agent-model" => save_model(channel, p),
        "settings:clip-key" => clip_key(app),
        "settings:use-clip-key" => use_clip_key(app),
        _ => return None,
    };
    Some(Ok(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_provider_defaults_to_openrouter() {
        assert_eq!(provider_of(&json!({})), "openrouter");
        assert_eq!(provider_of(&json!({ "provider": " gemini " })), "gemini");
        assert_eq!(p_str(&json!({ "key": 5 }), "key"), "5");
    }
}
