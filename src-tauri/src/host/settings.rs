//! `LlmSettings` over split (a)'s store (ARCHITECTURE.md section 12), and
//! the Settings channels about providers, keys and models (contract K-15):
//! `route` runs first from `host::ask::route`, so a channel listed here is
//! answered here and never by the core rows in ipc.rs. The list of channels
//! it owns is kept in `.claude/rust-rewrite/entries/llm.md`.

use std::sync::OnceLock;

use parking_lot::Mutex;
use serde_json::{json, Map, Value};
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;

use crate::llm::auth;
use crate::llm::catalogue::Kind;
use crate::llm::client::{KeyCheck, Llm};
use crate::llm::errors::{self, ApiError};
use crate::llm::payloads::{
    ClipKeyReply, FailoverPrefs, ProviderRow, ProviderStatusEvent, SaveKeyReply,
};
use crate::llm::provider::Provider;
use crate::llm::settings::{self as llm_settings, model_for, LlmSettings};
use crate::llm::status::{self, Checked, Status, Table};
use crate::store;
use crate::windows;

/// The settings store, read live on every call.
pub struct StoreSettings;

/// An OpenRouter id has an owner (`google/gemma-4-26b-a4b-it:free`); a native
/// provider's id does not (`claude-opus-5-5`), and must never be written over
/// an OpenRouter choice.
fn is_openrouter_id(id: &str) -> bool {
    id.contains('/')
}

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
        if !is_openrouter_id(id) {
            return;
        }
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

    fn failover(&self) -> FailoverPrefs {
        store::get()
            .raw("failover")
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
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

/// The status the row shows now, and `provider:status` for Settings and the
/// overlay. `notice` is the one-time bar notice about a newly rejected key.
pub fn emit_status(app: &AppHandle, provider: Provider, notice: Option<String>) {
    let event = status_event(&StoreSettings, &status::snapshot(), provider, notice);
    let payload = json!(event);
    windows::emit(app, windows::SETTINGS, "provider:status", payload.clone());
    windows::emit(app, windows::OVERLAY, "provider:status", payload);
}

fn status_event(
    s: &dyn LlmSettings,
    table: &Table,
    provider: Provider,
    notice: Option<String>,
) -> ProviderStatusEvent {
    let row = row_of(s, table, provider, 0, status::now_ms());
    ProviderStatusEvent {
        id: row.id,
        status: row.status,
        status_text: row.status_text,
        busy_until: row.busy_until,
        notice,
    }
}

fn row_of(
    s: &dyn LlmSettings,
    table: &Table,
    provider: Provider,
    position: usize,
    now: u64,
) -> ProviderRow {
    let base = ProviderRow {
        id: provider.id().to_string(),
        name: provider.name().to_string(),
        order: position,
        ..ProviderRow::default()
    };
    let key = s
        .api_key(provider)
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty());
    let Some(key) = key else {
        return ProviderRow {
            status: Status::NotAdded.as_str().to_string(),
            ..base
        };
    };
    let entry = table.get(provider, &key);
    let (word, text) = status::shown(&entry, now);
    ProviderRow {
        has_key: true,
        key_hint: crate::ipc::mask_key(&key),
        status: word.as_str().to_string(),
        status_text: text,
        checked_at: entry.checked_at,
        busy_until: entry.busy_until.filter(|t| *t > now),
        credit: entry.credit,
        ..base
    }
}

/// The rows of `settings:get.providers`, in try order.
fn provider_rows(s: &dyn LlmSettings, table: &Table, now: u64) -> Vec<ProviderRow> {
    let order = llm_settings::order(s);
    order
        .into_iter()
        .enumerate()
        .map(|(i, p)| row_of(s, table, p, i + 1, now))
        .collect()
}

/// `settings:get` fields about providers (design-ux 3.5).
fn providers_extra(s: &dyn LlmSettings, table: &Table, now: u64, storage: &str) -> Value {
    let rows = provider_rows(s, table, now);
    let hints: Map<String, Value> = rows
        .iter()
        .map(|r| (r.id.clone(), json!(r.key_hint)))
        .collect();
    json!({
        "providers": rows,
        "failover": s.failover(),
        "keyHints": hints,
        "keyStorage": storage,
    })
}

/// [`providers_extra`] as the app is now, for `settings_extra`.
pub fn settings_fields() -> Value {
    let storage = store::get().key_storage();
    providers_extra(&StoreSettings, &status::snapshot(), status::now_ms(), storage)
}

/// The status change an answer's outcome means for its provider (B-046
/// cooldowns): a rejected key, an empty account, or a native provider
/// rate-limiting the account. A busy OpenRouter free model says nothing
/// about OpenRouter, whose other free models still answer. An answer clears
/// any mark. Returns true when what people see changed.
fn record_answer(
    t: &mut Table,
    provider: Provider,
    key: &str,
    outcome: Result<(), &ApiError>,
    model: &str,
    now: u64,
) -> bool {
    let p = provider.name();
    let Err(err) = outcome else {
        return t.mark_answered(provider, key, now);
    };
    match errors::code_of(provider, err, model) {
        errors::codes::BAD_KEY => {
            let text = format!(
                "{p} rejected this key. It may have been revoked or mistyped. AevalBuddy is \
skipping {p} until you replace it."
            );
            t.mark_rejected(provider, key, &text)
        }
        errors::codes::NO_CREDIT if provider.is_native() => {
            let text = format!(
                "The key works, but the {p} account has no credit. AevalBuddy is skipping {p}."
            );
            t.mark_no_credit(provider, key, now, &text)
        }
        errors::codes::RATE_LIMITED | errors::codes::FREE_BUSY if provider.is_native() => {
            let text = format!("{p} is busy right now.");
            t.mark_busy(provider, key, err.retry_after_ms, now, &text)
        }
        _ => false,
    }
}

/// An answer's outcome, into the live table and out to the pages.
pub fn note_answer(
    app: &AppHandle,
    provider: Provider,
    key: &str,
    outcome: Result<(), &ApiError>,
    model: &str,
) {
    if key.trim().is_empty() {
        return;
    }
    let now = status::now_ms();
    let changed = status::update(|t| record_answer(t, provider, key, outcome, model, now));
    if changed {
        emit_status(app, provider, None);
        windows::send_overlay_state(app);
    }
}

/// Writes what a key test found into the live table and tells the pages.
fn note_check(app: &AppHandle, provider: Provider, key: &str, check: &KeyCheck) {
    let checked = Checked {
        status: check.status,
        text: &check.message,
        credit: check.credit,
    };
    let now = status::now_ms();
    status::update(|t| t.record_check(provider, key, now, &checked));
    emit_status(app, provider, None);
    windows::send_overlay_state(app);
}

/// `This is an OpenAI key.`: a pasted key whose prefix names another
/// provider, never stored under the wrong one.
fn wrong_provider(named: Provider, key: &str) -> Option<SaveKeyReply> {
    let owner = Provider::detect(key).filter(|o| *o != named)?;
    let name = owner.name();
    let article = match name.chars().next() {
        Some('A' | 'E' | 'I' | 'O' | 'U') => "an",
        _ => "a",
    };
    Some(SaveKeyReply {
        ok: false,
        error: Some(format!("This is {article} {name} key.")),
        detected: Some(owner.id().to_string()),
        ..SaveKeyReply::default()
    })
}

/// Tests `key`, then stores it unless the provider refused it. A key that
/// could not be tested is kept and checked again later (design-ux 3.2).
async fn tested_save(
    llm: &Llm,
    provider: Provider,
    key: &str,
    store_key: impl FnOnce(),
) -> (KeyCheck, SaveKeyReply) {
    let check = llm.test_key(provider, key).await;
    let p = provider.name();
    if check.status == Status::Rejected {
        let reply = SaveKeyReply {
            ok: false,
            status: Some(Status::Rejected.as_str().to_string()),
            error: Some(format!(
                "{p} rejected this key, so it wasn't saved. Check it and paste it again."
            )),
            ..SaveKeyReply::default()
        };
        return (check, reply);
    }
    store_key();
    let message = if check.status == Status::Unchecked {
        format!(
            "Can't reach {p} right now. The key is saved; AevalBuddy checks again when you're \
online."
        )
    } else {
        check.message.clone()
    };
    let reply = SaveKeyReply {
        ok: true,
        status: Some(check.status.as_str().to_string()),
        message: Some(message),
        key_hint: Some(crate::ipc::mask_key(key)),
        ..SaveKeyReply::default()
    };
    (check, reply)
}

/// The test, the store, the status and the tour, for a key from any source.
async fn save_tested(app: &AppHandle, provider: Provider, key: &str) -> Value {
    let llm = &super::services().llm;
    let s = store::get();
    let save = || {
        s.set_provider_key(provider.id(), key);
    };
    let (check, mut reply) = tested_save(llm, provider, key, save).await;
    if reply.ok {
        note_check(app, provider, key, &check);
        reply.keys = Some(s.provider_keys_present());
    }
    super::tour_key_tested(&check);
    let mut out = json!(reply);
    out["provider"] = json!(provider.id());
    out["credit"] = json!(check.credit);
    out
}

/// `settings:save-key`: one key field takes any of the four providers'
/// keys; the prefix says whose it is unless the page names the provider. The
/// key is tested before it is stored.
async fn save_key(app: &AppHandle, p: &Value) -> Value {
    let value = p_str(p, "key").trim().to_string();
    if value.is_empty() {
        return json!({ "ok": false, "error": "Enter a key before saving." });
    }
    let named = p_str(p, "provider");
    let provider = match named.trim() {
        "" => Some(Provider::detect(&value).unwrap_or(Provider::OpenRouter)),
        id => Provider::from_id(id),
    };
    let Some(provider) = provider else {
        return json!({ "ok": false, "error": "Unknown provider." });
    };
    if let Some(reply) = wrong_provider(provider, &value) {
        return json!(reply);
    }
    save_tested(app, provider, &value).await
}

/// `settings:clear-key {provider}`.
fn clear_key(app: &AppHandle, p: &Value) -> Value {
    let s = store::get();
    let provider = provider_of(p);
    if !store::PROVIDERS.contains(&provider.as_str()) {
        return json!({ "ok": false, "error": "Unknown provider." });
    }
    s.clear_provider_key(&provider);
    if let Some(known) = Provider::from_id(&provider) {
        emit_status(app, known, None);
    }
    windows::send_overlay_state(app);
    json!({ "ok": true, "keys": s.provider_keys_present() })
}

fn kind_of_channel(channel: &str) -> Kind {
    match channel {
        "settings:save-brain" => Kind::Text,
        "settings:save-find-model" => Kind::Find,
        "settings:save-agent-model" => Kind::Agent,
        _ => Kind::Vision,
    }
}

/// `nativeModels` with one job's pick for one provider set, or cleared when
/// `model` is empty so the provider's default applies again.
fn with_native_pick(current: Option<Value>, provider: Provider, kind: Kind, model: &str) -> Value {
    let mut all = match current {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    };
    let entry = all.entry(provider.id()).or_insert_with(|| json!({}));
    if !entry.is_object() {
        *entry = json!({});
    }
    if let Some(picks) = entry.as_object_mut() {
        match model.trim() {
            "" => {
                picks.remove(kind.as_str());
            }
            id => {
                picks.insert(kind.as_str().to_string(), json!(id));
            }
        }
    }
    Value::Object(all)
}

/// The four model pickers. With a native `provider` the pick goes under
/// `nativeModels` and never near the OpenRouter choice or its healing.
fn save_model(app: &AppHandle, channel: &str, p: &Value) -> Value {
    let s = store::get();
    let model = p_str(p, "model");
    let native = Provider::from_id(&p_str(p, "provider")).filter(|x| x.is_native());
    if let Some(provider) = native {
        let kind = kind_of_channel(channel);
        let picks = with_native_pick(s.raw("nativeModels"), provider, kind, &model);
        s.set_raw("nativeModels", picks);
        windows::send_overlay_state(app);
        let used = model_for(&StoreSettings, provider, kind);
        let field = if kind == Kind::Text { "brain" } else { "model" };
        return json!({ "ok": true, field: used, "provider": provider.id() });
    }
    match channel {
        "settings:save-brain" => json!({ "ok": true, "brain": s.set_brain(&model) }),
        "settings:save-find-model" => json!({ "ok": true, "model": s.set_find_model(&model) }),
        "settings:save-agent-model" => json!({ "ok": true, "model": s.set_agent_model(&model) }),
        _ => json!({ "ok": true, "model": s.set_model(&model) }),
    }
}

/// A key on the clipboard, by its prefix: exactly one token of the shape
/// the four providers issue.
fn clip_key_of(text: &str) -> Option<(Provider, String)> {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    const SHAPE: &str = r"^(?:sk-(?:or-|ant-)?|AIza)[A-Za-z0-9_-]{20,200}$";
    let re = RE.get_or_init(|| regex::Regex::new(SHAPE).expect("key shape"));
    let t = text.trim();
    if !re.is_match(t) {
        return None;
    }
    Provider::detect(t).map(|p| (p, t.to_string()))
}

/// `settings:clip-key`: a key on the clipboard for a provider that has
/// none, masked. The key stays in Rust.
fn clip_key(app: &AppHandle) -> Value {
    let found = clip_key_of(&clipboard_text(app))
        .filter(|(p, _)| !store::get().has_provider_key(p.id()));
    let reply = match found {
        Some((p, k)) => ClipKeyReply {
            found: true,
            masked: crate::ipc::mask_key(&k),
            provider: Some(p.id().to_string()),
        },
        None => ClipKeyReply::default(),
    };
    json!(reply)
}

/// `settings:use-clip-key`: saves the clipboard's key under the provider it
/// belongs to, tested first like any other (Decision D-10).
async fn use_clip_key(app: &AppHandle) -> Value {
    let Some((provider, key)) = clip_key_of(&clipboard_text(app)) else {
        return json!({ "ok": false, "error": "The clipboard no longer holds a key." });
    };
    save_tested(app, provider, &key).await
}

/// `settings:test-key`: the named provider's key, else the one in use.
async fn test_key(app: &AppHandle, p: &Value) -> Value {
    let sv = super::services();
    let named = p
        .get("provider")
        .and_then(Value::as_str)
        .and_then(Provider::from_id);
    let provider = named
        .or_else(|| llm_settings::active(&*sv.settings).map(|a| a.provider))
        .unwrap_or(Provider::OpenRouter);
    let key = store::get().get_provider_key(provider.id());
    if !key.trim().is_empty() {
        status::update(|t| t.mark_checking(provider, &key));
        emit_status(app, provider, None);
    }
    let check = sv.llm.test_key(provider, &key).await;
    if !key.trim().is_empty() {
        note_check(app, provider, &key, &check);
    }
    json!(check.to_reply())
}

/// `settings:set-provider-order {order}`: known ids, first tried first.
fn set_provider_order(app: &AppHandle, p: &Value) -> Value {
    let Some(ids) = p.get("order").and_then(Value::as_array) else {
        return json!({ "ok": false, "error": "No order given." });
    };
    let mut order: Vec<&'static str> = Vec::new();
    for known in ids
        .iter()
        .filter_map(|v| v.as_str().and_then(Provider::from_id))
    {
        if !order.contains(&known.id()) {
            order.push(known.id());
        }
    }
    store::get().set_raw("providerOrder", json!(order));
    windows::send_overlay_state(app);
    json!({ "ok": true, "order": order })
}

/// `settings:set-failover`: the fields it carries change; the rest stay.
fn merged_failover(now: FailoverPrefs, p: &Value) -> FailoverPrefs {
    let flag = |k: &str, was: bool| p.get(k).and_then(Value::as_bool).unwrap_or(was);
    FailoverPrefs {
        across_providers: flag("acrossProviders", now.across_providers),
        ask_before_paid: flag("askBeforePaid", now.ask_before_paid),
    }
}

fn set_failover(p: &Value) -> Value {
    let next = merged_failover(StoreSettings.failover(), p);
    store::get().set_raw("failover", json!(next));
    json!({ "ok": true, "failover": next })
}

/// The connect in progress: pressing Connect again replaces it.
fn running_connect() -> &'static Mutex<Option<CancellationToken>> {
    static C: OnceLock<Mutex<Option<CancellationToken>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

/// `auth:openrouter-connect`: the browser approves, the key is stored and
/// tested, the tour moves on (Decision D-39).
async fn connect_openrouter(app: &AppHandle) -> Value {
    let cancel = CancellationToken::new();
    if let Some(old) = running_connect().lock().replace(cancel.clone()) {
        old.cancel();
    }
    let llm = &super::services().llm;
    let transport = llm.transport();
    let open = |url: &str| windows::open_url(app, url);
    let wait = auth::CONNECT_WAIT;
    let key = match auth::connect(&*transport, open, &cancel, wait).await {
        Ok(key) => key,
        Err(e) => return json!({ "ok": false, "error": e.text() }),
    };
    let provider = Provider::OpenRouter;
    store::get().set_provider_key(provider.id(), &key);
    let check = llm.test_key(provider, &key).await;
    note_check(app, provider, &key, &check);
    super::tour_key_tested(&check);
    json!({
        "ok": true,
        "provider": provider.id(),
        "status": check.status.as_str(),
        "message": check.message,
        "credit": check.credit,
        "keyHint": crate::ipc::mask_key(&key),
    })
}

/// `skills:open-folder` (B-181): makes `<exe>\skills` if needed and shows
/// it in Explorer, by absolute path so nothing on `PATH` can stand in.
fn open_skills_folder() -> Value {
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| crate::skills::folder_of(&exe))
    else {
        return json!({ "ok": false });
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return json!({ "ok": false });
    }
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    let explorer = std::path::Path::new(&root).join("explorer.exe");
    let opened = std::process::Command::new(explorer).arg(&dir).spawn();
    json!({ "ok": opened.is_ok() })
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
        "settings:save-key" => save_key(app, p).await,
        "settings:clear-key" => clear_key(app, p),
        "settings:save-model"
        | "settings:save-brain"
        | "settings:save-find-model"
        | "settings:save-agent-model" => save_model(app, channel, p),
        "settings:clip-key" => clip_key(app),
        "settings:use-clip-key" => use_clip_key(app).await,
        "settings:test-key" => test_key(app, p).await,
        "settings:set-provider-order" => set_provider_order(app, p),
        "settings:set-failover" => set_failover(p),
        "auth:openrouter-connect" => connect_openrouter(app).await,
        "skills:open-folder" => open_skills_folder(),
        _ => return None,
    };
    Some(Ok(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::client::{Reply, Transport};
    use crate::llm::errors::ApiError;
    use crate::llm::settings::MemorySettings;
    use crate::llm::wire::HttpRequest;
    use async_trait::async_trait;
    use std::cell::Cell;
    use std::sync::Arc;

    struct One(u16);

    #[async_trait]
    impl Transport for One {
        async fn send(&self, _: HttpRequest, _: &CancellationToken) -> Result<Reply, ApiError> {
            match self.0 {
                0 => Err(ApiError::network("down")),
                status => Ok(Reply::full(status, "{}")),
            }
        }
    }

    fn llm(status: u16) -> Llm {
        let mut llm = Llm::new(Arc::new(One(status)), None);
        llm.set_offline(false);
        llm
    }

    #[test]
    fn payload_provider_defaults_to_openrouter() {
        assert_eq!(provider_of(&json!({})), "openrouter");
        assert_eq!(provider_of(&json!({ "provider": " gemini " })), "gemini");
        assert_eq!(p_str(&json!({ "key": 5 }), "key"), "5");
    }

    #[tokio::test]
    async fn save_key_tests_before_storing() {
        let p = Provider::Anthropic;
        let key = "sk-ant-api03-abcdefghijklmnop";

        // Refused: nothing stored, and the sentence says so.
        let stored = Cell::new(false);
        let (check, reply) = tested_save(&llm(401), p, key, || stored.set(true)).await;
        assert!(!stored.get());
        assert!(!reply.ok);
        assert_eq!(check.status, Status::Rejected);
        let want = "Anthropic rejected this key, so it wasn't saved. Check it and paste it again.";
        assert_eq!(reply.error.as_deref(), Some(want));

        // Works: stored, with the masked key for the row.
        let stored = Cell::new(false);
        let (_, reply) = tested_save(&llm(200), p, key, || stored.set(true)).await;
        assert!(stored.get());
        assert!(reply.ok);
        assert_eq!(reply.status.as_deref(), Some("ready"));
        assert_eq!(reply.message.as_deref(), Some("Works. Checked just now."));
        assert_eq!(reply.key_hint.as_deref(), Some("sk-ant-ap\u{2026}mnop"));
        let v = json!(reply);
        assert!(!v.to_string().contains(key), "the key never reaches a page");

        // Unreachable, or the provider is having a bad minute: kept.
        let want = "Can't reach Anthropic right now. The key is saved; AevalBuddy checks again \
when you're online.";
        for status in [0, 503] {
            let stored = Cell::new(false);
            let (_, reply) = tested_save(&llm(status), p, key, || stored.set(true)).await;
            assert!(stored.get(), "{status}");
            assert_eq!(reply.status.as_deref(), Some("unchecked"));
            assert_eq!(reply.message.as_deref(), Some(want));
        }
    }

    #[test]
    fn wrong_prefix_reports_detected() {
        let reply = wrong_provider(Provider::Anthropic, "sk-proj-abcdefg").unwrap();
        assert!(!reply.ok);
        assert_eq!(reply.error.as_deref(), Some("This is an OpenAI key."));
        assert_eq!(reply.detected.as_deref(), Some("openai"));
        let reply = wrong_provider(Provider::OpenAi, "AIzaSyAbc").unwrap();
        assert_eq!(reply.error.as_deref(), Some("This is a Google Gemini key."));
        assert!(wrong_provider(Provider::OpenAi, "sk-proj-abc").is_none());
        // A key with no known prefix is trusted to the field it was put in.
        assert!(wrong_provider(Provider::Gemini, "custom-key").is_none());
    }

    #[test]
    fn settings_extra_shape() {
        let s = MemorySettings::with_key("sk-or-v1-abcdefghijklmn");
        *s.order.lock() = vec![Provider::Anthropic];
        s.keys
            .lock()
            .insert(Provider::Anthropic, "sk-ant-api03-abcdefgh".into());
        let mut t = Table::default();
        t.mark_busy(
            Provider::OpenRouter,
            "sk-or-v1-abcdefghijklmn",
            Some(5_000),
            1_000,
            "busy",
        );
        let v = providers_extra(&s, &t, 2_000, "encrypted");
        let rows = v["providers"].as_array().unwrap();
        let ids: Vec<&str> = rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["anthropic", "openrouter", "openai", "gemini"]);
        for k in [
            "id",
            "name",
            "order",
            "hasKey",
            "keyHint",
            "status",
            "statusText",
            "checkedAt",
            "busyUntil",
            "credit",
        ] {
            assert!(rows[0].get(k).is_some(), "{k}");
        }
        assert_eq!(rows[0]["order"], 1);
        assert_eq!(rows[0]["status"], "unchecked");
        assert_eq!(rows[1]["status"], "busy");
        assert_eq!(rows[1]["busyUntil"], 6_000);
        assert_eq!(rows[2]["status"], "not-added");
        assert_eq!(rows[2]["hasKey"], false);
        let failover = json!({ "acrossProviders": true, "askBeforePaid": true });
        assert_eq!(v["failover"], failover);
        assert_eq!(v["keyHints"]["openai"], "");
        assert_eq!(v["keyHints"]["anthropic"], "sk-ant-ap\u{2026}efgh");
        assert_eq!(v["keyStorage"], "encrypted");
        assert!(!v.to_string().contains("sk-ant-api03-abcdefgh"));
    }

    #[test]
    fn native_pick_is_saved_per_provider_and_not_healed() {
        let a = Provider::Anthropic;
        let v = with_native_pick(None, a, Kind::Vision, "claude-opus-5-5");
        let v = with_native_pick(Some(v), a, Kind::Find, "claude-haiku-5-5");
        let v = with_native_pick(Some(v), Provider::Gemini, Kind::Vision, " gemini-4-argon ");
        assert_eq!(v["anthropic"]["vision"], "claude-opus-5-5");
        assert_eq!(v["anthropic"]["find"], "claude-haiku-5-5");
        assert_eq!(v["gemini"]["vision"], "gemini-4-argon");
        // Empty goes back to the provider's default.
        let v = with_native_pick(Some(v), a, Kind::Vision, "");
        assert!(v["anthropic"].get("vision").is_none());
        assert_eq!(v["anthropic"]["find"], "claude-haiku-5-5");
        // The healer takes OpenRouter ids and nothing else.
        assert!(is_openrouter_id("google/gemma-4-26b-a4b-it:free"));
        assert!(!is_openrouter_id("claude-opus-5-5"));
        assert_eq!(kind_of_channel("settings:save-find-model"), Kind::Find);
        assert_eq!(kind_of_channel("settings:save-model"), Kind::Vision);
    }

    #[test]
    fn clipboard_keys_are_recognised_for_all_four() {
        let tail = "abcdefghijklmnopqrstuvwxyz";
        let cases = [
            (format!("sk-or-v1-{tail}"), Provider::OpenRouter),
            (format!("sk-ant-api03-{tail}"), Provider::Anthropic),
            (format!("sk-proj-{tail}"), Provider::OpenAi),
            (format!("AIzaSy{tail}"), Provider::Gemini),
        ];
        for (key, want) in cases {
            let padded = format!("  {key}\n");
            let got = clip_key_of(&padded).unwrap();
            assert_eq!(got, (want, key));
        }
        assert_eq!(clip_key_of("sk-or-short"), None);
        assert_eq!(clip_key_of(&format!("hello sk-or-v1-{tail}")), None);
        assert_eq!(clip_key_of("just some words"), None);
    }

    #[test]
    fn answers_mark_and_clear_cooldowns() {
        let mut t = Table::default();
        let a = Provider::Anthropic;
        let model = "claude-sonnet-5-5";
        let busy = ApiError::http(429, "rate limited");
        assert!(record_answer(&mut t, a, "k", Err(&busy), model, 1_000));
        assert_eq!(t.get(a, "k").status, Status::Busy);
        assert!(record_answer(&mut t, a, "k", Ok(()), model, 2_000));
        assert_eq!(t.get(a, "k").status, Status::Ready);
        let rejected = ApiError::http(401, "invalid x-api-key");
        assert!(record_answer(&mut t, a, "k", Err(&rejected), model, 3_000));
        assert_eq!(t.get(a, "k").status, Status::Rejected);
        // A busy free model leaves OpenRouter usable.
        let or = Provider::OpenRouter;
        let free = "google/gemma-4-26b-a4b-it:free";
        assert!(!record_answer(&mut t, or, "k", Err(&busy), free, 1_000));
        assert_ne!(t.get(or, "k").status, Status::Busy);
    }

    #[test]
    fn failover_prefs_merge_field_by_field() {
        let now = FailoverPrefs::default();
        let next = merged_failover(now, &json!({ "askBeforePaid": false }));
        assert!(next.across_providers);
        assert!(!next.ask_before_paid);
        let next = merged_failover(next, &json!({ "acrossProviders": false }));
        assert!(!next.across_providers);
        assert!(!next.ask_before_paid);
    }
}
