//! The renderer's only door into Rust: one Tauri command, `aeval_invoke`,
//! routed by channel name. The pages keep calling
//! `window.aeval.invoke(channel, payload)` and `window.aeval.on(channel, cb)`
//! exactly as they did through preload.js; `bridge/aeval-bridge.js` provides
//! that object on top of Tauri. ARCHITECTURE.md section 4 is the contract.
//!
//! Split (c)'s rows are asked first (`desktop::channels`), then the glue in
//! `host/` (Ask, Find, Agent, context menus), then the core rows here. Every
//! allowlisted invoke channel has a row (`every_invoke_channel_has_a_route`).

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::Mutex;

use futures_util::FutureExt;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State, WebviewWindow};

use crate::state::{shared, AppState, Mode};
use crate::windows::{self, outputs};
use crate::{autostart, history, hotkeys, store, updater};

/// Channels a page may invoke. Must equal `INVOKE_CHANNELS` in preload.js
/// (checked by `invoke_allowlist_matches_preload`).
pub const INVOKE_CHANNELS: &[&str] = &[
    "overlay:trigger",
    "overlay:collapse",
    "overlay:minimize",
    "overlay:resize",
    "overlay:set-ignore",
    "overlay:open-settings",
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
    "settings:set-show-button",
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
    // ---- lane: windows (do not edit outside your lane) ----
    "settings:set-text-scale",
    "output:blur",
    "output:cycle-target",
    "output:close-all",
    "output:last-answer",
    "output:undo-reset",
    "output:glass",
    "output:gone",
    "window:nudge",
    "bar:dirty",
    "bar:ready",
    "bar:folded",
    "bar:prefill-open",
    "a11y:announce",
    // ---- end: windows ----
    // ---- lane: capture ----
    "lens:ready",
    "buddy:ready",
    "buddy:cleared",
    // ---- end: capture ----
    // ---- lane: llm ----
    "settings:set-provider-order",
    "settings:set-failover",
    "auth:openrouter-connect",
    "skills:open-folder",
    "output:sent-image",
    // ---- end: llm ----
    // ---- lane: agent ----
    // ---- end: agent ----
    // voice lane
    "voice:start",
    "voice:stop",
    "voice:cancel",
    "voice:get",
    "voice:set",
    "voice:download",
    "voice:capture-hotkey",
    // end voice lane
    // agents lane
    "agents:ready",
    "agents:size",
    "agents:hidden",
    "agents:hover",
    "agents:open",
    "agents:dismiss",
    "agents:act",
    "agents:get",
    "agents:set",
    "agents:test",
    // end agents lane
];

/// Channels a page may listen on. Must equal `LISTEN_CHANNELS` in preload.js.
pub const LISTEN_CHANNELS: &[&str] = &[
    "overlay:state",
    "hud:snapped",
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
    // ---- lane: windows (do not edit outside your lane) ----
    "output:meta",
    "output:away",
    "ui:text-scale",
    "bar:clear",
    "bar:note",
    "a11y:say",
    // ---- end: windows ----
    // ---- lane: capture ----
    "buddy:reset",
    // ---- end: capture ----
    // ---- lane: llm ----
    "provider:status",
    // ---- end: llm ----
    // ---- lane: agent ----
    // ---- end: agent ----
    // voice lane
    "voice:state",
    "voice:level",
    "voice:progress",
    // end voice lane
    // agents lane
    "agents:card",
    "agents:hide",
    "agents:state",
    // end agents lane
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

/// Which kind of window is calling (B-089).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caller {
    /// The command bar (`overlay`).
    Bar,
    Settings,
    History,
    /// An answer window (`output-<n>`).
    Output,
    Buddy,
    /// A lens window (`lens-<display>`).
    Lens,
    /// The area picker.
    Region,
    /// The agents card (agents lane).
    Agents,
    /// Anything else (a test fixture page).
    Other,
}

impl Caller {
    pub fn of(label: &str) -> Caller {
        match label {
            "overlay" => Caller::Bar,
            "settings" => Caller::Settings,
            "history" => Caller::History,
            "buddy" => Caller::Buddy,
            "region" => Caller::Region,
            crate::agents::LABEL => Caller::Agents,
            l if outputs::is_output(l) => Caller::Output,
            l if l.starts_with("lens-") => Caller::Lens,
            _ => Caller::Other,
        }
    }
}

const ANY: &[Caller] = &[
    Caller::Bar,
    Caller::Settings,
    Caller::History,
    Caller::Output,
    Caller::Buddy,
    Caller::Lens,
    Caller::Region,
    Caller::Agents,
];
const BAR: &[Caller] = &[Caller::Bar];
const APP: &[Caller] = &[Caller::Bar, Caller::Settings];
const APP_HISTORY: &[Caller] = &[Caller::Bar, Caller::Settings, Caller::History];
const PAGES: &[Caller] = &[
    Caller::Bar,
    Caller::Settings,
    Caller::History,
    Caller::Output,
];
const GLASS_PAGES: &[Caller] = &[
    Caller::Bar,
    Caller::Settings,
    Caller::History,
    Caller::Output,
    Caller::Buddy,
    Caller::Agents,
];
const ASK: &[Caller] = &[Caller::Bar, Caller::Output];
const BUDDY: &[Caller] = &[Caller::Bar, Caller::Settings, Caller::Buddy];
const LENS: &[Caller] = &[Caller::Lens];
const REGION: &[Caller] = &[Caller::Bar, Caller::Region];
// agents lane: the card's own calls and Settings' Agents page.
const AGENTS: &[Caller] = &[Caller::Settings, Caller::Agents];

/// Channels whose callers differ from their prefix's row. A table, not
/// match arms, so `every_invoke_channel_has_a_route` never takes a rule
/// for a route.
const EXCEPTIONS: &[(&str, &[Caller])] = &[
    ("glass:hello", ANY),
    ("glass:passthrough", ANY),
    ("a11y:announce", ANY),
    ("open-external", PAGES),
    ("clipboard:write", PAGES),
    ("app:help", PAGES),
    ("window:nudge", PAGES),
    // Every page that loads glass.js, the buddy card included, reads the
    // stored text size and takes Ctrl+= and Ctrl+- (D-24). `settings:get`
    // reports only whether keys are set, never a key.
    ("settings:get", GLASS_PAGES),
    ("settings:set-text-scale", GLASS_PAGES),
    ("overlay:collapse", PAGES),
    // The bar and an answer window fold everything back into the pill.
    ("overlay:minimize", ASK),
    ("overlay:open-history", PAGES),
    ("overlay:open-settings", PAGES),
    ("bar:prefill-open", ASK),
    // An output's trail may start only its own task again (`authorize`).
    ("agent:start", BAR),
    // llm lane: Settings' Connect OpenRouter and custom answer styles.
    ("auth:openrouter-connect", APP),
    ("skills:open-folder", APP),
];

/// B-089: the windows that may call `channel`; None for a channel with no
/// rule (refused). Agent, settings, shortcut, update and quit channels come
/// from the bar and Settings (History for its own); an output gets its own
/// channels, the Ask's, the agent's run controls its trail shows, Find's
/// step, and the harmless page ones; buddy, lens and the area picker get
/// their own and `glass:*`. A new channel falls under its prefix's row.
pub fn rule(channel: &str) -> Option<&'static [Caller]> {
    EXCEPTIONS
        .iter()
        .find(|(c, _)| *c == channel)
        .map(|(_, callers)| *callers)
        .or_else(|| family(channel))
}

fn family(channel: &str) -> Option<&'static [Caller]> {
    let prefix = channel.split(':').next().unwrap_or_default();
    Some(match prefix {
        "settings" | "shortcuts" | "update" | "tour" | "models" | "app" | "hud" => APP,
        "history" => APP_HISTORY,
        "query" | "chat" | "output" | "ui" | "find" | "agent" => ASK,
        "buddy" => BUDDY,
        "lens" => LENS,
        "region" => REGION,
        "bar" | "overlay" => BAR,
        "glass" | "a11y" => ANY,
        "window" => PAGES,
        // voice lane: the bar's mic and Settings' Voice section.
        "voice" => APP,
        // agents lane.
        "agents" => AGENTS,
        _ => return None,
    })
}

/// Whether a window of kind `caller` may call `channel`.
pub fn permitted(caller: Caller, channel: &str) -> bool {
    rule(channel).is_some_and(|allowed| allowed.contains(&caller))
}

/// Whether `task` is a question this transcript already holds (the trail's
/// Retry).
pub fn asks_for(messages: &[Value], task: &str) -> bool {
    let asked = |m: &Value| m["role"] == "user" && m["text"].as_str() == Some(task);
    !task.trim().is_empty() && messages.iter().any(asked)
}

/// The one call an output makes outside its rows: its trail's Retry. A
/// const, so the route scan never takes this comparison for a route.
const RETRY: &str = "agent:start";

/// B-089: a call from a window with no business making it is refused.
fn authorize(window: &WebviewWindow, channel: &str, p: &Value) -> Result<(), String> {
    let label = window.label();
    let caller = Caller::of(label);
    if permitted(caller, channel) {
        return Ok(());
    }
    if caller == Caller::Output && channel == RETRY {
        let task = p_str(p, "task");
        let app = window.app_handle();
        let own = outputs::with(app, label, |o| asks_for(&o.chat.messages, &task));
        if own == Some(true) {
            return Ok(());
        }
    }
    Err(format!("not allowed from {label}"))
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
    let payload = payload.unwrap_or(Value::Null);
    authorize(&window, &channel, &payload)?;
    seen.record(window.label(), &channel);
    let app = window.app_handle().clone();
    let label = window.label().to_string();
    let was_busy = shared(&app).lock().busy;
    // A route that panicked may have left the caller's Ask marked running.
    let settle = || {
        {
            let st = shared(&app);
            let mut s = st.lock();
            settle_after_panic(&mut s, &label, was_busy);
        }
        windows::send_overlay_state(&app);
    };
    guarded(&channel, dispatch(&window, &channel, payload), settle).await
}

/// B-090: what a panicked call from `label` leaves behind. Its own Ask
/// entry goes; the single `busy` flag goes back to what it was before the
/// call when no Ask is listed (an Ask that set only `busy` and is still
/// running elsewhere keeps it).
pub fn settle_after_panic(s: &mut AppState, label: &str, was_busy: bool) {
    s.query_forget(label);
    if s.queries.is_empty() && !was_busy {
        s.busy = false;
    }
}

/// B-090: runs a route; a panic in it settles the page's promise with an
/// error instead of leaving it waiting, and `on_panic` runs. The panic
/// hook (`crash::install`) has already logged it and shown the toast.
pub async fn guarded<F>(channel: &str, route: F, on_panic: impl FnOnce()) -> Result<Value, String>
where
    F: Future<Output = Result<Value, String>>,
{
    match AssertUnwindSafe(route).catch_unwind().await {
        Ok(r) => r,
        Err(cause) => {
            on_panic();
            let why = crate::crash::message(&*cause);
            Err(format!("{channel} failed: {why}"))
        }
    }
}

/// One call: split (c)'s rows, the host routers, then the core's.
async fn dispatch(window: &WebviewWindow, channel: &str, payload: Value) -> Result<Value, String> {
    let app = window.app_handle().clone();
    if first_request(channel, &payload) && !store::get().get_has_sent_first_request() {
        store::get().mark_first_request_sent();
        windows::send_overlay_state(&app);
    }
    // Rows owned by split (c): region:*, buddy:*, glass:hello.
    if let Some(r) = crate::desktop::channels::route(window, channel, &payload) {
        return r;
    }
    let label = window.label().to_string();
    if let Some(r) = crate::host::ask::route(&app, &label, channel, &payload).await {
        return r;
    }
    if let Some(r) = crate::host::find::route(&app, channel, &payload).await {
        return r;
    }
    if let Some(r) = crate::host::agent::route(&app, channel, &payload).await {
        return r;
    }
    // voice lane
    if let Some(r) = crate::voice::ipc::route(&app, &label, channel, &payload).await {
        return r;
    }
    // agents lane
    if let Some(r) = crate::agents::ipc::route(&app, channel, &payload).await {
        return r;
    }
    if channel == "ui:context-menu" {
        return Ok(crate::host::menu::popup(&app, &label, &payload));
    }
    route(&app, window, channel, payload).await
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
///
/// K-22: `textScale` is the person's own factor (1.0 to 2.0) and
/// `osTextScale` the Windows text size; pages apply their product, which
/// arrives as `ui:text-scale {scale}`. K-24: `overlayPos` is the bar's
/// anchor `{h, v, offX, offY}`.
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
    v["textScale"] = json!(windows::text_scale::user_factor());
    v["osTextScale"] = json!(windows::text_scale::os_factor());
    v["overlayPos"] = s.get_overlay_pos();
    v["showButton"] = json!(s.get_show_button());
    let extra = shared(app).call_value(app, |h| h.settings_extra.as_ref());
    if let (Some(Value::Object(extra)), Value::Object(base)) = (extra, &mut v) {
        for (k, val) in extra {
            base.insert(k, val);
        }
    }
    v
}

/// K-24: a conversation's whole first question. A row's `title` is cut at
/// 90 characters, which would truncate what Ask again re-asks.
fn first_question(entry: &Value) -> String {
    let empty = Vec::new();
    let messages = entry["messages"].as_array().unwrap_or(&empty);
    let asked = messages.iter().find(|m| m["role"] == "user");
    asked
        .and_then(|m| m["text"].as_str())
        .unwrap_or_default()
        .to_string()
}

/// `history:list`'s reply (K-24).
pub fn history_list(rows: Vec<Value>, enabled: bool) -> Value {
    json!({ "rows": rows, "enabled": enabled })
}

/// The output a window channel acts on: `label` in the payload, else the
/// calling output, else the bar's target while it is open.
fn output_for(app: &AppHandle, caller: &str, p: &Value) -> Option<String> {
    let named = p_str(p, "label");
    if outputs::is_output(&named) {
        return Some(named);
    }
    if outputs::is_output(caller) {
        return Some(caller.to_string());
    }
    outputs::focused(app).filter(|l| outputs::with(app, l, |o| o.active).unwrap_or(false))
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
                windows::collapse_panel(app);
            }
        }
        "overlay:minimize" => windows::minimize(app),
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
        // `{label?}`: that window, else the calling output, else the bar's
        // target. With no window at all the bar folds, as before.
        "output:close" => match output_for(app, &label, &p) {
            // Its own page has played the close; any other window fades
            // first.
            Some(l) if l == label => outputs::close(app, &l),
            Some(l) => outputs::close_shown(app, &l),
            None => windows::collapse_panel(app),
        },
        "output:close-all" => outputs::close_all(app),
        // Esc in an output: the foreground goes back to the app (D-01).
        "output:blur" => {
            if let Some(h) = windows::flags::hwnd_of(win) {
                windows::flags::blur(h);
            }
        }
        // `{dir: 1|-1}` -> `{label}`: the new target, or null.
        "output:cycle-target" => {
            let dir = if p_num(&p, "dir") < 0.0 { -1 } else { 1 };
            return Ok(json!({ "label": outputs::cycle_target(app, dir) }));
        }
        // `{label?}` -> `{text}`: the window's last answer, '' if none.
        "output:last-answer" => {
            let text = output_for(app, &label, &p)
                .map(|l| outputs::last_answer(app, &l))
                .unwrap_or_default();
            return Ok(json!({ "text": text }));
        }
        // `{label?}` -> `{ok}`: "Start over" undone within its 10 s.
        "output:undo-reset" => {
            let done = output_for(app, &label, &p).is_some_and(|l| outputs::undo_reset(app, &l));
            return Ok(json!({ "ok": done }));
        }
        // `{top, height}`: the visible sheet as the page measures it, `top`
        // from the window's edge; `set_sheet` keeps it in glass DIPs.
        "output:glass" => outputs::set_sheet(app, &label, p_num(&p, "top"), p_num(&p, "height")),
        // The sheet has faded and the frame without it is painted: the
        // window may hide.
        "output:gone" => outputs::page_gone(app, &label),
        // `{dx, dy, dw, dh}` in DIPs -> `{ok}`.
        "window:nudge" => {
            let d = (
                p_num(&p, "dx"),
                p_num(&p, "dy"),
                p_num(&p, "dw"),
                p_num(&p, "dh"),
            );
            return Ok(json!({ "ok": windows::nudge_window(app, &label, d) }));
        }
        // `{dirty}`: the bar holds typed text (Esc clears it before it
        // stops a task, B-102).
        "bar:dirty" => {
            shared(app).lock().bar_dirty = p_bool(&p, "dirty");
        }
        // The bar's field has focus after a hotkey (B-104).
        "bar:ready" => windows::bar_ready(&p),
        // The bar has folded and painted the frame without it: the window
        // may shrink or go.
        "bar:folded" => windows::bar_folded(app),
        // `{text}`: opens the bar with the text in it.
        "bar:prefill-open" => windows::open_bar(app, None, Some(p_str(&p, "text"))),
        // `{text, priority}` -> `{to}`: relayed as `a11y:say` (B-023).
        "a11y:announce" => {
            let to = windows::announce(app, &p_str(&p, "text"), &p_str(&p, "priority"));
            return Ok(json!({ "to": to }));
        }
        // `{scale}`: the person's factor, 1.0 to 2.0 -> `{ok, textScale,
        // osTextScale, scale}`; every window gets `ui:text-scale {scale}`.
        "settings:set-text-scale" => {
            let user = windows::text_scale::set_user(app, p_num(&p, "scale"));
            return Ok(json!({
                "ok": true,
                "textScale": user,
                "osTextScale": windows::text_scale::os_factor(),
                "scale": windows::text_scale::scale(),
            }));
        }
        "open-external" => {
            windows::open_url(app, &p_str(&p, "url"));
        }
        "app:help" => return Ok(windows::open_help(app)),
        "chat:new" => {
            if outputs::is_output(&label) {
                outputs::reset(app, &label);
            } else {
                // A new conversation is about the whole screen again.
                crate::region::clear_active();
                windows::new_chat(app);
            }
        }
        "overlay:open-history" => windows::open_history(app),
        // K-24: `{rows, enabled}`; `enabled` is whether saving is on.
        "history:list" => {
            let hist = history::get();
            let mut rows = hist.list(&p_str(&p, "query"));
            for row in &mut rows {
                let entry = hist.entry(row["id"].as_str().unwrap_or_default());
                row["question"] = json!(entry.as_ref().map_or_else(String::new, first_question));
            }
            return Ok(history_list(rows, s.get_save_history()));
        }
        "history:open" => {
            let Some(entry) = history::get().entry(&p_str(&p, "id")) else {
                return Ok(json!({ "ok": false }));
            };
            // IPC-18: the reopened conversation is about the whole screen.
            // Only the window it lands in stops its Ask (outputs.rs).
            crate::region::clear_active();
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
        "settings:save-skill" => {
            let skill = s.set_skill_id(&p_str(&p, "skill"), crate::host::skill_exists);
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
        "settings:set-show-button" => {
            let on = s.set_show_button(p_bool(&p, "on"));
            windows::button_changed(app);
            return Ok(json!({ "ok": true, "showButton": on, "overlayPos": s.get_overlay_pos() }));
        }
        "settings:set-surface" => {
            let v = s.set_surface(&p_str(&p, "surface"));
            crate::backdrop::broadcast_tier();
            let glass = glass_tier(app);
            return Ok(json!({ "surface": v, "tier": glass["tier"], "reasons": glass["reasons"] }));
        }
        "settings:set-glass" => {
            let v = s.set_glass_intensity(p_num(&p, "glassIntensity"));
            crate::backdrop::set_intensity(v);
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
            // The page says "Copied" only when it was.
            let done = app.clipboard().write_text(p_str(&p, "text")).is_ok();
            return Ok(json!({ "ok": done }));
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

/// No row answers this channel. Unreachable for an allowlisted channel
/// (`every_invoke_channel_has_a_route`); kept so a future allowlist entry
/// without a row fails loudly instead of hanging the page.
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

    /// Every invoke channel preload.js allows has a match arm in one of the
    /// routers, so none falls through to `not_ported`.
    #[test]
    fn every_invoke_channel_has_a_route() {
        let routers = [
            include_str!("ipc.rs"),
            include_str!("desktop/channels.rs"),
            include_str!("host/ask.rs"),
            include_str!("host/find.rs"),
            include_str!("host/agent.rs"),
            include_str!("host/settings.rs"),
            include_str!("voice/ipc.rs"),  // voice lane
            include_str!("agents/ipc.rs"), // agents lane
        ]
        .concat();
        // Arms are `"x" =>`, `"x" | "y" =>`, or `channel == "x"`.
        let routed = |c: &str| {
            let q = format!("\"{c}\"");
            routers.match_indices(&q).any(|(i, _)| {
                let after = routers[i + q.len()..].trim_start();
                let before = routers[..i].trim_end();
                after.starts_with("=>")
                    || after.starts_with('|')
                    || before.ends_with('|')
                    || before.ends_with("==")
            })
        };
        let missing: Vec<String> = preload_set("INVOKE_CHANNELS")
            .into_iter()
            .filter(|c| !routed(c))
            .collect();
        assert!(missing.is_empty(), "no route for {missing:?}");
    }

    #[test]
    fn invoke_allowlist_matches_preload() {
        assert_eq!(ours(INVOKE_CHANNELS), preload_set("INVOKE_CHANNELS"));
    }

    #[test]
    fn listen_allowlist_matches_preload() {
        assert_eq!(ours(LISTEN_CHANNELS), preload_set("LISTEN_CHANNELS"));
    }

    /// Every Rust file under src/, read at test time so a file another
    /// lane adds is scanned too.
    fn rust_sources() -> Vec<(std::path::PathBuf, String)> {
        fn walk(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let text = std::fs::read_to_string(&p).unwrap_or_default();
                    out.push((p, text));
                }
            }
        }
        let src = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
        let mut out = Vec::new();
        walk(&src, &mut out);
        out
    }

    /// B-095: every channel a page may listen on is sent from somewhere: its
    /// quoted name appears in a Rust file other than this one, and not as a
    /// match arm.
    #[test]
    fn every_listen_channel_has_a_sender() {
        let sources: Vec<String> = rust_sources()
            .into_iter()
            .filter(|(p, _)| p.file_name().is_some_and(|n| n != "ipc.rs"))
            .map(|(_, text)| text)
            .collect();
        assert!(sources.len() > 20, "read the sources");
        let sent = |c: &str| {
            let q = format!("\"{c}\"");
            sources.iter().any(|t| {
                t.match_indices(&q).any(|(i, _)| {
                    let after = t[i + q.len()..].trim_start();
                    let before = t[..i].trim_end();
                    !after.starts_with("=>")
                        && !after.starts_with('|')
                        && !before.ends_with('|')
                        && !before.ends_with("==")
                })
            })
        };
        let missing: Vec<&str> = LISTEN_CHANNELS
            .iter()
            .copied()
            .filter(|&c| !sent(c))
            .collect();
        assert!(missing.is_empty(), "nobody sends {missing:?}");
    }

    /// The per-lane blocks (BACKLOG "Hotspots") sit in the same order in
    /// both files, once in each list.
    #[test]
    fn lane_blocks_match_preload() {
        let markers = |text: &str| -> Vec<String> {
            text.lines()
                .map(str::trim)
                .filter(|l| l.starts_with("// ---- "))
                .map(String::from)
                .collect()
        };
        let rust = markers(include_str!("ipc.rs"));
        assert_eq!(rust, markers(PRELOAD));
        assert_eq!(rust.len(), 16);
        assert_eq!(
            rust[0],
            "// ---- lane: windows (do not edit outside your lane) ----"
        );
        assert_eq!(rust[7], "// ---- end: agent ----");
    }

    /// B-096: `settings:get` carries main.js's 28 keys (main.js:2525-2562)
    /// and the documented Tauri additions. The `settings_extra` hook adds
    /// its own over them; none of those is a core key.
    #[test]
    fn settings_get_key_set() {
        const ELECTRON: [&str; 28] = [
            "findModel",
            "defaultFindModel",
            "agentModel",
            "defaultAgentModel",
            "find",
            "agent",
            "winBuild",
            "hasKey",
            "model",
            "defaultModel",
            "brain",
            "brainEnabled",
            "defaultBrain",
            "showPaid",
            "skills",
            "skill",
            "shortcuts",
            "active",
            "defaults",
            "version",
            "update",
            "canHide",
            "saveHistory",
            "historyCount",
            "launchAtLogin",
            "surface",
            "glass",
            "launchAtLoginLive",
        ];
        const TAURI: [&str; 7] = [
            "keys",
            "glassIntensity",
            "hasSentFirstRequest",
            "textScale",
            "osTextScale",
            "overlayPos",
            "showButton",
        ];
        const HOOKED: [&str; 5] = [
            "providers",
            "failover",
            "keyHints",
            "keyStorage",
            "recipients",
        ];
        let src = include_str!("ipc.rs");
        let start = src.find("pub fn settings_data(").unwrap();
        let body = &src[start..];
        // The function ends at the first brace in column 0 (LF or CRLF).
        let body = &body[..body.find("\n}").unwrap()];
        let mut got: Vec<&str> = Vec::new();
        for line in body.lines().map(str::trim) {
            if let Some(rest) = line.strip_prefix('"') {
                if let Some(end) = rest.find("\":") {
                    got.push(&rest[..end]);
                }
            } else if let Some(rest) = line.strip_prefix("v[\"") {
                if let Some(end) = rest.find('"') {
                    got.push(&rest[..end]);
                }
            }
        }
        let mut want: Vec<&str> = ELECTRON.iter().chain(TAURI.iter()).copied().collect();
        got.sort_unstable();
        want.sort_unstable();
        assert_eq!(got, want);
        assert!(HOOKED.iter().all(|k| !want.contains(k)));
    }

    #[test]
    fn output_cannot_start_agent() {
        assert!(!permitted(Caller::of("output-2"), "agent:start"));
        assert!(permitted(Caller::of("overlay"), "agent:start"));
        // The trail's Retry starts the window's own task again, nothing else.
        let own = vec![json!({ "role": "user", "text": "open notepad" })];
        assert!(asks_for(&own, "open notepad"));
        assert!(!asks_for(&own, "delete my files"));
        assert!(!asks_for(&own, " "));
        // An output keeps its own controls; settings stay with Settings.
        let own_controls = [
            "agent:stop",
            "agent:confirm",
            "query:submit",
            "output:close",
            "find:step",
        ];
        for c in own_controls {
            assert!(permitted(Caller::Output, c), "{c}");
        }
        let settings_only = [
            "settings:save-key",
            "shortcuts:set",
            "update:restart",
            "app:quit",
        ];
        for c in settings_only {
            assert!(!permitted(Caller::Output, c), "{c}");
            assert!(permitted(Caller::Settings, c), "{c}");
        }
        assert!(!permitted(Caller::Output, "history:clear"));
        assert!(permitted(Caller::History, "history:clear"));
        // Lens and buddy: their own channels and glass:*.
        assert!(permitted(Caller::of("lens-1"), "glass:hello"));
        assert!(!permitted(Caller::Lens, "query:submit"));
        assert!(permitted(Caller::of("buddy"), "buddy:advance"));
        assert!(!permitted(Caller::Buddy, "agent:stop"));
        assert!(!permitted(Caller::of("fixture"), "settings:get"));
    }

    #[test]
    fn every_invoke_channel_has_a_label_rule() {
        let missing: Vec<&str> = INVOKE_CHANNELS
            .iter()
            .copied()
            .filter(|&c| rule(c).is_none())
            .collect();
        assert!(missing.is_empty(), "no caller rule for {missing:?}");
        let stale: Vec<&str> = EXCEPTIONS
            .iter()
            .map(|(c, _)| *c)
            .filter(|c| !allowed(c))
            .collect();
        assert!(stale.is_empty(), "stale rules: {stale:?}");
    }

    /// The quoted channels a page script passes to `invoke(`.
    fn invoked(js: &str) -> Vec<&str> {
        let mut out = Vec::new();
        for (i, _) in js.match_indices("invoke(") {
            let rest = js[i + "invoke(".len()..].trim_start();
            let Some(q) = rest.chars().next().filter(|c| *c == '\'' || *c == '"') else {
                continue;
            };
            if let Some(end) = rest[1..].find(q) {
                out.push(&rest[1..1 + end]);
            }
        }
        out
    }

    /// B-089 must not cost a feature: every allowlisted channel a page's
    /// scripts call today is allowed from that page.
    #[test]
    fn pages_may_call_what_they_call() {
        let glass = include_str!("../../renderer/glass.js");
        let frame = include_str!("../../renderer/frame.js");
        let pages: [(Caller, Vec<&str>); 8] = [
            (
                Caller::Bar,
                vec![
                    glass,
                    frame,
                    include_str!("../../renderer/intent.js"),
                    include_str!("../../renderer/overlay.js"),
                    include_str!("../../renderer/bar.js"),
                    include_str!("../../renderer/voice.js"), // voice lane
                ],
            ),
            (
                Caller::Output,
                vec![
                    glass,
                    frame,
                    include_str!("../../renderer/output.js"),
                    include_str!("../../renderer/trail.js"),
                    include_str!("../../renderer/glob.js"),
                ],
            ),
            (
                Caller::Settings,
                vec![
                    glass,
                    frame,
                    include_str!("../../settings/settings.js"),
                    include_str!("../../renderer/voice-settings.js"), // voice lane
                    include_str!("../../renderer/agents-settings.js"), // agents lane
                ],
            ),
            (
                Caller::History,
                vec![glass, frame, include_str!("../../history/history.js")],
            ),
            (
                Caller::Buddy,
                vec![glass, include_str!("../../renderer/buddy.js")],
            ),
            (Caller::Lens, vec![include_str!("../../renderer/lens.js")]),
            (
                Caller::Region,
                vec![include_str!("../../renderer/region.js")],
            ),
            // agents lane
            (
                Caller::Agents,
                vec![glass, include_str!("../../renderer/agents.js")],
            ),
        ];
        let mut refused = Vec::new();
        for (caller, scripts) in &pages {
            for js in scripts {
                for c in invoked(js) {
                    // The trail's Retry: allowed for the window's own task.
                    let retry = *caller == Caller::Output && c == RETRY;
                    if allowed(c) && !permitted(*caller, c) && !retry {
                        refused.push(format!("{caller:?} {c}"));
                    }
                }
            }
        }
        assert!(refused.is_empty(), "refused: {refused:?}");
    }

    #[test]
    fn panicking_route_returns_err() {
        async fn boom() -> Result<Value, String> {
            panic!("boom")
        }
        let mut settled = false;
        let r = guarded("query:submit", boom(), || settled = true).now_or_never();
        assert_eq!(r, Some(Err("query:submit failed: boom".to_string())));
        assert!(settled);
        let ok = guarded("settings:get", async { Ok(json!(1)) }, || {}).now_or_never();
        assert_eq!(ok, Some(Ok(json!(1))));

        // The caller's Ask entry goes; another window's stays busy.
        let mut s = AppState::default();
        s.query_started("output-1", 4);
        s.query_started("output-2", 7);
        settle_after_panic(&mut s, "output-2", true);
        assert!(s.query_running("output-1"));
        assert!(!s.query_running("output-2"));
        assert!(s.busy);
        settle_after_panic(&mut s, "output-1", true);
        assert!(s.queries.is_empty() && !s.busy);
        // `busy` set by the panicked call alone goes back; set before it stays.
        s.busy = true;
        settle_after_panic(&mut s, "overlay", false);
        assert!(!s.busy);
        s.busy = true;
        settle_after_panic(&mut s, "overlay", true);
        assert!(s.busy);
    }

    #[test]
    fn history_row_question_is_the_whole_first_message() {
        let long = "why ".repeat(60);
        let entry = json!({ "messages": [
            { "role": "assistant", "text": "hello" },
            { "role": "user", "text": long },
            { "role": "user", "text": "later" },
        ] });
        assert_eq!(first_question(&entry), long);
        assert_eq!(first_question(&json!({})), "");
    }

    #[test]
    fn history_list_has_enabled() {
        let v = history_list(vec![json!({ "id": "c1" })], false);
        assert_eq!(v["rows"][0]["id"], "c1");
        assert_eq!(v["enabled"], false);
        assert_eq!(history_list(Vec::new(), true)["enabled"], true);
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
    }

    #[test]
    fn bridge_script_carries_both_allowlists() {
        let s = bridge_script();
        assert!(s.contains("aevalBridge([\"overlay:trigger\""));
        let invoke = js_list(INVOKE_CHANNELS);
        let listen = js_list(LISTEN_CHANNELS);
        assert!(s.contains(&format!("aevalBridge({invoke}, {listen});")));
        assert!(listen.contains("\"lens:loupe\""));
        assert!(invoke.contains("\"settings:set-glass\""));
    }
}
