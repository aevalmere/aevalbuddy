//! The renderer's only door into Rust: one Tauri command, `aeval_invoke`,
//! routed by channel name. The pages keep calling
//! `window.aeval.invoke(channel, payload)` and `window.aeval.on(channel, cb)`
//! exactly as they did through preload.js; `bridge/aeval-bridge.js` provides
//! that object on top of Tauri. ARCHITECTURE.md section 4 is the contract.
//!
//! Core channels (agent a) are routed here; the rest stay `not_ported` until
//! their agent replaces the row.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State, WebviewWindow};

use crate::state::{shared, Mode};
use crate::windows::{self, outputs};
use crate::{autostart, history, hotkeys, store, updater};

/// Channels a page may invoke. Must equal `INVOKE_CHANNELS` in preload.js
/// (checked by `invoke_allowlist_matches_preload`).
pub const INVOKE_CHANNELS: &[&str] = &[
    "overlay:trigger",
    "overlay:collapse",
    "overlay:resize",
    "overlay:set-ignore",
    "overlay:open-settings",
    "output:resize",
    "open-external",
    "query:submit",
    "query:cancel",
    "query:retry",
    "chat:new",
    "region:pick",
    "region:clear",
    "region:done",
    "region:cancel",
    "overlay:open-history",
    "history:list",
    "history:open",
    "history:remove",
    "history:clear",
    "history:close",
    "settings:get",
    "settings:save-key",
    "settings:clear-key",
    "settings:save-model",
    "settings:save-skill",
    "settings:save-brain",
    "settings:set-brain-enabled",
    "settings:set-show-paid",
    "settings:set-save-history",
    "settings:clear-history",
    "settings:set-launch-at-login",
    "settings:test-key",
    "update:check",
    "update:restart",
    "settings:close",
    "models:list",
    "clipboard:write",
    "hud:enter",
    "hud:exit",
    "hud:set-dock",
    "shortcuts:set",
    "shortcuts:reset",
    "shortcuts:capture-mode",
    "buddy:resolve-result",
    "buddy:dismiss",
    "buddy:advance",
    "buddy:set-ignore",
    "app:quit",
    "bar:mode",
    "find:open",
    "find:query",
    "find:step",
    "find:scope",
    "find:to-ask",
    "find:close",
    "agent:start",
    "agent:stop",
    "agent:confirm",
    "agent:continue",
    "agent:consent",
    "agent:do-it",
    "ui:context-menu",
    "settings:save-find-model",
    "settings:save-agent-model",
    "settings:set-find",
    "settings:set-agent",
    "tour:replay",
    "buddy:card",
    "buddy:skip",
    "find:related",
    "bar:history",
    "bar:history-push",
    "ui:focus-answer",
    "ui:focus-bar",
    "app:help",
    "settings:clip-key",
    "settings:use-clip-key",
    "hud:nudge",
    "buddy:focus",
    "glass:hello",
    "glass:passthrough",
    "settings:set-surface",
    // Split (d): floating output windows and the glass slider
    // (ARCHITECTURE.md section 10).
    "output:bar",
    "output:close",
    "settings:set-glass",
];

/// Channels a page may listen on. Must equal `LISTEN_CHANNELS` in preload.js.
pub const LISTEN_CHANNELS: &[&str] = &[
    "overlay:state",
    "hud:snapped",
    "hud:docked",
    "query:start",
    "query:stage",
    "query:delta",
    "query:done",
    "query:error",
    "chat:user",
    "chat:reset",
    "chat:sync",
    "history:changed",
    "buddy:resolve",
    "buddy:watch",
    "buddy:show",
    "buddy:hide",
    "buddy:done",
    "buddy:move",
    "settings:data",
    "update:state",
    "find:hits",
    "find:state",
    "agent:begin",
    "agent:step",
    "agent:state",
    "agent:confirm-request",
    "agent:done",
    "agent:consent-request",
    "lens:draw",
    "view:set",
    "bar:prefill",
    "settings:section",
    "bar:open",
    "bar:context",
    "glass:frame",
    "glass:tier",
    "lens:loupe",
];

const BRIDGE_JS: &str = include_str!("../bridge/aeval-bridge.js");

/// The initialization script injected into every webview: the bridge plus
/// the allowlists, so the JS and Rust lists come from one place.
pub fn bridge_script() -> String {
    let invoke = js_list(INVOKE_CHANNELS);
    let listen = js_list(LISTEN_CHANNELS);
    format!("(() => {{\n{BRIDGE_JS}\naevalBridge({invoke}, {listen});\n}})();\n")
}

fn js_list(list: &[&str]) -> String {
    serde_json::to_string(list).unwrap_or_default()
}

fn allowed(channel: &str) -> bool {
    INVOKE_CHANNELS.contains(&channel)
}

/// NEW-10: the first request sent with text, of any kind, ends the bar's
/// suggestions for good (ARCHITECTURE.md 10.2).
pub fn first_request(channel: &str, p: &Value) -> bool {
    match channel {
        "query:submit" => !p_str(p, "question").trim().is_empty(),
        "agent:start" => !p_str(p, "task").trim().is_empty(),
        "find:step" => true,
        _ => false,
    }
}

/// Which windows have called through the bridge, and with which channels.
/// The scaffold smoke reads it to prove every page loaded with a working
/// `window.aeval`.
#[derive(Default)]
pub struct Seen(Mutex<BTreeMap<String, BTreeSet<String>>>);

impl Seen {
    pub fn snapshot(&self) -> BTreeMap<String, BTreeSet<String>> {
        self.0.lock().map(|m| m.clone()).unwrap_or_default()
    }

    fn record(&self, label: &str, channel: &str) {
        if let Ok(mut map) = self.0.lock() {
            map.entry(label.to_string())
                .or_default()
                .insert(channel.to_string());
        }
    }
}

#[tauri::command]
pub async fn aeval_invoke(
    window: WebviewWindow,
    seen: State<'_, Seen>,
    channel: String,
    payload: Option<Value>,
) -> Result<Value, String> {
    if !allowed(&channel) {
        return Err(format!("Blocked IPC channel: {channel}"));
    }
    seen.record(window.label(), &channel);
    let app = window.app_handle().clone();
    let payload = payload.unwrap_or(Value::Null);
    if first_request(&channel, &payload) && !store::get().get_has_sent_first_request() {
        store::get().mark_first_request_sent();
        windows::send_overlay_state(&app);
    }
    // Rows owned by split (c): region:*, buddy:*, glass:hello.
    if let Some(r) = crate::desktop::channels::route(&window, &channel, &payload) {
        return r;
    }
    route(&app, &window, &channel, payload).await
}

fn p_str(p: &Value, key: &str) -> String {
    match p.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(v) => v.to_string(),
    }
}

fn p_bool(p: &Value, key: &str) -> bool {
    store::truthy(p.get(key))
}

fn p_num(p: &Value, key: &str) -> f64 {
    let n = store::js_number(p.get(key));
    if n.is_finite() {
        n
    } else {
        0.0
    }
}

fn provider_of(p: &Value) -> String {
    let v = p_str(p, "provider");
    if v.is_empty() {
        "openrouter".into()
    } else {
        v
    }
}

fn ok() -> Result<Value, String> {
    Ok(json!({ "ok": true }))
}

/// An OpenRouter key on the clipboard (IPC-12). The key stays in Rust.
pub fn clip_key_of(text: &str) -> String {
    let t = text.trim();
    let re = regex::Regex::new(r"^sk-or-[A-Za-z0-9_-]{20,200}$").expect("static regex");
    if re.is_match(t) {
        t.to_string()
    } else {
        String::new()
    }
}

/// `${k.slice(0, 9)}…${k.slice(-4)}`; empty for no key.
pub fn mask_key(k: &str) -> String {
    let chars: Vec<char> = k.chars().collect();
    if chars.len() < 13 {
        return String::new();
    }
    let head: String = chars[..9].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}\u{2026}{tail}")
}

fn clipboard_text(app: &AppHandle) -> String {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().read_text().unwrap_or_default()
}

fn key_saved(app: &AppHandle, provider: &str) {
    let st = shared(app);
    if !st.call_str(app, provider, |h| h.key_saved.as_ref()) {
        st.call_str(app, "key-saved", |h| h.tour_notify.as_ref());
    }
}

/// `{tier, reasons}` from agent c's backdrop (through `settings_extra`), or
/// full glass when it is not hooked.
fn glass_tier(app: &AppHandle) -> Value {
    shared(app)
        .call_value(app, |h| h.settings_extra.as_ref())
        .and_then(|v| v.get("glass").cloned())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({ "tier": "full", "reasons": [] }))
}

/// `settings:get`: the core fields, with `settings_extra` (models resolved
/// from the catalogue, skills, glass tier) merged over them.
pub fn settings_data(app: &AppHandle) -> Value {
    let s = store::get();
    let mut v = json!({
        "findModel": s.get_find_model(),
        "defaultFindModel": store::DEFAULT_FIND,
        "agentModel": s.get_agent_model(),
        "defaultAgentModel": store::DEFAULT_AGENT,
        "find": s.get_find_settings(),
        "agent": s.get_agent_settings(),
        "winBuild": windows::flags::win_build(),
        "hasKey": s.has_api_key(),
        "keys": s.provider_keys_present(),
        "model": s.get_model(),
        "defaultModel": store::DEFAULT_MODEL,
        "brain": s.get_brain(),
        "brainEnabled": s.get_brain_enabled(),
        "defaultBrain": store::DEFAULT_BRAIN,
        "showPaid": s.get_show_paid_models(),
        "skills": [],
        "skill": s.get_skill_id(|_| true),
        "shortcuts": s.get_shortcuts(),
        "active": hotkeys::active(app),
        "defaults": store::default_shortcuts(),
        "version": app.package_info().version.to_string(),
        "update": updater::get(),
        "canHide": windows::flags::can_hide_from_capture(),
        "saveHistory": s.get_save_history(),
        "historyCount": history::get().count(),
        "launchAtLogin": s.get_launch_at_login(),
        "surface": s.get_surface(),
        "glass": glass_tier(app),
        "glassIntensity": s.get_glass_intensity(),
        "hasSentFirstRequest": s.get_has_sent_first_request(),
        "launchAtLoginLive": autostart::live(),
    });
    let extra = shared(app).call_value(app, |h| h.settings_extra.as_ref());
    if let (Some(Value::Object(extra)), Value::Object(base)) = (extra, &mut v) {
        for (k, val) in extra {
            base.insert(k, val);
        }
    }
    v
}

async fn route(
    app: &AppHandle,
    win: &WebviewWindow,
    channel: &str,
    p: Value,
) -> Result<Value, String> {
    let label = win.label().to_string();
    let s = store::get();
    match channel {
        "overlay:trigger" => windows::trigger_or_trail(app),
        "overlay:collapse" => {
            if shared(app).lock().mode == Mode::Hud {
                windows::exit_hud(app);
            } else {
                windows::collapse_panel(app, false);
            }
        }
        "overlay:resize" => windows::overlay_resize(app, p_num(&p, "height")),
        "overlay:set-ignore" => windows::overlay_set_ignore(app, p_bool(&p, "ignore")),
        "glass:passthrough" => {
            // The shadow margin lets clicks through; the page says when the
            // pointer is over glass. Output windows only, as main.js.
            if outputs::is_output(&label) && shared(app).lock().mode != Mode::Hud {
                windows::flags::set_click_through(win, p_bool(&p, "ignore"));
            }
        }
        "overlay:open-settings" => {
            let section = p_str(&p, "section");
            windows::open_settings(app, Some(section.as_str()).filter(|s| !s.is_empty()));
        }
        "output:resize" => {
            if shared(app).lock().mode != Mode::Hud {
                outputs::resize(app, &label, p_num(&p, "height"));
            }
        }
        "output:bar" => {
            let open = {
                let st = shared(app);
                let s = st.lock();
                s.mode == Mode::Panel && !s.hidden
            };
            return Ok(if open {
                json!(windows::bar_rect(app))
            } else {
                Value::Null
            });
        }
        "output:close" => {
            if outputs::is_output(&label) {
                outputs::close(app, &label);
            } else {
                windows::collapse_panel(app, false);
            }
        }
        "open-external" => {
            windows::open_url(app, &p_str(&p, "url"));
        }
        "app:help" => return Ok(windows::open_help(app)),
        "chat:new" => {
            if outputs::is_output(&label) {
                outputs::reset(app, &label);
            } else {
                windows::new_chat(app);
            }
        }
        "overlay:open-history" => windows::open_history(app),
        "history:list" => {
            return Ok(json!({ "rows": history::get().list(&p_str(&p, "query")) }));
        }
        "history:open" => {
            let Some(entry) = history::get().entry(&p_str(&p, "id")) else {
                return Ok(json!({ "ok": false }));
            };
            shared(app).call(app, |h| h.abort_query.as_ref());
            windows::close_history(app);
            shared(app).lock().hidden = false;
            windows::expand_panel(app);
            let done = outputs::open_conversation(app, &entry);
            windows::send_overlay_state(app);
            return Ok(json!({ "ok": done }));
        }
        "history:remove" => {
            return Ok(json!({ "ok": history::get().remove(&p_str(&p, "id")) }));
        }
        "history:clear" => return Ok(json!({ "ok": history::get().clear() })),
        "history:close" => windows::close_history(app),
        "hud:enter" => windows::enter_hud(app),
        "hud:exit" => windows::exit_hud(app),
        "hud:nudge" => windows::hud_nudge(app, p_num(&p, "dx"), p_num(&p, "dy")),
        // NEW-04: outputs float on their own; the editor has no dock switch.
        "hud:set-dock" => {}
        "bar:mode" => {
            windows::set_bar_mode(app, &p_str(&p, "mode"));
            let mode = shared(app).lock().bar_mode.clone();
            return Ok(json!({ "mode": mode }));
        }
        "bar:history" => return Ok(json!(s.get_input_history())),
        "bar:history-push" => return Ok(json!(s.push_input_history(&p_str(&p, "text")))),
        "ui:focus-answer" => {
            let target = outputs::focused(app)
                .filter(|l| outputs::with(app, l, |o| o.active).unwrap_or(false))
                .and_then(|l| windows::window(app, &l));
            return match target {
                Some(w) => {
                    windows::flags::show_focused(&w);
                    ok()
                }
                None => Ok(json!({ "ok": false })),
            };
        }
        "ui:focus-bar" => {
            if shared(app).lock().mode != Mode::Panel {
                windows::open_bar(app, None, None);
            } else if let Some(w) = windows::window(app, windows::OVERLAY) {
                let _ = w.set_focus();
            }
            return ok();
        }
        "tour:replay" => {
            windows::close_settings(app);
            if shared(app).lock().mode != Mode::Hud {
                shared(app).call(app, |h| h.start_tour.as_ref());
                hotkeys::apply(app);
            }
        }
        "app:quit" => app.exit(0),
        "settings:get" => return Ok(settings_data(app)),
        "settings:close" => windows::close_settings(app),
        "settings:clip-key" => {
            let k = if s.has_api_key() {
                String::new()
            } else {
                clip_key_of(&clipboard_text(app))
            };
            return Ok(json!({ "found": !k.is_empty(), "masked": mask_key(&k) }));
        }
        "settings:use-clip-key" => {
            let k = clip_key_of(&clipboard_text(app));
            if k.is_empty() {
                return Ok(json!({
                    "ok": false,
                    "error": "The clipboard no longer holds an OpenRouter key."
                }));
            }
            s.set_api_key(&k);
            windows::send_overlay_state(app);
            key_saved(app, "openrouter");
            return ok();
        }
        "settings:save-key" => {
            let value = p_str(&p, "key").trim().to_string();
            if value.is_empty() {
                return Ok(json!({ "ok": false, "error": "Enter a key before saving." }));
            }
            let provider = provider_of(&p);
            if !store::PROVIDERS.contains(&provider.as_str()) {
                return Ok(json!({ "ok": false, "error": "Unknown provider." }));
            }
            if provider == "openrouter" {
                s.set_api_key(&value);
            } else {
                s.set_provider_key(&provider, &value);
            }
            windows::send_overlay_state(app);
            key_saved(app, &provider);
            return Ok(json!({ "ok": true, "keys": s.provider_keys_present() }));
        }
        "settings:clear-key" => {
            let provider = provider_of(&p);
            if provider == "openrouter" {
                s.clear_api_key();
            } else {
                s.clear_provider_key(&provider);
            }
            windows::send_overlay_state(app);
            return Ok(json!({ "ok": true, "keys": s.provider_keys_present() }));
        }
        "settings:save-model" => {
            return Ok(json!({ "ok": true, "model": s.set_model(&p_str(&p, "model")) }));
        }
        "settings:save-brain" => {
            return Ok(json!({ "ok": true, "brain": s.set_brain(&p_str(&p, "model")) }));
        }
        "settings:save-find-model" => {
            return Ok(json!({ "ok": true, "model": s.set_find_model(&p_str(&p, "model")) }));
        }
        "settings:save-agent-model" => {
            return Ok(json!({ "ok": true, "model": s.set_agent_model(&p_str(&p, "model")) }));
        }
        "settings:save-skill" => {
            let skill = s.set_skill_id(&p_str(&p, "skill"), |_| true);
            return Ok(json!({ "ok": true, "skill": skill }));
        }
        "settings:set-find" => {
            return Ok(json!({ "ok": true, "find": s.set_find_settings(&p) }));
        }
        "settings:set-agent" => {
            return Ok(json!({ "ok": true, "agent": s.set_agent_settings(&p) }));
        }
        "settings:set-brain-enabled" => {
            let on = s.set_brain_enabled(p_bool(&p, "on"));
            return Ok(json!({ "ok": true, "brainEnabled": on }));
        }
        "settings:set-show-paid" => {
            let on = s.set_show_paid_models(p_bool(&p, "on"));
            return Ok(json!({ "ok": true, "showPaid": on }));
        }
        "settings:set-save-history" => {
            let on = s.set_save_history(p_bool(&p, "on"));
            history::get().set_enabled(on);
            let count = history::get().count();
            return Ok(json!({ "ok": true, "saveHistory": on, "historyCount": count }));
        }
        "settings:clear-history" => {
            history::get().clear();
            windows::notify_history_changed(app);
            return Ok(json!({ "ok": true, "historyCount": history::get().count() }));
        }
        "settings:set-launch-at-login" => {
            s.set_launch_at_login(p_bool(&p, "on"));
            autostart::apply();
            return Ok(json!({ "ok": true, "launchAtLogin": s.get_launch_at_login() }));
        }
        "settings:set-surface" => {
            let v = s.set_surface(&p_str(&p, "surface"));
            return Ok(json!({ "surface": v, "tier": "full", "reasons": [] }));
        }
        "settings:set-glass" => {
            let v = s.set_glass_intensity(p_num(&p, "glassIntensity"));
            let glass = glass_tier(app);
            let mut tier = glass.clone();
            tier["intensity"] = json!(v);
            for (l, _) in app.webview_windows() {
                windows::emit(app, &l, "glass:tier", tier.clone());
            }
            windows::send_overlay_state(app);
            return Ok(json!({ "glassIntensity": v, "glass": glass }));
        }
        "update:check" => return Ok(updater::check(app).await),
        "update:restart" => return Ok(json!({ "ok": updater::restart(app) })),
        "clipboard:write" => {
            use tauri_plugin_clipboard_manager::ClipboardExt;
            let _ = app.clipboard().write_text(p_str(&p, "text"));
            return ok();
        }
        "shortcuts:capture-mode" => hotkeys::set_capture_mode(app, p_bool(&p, "on")),
        "shortcuts:set" => {
            let (action, acc) = (p_str(&p, "action"), p_str(&p, "accelerator"));
            return Ok(hotkeys::set(app, &action, &acc));
        }
        "shortcuts:reset" => return Ok(hotkeys::reset(app)),
        _ => return not_ported(channel, &p),
    }
    Ok(Value::Null)
}

fn not_ported(channel: &str, payload: &Value) -> Result<Value, String> {
    let shape = match payload {
        Value::Object(m) => m.keys().cloned().collect::<Vec<_>>().join(","),
        Value::Null => String::new(),
        _ => "value".to_string(),
    };
    Err(format!(
        "{channel} is not ported to Rust yet (payload keys: [{shape}])"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRELOAD: &str = include_str!("../../preload.js");

    fn preload_set(name: &str) -> Vec<String> {
        let start = PRELOAD
            .find(&format!("const {name} = new Set(["))
            .expect("set declared in preload.js");
        let body = &PRELOAD[start..];
        let end = body.find("]);").expect("set closed in preload.js");
        body[..end]
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with('\''))
            .map(|l| l.trim_end_matches(',').trim_matches('\'').to_string())
            .collect()
    }

    fn ours(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn invoke_allowlist_matches_preload() {
        assert_eq!(ours(INVOKE_CHANNELS), preload_set("INVOKE_CHANNELS"));
    }

    #[test]
    fn listen_allowlist_matches_preload() {
        assert_eq!(ours(LISTEN_CHANNELS), preload_set("LISTEN_CHANNELS"));
    }

    #[test]
    fn split_d_channels_are_allowed() {
        for c in ["output:bar", "output:close", "settings:set-glass"] {
            assert!(allowed(c));
        }
        assert!(!allowed("fs:read"));
    }

    #[test]
    fn first_request_needs_text() {
        assert!(first_request("query:submit", &json!({ "question": "hi" })));
        assert!(!first_request("query:submit", &json!({ "question": "  " })));
        assert!(first_request(
            "agent:start",
            &json!({ "task": "open notepad" })
        ));
        assert!(!first_request("agent:start", &json!({})));
        assert!(first_request("find:step", &json!({ "by": 1 })));
        assert!(!first_request("bar:mode", &json!({ "mode": "ask" })));
    }

    #[test]
    fn unknown_channel_is_refused_with_the_electron_wording() {
        let err = not_ported("query:submit", &serde_json::json!({ "question": "x" }));
        assert_eq!(
            err.unwrap_err(),
            "query:submit is not ported to Rust yet (payload keys: [question])"
        );
    }

    #[test]
    fn clipboard_keys_are_matched_and_masked() {
        let k = "sk-or-v1-abcdefghijklmnopqrstuvwxyz0123";
        assert_eq!(clip_key_of(&format!("  {k}\n")), k);
        assert_eq!(clip_key_of("sk-or-short"), "");
        assert_eq!(clip_key_of("hello sk-or-v1-abcdefghijklmnopqrstuvwxyz"), "");
        assert_eq!(mask_key(k), "sk-or-v1-\u{2026}0123");
        assert_eq!(mask_key(""), "");
    }

    #[test]
    fn payload_helpers_read_like_javascript() {
        let p = json!({ "height": "120", "on": 1, "s": 5, "bad": "x" });
        assert_eq!(p_num(&p, "height"), 120.0);
        assert_eq!(p_num(&p, "bad"), 0.0);
        assert_eq!(p_num(&p, "missing"), 0.0);
        assert!(p_bool(&p, "on"));
        assert_eq!(p_str(&p, "s"), "5");
        assert_eq!(provider_of(&p), "openrouter");
    }

    #[test]
    fn bridge_script_carries_both_allowlists() {
        let s = bridge_script();
        assert!(s.contains("aevalBridge([\"overlay:trigger\""));
        assert!(s.contains("\"lens:loupe\"]);"));
        assert!(s.contains("\"settings:set-glass\"]"));
    }
}
