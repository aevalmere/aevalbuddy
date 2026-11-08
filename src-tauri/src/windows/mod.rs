//! Window manager (PARITY WIN-*, POS-*, NEW-01..07; ARCHITECTURE.md
//! section 9 documents the API agent d's renderer and the other agents use).
//!
//! Windows: `overlay` (the orb and the bar), any number of floating
//! `output-<n>` windows (outputs.rs), `settings`, `history`. Every one is a
//! frameless, transparent, topmost tool window hidden from capture; the
//! overlay and outputs carry the 20 px stage margin and everything here
//! speaks their glass rectangle in DIPs.

pub mod flags;
pub mod layout;
pub mod motion;
pub mod outputs;
pub mod screen;
pub mod stage;

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::state::{shared, BusEvent, Mode, Rect};
use crate::{hotkeys, ipc, paths, store};
use flags::{Meta, Resize};
use layout::Anchor;

pub const OVERLAY: &str = "overlay";
pub const SETTINGS: &str = "settings";
pub const HISTORY: &str = "history";
pub const HELP_URL: &str = "https://github.com/aevalmere/aevalbuddy/issues/new/choose";

/// The windows a session opens at boot: the overlay and one warm, hidden
/// output window, so the first answer globs out with no page load. The
/// scaffold smoke waits for each to reach Rust through `window.aeval`.
pub const SCAFFOLD_WINDOWS: [(&str, &str); 2] = [
    (OVERLAY, "renderer/index.html"),
    ("output-1", "renderer/output.html"),
];

static BOOT: OnceLock<std::time::Instant> = OnceLock::new();

/// Marks process start, for `bootMs` (LIFE-08).
pub fn mark_start() {
    let _ = BOOT.set(std::time::Instant::now());
}

// ------------------------------------------------------------- plumbing

/// Sends an event to one window, as Electron's per-webContents send, and
/// mirrors it on the bus for the smoke run.
pub fn emit(app: &AppHandle, label: &str, channel: &str, payload: Value) {
    let _ = app.emit_to(label, channel, payload.clone());
    let _ = shared(app).bus.send(BusEvent {
        channel: channel.to_string(),
        target: label.to_string(),
        payload,
    });
}

pub fn window(app: &AppHandle, label: &str) -> Option<WebviewWindow> {
    app.get_webview_window(label)
}

fn hwnd(app: &AppHandle, label: &str) -> Option<isize> {
    flags::hwnd_for(label).or_else(|| window(app, label).and_then(|w| flags::hwnd_of(&w)))
}

/// Debounce by key: only the newest call within `ms` runs.
fn debounce(key: &str, ms: u64, f: impl FnOnce() + Send + 'static) {
    static GEN: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    let gens = GEN.get_or_init(|| Mutex::new(HashMap::new()));
    let ticket = {
        let mut g = gens.lock();
        let e = g.entry(key.to_string()).or_insert(0);
        *e += 1;
        *e
    };
    let key = key.to_string();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        if gens.lock().get(&key).copied() == Some(ticket) {
            f();
        }
    });
}

/// Telling an app-driven bounds change from a drag (main.js markApp).
fn app_marks() -> &'static Mutex<HashMap<String, u64>> {
    static M: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn mark_app(label: &str) {
    app_marks()
        .lock()
        .insert(label.to_string(), crate::state::now_ms());
}

fn app_driven(app: &AppHandle, label: &str) -> bool {
    let Some(h) = hwnd(app, label) else {
        return true;
    };
    if motion::is_animating(h) {
        return true;
    }
    let at = app_marks().lock().get(label).copied().unwrap_or(0);
    crate::state::now_ms().saturating_sub(at) < 500
}

/// Pages shown once they have loaded (Electron's ready-to-show).
fn show_on_load() -> &'static Mutex<HashSet<String>> {
    static S: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashSet::new()))
}

fn pending_section() -> &'static Mutex<Option<String>> {
    static S: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// One page in a frameless, transparent, topmost tool window that is hidden
/// from capture and sees only `window.aeval`. `outer` is the OS window in
/// DIPs (glass plus margin).
pub fn create_page(
    app: &AppHandle,
    label: &str,
    page: &str,
    outer: Rect,
    meta: Meta,
) -> tauri::Result<WebviewWindow> {
    let bridge = ipc::bridge_script();
    let w = WebviewWindowBuilder::new(app, label, WebviewUrl::App(page.into()))
        .title("AevalBuddy")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(true)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .content_protected(true)
        .focused(false)
        .visible(false)
        .position(outer.x, outer.y)
        .inner_size(outer.width, outer.height)
        .data_directory(paths::webview_data_dir())
        .initialization_script(&bridge)
        .build()?;
    flags::adopt(app, &w, meta);
    Ok(w)
}

/// Called for every page load (lib.rs on_page_load).
pub fn on_page_loaded(app: &AppHandle, label: &str) {
    let app = app.clone();
    let label = label.to_string();
    tauri::async_runtime::spawn(async move {
        if label == OVERLAY {
            let first = {
                let st = shared(&app);
                let mut s = st.lock();
                if s.boot_ms == 0 {
                    let ms = BOOT.get().map(|b| b.elapsed().as_millis() as u64);
                    s.boot_ms = ms.unwrap_or(1).max(1);
                    true
                } else {
                    false
                }
            };
            if first {
                if let Some(h) = hwnd(&app, OVERLAY) {
                    flags::show_inactive(h);
                }
            }
            send_overlay_state(&app);
        } else if outputs::is_output(&label) {
            outputs::on_loaded(&app, &label);
        } else if label == SETTINGS {
            // The section goes with the page's hello; this is the fallback
            // for a page that never says it.
            let a = app.clone();
            debounce("settings-section", 1500, move || send_section(&a));
        }
        let show = show_on_load().lock().remove(&label);
        if show {
            if let Some(w) = window(&app, &label) {
                flags::show_focused(&w);
            }
            if label == SETTINGS {
                tour_notify(&app, "settings-opened");
            }
        }
    });
}

fn send_section(app: &AppHandle) {
    if let Some(id) = pending_section().lock().take() {
        emit(app, SETTINGS, "settings:section", json!({ "id": id }));
    }
}

/// A page's first invoke (`glass:hello`). Its listeners are registered by
/// now (the bridge holds invokes until they are), so this is when the first
/// state goes (ARCHITECTURE.md 10.3). Whoever answers `glass:hello` calls
/// this.
pub fn on_hello(app: &AppHandle, label: &str) {
    if label == OVERLAY {
        emit(app, OVERLAY, "overlay:state", overlay_state(app, None));
    } else if outputs::is_output(label) {
        outputs::on_loaded(app, label);
    } else if label == SETTINGS {
        send_section(app);
    }
}

fn tour_notify(app: &AppHandle, event: &str) {
    shared(app).call_str(app, event, |h| h.tour_notify.as_ref());
}

// --------------------------------------------------------------- state

fn anchor() -> Anchor {
    Anchor::from_value(&store::get().get_overlay_pos())
}

fn pill_rect() -> Rect {
    layout::pill_bounds(screen::work_area(), &anchor())
}

fn input_rect(height: f64) -> Rect {
    let w = store::get().get_input_width() as f64;
    layout::input_bounds(screen::work_area(), &anchor(), w, height)
}

/// The bar's glass rect as it is now, or where it would be.
pub fn bar_rect(app: &AppHandle) -> Rect {
    match hwnd(app, OVERLAY) {
        Some(h) if flags::is_visible(h) => motion::glass(h),
        _ => input_rect(shared(app).lock().input_height),
    }
}

/// The `overlay:state` payload (POS-13), plus the Tauri additions:
/// `glassIntensity`, `hasSentFirstRequest`, `keys`, and for an output
/// window its own `label` and `outputFixedHeight`.
pub fn overlay_state(app: &AppHandle, for_label: Option<&str>) -> Value {
    let store = store::get();
    let wa = screen::work_area();
    let height = shared(app).lock().input_height;
    let pb = pill_rect();
    let ib = input_rect(height);
    let st = shared(app);
    let s = st.lock();
    let focused = s.focused_output.clone();
    let any_chat = s.outputs.values().any(|o| !o.chat.messages.is_empty());
    let region = focused
        .as_ref()
        .and_then(|l| s.outputs.get(l))
        .and_then(|o| o.chat.region)
        .map(|r| json!({ "width": r.width, "height": r.height }))
        .unwrap_or(Value::Null);
    let own = for_label.and_then(|l| s.outputs.get(l));
    let fixed = own.map(|o| o.user_height > 0.0).unwrap_or(false);
    let pill_state = if s.agent.running {
        "agent"
    } else if s.busy {
        "busy"
    } else if any_chat {
        "chat"
    } else {
        "idle"
    };
    let agent = if s.agent.running {
        json!({ "running": true, "step": s.agent.step, "state": s.agent.state })
    } else {
        json!({ "running": false })
    };
    let hotkey_failed = s.hotkeys.get("captureQuery").copied().flatten() == Some(false);
    let mut v = json!({
        "mode": s.mode.as_str(),
        "orbAt": { "x": pb.x - ib.x, "y": pb.y - ib.y, "width": pb.width, "height": pb.height },
        "reducedMotion": motion::reduced() || s.reduced_motion,
        "hasKey": store.has_api_key(),
        "captureOk": s.capture_ok,
        "inputAnchor": store.get_overlay_pos(),
        "outputPos": store.get_output_pos(),
        "pillState": pill_state,
        "inputWidth": store.get_input_width(),
        "outputWidth": store.get_output_width(),
        "outputFixedHeight": fixed,
        "chatMax": layout::output_max_height(wa) - 56.0,
        "region": region,
        "busy": s.busy,
        "barMode": s.bar_mode,
        "agent": agent,
        "pillError": s.pill_error,
        "offline": !crate::updater::online(),
        "findOpen": s.find_open,
        "shortcuts": store.get_shortcuts(),
        "hotkeyFailed": if hotkey_failed { store.shortcut("captureQuery") } else { String::new() },
        "glassIntensity": store.get_glass_intensity(),
        "hasSentFirstRequest": store.get_has_sent_first_request(),
        "keys": store.provider_keys_present(),
        "outputs": s.outputs.values().filter(|o| o.active).count(),
    });
    if let Some(l) = for_label {
        v["label"] = json!(l);
        v["focused"] = json!(focused.as_deref() == Some(l));
    }
    v
}

/// Sends `overlay:state` to the overlay and every output window.
pub fn send_overlay_state(app: &AppHandle) {
    emit(app, OVERLAY, "overlay:state", overlay_state(app, None));
    let labels: Vec<String> = shared(app).lock().outputs.keys().cloned().collect();
    for l in labels {
        emit(app, &l, "overlay:state", overlay_state(app, Some(&l)));
    }
}

// --------------------------------------------------------------- boot

/// Creates the overlay and the warm output window (LIFE-06).
pub fn boot(app: &AppHandle) -> tauri::Result<()> {
    let glass = pill_rect();
    let outer = stage::inflate(glass, stage::MARGIN);
    let meta = Meta {
        margin: stage::MARGIN,
        resize: Resize::None,
        min: layout::PILL,
        max: layout::PILL,
    };
    let w = create_page(app, OVERLAY, SCAFFOLD_WINDOWS[0].1, outer, meta)?;
    if let Some(h) = flags::hwnd_of(&w) {
        flags::set_focusable(h, false);
        motion::set_glass(h, glass);
    }
    flags::set_click_through(&w, true);
    outputs::create_spare(app)?;
    flags::start_poll(app);
    Ok(())
}

/// LIFE-19: every 4 s, every visible window back on top.
pub fn start_keep_on_top(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(4)).await;
            keep_on_top(&app);
        }
    });
}

pub fn keep_on_top(app: &AppHandle) {
    for (label, _) in app.webview_windows() {
        let keep = label == OVERLAY
            || label == SETTINGS
            || outputs::is_output(&label)
            || label.starts_with("lens-");
        if !keep || lowered().lock().contains(&label) {
            continue;
        }
        if let Some(h) = hwnd(app, &label) {
            if flags::is_visible(h) {
                flags::pin_on_top(h);
            }
        }
    }
}

// ------------------------------------------------------- expand/collapse

fn apply_input_limits(app: &AppHandle) {
    let Some(h) = hwnd(app, OVERLAY) else {
        return;
    };
    let mode = shared(app).lock().mode;
    let b = motion::glass(h);
    if mode != Mode::Panel {
        // The pill and the layout ghost never take the panel limits, or
        // Windows widens the pill and it reads back as a chosen width.
        flags::set_limits(h, (b.width, b.height), (b.width, b.height));
        flags::set_meta(h, |m| m.resize = Resize::None);
        return;
    }
    let (lo, hi) = (store::MIN_W as f64, store::MAX_W as f64);
    flags::set_limits(h, (lo, b.height), (hi, b.height));
    flags::set_meta(h, |m| m.resize = Resize::Horizontal);
}

/// Every programmatic change to the bar: unlock, move, re-lock, mark.
fn set_input_bounds(app: &AppHandle, r: Rect) {
    let Some(h) = hwnd(app, OVERLAY) else {
        return;
    };
    mark_app(OVERLAY);
    flags::set_limits(h, (1.0, 1.0), (0.0, 0.0));
    motion::set_glass(h, r);
    apply_input_limits(app);
}

fn sync_input_bounds(app: &AppHandle) {
    keep_on_top(app);
    let (mode, height) = {
        let st = shared(app);
        let s = st.lock();
        (s.mode, s.input_height)
    };
    let Some(h) = hwnd(app, OVERLAY) else {
        return;
    };
    if mode != Mode::Panel {
        return;
    }
    let want = input_rect(height);
    let have = motion::glass(h);
    if (have.height - want.height).abs() >= 1.0 || (have.y - want.y).abs() >= 1.0 {
        set_input_bounds(app, want);
    } else {
        apply_input_limits(app);
    }
    mark_app(OVERLAY);
}

fn collapse_gen() -> &'static Mutex<u64> {
    static G: OnceLock<Mutex<u64>> = OnceLock::new();
    G.get_or_init(|| Mutex::new(0))
}

/// POS-08.
pub fn expand_panel(app: &AppHandle) {
    let Some(w) = window(app, OVERLAY) else {
        return;
    };
    let Some(h) = flags::hwnd_of(&w) else {
        return;
    };
    let (was_pill, height) = {
        let st = shared(app);
        let mut s = st.lock();
        if s.mode == Mode::Hud {
            return;
        }
        let was = s.mode == Mode::Pill;
        s.mode = Mode::Panel;
        s.hidden = false;
        (was, s.input_height)
    };
    *collapse_gen().lock() += 1;
    flags::set_click_through(&w, false);
    flags::set_focusable(h, true);
    flags::pin_on_top(h);
    send_overlay_state(app);
    motion::stop(h);
    set_input_bounds(app, input_rect(height));
    if was_pill {
        let app2 = app.clone();
        debounce("sync-input", layout::T_EXPAND, move || {
            sync_input_bounds(&app2)
        });
    }
    flags::show_focused(&w);
    hotkeys::apply(app);
}

/// POS-09. Output windows hide with the bar unless `keep_output`.
pub fn collapse_panel(app: &AppHandle, keep_output: bool) {
    let Some(w) = window(app, OVERLAY) else {
        return;
    };
    let Some(h) = flags::hwnd_of(&w) else {
        return;
    };
    if !keep_output {
        shared(app).call(app, |k| k.abort_query.as_ref());
        outputs::hide_all(app);
    }
    let find_open = shared(app).lock().find_open;
    if find_open {
        shared(app).call(app, |k| k.close_find.as_ref());
    }
    let was_panel = {
        let st = shared(app);
        let mut s = st.lock();
        let was = s.mode == Mode::Panel;
        s.mode = Mode::Pill;
        s.input_height = layout::DEFAULT_INPUT_HEIGHT;
        was
    };
    flags::set_focusable(h, false);
    flags::pin_on_top(h);
    flags::set_click_through(&w, true);
    flags::show_inactive(h);
    motion::stop(h);
    if was_panel {
        // The bar folds into the orb inside the page first; the window
        // shrinks once that is over, invisible because the orb keeps its
        // corner.
        send_overlay_state(app);
        let gen = {
            let mut g = collapse_gen().lock();
            *g += 1;
            *g
        };
        let wait = if motion::reduced() {
            0
        } else {
            layout::T_COLLAPSE + 30
        };
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(wait)).await;
            if *collapse_gen().lock() != gen || shared(&app2).lock().mode != Mode::Pill {
                return;
            }
            set_input_bounds(&app2, pill_rect());
            send_overlay_state(&app2);
        });
    } else {
        set_input_bounds(app, pill_rect());
        send_overlay_state(app);
    }
    hotkeys::apply(app);
}

/// main.js triggerCapture: the bar opens, and conversations come back.
pub fn trigger_capture(app: &AppHandle) {
    {
        let st = shared(app);
        let mut s = st.lock();
        if s.mode == Mode::Hud {
            return;
        }
        s.hidden = false;
    }
    expand_panel(app);
    outputs::show_with_chat(app);
}

pub fn set_bar_mode(app: &AppHandle, mode: &str) {
    if !shared(app).call_str(app, mode, |h| h.set_bar_mode.as_ref()) {
        let next = if mode == "find" || mode == "agent" {
            mode
        } else {
            "ask"
        };
        shared(app).lock().bar_mode = next.to_string();
        send_overlay_state(app);
        hotkeys::apply(app);
    }
}

/// main.js openBar(mode, { prefill }): `None` opens the one inferring input;
/// a mode locks the bar to it.
pub fn open_bar(app: &AppHandle, mode: Option<&str>, prefill: Option<String>) {
    if window(app, OVERLAY).is_none() || shared(app).lock().mode == Mode::Hud {
        return;
    }
    let lock = mode.filter(|m| matches!(*m, "ask" | "find" | "agent"));
    shared(app).lock().pill_error = false;
    set_bar_mode(app, if lock == Some("find") { "find" } else { "ask" });
    emit(app, OVERLAY, "bar:open", json!({ "lock": lock }));
    trigger_capture(app);
    shared(app).call(app, |h| h.prefetch_bar_context.as_ref());
    if let Some(text) = prefill {
        let m = lock.unwrap_or("ask");
        emit(
            app,
            OVERLAY,
            "bar:prefill",
            json!({ "text": text, "mode": m }),
        );
    }
}

/// The pill and the tray: the trail while the agent works, else the bar.
pub fn trigger_or_trail(app: &AppHandle) {
    if shared(app).lock().agent.running {
        if !shared(app).call(app, |h| h.show_trail.as_ref()) {
            outputs::show_trail(app, false);
        }
        return;
    }
    open_bar(app, None, None);
}

/// KEY-06.
pub fn toggle_visibility(app: &AppHandle) {
    let Some(w) = window(app, OVERLAY) else {
        return;
    };
    let (hidden, mode, buddy) = {
        let st = shared(app);
        let mut s = st.lock();
        if s.mode == Mode::Hud {
            return;
        }
        s.hidden = !s.hidden;
        (s.hidden, s.mode, s.buddy_active)
    };
    if hidden {
        if buddy {
            shared(app).call(app, |h| h.cancel_tour.as_ref());
        }
        if let Some(h) = flags::hwnd_of(&w) {
            motion::stop(h);
        }
        flags::hide(&w);
        outputs::hide_windows(app);
    } else {
        if mode == Mode::Panel {
            flags::show_focused(&w);
        } else if let Some(h) = flags::hwnd_of(&w) {
            flags::show_inactive(h);
        }
        outputs::show_active_windows(app);
    }
    crate::tray::refresh(app);
    hotkeys::apply(app);
}

/// Global Escape, in main.js's priority order (KEY-08).
pub fn on_escape(app: &AppHandle) {
    let (agent, find, region, buddy, mode) = {
        let st = shared(app);
        let s = st.lock();
        (
            s.agent.running,
            s.find_open,
            s.region_active,
            s.buddy_active,
            s.mode,
        )
    };
    let st = shared(app);
    if agent && st.call(app, |h| h.stop_agent.as_ref()) {
        return;
    }
    if find && st.call(app, |h| h.close_find.as_ref()) {
        return;
    }
    if region && st.call(app, |h| h.cancel_region.as_ref()) {
        return;
    }
    if buddy && st.call(app, |h| h.cancel_tour.as_ref()) {
        return;
    }
    if mode == Mode::Hud {
        exit_hud(app);
        return;
    }
    for label in [SETTINGS, HISTORY] {
        if let Some(w) = window(app, label) {
            if w.is_focused().unwrap_or(false) {
                let _ = w.close();
                return;
            }
        }
    }
    if mode == Mode::Panel {
        collapse_panel(app, false);
    }
}

/// A global shortcut fired (hotkeys.rs).
pub fn on_hotkey(app: &AppHandle, action: &str) {
    match action {
        "captureQuery" => open_bar(app, None, None),
        "findOnScreen" => open_bar(app, Some("find"), None),
        "agentTask" => {
            if shared(app).lock().agent.running {
                if !shared(app).call(app, |h| h.show_trail.as_ref()) {
                    outputs::show_trail(app, false);
                }
            } else {
                open_bar(app, Some("agent"), None);
            }
        }
        "captureRegion" => {
            shared(app).call(app, |h| h.start_region.as_ref());
        }
        "toggleVisibility" => toggle_visibility(app),
        "escape" => on_escape(app),
        _ => {}
    }
}

/// IPC-03: the bar's content height changed.
pub fn overlay_resize(app: &AppHandle, height: f64) {
    {
        let st = shared(app);
        let mut s = st.lock();
        if s.mode != Mode::Panel {
            return;
        }
        s.input_height = height;
    }
    let Some(h) = hwnd(app, OVERLAY) else {
        return;
    };
    if motion::is_animating(h) {
        return;
    }
    set_input_bounds(app, input_rect(height));
}

/// IPC-04.
pub fn overlay_set_ignore(app: &AppHandle, ignore: bool) {
    let mode = shared(app).lock().mode;
    if mode != Mode::Pill && mode != Mode::Panel {
        return;
    }
    if let Some(w) = window(app, OVERLAY) {
        flags::set_click_through(&w, ignore);
    }
}

// ---------------------------------------------------------------- HUD

/// POS-14 with NEW-04: the layout editor places the bar only.
pub fn enter_hud(app: &AppHandle) {
    let Some(w) = window(app, OVERLAY) else {
        return;
    };
    let Some(h) = flags::hwnd_of(&w) else {
        return;
    };
    let buddy = {
        let st = shared(app);
        let s = st.lock();
        if s.mode == Mode::Hud {
            return;
        }
        s.buddy_active
    };
    if buddy {
        shared(app).call(app, |k| k.cancel_tour.as_ref());
    }
    shared(app).call(app, |k| k.abort_query.as_ref());
    motion::stop(h);
    let pending = store::get().get_overlay_pos();
    {
        let st = shared(app);
        let mut s = st.lock();
        s.mode = Mode::Hud;
        s.hidden = false;
        s.hud_pending = Some(pending.clone());
    }
    outputs::hide_all(app);
    flags::set_click_through(&w, false);
    flags::set_focusable(h, true);
    let size = hud_size();
    let r = layout::bounds_for_size(screen::work_area(), size, &Anchor::from_value(&pending));
    mark_app(OVERLAY);
    flags::set_limits(h, (1.0, 1.0), (0.0, 0.0));
    motion::set_glass(h, r);
    flags::set_limits(h, size, size);
    flags::set_meta(h, |m| m.resize = Resize::None);
    flags::pin_on_top(h);
    flags::show_focused(&w);
    hotkeys::apply(app);
    send_overlay_state(app);
}

fn hud_size() -> (f64, f64) {
    (
        store::get().get_input_width() as f64,
        layout::HUD_INPUT_HEIGHT,
    )
}

pub fn exit_hud(app: &AppHandle) {
    let Some(h) = hwnd(app, OVERLAY) else {
        return;
    };
    let pending = {
        let st = shared(app);
        let mut s = st.lock();
        if s.mode != Mode::Hud {
            return;
        }
        s.hud_pending.take()
    };
    let pos = pending
        .unwrap_or_else(|| layout::pos_from_rect(screen::work_area(), motion::glass(h)).to_value());
    store::get().set_overlay_pos(&pos);
    apply_input_limits(app);
    collapse_panel(app, false);
    if let Some(s) = window(app, SETTINGS) {
        let _ = s.set_focus();
    }
}

fn settle_hud_drag(app: &AppHandle) {
    let Some(h) = hwnd(app, OVERLAY) else {
        return;
    };
    if shared(app).lock().mode != Mode::Hud {
        return;
    }
    let wa = screen::work_area();
    let rect = motion::glass(h);
    let pending = match layout::find_snap(wa, rect, hud_size()) {
        Some((pos, r)) => {
            mark_app(OVERLAY);
            motion::set_glass(h, r);
            emit(app, OVERLAY, "hud:snapped", json!({}));
            pos
        }
        None => layout::pos_from_rect(wa, rect),
    };
    shared(app).lock().hud_pending = Some(pending.to_value());
}

/// POS-18.
pub fn hud_nudge(app: &AppHandle, dx: f64, dy: f64) {
    let Some(h) = hwnd(app, OVERLAY) else {
        return;
    };
    if shared(app).lock().mode != Mode::Hud {
        return;
    }
    let r = layout::nudge(screen::work_area(), motion::glass(h), dx, dy);
    mark_app(OVERLAY);
    motion::set_glass(h, r);
    settle_hud_drag(app);
}

// ------------------------------------------------------ settings, history

fn lowered() -> &'static Mutex<HashSet<String>> {
    static L: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(HashSet::new()))
}

/// WIN-17: Settings and History step down from topmost until clicked, so a
/// link they open is not underneath them.
pub fn lower_for_browser(app: &AppHandle) {
    for label in [SETTINGS, HISTORY] {
        if let Some(w) = window(app, label) {
            let _ = w.set_always_on_top(false);
            lowered().lock().insert(label.to_string());
        }
    }
}

/// IPC-07: http(s) only.
pub fn open_url(app: &AppHandle, url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return false;
    }
    lower_for_browser(app);
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(url, None::<&str>).is_ok()
}

pub fn open_help(app: &AppHandle) -> Value {
    open_url(app, HELP_URL);
    json!({ "ok": true })
}

fn frame_rect(width: i64, height: i64) -> Rect {
    let wa = screen::work_area();
    let h = layout::clamp(height as f64, 360.0, wa.height - 40.0);
    let w = width as f64;
    Rect::new(
        wa.x + ((wa.width - w) / 2.0).round(),
        wa.y + ((wa.height - h) / 2.0).round(),
        w,
        h,
    )
}

fn frame_meta() -> Meta {
    Meta {
        margin: 0.0,
        resize: Resize::All,
        min: (380.0, 360.0),
        max: (900.0, 0.0),
    }
}

/// WIN-06: opens Settings, or focuses it and jumps to a section.
pub fn open_settings(app: &AppHandle, section: Option<&str>) {
    if let Some(w) = window(app, SETTINGS) {
        flags::show_focused(&w);
        if let Some(id) = section {
            emit(app, SETTINGS, "settings:section", json!({ "id": id }));
        }
        tour_notify(app, "settings-opened");
        return;
    }
    let (w, h) = store::get().get_settings_size();
    *pending_section().lock() = section.map(String::from);
    show_on_load().lock().insert(SETTINGS.into());
    let r = frame_rect(w, h);
    if let Err(e) = create_page(app, SETTINGS, "settings/settings.html", r, frame_meta()) {
        crate::crash::write("open-settings", &e.to_string());
    }
}

pub fn close_settings(app: &AppHandle) {
    if let Some(w) = window(app, SETTINGS) {
        let _ = w.close();
    }
}

/// WIN-07.
pub fn open_history(app: &AppHandle) {
    if let Some(w) = window(app, HISTORY) {
        flags::show_focused(&w);
        return;
    }
    let (w, h) = store::get().get_history_size();
    show_on_load().lock().insert(HISTORY.into());
    let r = frame_rect(w, h);
    if let Err(e) = create_page(app, HISTORY, "history/history.html", r, frame_meta()) {
        crate::crash::write("open-history", &e.to_string());
    }
}

pub fn close_history(app: &AppHandle) {
    if let Some(w) = window(app, HISTORY) {
        let _ = w.close();
    }
}

pub fn notify_history_changed(app: &AppHandle) {
    if window(app, HISTORY).is_some() {
        emit(app, HISTORY, "history:changed", json!({}));
    }
}

/// New chat from the tray or the bar: the next request opens a new window.
pub fn new_chat(app: &AppHandle) {
    if !shared(app).call(app, |h| h.abort_query.as_ref()) {
        shared(app).lock().busy = false;
    }
    outputs::detach_focus(app);
    if shared(app).lock().mode == Mode::Panel {
        if let Some(w) = window(app, OVERLAY) {
            let _ = w.set_focus();
        }
    }
    send_overlay_state(app);
}

// ------------------------------------------------------------- events

/// lib.rs routes every window event here (main thread: no blocking).
pub fn on_window_event(w: &tauri::Window, event: &tauri::WindowEvent) {
    use tauri::WindowEvent as E;
    let app = w.app_handle().clone();
    let label = w.label().to_string();
    match event {
        E::Focused(true) => {
            if lowered().lock().remove(&label) {
                let _ = w.set_always_on_top(true);
            }
            if outputs::is_output(&label) {
                outputs::touch(&app, &label);
            }
        }
        E::Resized(_) => on_resized(&app, &label),
        E::Moved(_) => on_moved(&app, &label),
        E::Destroyed => on_destroyed(&app, &label),
        _ => {}
    }
}

fn on_resized(app: &AppHandle, label: &str) {
    let a = app.clone();
    if label == OVERLAY {
        // POS-19: width only, 160 ms after the drag, unless the app moved it.
        debounce("overlay-resize", layout::DROP_SETTLE_MS, move || {
            if shared(&a).lock().mode != Mode::Panel || app_driven(&a, OVERLAY) {
                return;
            }
            if let Some(h) = hwnd(&a, OVERLAY) {
                store::get().set_input_width(motion::glass(h).width);
                apply_input_limits(&a);
                send_overlay_state(&a);
            }
        });
    } else if outputs::is_output(label) {
        let l = label.to_string();
        debounce(
            &format!("{label}-resize"),
            layout::DROP_SETTLE_MS,
            move || {
                outputs::on_user_resize(&a, &l);
            },
        );
    } else if label == SETTINGS {
        debounce("settings-resize", layout::DROP_SETTLE_MS, move || {
            if let Some(w) = window(&a, SETTINGS) {
                if let (Ok(s), Ok(f)) = (w.inner_size(), w.scale_factor()) {
                    let l = s.to_logical::<f64>(f);
                    store::get().set_settings_size(l.width, l.height);
                }
            }
        });
    } else if label == HISTORY {
        debounce("history-resize", 50, move || {
            if let Some(w) = window(&a, HISTORY) {
                if let (Ok(s), Ok(f)) = (w.inner_size(), w.scale_factor()) {
                    let l = s.to_logical::<f64>(f);
                    store::get().set_history_size(l.width, l.height);
                }
            }
        });
    }
}

fn on_moved(app: &AppHandle, label: &str) {
    let a = app.clone();
    if label == OVERLAY {
        if shared(app).lock().mode != Mode::Hud {
            return;
        }
        // POS-15: decide the snap once the drag has been still for 160 ms.
        debounce("hud-drag", layout::DROP_SETTLE_MS, move || {
            if !app_driven(&a, OVERLAY) {
                settle_hud_drag(&a);
            }
        });
    } else if outputs::is_output(label) {
        let l = label.to_string();
        debounce(
            &format!("{label}-move"),
            layout::DROP_SETTLE_MS,
            move || {
                outputs::on_user_move(&a, &l);
            },
        );
    }
}

fn on_destroyed(app: &AppHandle, label: &str) {
    flags::forget(label);
    lowered().lock().remove(label);
    if label == SETTINGS {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            hotkeys::set_capture_mode(&app, false);
            tour_notify(&app, "settings-closed");
        });
    } else if outputs::is_output(label) {
        outputs::on_destroyed(app, label);
    }
}
