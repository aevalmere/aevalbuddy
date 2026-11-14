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
mod rebuild;
pub mod screen;
pub mod stage;
pub mod text_scale;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

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
pub const HELP_URL: &str = "https://github.com/aevalmere/aevaleron/issues/new/choose";

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
/// mirrors it on the bus for the smoke run. An output window whose page has
/// not said hello yet gets it on hello instead (B-019).
pub fn emit(app: &AppHandle, label: &str, channel: &str, payload: Value) {
    if let Some(payload) = hold_if_unready(label, channel, payload) {
        send_now(app, label, channel, payload);
    }
}

/// Sends at once, even to a window whose events are held.
pub fn send_now(app: &AppHandle, label: &str, channel: &str, payload: Value) {
    let _ = app.emit_to(label, channel, payload.clone());
    let _ = shared(app).bus.send(BusEvent {
        channel: channel.to_string(),
        target: label.to_string(),
        payload,
    });
}

/// At most this many events wait for one page; the oldest go first.
const HELD_MAX: usize = 2000;

/// Per output label, the `(channel, payload)` events waiting for its page.
type Held = HashMap<String, Vec<(String, Value)>>;

/// Events for output windows whose page has not said hello (B-019). Tauri
/// drops an event nobody listens for yet, so they wait here, in order.
fn held() -> &'static Mutex<Held> {
    static H: OnceLock<Mutex<Held>> = OnceLock::new();
    H.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A new output window: its events wait for its hello.
pub fn hold_events(label: &str) {
    held().lock().insert(label.to_string(), Vec::new());
}

/// Keeps the event if `label` is held; else hands the payload back.
fn hold_if_unready(label: &str, channel: &str, payload: Value) -> Option<Value> {
    let mut h = held().lock();
    let Some(q) = h.get_mut(label) else {
        return Some(payload);
    };
    if q.len() >= HELD_MAX {
        q.remove(0);
    }
    q.push((channel.to_string(), payload));
    None
}

fn is_held(label: &str) -> bool {
    held().lock().contains_key(label)
}

/// What is held for `label` so far; it stays held.
pub fn take_held(label: &str) -> Option<Vec<(String, Value)>> {
    held().lock().get_mut(label).map(std::mem::take)
}

/// Sends what arrived since `take_held`, in order, then stops holding.
pub fn release_held(app: &AppHandle, label: &str) {
    loop {
        let batch = {
            let mut h = held().lock();
            match h.get_mut(label) {
                Some(q) if !q.is_empty() => std::mem::take(q),
                _ => {
                    h.remove(label);
                    return;
                }
            }
        };
        for (channel, payload) in batch {
            send_now(app, label, &channel, payload);
        }
    }
}

/// The window is gone: what it held goes with it.
pub fn drop_held(label: &str) {
    held().lock().remove(label);
}

pub fn window(app: &AppHandle, label: &str) -> Option<WebviewWindow> {
    app.get_webview_window(label)
}

fn hwnd(app: &AppHandle, label: &str) -> Option<isize> {
    flags::hwnd_for(label).or_else(|| window(app, label).and_then(|w| flags::hwnd_of(&w)))
}

fn debounce_gens() -> &'static Mutex<HashMap<String, u64>> {
    static GEN: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    GEN.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The next ticket for `key`; only the newest one runs.
fn debounce_ticket(key: &str) -> u64 {
    let mut g = debounce_gens().lock();
    let e = g.entry(key.to_string()).or_insert(0);
    *e += 1;
    *e
}

/// Debounce by key: only the newest call within `ms` runs.
fn debounce(key: &str, ms: u64, f: impl FnOnce() + Send + 'static) {
    let ticket = debounce_ticket(key);
    let key = key.to_string();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        if debounce_gens().lock().get(&key).copied() == Some(ticket) {
            f();
        }
    });
}

/// B-093: a closed window's debounce keys (`<label>-resize`, `-move`,
/// `-hello`) go with it; a pending call for it never runs.
fn forget_debounces(label: &str) {
    let prefix = format!("{label}-");
    let mut gens = debounce_gens().lock();
    gens.retain(|k, _| !k.starts_with(&prefix));
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
        .title("Aevaleron")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(true)
        // Settings, History and outputs maximize (B-024); the bar never.
        .maximizable(label != OVERLAY)
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
    // A dead renderer reloads; a dead browser process rebuilds the window.
    crate::crash::guard_webview(&w, rebuild::on_browser_exit);
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
            if (first || rebuild::overlay_loaded()) && button() {
                if let Some(h) = hwnd(&app, OVERLAY) {
                    flags::show_inactive(h);
                }
            }
            send_overlay_state(&app);
        } else if outputs::is_output(&label) {
            outputs::on_loaded(&app, &label);
            // A page that never says hello still gets what was held for it.
            let (a, l) = (app.clone(), label.clone());
            debounce(&format!("{label}-hello"), 3000, move || {
                if is_held(&l) {
                    outputs::on_hello(&a, &l);
                }
            });
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
        outputs::on_hello(app, label);
    } else if label == SETTINGS {
        send_section(app);
    }
    text_scale::send_to(app, label);
}

fn tour_notify(app: &AppHandle, event: &str) {
    shared(app).call_str(app, event, |h| h.tour_notify.as_ref());
}

// --------------------------------------------------------------- state

fn anchor() -> Anchor {
    Anchor::from_value(&store::get().get_overlay_pos())
}

/// Whether the round button floats on screen while the bar is closed. Off
/// by default: the closed bar is not on screen at all, and the shortcut or
/// the tray brings it.
fn button() -> bool {
    store::get().get_show_button()
}

/// Settings turned the button on or off: the closed bar shows or goes, at
/// the place that belongs to that way of opening it.
pub fn button_changed(app: &AppHandle) {
    let Some(w) = window(app, OVERLAY) else {
        return;
    };
    let (mode, hidden) = {
        let st = shared(app);
        let s = st.lock();
        (s.mode, s.hidden)
    };
    if mode == Mode::Hud {
        return;
    }
    if let Some(h) = flags::hwnd_of(&w) {
        motion::stop(h);
    }
    set_input_bounds(app, bar_home(app));
    if mode == Mode::Pill {
        if !button() {
            // The page takes the button away first; the window goes when
            // it says so (`bar:folded`), with nothing left on it.
            await_fold(app);
        } else if !hidden {
            if let Some(h) = flags::hwnd_of(&w) {
                flags::show_inactive(h);
            }
        }
    }
    send_overlay_state(app);
}

/// B-025: where the bar and new windows go: the work area of the display
/// the person was on when they last called the bar up.
pub fn bar_area() -> Rect {
    screen::work_area_of(screen::latched())
}

fn pill_rect() -> Rect {
    layout::pill_bounds(bar_area(), &anchor())
}

fn input_rect(height: f64) -> Rect {
    let w = store::get().get_input_width() as f64;
    layout::input_bounds(bar_area(), &anchor(), w, height, text_scale::scale())
}

/// Whether the bar's window (the open bar or its button) is on screen.
pub fn bar_on_screen(app: &AppHandle) -> bool {
    hwnd(app, OVERLAY).is_some_and(flags::is_visible)
}

/// The bar's glass rect as it is now, or where it would be.
pub fn bar_rect(app: &AppHandle) -> Rect {
    match hwnd(app, OVERLAY) {
        Some(h) if flags::is_visible(h) => motion::glass(h),
        _ => input_rect(shared(app).lock().input_height),
    }
}

/// The `overlay:state` payload (POS-13), plus the Tauri additions:
/// `glassIntensity`, `hasSentFirstRequest`, `keys`, `outputs`, `target`
/// (K-05: `{label, title, kind}` of the window the next question continues,
/// or null), `visibleOutputs` (K-05), the `overlay_extra` hook's fields
/// (K-07), and for an output window its own `label` and
/// `outputFixedHeight`.
///
/// Everything slow or locking (the hooks, the store, the area, the network
/// check) is read before the shared lock, which is held only to copy.
pub fn overlay_state(app: &AppHandle, for_label: Option<&str>) -> Value {
    crate::state::refresh(app);
    let extra = shared(app).call_value(app, |h| h.overlay_extra.as_ref());
    let store = store::get();
    let wa = bar_area();
    // main.js sent `activeRegion`: the area picked for the next question,
    // or the one the focused conversation is about.
    let picked = crate::region::active_region().map(|r| (r.width, r.height));
    let offline = !crate::updater::online();
    let reduced = motion::reduced();
    let main_chord = store.shortcut("captureQuery");
    let has_key = store.has_api_key();
    let input_anchor = store.get_overlay_pos();
    let output_pos = store.get_output_pos();
    let input_width = store.get_input_width();
    let output_width = store.get_output_width();
    let shortcuts = store.get_shortcuts();
    let glass = store.get_glass_intensity();
    let first_sent = store.get_has_sent_first_request();
    let keys = store.provider_keys_present();
    let button = store.get_show_button();
    let height = shared(app).lock().input_height;
    let pb = pill_rect();
    let ib = input_rect(height);
    let st = shared(app);
    let s = st.lock();
    let focused = s.focused_output.clone();
    let any_chat = s.outputs.values().any(|o| !o.chat.messages.is_empty());
    let (target, visible) = outputs::target_fields(&s);
    let region = picked
        .or_else(|| {
            focused
                .as_ref()
                .and_then(|l| s.outputs.get(l))
                .and_then(|o| o.chat.region)
                .map(|r| (r.width, r.height))
        })
        .map(|(w, h)| json!({ "width": w, "height": h }))
        .unwrap_or(Value::Null);
    let own = for_label.and_then(|l| s.outputs.get(l));
    let fixed = own.map(|o| o.user_height > 0.0).unwrap_or(false);
    // B-027: what fits in this window's glass under the title bar.
    let room = layout::output_max_height(wa);
    let chat_max = own.and_then(|o| o.rect).map_or(room, |r| r.height) - 56.0;
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
        "reducedMotion": reduced || s.reduced_motion,
        "hasKey": has_key,
        "captureOk": s.capture_ok,
        "inputAnchor": input_anchor,
        "outputPos": output_pos,
        "pillState": pill_state,
        "inputWidth": input_width,
        "outputWidth": output_width,
        "outputFixedHeight": fixed,
        "chatMax": chat_max,
        "region": region,
        "busy": s.busy,
        "barMode": s.bar_mode,
        "agent": agent,
        "pillError": s.pill_error,
        "offline": offline,
        "findOpen": s.find_open,
        "shortcuts": shortcuts,
        "hotkeyFailed": if hotkey_failed { main_chord } else { String::new() },
        "glassIntensity": glass,
        "hasSentFirstRequest": first_sent,
        "keys": keys,
        "outputs": s.outputs.values().filter(|o| o.active).count(),
        "button": button,
    });
    drop(s);
    v["target"] = target;
    v["visibleOutputs"] = json!(visible);
    if let (Some(Value::Object(extra)), Value::Object(base)) = (extra, &mut v) {
        for (k, val) in extra {
            base.insert(k, val);
        }
    }
    if let Some(l) = for_label {
        v["label"] = json!(l);
        v["focused"] = json!(focused.as_deref() == Some(l));
    }
    v
}

/// Sends `overlay:state` to the overlay and every output window, and
/// `output:meta` to the outputs whose meta changed.
pub fn send_overlay_state(app: &AppHandle) {
    emit(app, OVERLAY, "overlay:state", overlay_state(app, None));
    let labels: Vec<String> = shared(app).lock().outputs.keys().cloned().collect();
    for l in labels {
        emit(app, &l, "overlay:state", overlay_state(app, Some(&l)));
    }
    outputs::send_meta(app);
}

// --------------------------------------------------------------- boot

/// Creates the overlay and the warm output window (LIFE-06).
pub fn boot(app: &AppHandle) -> tauri::Result<()> {
    screen::latch_active();
    create_overlay(app)?;
    outputs::create_spare(app)?;
    flags::start_poll(app);
    text_scale::start_watch(app);
    Ok(())
}

/// The overlay window as the orb, hidden until its page loads.
fn create_overlay(app: &AppHandle) -> tauri::Result<()> {
    let glass = pill_rect();
    let outer = stage::inflate(glass, stage::MARGIN);
    let meta = Meta {
        margin: stage::MARGIN,
        resize: Resize::None,
        min: layout::PILL,
        max: layout::PILL,
        sheet: None,
    };
    let w = create_page(app, OVERLAY, SCAFFOLD_WINDOWS[0].1, outer, meta)?;
    if let Some(h) = flags::hwnd_of(&w) {
        flags::set_focusable(h, false);
        motion::set_glass(h, glass);
    }
    flags::set_click_through(&w, true);
    Ok(())
}

/// B-090: the bar's page went with the browser process; it comes back as
/// the orb.
fn rebuild_overlay(app: &AppHandle) -> tauri::Result<()> {
    {
        let st = shared(app);
        let mut s = st.lock();
        s.mode = Mode::Pill;
        s.input_height = layout::DEFAULT_INPUT_HEIGHT;
    }
    create_overlay(app)?;
    hotkeys::apply(app);
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
    motion::set_glass_on(h, r, screen::latched());
    apply_input_limits(app);
}

/// The rect the bar's glass belongs on in its current mode.
fn bar_home(app: &AppHandle) -> Rect {
    let (mode, height) = {
        let st = shared(app);
        let s = st.lock();
        (s.mode, s.input_height)
    };
    if mode == Mode::Panel {
        input_rect(height)
    } else {
        pill_rect()
    }
}

/// B-025: the person called the bar up here, so it and new windows belong
/// on this display. The bar jumps over rather than gliding across screens.
fn latch_display(app: &AppHandle) {
    let d = screen::latch_active();
    let Some(h) = hwnd(app, OVERLAY) else {
        return;
    };
    if screen::matching_px(flags::rect_px(h)).id != d.id {
        motion::stop(h);
        set_input_bounds(app, bar_home(app));
    }
}

/// B-025: a display came, went or changed scale. The bar goes back to its
/// place and every output stays inside its own display.
pub fn on_display_change(app: &AppHandle) {
    let a = app.clone();
    debounce("display-change", layout::DROP_SETTLE_MS, move || {
        replace_windows(&a);
    });
}

fn replace_windows(app: &AppHandle) {
    if shared(app).lock().mode != Mode::Hud {
        set_input_bounds(app, bar_home(app));
    }
    outputs::clamp_all(app);
    send_overlay_state(app);
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

/// How long the window waits for the page to say `bar:folded` before it
/// goes anyway. The page's fold takes `T_COLLAPSE` and two frames; this
/// only runs out when the page cannot answer.
const FOLD_WAIT: u64 = layout::T_COLLAPSE + 280;

/// The fold the page is playing now, by its collapse generation. Set by
/// `collapse_panel`; taken by the page's `bar:folded`, by the wait running
/// out, or by the bar opening again.
fn fold_pending() -> &'static Mutex<Option<u64>> {
    static P: OnceLock<Mutex<Option<u64>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(None))
}

/// Takes the pending fold: any one for the page (`gen` None), only its own
/// for a timer. False when there is nothing to finish, so a fold is
/// finished once however many ask.
fn claim_fold(pending: &mut Option<u64>, gen: Option<u64>) -> bool {
    let hit = match (*pending, gen) {
        (Some(p), Some(g)) => p == g,
        (Some(_), None) => true,
        (None, _) => false,
    };
    if hit {
        *pending = None;
    }
    hit
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
    *fold_pending().lock() = None;
    flags::set_click_through(&w, false);
    flags::set_focusable(h, true);
    flags::pin_on_top(h);
    bar_mark("top");
    send_overlay_state(app);
    bar_mark("state");
    motion::stop(h);
    set_input_bounds(app, input_rect(height));
    if was_pill {
        let app2 = app.clone();
        debounce("sync-input", layout::T_EXPAND, move || {
            sync_input_bounds(&app2)
        });
    }
    bar_mark("placed");
    flags::show_focused(&w);
    bar_mark("focus");
    hotkeys::apply(app);
}

/// POS-09 with D-01: the bar folds into the orb and the foreground goes
/// back to the app that had it. Answers keep streaming and output windows
/// stay where they are (they float on their own since REQUESTS #11).
pub fn collapse_panel(app: &AppHandle) {
    crate::state::refresh(app);
    let Some(w) = window(app, OVERLAY) else {
        return;
    };
    let Some(h) = flags::hwnd_of(&w) else {
        return;
    };
    let find_open = shared(app).lock().find_open;
    if find_open {
        shared(app).call(app, |k| k.close_find.as_ref());
    }
    // From the open bar or the layout editor: either leaves something on
    // the window that the page has to take away first.
    let was_open = {
        let st = shared(app);
        let mut s = st.lock();
        let was = s.mode != Mode::Pill;
        s.mode = Mode::Pill;
        s.input_height = layout::DEFAULT_INPUT_HEIGHT;
        was
    };
    drop_bar_request();
    flags::blur(h);
    flags::set_focusable(h, false);
    flags::pin_on_top(h);
    flags::set_click_through(&w, true);
    let button = button();
    if button {
        flags::show_inactive(h);
    }
    motion::stop(h);
    if was_open {
        // The bar folds into the orb inside the page first. The window
        // shrinks or goes only when the page says it has painted the
        // folded bar away (`bar:folded`), so the last frame the window
        // shows, and the first it shows next time, is an empty one.
        await_fold(app);
        send_overlay_state(app);
    } else if fold_pending().lock().is_none() {
        // A second collapse while the page still folds (Esc twice) leaves
        // that fold to finish instead of cutting the window off under it.
        fold_done(app, button);
    }
    hotkeys::apply(app);
}

/// Starts the wait for the page's `bar:folded`. The caller sends the state
/// that makes the page fold.
fn await_fold(app: &AppHandle) {
    let gen = {
        let mut g = collapse_gen().lock();
        *g += 1;
        *g
    };
    *fold_pending().lock() = Some(gen);
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(FOLD_WAIT)).await;
        finish_fold(&app2, Some(gen));
    });
}

/// `bar:folded`: the page has folded the bar and painted the frame without
/// it.
pub fn bar_folded(app: &AppHandle) {
    finish_fold(app, None);
}

fn finish_fold(app: &AppHandle, gen: Option<u64>) {
    let mine = claim_fold(&mut fold_pending().lock(), gen);
    if !mine || shared(app).lock().mode != Mode::Pill {
        return;
    }
    fold_done(app, button());
}

/// The bar has folded: the window shrinks to the button, or with no button
/// leaves the screen.
fn fold_done(app: &AppHandle, button: bool) {
    if button {
        set_input_bounds(app, pill_rect());
    } else if let Some(w) = window(app, OVERLAY) {
        flags::hide(&w);
    }
    send_overlay_state(app);
}

/// The way back to the pill: the bar folds and every answer window goes
/// with it. Nothing is closed or stopped; the conversations stay, and the
/// next open (the pill, the hotkey, the tray) brings back each one with
/// content (`trigger_capture`'s `show_with_chat`). Collapse alone (Esc,
/// D-01) still leaves the answers floating.
pub fn minimize(app: &AppHandle) {
    crate::state::refresh(app);
    if shared(app).lock().mode == Mode::Hud {
        exit_hud(app);
        return;
    }
    outputs::hide_windows(app);
    collapse_panel(app);
}

/// main.js triggerCapture: the bar opens, and conversations come back.
pub fn trigger_capture(app: &AppHandle) {
    flags::remember_foreground();
    bar_mark("fg");
    {
        let st = shared(app);
        let mut s = st.lock();
        if s.mode == Mode::Hud {
            return;
        }
        s.hidden = false;
    }
    latch_display(app);
    bar_mark("display");
    expand_panel(app);
    bar_mark("shown");
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
    bar_mark("mode");
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
    crate::state::refresh(app);
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
    crate::state::refresh(app);
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
        latch_display(app);
        if mode == Mode::Panel {
            flags::show_focused(&w);
        } else if button() {
            if let Some(h) = flags::hwnd_of(&w) {
                flags::show_inactive(h);
            }
        }
        outputs::show_active_windows(app);
    }
    crate::tray::refresh(app);
    hotkeys::apply(app);
}

const ESC_CLEARED: &str = "Cleared. Press Esc again to stop the task.";

/// B-102: while a task waits on the person, Esc in the bar first clears a
/// half-typed answer; the next Esc stops the task.
pub fn escape_clears_bar(agent_state: &str, dirty: bool, bar_focused: bool) -> bool {
    dirty && bar_focused && matches!(agent_state, "asking" | "plan" | "paused")
}

/// Global Escape, in main.js's priority order (KEY-08). It never stops an
/// answer or hides an output window (D-01).
pub fn on_escape(app: &AppHandle) {
    crate::state::refresh(app);
    let (agent, agent_state, dirty, find, region, buddy, mode) = {
        let st = shared(app);
        let s = st.lock();
        (
            s.agent.running,
            s.agent.state.clone(),
            s.bar_dirty,
            s.find_open,
            s.region_active,
            s.buddy_active,
            s.mode,
        )
    };
    let st = shared(app);
    let bar_focused = flags::foreground_label().as_deref() == Some(OVERLAY);
    if agent && escape_clears_bar(&agent_state, dirty, bar_focused) {
        st.lock().bar_dirty = false;
        emit(app, OVERLAY, "bar:clear", json!({}));
        announce(app, ESC_CLEARED, "assertive");
        return;
    }
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
        collapse_panel(app);
    }
}

/// A global shortcut fired (hotkeys.rs).
pub fn on_hotkey(app: &AppHandle, action: &str) {
    crate::state::refresh(app);
    match action {
        "captureQuery" => {
            mark_bar_requested();
            open_bar(app, None, None);
        }
        "findOnScreen" => {
            mark_bar_requested();
            open_bar(app, Some("find"), None);
        }
        "agentTask" => {
            if shared(app).lock().agent.running {
                if !shared(app).call(app, |h| h.show_trail.as_ref()) {
                    outputs::show_trail(app, false);
                }
            } else {
                mark_bar_requested();
                open_bar(app, Some("agent"), None);
            }
        }
        "captureRegion" => {
            latch_display(app);
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
        // A menu opened while the bar glides in: the height is kept above
        // and applied once the glide ends, so the menu is never left cut
        // off by a window still at its old size.
        let app2 = app.clone();
        debounce("sync-input", layout::T_EXPAND, move || {
            sync_input_bounds(&app2)
        });
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

// ------------------------------------------------- keyboard and a11y

/// Where an announcement goes: the Aevaleron window with focus, else the
/// open bar, else nowhere.
pub fn announce_target(focused: Option<&str>, bar_open: bool) -> Option<String> {
    match focused {
        Some(l) => Some(l.to_string()),
        None if bar_open => Some(OVERLAY.to_string()),
        None => None,
    }
}

/// `a11y:announce {text, priority}` (design-ux U7): screen readers speak
/// live regions of the focused window only, so a page without focus hands
/// its announcement here and it goes out as `a11y:say {text, priority}`
/// (`polite` or `assertive`, at most 300 characters). Returns the label it
/// went to.
pub fn announce(app: &AppHandle, text: &str, priority: &str) -> Option<String> {
    let text: String = text.trim().chars().take(300).collect();
    if text.is_empty() {
        return None;
    }
    let priority = if priority == "assertive" {
        "assertive"
    } else {
        "polite"
    };
    let bar_open = {
        let st = shared(app);
        let s = st.lock();
        s.mode == Mode::Panel && !s.hidden
    };
    let to = announce_target(flags::foreground_label().as_deref(), bar_open)?;
    emit(
        app,
        &to,
        "a11y:say",
        json!({ "text": text, "priority": priority }),
    );
    Some(to)
}

fn bar_requested() -> &'static Mutex<Option<Instant>> {
    static T: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(None))
}

fn bar_open_ms() -> &'static Mutex<VecDeque<u64>> {
    static T: OnceLock<Mutex<VecDeque<u64>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// The bar's hotkey fired: the clock for `barOpenMs` starts (B-104).
pub fn mark_bar_requested() {
    bar_marks().lock().clear();
    *bar_requested().lock() = Some(Instant::now());
}

/// The bar folded before its field had the focus: that open is not one to
/// time. Without this the clock ran on to the next open, however it was
/// made, and a press on a bar that was already open read as a slow one.
fn drop_bar_request() {
    bar_requested().lock().take();
    bar_marks().lock().clear();
}

fn bar_marks() -> &'static Mutex<Vec<(&'static str, u64)>> {
    static T: OnceLock<Mutex<Vec<(&'static str, u64)>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(Vec::new()))
}

fn bar_open_notes() -> &'static Mutex<VecDeque<String>> {
    static T: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// One step of a hotkey's bar open is done, so many milliseconds after the
/// hotkey (`barOpenStages`). Outside a hotkey's open there is no clock and
/// nothing is kept.
fn bar_mark(step: &'static str) {
    let requested = *bar_requested().lock();
    if let Some(t0) = requested {
        bar_marks()
            .lock()
            .push((step, t0.elapsed().as_millis() as u64));
    }
}

/// `bar:ready {pageMs, via}`: the bar's field has focus. Keeps the last 20
/// latencies, and for each where the time went: when each of Rust's steps
/// was done ([`bar_mark`]), how long the page then took from its new state
/// to the focused field (`page`), and whether the window already had the
/// focus when the state came (`via=state`) or the page had to wait for it
/// (`via=focus`).
pub fn bar_ready(p: &Value) {
    let Some(t0) = bar_requested().lock().take() else {
        return;
    };
    let ms = t0.elapsed().as_millis() as u64;
    let steps: Vec<String> = bar_marks()
        .lock()
        .drain(..)
        .map(|(step, at)| format!("{step}={at}"))
        .collect();
    let steps = steps.join(",");
    let page = p.get("pageMs").and_then(Value::as_f64).unwrap_or(-1.0);
    let via = p.get("via").and_then(Value::as_str).unwrap_or("?");
    {
        let mut notes = bar_open_notes().lock();
        notes.push_back(format!("{ms}({steps},page={page:.0},via={via})"));
        while notes.len() > 20 {
            notes.pop_front();
        }
    }
    let mut list = bar_open_ms().lock();
    list.push_back(ms);
    while list.len() > 20 {
        list.pop_front();
    }
}

/// [`bar_open_samples`] with where each open's time went, oldest first.
pub fn bar_open_stages() -> Vec<String> {
    bar_open_notes().lock().iter().cloned().collect()
}

/// Hotkey-to-field latencies, oldest first (the windows probe reads them).
pub fn bar_open_samples() -> Vec<u64> {
    bar_open_ms().lock().iter().copied().collect()
}

/// `window:nudge {dx, dy, dw, dh}` (B-101): moves or resizes the calling
/// window by DIPs, inside its display's work area and its size limits. An
/// output keeps the result as if it had been dragged. The bar moves with
/// the layout editor instead (`hud:nudge`).
pub fn nudge_window(app: &AppHandle, label: &str, d: (f64, f64, f64, f64)) -> bool {
    if label == OVERLAY {
        return false;
    }
    let Some(h) = hwnd(app, label) else {
        return false;
    };
    let m = flags::meta(h);
    let from = motion::glass(h);
    let wa = screen::matching_px(flags::rect_px(h)).work_area;
    let to = layout::nudge_rect(wa, from, d, m.min, m.max);
    mark_app(label);
    motion::stop(h);
    motion::set_glass(h, to);
    if outputs::is_output(label) {
        let resized = d.2.abs() > 0.0 || d.3.abs() > 0.0;
        outputs::keep_user_rect(app, label, to, resized);
    }
    true
}

// ---------------------------------------------------------------- HUD

/// POS-14 with NEW-04: the layout editor places the bar only.
pub fn enter_hud(app: &AppHandle) {
    crate::state::refresh(app);
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
    let r = layout::bounds_for_size(bar_area(), size, &Anchor::from_value(&pending));
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
    let pos =
        pending.unwrap_or_else(|| layout::pos_from_rect(bar_area(), motion::glass(h)).to_value());
    store::get().set_overlay_pos(&pos);
    apply_input_limits(app);
    collapse_panel(app);
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
    let wa = bar_area();
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
    let r = layout::nudge(bar_area(), motion::glass(h), dx, dy);
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
    let wa = bar_area();
    let h = layout::clamp(height as f64, 360.0, wa.height - 40.0);
    let w = width as f64;
    Rect::new(
        wa.x + ((wa.width - w) / 2.0).round(),
        wa.y + ((wa.height - h) / 2.0).round(),
        w,
        h,
    )
}

/// Settings and History: no maximum, so a drag or a maximize may take the
/// whole work area (B-024).
fn frame_meta() -> Meta {
    Meta {
        margin: 0.0,
        resize: Resize::All,
        min: (380.0, 360.0),
        max: (0.0, 0.0),
        sheet: None,
    }
}

/// Whether `label` is maximized now.
fn maximized(app: &AppHandle, label: &str) -> bool {
    hwnd(app, label).is_some_and(flags::is_maximized)
}

/// B-024: a maximized output has no stage margin (its page drops its own
/// under html[data-maximized]), so the hit-test and click-through meet the
/// screen's edge; restored, the margin comes back.
fn sync_output_margin(app: &AppHandle, label: &str) {
    if let Some(h) = hwnd(app, label) {
        let margin = if flags::is_maximized(h) {
            0.0
        } else {
            stage::MARGIN
        };
        flags::set_meta(h, |m| m.margin = margin);
    }
}

/// Per label, a Settings or History window on its way out, and
/// `Some(section)` when it was asked to open again meanwhile.
type Closing = HashMap<String, Option<Option<String>>>;

fn closing() -> &'static Mutex<Closing> {
    static C: OnceLock<Mutex<Closing>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Opens `label` again once Tauri has let go of the old window.
async fn reopen(app: &AppHandle, label: &str, section: Option<String>) {
    for _ in 0..40 {
        if window(app, label).is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    if label == SETTINGS {
        open_settings(app, section.as_deref());
    } else if label == HISTORY {
        open_history(app);
    }
}

/// B-092: asked to open while the old window is closing, it opens again
/// once that one is gone instead of focusing a window that never shows.
fn reopen_after_close(label: &str, section: Option<&str>) -> bool {
    match closing().lock().get_mut(label) {
        Some(again) => {
            *again = Some(section.map(String::from));
            true
        }
        None => false,
    }
}

fn close_frame(app: &AppHandle, label: &str) {
    if let Some(w) = window(app, label) {
        closing().lock().entry(label.to_string()).or_insert(None);
        let _ = w.close();
    }
}

/// WIN-06: opens Settings, or focuses it and jumps to a section.
pub fn open_settings(app: &AppHandle, section: Option<&str>) {
    if reopen_after_close(SETTINGS, section) {
        return;
    }
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
    open_frame(app, SETTINGS, "settings/settings.html", r);
}

/// Settings or History at `r`, written through the latched display: the
/// builder's position cannot say which display's DIPs it means (B-025).
fn open_frame(app: &AppHandle, label: &str, page: &str, r: Rect) {
    match create_page(app, label, page, r, frame_meta()) {
        Ok(w) => {
            if let Some(h) = flags::hwnd_of(&w) {
                motion::set_glass_on(h, r, screen::latched());
            }
        }
        Err(e) => crate::crash::write(&format!("open-{label}"), &e.to_string()),
    }
}

pub fn close_settings(app: &AppHandle) {
    close_frame(app, SETTINGS);
}

/// WIN-07.
pub fn open_history(app: &AppHandle) {
    if reopen_after_close(HISTORY, None) {
        return;
    }
    if let Some(w) = window(app, HISTORY) {
        flags::show_focused(&w);
        return;
    }
    let (w, h) = store::get().get_history_size();
    show_on_load().lock().insert(HISTORY.into());
    let r = frame_rect(w, h);
    open_frame(app, HISTORY, "history/history.html", r);
}

pub fn close_history(app: &AppHandle) {
    close_frame(app, HISTORY);
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
        E::ScaleFactorChanged { .. } => on_display_change(&app),
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
        sync_output_margin(app, label);
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
            if let Some((w, h)) = restored_size(&a, SETTINGS) {
                store::get().set_settings_size(w, h);
            }
        });
    } else if label == HISTORY {
        debounce("history-resize", 50, move || {
            if let Some((w, h)) = restored_size(&a, HISTORY) {
                store::get().set_history_size(w, h);
            }
        });
    }
}

/// The window's size in DIPs, unless it is maximized: the size to open at
/// next is the one the person chose (B-024).
fn restored_size(app: &AppHandle, label: &str) -> Option<(f64, f64)> {
    if maximized(app, label) {
        return None;
    }
    let w = window(app, label)?;
    let scale = w.scale_factor().ok()?;
    let l = w.inner_size().ok()?.to_logical::<f64>(scale);
    Some((l.width, l.height))
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
    // The window's flags, frame and animation entries went with its handle
    // (flags::forget_hwnd on WM_NCDESTROY), so a window reopened under the
    // same label at once keeps its own (B-092).
    let rebuilt = rebuild::take(label);
    crate::backdrop::forget(label);
    lowered().lock().remove(label);
    forget_debounces(label);
    app_marks().lock().remove(label);
    if rebuilt && outputs::is_output(label) {
        // What is sent while the window is built again waits for its page.
        held().lock().entry(label.to_string()).or_default();
    } else {
        drop_held(label);
    }
    let again = closing().lock().remove(label).flatten();
    if let Some(section) = again {
        let (a, l) = (app.clone(), label.to_string());
        tauri::async_runtime::spawn(async move {
            reopen(&a, &l, section).await;
        });
    }
    if rebuilt {
        let (a, l) = (app.clone(), label.to_string());
        tauri::async_runtime::spawn(rebuild::rebuild(a, l));
    }
    if label == SETTINGS {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            hotkeys::set_capture_mode(&app, false);
            tour_notify(&app, "settings-closed");
        });
    } else if outputs::is_output(label) && !rebuilt {
        outputs::on_destroyed(app, label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, Output};

    #[test]
    fn text_scale_clamps_and_multiplies() {
        use super::text_scale::{clamp_user, effective};
        assert_eq!(clamp_user(1.5), 1.5);
        assert_eq!(clamp_user(0.5), 1.0);
        assert_eq!(clamp_user(3.0), 2.0);
        assert_eq!(clamp_user(1.26), 1.3);
        assert_eq!(clamp_user(f64::NAN), 1.0);
        assert_eq!(effective(1.0, 1.5), 1.5);
        assert_eq!(effective(1.25, 1.5), 1.875);
        assert_eq!(effective(1.5, 2.0), 2.5);
        assert_eq!(effective(2.25, 2.0), 2.5);
        assert_eq!(effective(0.0, 1.2), 1.2);
    }

    #[test]
    fn a_fold_is_finished_once() {
        // The page answers first: the timer for the same fold finds nothing.
        let mut p = Some(3);
        assert!(claim_fold(&mut p, None));
        assert!(!claim_fold(&mut p, Some(3)));
        // A timer left over from an earlier fold never ends a later one.
        let mut p = Some(4);
        assert!(!claim_fold(&mut p, Some(3)));
        assert_eq!(p, Some(4));
        assert!(claim_fold(&mut p, Some(4)));
        // The bar opened again: neither the page nor the timer hides it.
        let mut p = None;
        assert!(!claim_fold(&mut p, None));
        assert!(!claim_fold(&mut p, Some(4)));
    }

    #[test]
    fn announce_goes_to_focused_window() {
        assert_eq!(
            announce_target(Some("output-2"), true),
            Some("output-2".into())
        );
        assert_eq!(announce_target(None, true), Some(OVERLAY.into()));
        assert_eq!(announce_target(None, false), None);
    }

    #[test]
    fn escape_clears_dirty_bar_while_asking() {
        for state in ["asking", "plan", "paused"] {
            assert!(escape_clears_bar(state, true, true));
            assert!(!escape_clears_bar(state, false, true));
            assert!(!escape_clears_bar(state, true, false));
        }
        assert!(!escape_clears_bar("running", true, true));
    }

    #[test]
    fn overlay_state_has_target_and_count() {
        let mut s = AppState::default();
        for (label, active) in [("output-1", true), ("output-2", true), ("output-3", false)] {
            let mut o = Output {
                label: label.into(),
                active,
                view: "chat".into(),
                ..Output::default()
            };
            o.chat
                .messages
                .push(json!({ "role": "user", "text": "What is   on\nmy screen?" }));
            s.outputs.insert(label.into(), o);
        }
        s.focused_output = Some("output-1".into());
        let (target, visible) = outputs::target_fields(&s);
        assert_eq!(target["label"], "output-1");
        assert_eq!(target["title"], "What is on my screen?");
        assert_eq!(target["kind"], "chat");
        assert_eq!(visible, 2);
        // A hidden window is no target.
        s.focused_output = Some("output-3".into());
        assert_eq!(outputs::target_fields(&s).0, Value::Null);
        s.hidden = true;
        assert_eq!(outputs::target_fields(&s).1, 0);
    }

    #[test]
    fn settings_history_have_no_max() {
        assert_eq!(frame_meta().max, (0.0, 0.0));
        assert_eq!(frame_meta().min, (380.0, 360.0));
        assert_eq!(outputs::meta().max, (0.0, 0.0));
        assert!(outputs::meta().min.0 > 0.0);
    }

    fn channels(events: &[(String, Value)]) -> Vec<&str> {
        events.iter().map(|(c, _)| c.as_str()).collect()
    }

    fn ev(channel: &str, gen: u64) -> (String, Value) {
        (channel.to_string(), json!({ "gen": gen }))
    }

    #[test]
    fn events_to_unready_output_are_replayed_in_order() {
        let label = "output-9001";
        hold_events(label);
        let sent = [
            ("chat:user", json!({ "gen": 1, "text": "q" })),
            ("query:start", json!({ "gen": 1 })),
            ("overlay:state", json!({})),
            ("query:delta", json!({ "gen": 1, "text": "a" })),
            ("lens:draw", json!({})),
        ];
        for (c, p) in sent {
            assert!(hold_if_unready(label, c, p).is_none());
        }
        // Another window's events go straight out.
        assert!(hold_if_unready("output-9002", "query:start", json!({})).is_some());
        let first = take_held(label).unwrap();
        let replay = outputs::replayable(first);
        assert_eq!(
            channels(&replay),
            ["query:start", "query:delta", "lens:draw"]
        );
        assert_eq!(replay[1].1["text"], "a");
        // Until it is released, a late event still waits its turn.
        assert!(hold_if_unready(label, "query:done", json!({ "gen": 1 })).is_none());
        assert_eq!(channels(&take_held(label).unwrap()), ["query:done"]);
        drop_held(label);
        assert!(hold_if_unready(label, "query:done", json!({})).is_some());
        // An Ask that finished before the page loaded is all in chat:sync;
        // one that failed keeps its start and its error.
        let ended = vec![
            ev("query:start", 2),
            ev("query:delta", 2),
            ev("query:done", 2),
            ev("query:start", 3),
            ev("query:stage", 3),
            ev("query:error", 3),
            ev("agent:step", 3),
        ];
        assert_eq!(
            channels(&outputs::replayable(ended)),
            ["query:start", "query:error", "agent:step"]
        );
    }

    #[test]
    fn closed_output_leaves_no_gen_entries() {
        debounce_ticket("output-9003-resize");
        debounce_ticket("output-9003-move");
        debounce_ticket("output-90031-move");
        forget_debounces("output-9003");
        {
            let g = debounce_gens().lock();
            assert!(!g.contains_key("output-9003-resize"));
            assert!(!g.contains_key("output-9003-move"));
            assert!(g.contains_key("output-90031-move"));
        }
        let h = 0x7_9003;
        motion::stop(h);
        assert!(motion::tracks(h));
        flags::forget_hwnd(h);
        assert!(!motion::tracks(h));
    }

    /// B-094: nothing slow or locking runs while `overlay_state` holds the
    /// shared lock: no store read, hook, area or network check between
    /// taking the lock and dropping it.
    #[test]
    fn overlay_state_lock_scope() {
        let src = include_str!("mod.rs");
        let start = src.find("pub fn overlay_state(").unwrap();
        let body = &src[start..];
        // The function ends at the first brace in column 0 (LF or CRLF).
        let body = &body[..body.find("\n}").unwrap()];
        let held = &body[body.find("st.lock()").unwrap()..body.find("drop(s);").unwrap()];
        for slow in ["store", "region::", "updater::", ".call", "motion::"] {
            assert!(!held.contains(slow), "{slow} under the lock");
        }
    }

    /// B-096: the `overlay:state` field set: main.js's (POS-13), then the
    /// Tauri additions, then the ones an output window alone gets.
    #[test]
    fn overlay_state_field_set() {
        const ELECTRON: [&str; 21] = [
            "mode",
            "orbAt",
            "reducedMotion",
            "hasKey",
            "captureOk",
            "inputAnchor",
            "outputPos",
            "pillState",
            "inputWidth",
            "outputWidth",
            "outputFixedHeight",
            "chatMax",
            "region",
            "busy",
            "barMode",
            "agent",
            "pillError",
            "offline",
            "findOpen",
            "shortcuts",
            "hotkeyFailed",
        ];
        const TAURI: [&str; 7] = [
            "glassIntensity",
            "hasSentFirstRequest",
            "keys",
            "outputs",
            "target",
            "visibleOutputs",
            "button",
        ];
        const OUTPUT_ONLY: [&str; 2] = ["label", "focused"];
        let src = include_str!("mod.rs");
        let start = src.find("pub fn overlay_state(").unwrap();
        let body = &src[start..];
        let body = &body[..body.find("\n}").unwrap()];
        let mut got = top_level_keys(body);
        let mut want: Vec<&str> = ELECTRON
            .iter()
            .chain(TAURI.iter())
            .chain(OUTPUT_ONLY.iter())
            .copied()
            .collect();
        got.sort_unstable();
        want.sort_unstable();
        assert_eq!(got, want);
    }

    /// The keys a function's `json!({ .. })` literal and `v["key"] = ..`
    /// lines set, one per line, as this crate's code lays them out.
    fn top_level_keys(body: &str) -> Vec<&str> {
        let mut keys = Vec::new();
        for line in body.lines().map(str::trim) {
            if let Some(rest) = line.strip_prefix('"') {
                if let Some(end) = rest.find("\":") {
                    keys.push(&rest[..end]);
                }
            } else if let Some(rest) = line.strip_prefix("v[\"") {
                if let Some(end) = rest.find('"') {
                    keys.push(&rest[..end]);
                }
            }
        }
        keys
    }
}
