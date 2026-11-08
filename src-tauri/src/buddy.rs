//! The tour and pointer (lib/cursor-buddy.js, PARITY TOUR-01..05).
//!
//! Draws an animated pointer in its own transparent, click-through window
//! above the app windows and walks a list of steps; or shows up to three
//! model-directed points; or glides the agent's cursor. It never moves the
//! real OS cursor. Steps advance on named app events (`notify("key-saved")`)
//! and the whole thing dismisses with Escape, the close button, or a click
//! away from the target.
//!
//! Click-through with forwarding: Electron's `setIgnoreMouseEvents(true,
//! {forward: true})` kept mousemove flowing to the page while clicks passed
//! through, which is how the callout's buttons turn hit-testing back on.
//! Tauri has no forward flag, so while the window ignores the mouse a 60 Hz
//! cursor poll replays mousemove into the page (ARCHITECTURE D10).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::capture::{self, screen, Rect};
use crate::desktop::win;
use crate::windows::flags;

pub const LABEL: &str = "buddy";
pub const POLL_MS: u64 = 150;
/// The tour's card that asks for a key.
pub const KEY_STEP: i64 = 1;
/// How long `agent_point` gives the cursor to arrive.
pub const GLIDE_WAIT_MS: u64 = 260;
const FORWARD_MS: u64 = 16;
/// The forwarding thread's pace between sessions.
const FORWARD_IDLE_MS: u64 = 250;
/// How long the window may take the mouse with no word from the page.
const WATCHDOG: Duration = Duration::from_secs(3);

const NEEDS_NO_KEY: &str = "no-key";
const NEEDS_KEY: &str = "key";
const NEEDS_BAR: &str = "bar";

const BAR_TARGET: &str = "#pill, #query-input";
const BAR_SCOPE: &str = "#pill, #panel";
const TOKEN_MESSAGE: &str = "This shows what Enter will do. AevalBuddy guesses from what you \
                             type, and a task always asks before anything risky; Esc stops it.";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Button {
    pub id: String,
    pub label: String,
    pub primary: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Step {
    /// `overlay` or `settings`; empty for a card.
    pub win: String,
    pub needs: String,
    pub target_selector: String,
    pub click_scope: String,
    pub message: String,
    pub advance_on: String,
    pub card: bool,
    pub buttons: Vec<Button>,
}

/// The live chords the tour quotes.
pub struct Chords<'a> {
    pub capture_query: &'a str,
    // The tour quotes only the main chord now (B-033); host/mod.rs still
    // fills these in.
    #[allow(dead_code)]
    pub find_on_screen: &'a str,
    #[allow(dead_code)]
    pub agent_task: &'a str,
}

/// The first-run tour (D-06): what works without a key comes first, each step
/// can be skipped, and each moves on when the thing it asks for actually
/// happens. It quotes only the main chord, and never an empty one.
pub fn walkthrough(k: &Chords<'_>) -> Vec<Step> {
    let chord = k.capture_query.trim();
    let first = if chord.is_empty() {
        "Click the orb, then type a word you can see on screen.".to_string()
    } else {
        format!("Press {chord}, then type a word you can see on screen.")
    };
    let button = |id: &str, label: &str, primary: bool| Button {
        id: id.into(),
        label: label.into(),
        primary,
    };
    vec![
        Step {
            win: "overlay".into(),
            target_selector: BAR_TARGET.into(),
            click_scope: BAR_SCOPE.into(),
            message: first,
            advance_on: "find-hit".into(),
            ..Default::default()
        },
        Step {
            card: true,
            needs: NEEDS_NO_KEY.into(),
            message: "To get answers, connect an AI provider. OpenRouter has free models.".into(),
            advance_on: "key-saved".into(),
            buttons: vec![
                button("connect", "Connect", true),
                button("later", "Later", false),
            ],
            ..Default::default()
        },
        Step {
            win: "overlay".into(),
            needs: NEEDS_KEY.into(),
            target_selector: BAR_TARGET.into(),
            click_scope: BAR_SCOPE.into(),
            message: "Now ask anything about your screen.".into(),
            advance_on: "first-answer".into(),
            ..Default::default()
        },
        Step {
            win: "overlay".into(),
            needs: NEEDS_BAR.into(),
            target_selector: "#pill, #intent".into(),
            click_scope: BAR_SCOPE.into(),
            message: TOKEN_MESSAGE.into(),
            buttons: vec![button("done", "Got it", true)],
            ..Default::default()
        },
    ]
}

/// Whether a step is for this install: the key card only without a key, the
/// first question only with one.
fn applies(step: &Step, has_key: bool) -> bool {
    match step.needs.as_str() {
        NEEDS_NO_KEY => !has_key,
        NEEDS_KEY => has_key,
        _ => true,
    }
}

/// The first step at or after `from` that applies.
fn first_applicable(steps: &[Step], from: i64, has_key: bool) -> Option<i64> {
    let from = from.max(0) as usize;
    (from..steps.len())
        .find(|&i| applies(&steps[i], has_key))
        .map(|i| i as i64)
}

/// Where a step stands among those that apply, for the progress pips.
fn position(steps: &[Step], index: i64, has_key: bool) -> (usize, usize) {
    let on = |i: &usize| applies(&steps[*i], has_key);
    let before = (0..index.max(0) as usize).filter(on).count();
    (before, (0..steps.len()).filter(on).count())
}

type Cb = Arc<dyn Fn() + Send + Sync>;

/// What main.js passed to `cursorBuddy.init`.
#[derive(Clone)]
pub struct Deps {
    /// Puts the app where the step's target can exist (opens Settings on the
    /// key section, leaves the HUD editor).
    pub ensure_step_prereq: Arc<dyn Fn(&Step) + Send + Sync>,
    pub on_done: Cb,
    pub on_cancel: Cb,
    /// The Agent card's "Try it": open the bar in Agent with a prefill.
    pub on_try: Cb,
}

impl Default for Deps {
    fn default() -> Deps {
        Deps {
            ensure_step_prereq: Arc::new(|_| {}),
            on_done: Arc::new(|| {}),
            on_cancel: Arc::new(|| {}),
            on_try: Arc::new(|| {}),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Steps,
    Points,
    Agent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub label: String,
}

#[derive(Default)]
struct State {
    app: Option<AppHandle>,
    deps: Option<Deps>,
    active: bool,
    mode: Mode,
    steps: Vec<Step>,
    index: i64,
    last_sent: Option<String>,
    points: Vec<Point>,
    point_index: usize,
    /// DIP bounds of the pointer window (its display).
    bounds: Rect,
    scale: f64,
    phys: screen::PxRect,
    ready: bool,
    queued: Option<Value>,
    ignoring: bool,
    focusable: bool,
    /// The window is on its way out; the next session builds a new one.
    destroying: bool,
    /// When click-through last changed.
    heard: Option<Instant>,
    /// The bubble, in page coordinates, when the page last took the mouse.
    callout: Option<Rect>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Bumped by every goTo / cancel, so a stale poll thread stops.
static GEN: AtomicU64 = AtomicU64::new(0);

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(|| State {
        index: -1,
        scale: 1.0,
        ..Default::default()
    }))
}

pub fn init(app: AppHandle, deps: Deps) {
    with(|s| {
        s.app = Some(app);
        s.deps = Some(deps);
    });
}

fn app() -> Option<AppHandle> {
    with(|s| s.app.clone())
}

fn deps() -> Deps {
    with(|s| s.deps.clone()).unwrap_or_default()
}

pub fn is_active() -> bool {
    with(|s| s.active)
}

pub fn step_index() -> i64 {
    with(|s| {
        if s.active && s.mode == Mode::Steps {
            s.index
        } else {
            -1
        }
    })
}

fn emit_to(label: &str, event: &str, payload: Value) {
    if let Some(app) = app() {
        let _ = app.emit_to(label, event, payload);
    }
}

/// The display a named app window is on, else the primary.
fn display_for_window(name: &str) -> screen::Display {
    let list = screen::all_displays();
    if let Some(w) = app().and_then(|a| a.get_webview_window(name)) {
        if let (Ok(p), Ok(sz)) = (w.outer_position(), w.outer_size()) {
            return screen::matching_phys(
                &list,
                &screen::PxRect {
                    x: p.x,
                    y: p.y,
                    width: sz.width as i32,
                    height: sz.height as i32,
                },
            );
        }
    }
    screen::primary(&list)
}

fn window() -> Option<tauri::WebviewWindow> {
    app()?.get_webview_window(LABEL)
}

/// Every payload sent to the page is relative to the window's bounds.
fn send_show(payload: Value) {
    let ready = with(|s| {
        if !s.ready {
            s.queued = Some(payload.clone());
        }
        s.ready
    });
    if ready {
        emit_to(LABEL, "buddy:show", payload);
        if let Some(w) = window() {
            win::raise(capture::hwnd_of(&w));
        }
    }
}

/// Creates the pointer window on `display`, or moves the existing one there
/// (a window left from the last sequence may be on another monitor).
fn create_window(display: &screen::Display) {
    with(|s| {
        s.bounds = display.bounds;
        s.scale = display.scale_factor;
        s.phys = display.phys;
    });
    if let Some(w) = window().filter(|_| !with(|s| s.destroying)) {
        win::place(&w, display.phys);
        let raw = capture::hwnd_of(&w);
        if !flags::is_visible(raw) {
            win::show_inactive(raw);
        }
        return;
    }
    let Some(app) = app() else { return };
    with(|s| {
        s.ready = false;
        s.ignoring = true;
        s.focusable = false;
    });
    let phys = display.phys;
    // Built off the event loop thread: WebView2 creation waits on it.
    let build = move || {
        // A window on its way out still holds the label.
        let deadline = Instant::now() + Duration::from_secs(1);
        while app.get_webview_window(LABEL).is_some() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let built =
            WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App("renderer/buddy.html".into()))
                .title("AevalBuddy")
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .resizable(false)
                .focusable(false)
                .focused(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .content_protected(true)
                .visible(false)
                .data_directory(crate::paths::webview_data_dir())
                .initialization_script(crate::ipc::bridge_script())
                .on_page_load(|_, p| match p.event() {
                    // A reload starts over; `buddy:ready` ends the wait.
                    tauri::webview::PageLoadEvent::Started => with(|s| s.ready = false),
                    // However it got here, a new page lets the mouse through.
                    tauri::webview::PageLoadEvent::Finished => set_click_through(true),
                })
                .build();
        if let Ok(w) = built {
            let raw = capture::hwnd_of(&w);
            win::tool_window(raw, true);
            win::place(&w, phys);
            let _ = w.set_ignore_cursor_events(true);
            win::show_inactive(raw);
            w.on_window_event(|e| {
                if matches!(e, tauri::WindowEvent::Destroyed) {
                    with(|s| s.destroying = false);
                }
            });
            crate::crash::guard_webview(&w, |_, _| browser_gone());
            start_forwarding();
        }
    };
    if std::thread::current().name() == Some("main") {
        // Sends made meanwhile wait in `queued` until the page has loaded.
        std::thread::spawn(build);
    } else {
        build();
    }
}

/// `buddy:ready`: the page has its listeners, so what waited can go, and a
/// page that came back from a reload is told what it was showing.
pub fn page_ready() {
    let (queued, active, mode, card) = with(|s| {
        s.ready = true;
        s.last_sent = None;
        let card = s
            .steps
            .get(s.index.max(0) as usize)
            .is_some_and(|st| st.card);
        (s.queued.take(), s.active, s.mode, card)
    });
    match queued {
        Some(q) => send_show(q),
        None if active && mode == Mode::Points => show_current_point(),
        None if active && mode == Mode::Steps && card => show_card(),
        None => {}
    }
}

pub fn is_ready() -> bool {
    with(|s| s.ready)
}

/// Whether the window really lets clicks through.
pub fn lets_mouse_through() -> bool {
    window().is_some_and(|w| win::is_transparent(capture::hwnd_of(&w)))
}

/// Puts the pointer away between sessions. The window and its page stay, so
/// the next session shows at once (B-031).
fn put_away() {
    with(|s| {
        s.queued = None;
        s.last_sent = None;
    });
    let Some(w) = window() else { return };
    emit_to(LABEL, "buddy:reset", json!(null));
    let raw = capture::hwnd_of(&w);
    if with(|s| std::mem::replace(&mut s.focusable, false)) {
        let _ = w.set_focusable(false);
        win::set_no_activate(raw, true);
    }
    set_click_through(true);
    // Win32, not `w.hide()`: a hide queued on the event loop would land after
    // the next session's show.
    win::hide(raw);
}

/// Drops the window; the next session builds a new one.
pub fn destroy() {
    with(|s| {
        s.ready = false;
        s.queued = None;
    });
    if let Some(w) = window() {
        with(|s| s.destroying = true);
        let _ = w.destroy();
    }
}

/// The browser process died and the webview with it: the session is over.
fn browser_gone() {
    std::thread::spawn(|| {
        cancel();
        destroy();
    });
}

/// The pointer's physical position in page coordinates.
fn to_page(x: i32, y: i32, phys: screen::PxRect, scale: f64) -> (f64, f64) {
    ((x - phys.x) as f64 / scale, (y - phys.y) as f64 / scale)
}

/// Whether to take the mouse back: the window has it, the page has said
/// nothing for a while, and the pointer is off the bubble (B-114). A page
/// that is alive hands the mouse back itself the moment the pointer leaves.
fn watchdog_due(quiet: Duration, at: Option<(f64, f64)>, bubble: Option<Rect>) -> bool {
    if quiet < WATCHDOG {
        return false;
    }
    match (at, bubble) {
        (Some((x, y)), Some(b)) => !b.contains(x, y),
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// One thread for the life of the app, idle between sessions.
fn start_forwarding() {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(forward_loop);
    });
}

/// Replays the pointer into the page while the window ignores the mouse, and
/// takes the mouse back from a page that stopped answering while it had it.
fn forward_loop() {
    let mut last: Option<(i32, i32)> = None;
    let mut inside = false;
    loop {
        let nap = if is_active() {
            FORWARD_MS
        } else {
            FORWARD_IDLE_MS
        };
        std::thread::sleep(Duration::from_millis(nap));
        let Some(w) = window().filter(|_| is_active()) else {
            last = None;
            inside = false;
            continue;
        };
        let (ignoring, phys, scale, heard, bubble) =
            with(|s| (s.ignoring, s.phys, s.scale, s.heard, s.callout));
        let at = screen::cursor_phys();
        if !ignoring {
            last = None;
            let quiet = heard.map_or(Duration::MAX, |t| t.elapsed());
            let page = at.map(|(x, y)| to_page(x, y, phys, scale));
            if watchdog_due(quiet, page, bubble) {
                set_click_through(true);
            }
            continue;
        }
        let Some((x, y)) = at else {
            continue;
        };
        if last == Some((x, y)) {
            continue;
        }
        last = Some((x, y));
        let now_inside = phys.contains(x, y);
        if now_inside {
            let (cx, cy) = to_page(x, y, phys, scale);
            let _ = w.eval(format!(
                "document.dispatchEvent(new MouseEvent('mousemove',{{clientX:{cx},clientY:{cy},bubbles:true}}))"
            ));
        } else if inside {
            let _ = w.eval("document.dispatchEvent(new MouseEvent('mouseleave'))");
        }
        inside = now_inside;
    }
}

/// Starts the walkthrough.
pub fn start(steps: Vec<Step>) {
    if is_active() {
        cancel();
    }
    with(|s| {
        s.mode = Mode::Steps;
        s.steps = steps;
        s.active = true;
    });
    create_window(&display_for_window("overlay"));
    go_to(0);
}

/// The agent's live cursor: glides to a DIP point, names the step, and
/// resolves once it has arrived, so the action fires where the user looks.
pub async fn agent_point(point: (f64, f64), label: &str, display: &screen::Display) {
    if with(|s| s.active && s.mode != Mode::Agent) {
        cancel();
    }
    with(|s| {
        s.mode = Mode::Agent;
        s.active = true;
        s.last_sent = None;
    });
    create_window(display);
    let start = Instant::now();
    while !with(|s| s.ready) && start.elapsed() < Duration::from_secs(5) {
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let b = with(|s| s.bounds);
    send_show(json!({
        "kind": "agent",
        "x": (point.0 - b.x).round(),
        "y": (point.1 - b.y).round(),
        "message": label,
        "glideMs": 240,
    }));
    tokio::time::sleep(Duration::from_millis(GLIDE_WAIT_MS)).await;
}

/// K-13: the agent's cursor at a physical point. The display comes from the
/// physical side, where it is unambiguous, and the point is mapped to DIPs
/// on that display.
// host/agent.rs (agent lane) moves to this from `agent_point`.
#[allow(dead_code)]
pub async fn agent_point_phys(pt_px: (f64, f64), label: &str) {
    let list = screen::all_displays();
    let at = screen::PxRect {
        x: pt_px.0.floor() as i32,
        y: pt_px.1.floor() as i32,
        width: 1,
        height: 1,
    };
    let d = screen::matching_phys(&list, &at);
    let p = d.point_to_dip(pt_px);
    agent_point(p, label, &d).await;
}

/// Takes the agent's cursor down at the end of a run.
pub fn agent_clear() {
    let was = with(|s| {
        let was = s.active && s.mode == Mode::Agent;
        if was {
            s.active = false;
        }
        was
    });
    if was {
        put_away();
    }
}

/// The tour's Skip: the next step that applies, or done after the last.
pub fn skip() {
    let (active, mode, index) = with(|s| (s.active, s.mode, s.index));
    if !active || mode != Mode::Steps {
        return;
    }
    go_to(index + 1);
}

/// Replaces what the current step says (after a failed key test).
pub fn set_message(text: &str) {
    let card = with(|s| {
        if !s.active || s.mode != Mode::Steps || s.index < 0 || s.index as usize >= s.steps.len() {
            return None;
        }
        let i = s.index as usize;
        s.steps[i].message = text.to_string();
        s.last_sent = None;
        Some(s.steps[i].card)
    });
    match card {
        Some(true) => show_card(),
        Some(false) => resolve_target(),
        None => {}
    }
}

/// A tour step with no pointer: a card near the overlay with its buttons.
fn show_card() {
    let has_key = crate::store::get().has_api_key();
    let (step, index, count, b) = with(|s| {
        let st = s
            .steps
            .get(s.index.max(0) as usize)
            .cloned()
            .unwrap_or_default();
        let (at, count) = position(&s.steps, s.index, has_key);
        (st, at, count, s.bounds)
    });
    let list = screen::all_displays();
    let anchor = app()
        .and_then(|a| a.get_webview_window("overlay"))
        .and_then(|w| {
            let p = w.outer_position().ok()?;
            let sz = w.outer_size().ok()?;
            Some(screen::screen_to_dip_rect(
                &list,
                screen::PxRect {
                    x: p.x,
                    y: p.y,
                    width: sz.width as i32,
                    height: sz.height as i32,
                },
            ))
        })
        .unwrap_or(Rect::new(b.x + b.width - 420.0, b.y + 40.0, 100.0, 40.0));
    send_show(json!({
        "kind": "card",
        "x": ((b.x + 16.0).max(anchor.x + anchor.width - 380.0) - b.x).round(),
        "y": (anchor.y + anchor.height + 12.0 - b.y).round(),
        "message": step.message,
        "buttons": button_values(&step.buttons),
        "stepIndex": index,
        "stepCount": count,
    }));
}

fn button_values(buttons: &[Button]) -> Vec<Value> {
    buttons
        .iter()
        .map(|x| {
            let mut v = json!({ "id": x.id, "label": x.label });
            if x.primary {
                v["primary"] = json!(true);
            }
            v
        })
        .collect()
}

/// Model-directed points, one at a time (DIP screen coordinates), on the
/// display the shot was read off (TOUR-04).
pub fn show_points(points: Vec<Point>, display: Option<&screen::Display>) {
    if points.is_empty() {
        return;
    }
    if is_active() {
        cancel();
    }
    with(|s| {
        s.mode = Mode::Points;
        s.points = points.into_iter().take(3).collect();
        s.point_index = 0;
        s.active = true;
        s.last_sent = None;
    });
    let d = display
        .cloned()
        .unwrap_or_else(|| screen::primary(&screen::all_displays()));
    create_window(&d);
    show_current_point();
}

fn show_current_point() {
    let Some((p, i, n, b)) = with(|s| {
        if !s.active || s.mode != Mode::Points {
            return None;
        }
        let p = s.points.get(s.point_index)?.clone();
        Some((p, s.point_index, s.points.len(), s.bounds))
    }) else {
        return;
    };
    send_show(json!({
        "kind": "point",
        "x": (p.x - b.x).round(),
        "y": (p.y - b.y).round(),
        "targetW": 28,
        "targetH": 28,
        "message": if p.label.is_empty() { "Here".to_string() } else { p.label },
        "stepIndex": i,
        "stepCount": n,
    }));
}

/// The callout's action button: next point, or done after the last.
pub fn advance() {
    let next = with(|s| {
        if !s.active || s.mode != Mode::Points {
            return None;
        }
        if s.point_index + 1 < s.points.len() {
            s.point_index += 1;
            Some(true)
        } else {
            Some(false)
        }
    });
    match next {
        Some(true) => show_current_point(),
        Some(false) => complete(),
        None => {}
    }
}

/// Goes to the first step at or after `i` that applies, or finishes.
fn go_to(i: i64) {
    let has_key = crate::store::get().has_api_key();
    let Some(i) = with(|s| first_applicable(&s.steps, i, has_key)) else {
        complete();
        return;
    };
    let step = with(|s| {
        s.index = i;
        s.last_sent = None;
        s.steps.get(i as usize).cloned()
    });
    let Some(step) = step else { return };
    (deps().ensure_step_prereq)(&step);
    if step.needs == NEEDS_BAR {
        open_bar();
    }
    broadcast_watch();
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    if step.card {
        show_card();
        return;
    }
    std::thread::spawn(move || loop {
        if GEN.load(Ordering::SeqCst) != gen || !is_active() {
            return;
        }
        resolve_target();
        std::thread::sleep(Duration::from_millis(POLL_MS));
    });
}

/// The step about the bar's token needs the bar open.
fn open_bar() {
    let Some(app) = app() else { return };
    if crate::state::shared(&app).lock().mode != crate::state::Mode::Pill {
        return;
    }
    let a = app.clone();
    let _ = app.run_on_main_thread(move || crate::windows::open_bar(&a, None, None));
}

/// Tells each app window which selector counts as on the walkthrough path,
/// so a click on the control the step asks for does not dismiss it.
fn broadcast_watch() {
    let step = with(|s| {
        if s.active && s.index >= 0 {
            s.steps.get(s.index as usize).cloned()
        } else {
            None
        }
    });
    for name in ["overlay", "settings"] {
        let selector = match &step {
            Some(st) if st.win == name => {
                let sel = if st.click_scope.is_empty() {
                    &st.target_selector
                } else {
                    &st.click_scope
                };
                json!(sel)
            }
            _ => Value::Null,
        };
        emit_to(name, "buddy:watch", json!({ "selector": selector }));
    }
}

fn resolve_target() {
    let Some(step) = with(|s| {
        if !s.active {
            return None;
        }
        s.steps.get(s.index.max(0) as usize).cloned()
    }) else {
        return;
    };
    let exists = app()
        .and_then(|a| a.get_webview_window(&step.win))
        .is_some();
    if !exists {
        hide_pointer();
        return;
    }
    emit_to(
        &step.win,
        "buddy:resolve",
        json!({ "selector": step.target_selector }),
    );
}

/// The page's answer to `buddy:resolve` (screen DIP). Identical frames are
/// dropped; only movement or a new message reaches the pointer.
pub fn handle_resolve_result(result: &Value) {
    if !is_active() || window().is_none() {
        return;
    }
    if !result["found"].as_bool().unwrap_or(false) {
        hide_pointer();
        return;
    }
    let f = |k: &str| result[k].as_f64().unwrap_or(0.0);
    let (b, step) = with(|s| {
        let st = s
            .steps
            .get(s.index.max(0) as usize)
            .cloned()
            .unwrap_or_default();
        (s.bounds, st)
    });
    let x = (f("cx") - b.x).round();
    let y = (f("cy") - b.y).round();
    let (tw, th) = (f("w").round(), f("h").round());
    let key = format!("{x}|{y}|{tw}|{th}|{}", step.message);
    let fresh = with(|s| {
        if s.last_sent.as_deref() == Some(key.as_str()) {
            false
        } else {
            s.last_sent = Some(key.clone());
            true
        }
    });
    if !fresh {
        return;
    }
    let has_key = crate::store::get().has_api_key();
    let (index, count) = with(|s| position(&s.steps, s.index, has_key));
    send_show(json!({
        "kind": "step",
        "x": x,
        "y": y,
        "targetW": tw,
        "targetH": th,
        "message": step.message,
        "skippable": step.buttons.is_empty(),
        "buttons": button_values(&step.buttons),
        "stepIndex": index,
        "stepCount": count,
    }));
}

/// A card is a dialog: it takes focus so its buttons work from the keyboard.
pub fn focus_card() {
    if let Some(w) = window() {
        with(|s| s.focusable = true);
        win::set_no_activate(capture::hwnd_of(&w), false);
        let _ = w.set_focusable(true);
        let _ = w.set_focus();
    }
}

fn hide_pointer() {
    with(|s| s.last_sent = None);
    if let Some(w) = window() {
        if with(|s| std::mem::replace(&mut s.focusable, false)) {
            let _ = w.set_focusable(false);
            win::set_no_activate(capture::hwnd_of(&w), true);
        }
        emit_to(LABEL, "buddy:hide", json!(null));
        // Hiding the bubble hands the mouse back at once: a full-screen
        // sheet must not stay over the desktop waiting for a mousemove.
        set_click_through(true);
    }
}

/// `buddy:set-ignore`: click-through except while the cursor is over the
/// callout, whose rectangle comes along when the page takes the mouse.
pub fn page_set_ignore(ignore: bool, callout: &Value) {
    let n = |k: &str| callout[k].as_f64();
    let rect = match (n("x"), n("y"), n("w"), n("h")) {
        (Some(x), Some(y), Some(w), Some(h)) => Some(Rect::new(x, y, w, h)),
        _ => None,
    };
    with(|s| s.callout = rect);
    set_click_through(ignore);
}

pub fn set_click_through(ignore: bool) {
    if let Some(w) = window() {
        with(|s| {
            s.ignoring = ignore;
            s.heard = Some(Instant::now());
        });
        let _ = w.set_ignore_cursor_events(ignore);
    }
}

/// App events: the step's `advanceOn` advances, `dismiss` cancels, and
/// closing Settings mid-tour falls back to the first step.
pub fn notify(event: &str) {
    if !is_active() {
        return;
    }
    if event == "dismiss" {
        cancel();
        return;
    }
    let (mode, step, index) = with(|s| {
        (
            s.mode,
            s.steps.get(s.index.max(0) as usize).cloned(),
            s.index,
        )
    });
    if mode != Mode::Steps {
        return;
    }
    let Some(step) = step else { return };
    if event == step.advance_on {
        go_to(index + 1);
        return;
    }
    if event == "settings-closed" && step.win == "settings" && index > 0 {
        go_to(0);
    }
}

fn complete() {
    if !with(|s| std::mem::replace(&mut s.active, false)) {
        return;
    }
    GEN.fetch_add(1, Ordering::SeqCst);
    broadcast_watch_cleared();
    if window().is_some() {
        emit_to(LABEL, "buddy:done", json!(null));
    }
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(700));
        if !is_active() {
            put_away();
        }
    });
    (deps().on_done)();
}

pub fn cancel() {
    if !with(|s| std::mem::replace(&mut s.active, false)) {
        return;
    }
    GEN.fetch_add(1, Ordering::SeqCst);
    broadcast_watch_cleared();
    put_away();
    (deps().on_cancel)();
}

/// `buddy:card`: Connect opens Settings and waits for the key; every other
/// button moves on, and `try` also opens the bar in Agent.
pub fn card(id: &str) {
    if id == "connect" {
        if let Some(a) = app().filter(|_| step_index() == KEY_STEP) {
            crate::windows::open_settings(&a, Some("providers"));
        }
        return;
    }
    skip();
    if id == "try" {
        (deps().on_try)();
    }
}

fn broadcast_watch_cleared() {
    for name in ["overlay", "settings"] {
        emit_to(name, "buddy:watch", json!({ "selector": null }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tour(chord: &str) -> Vec<Step> {
        walkthrough(&Chords {
            capture_query: chord,
            find_on_screen: "",
            agent_task: "",
        })
    }

    #[test]
    fn walkthrough_order_and_no_double_spaces() {
        let w = tour("Ctrl+Alt+Space");
        let events: Vec<&str> = w.iter().map(|s| s.advance_on.as_str()).collect();
        assert_eq!(events, ["find-hit", "key-saved", "first-answer", ""]);
        assert_eq!(
            w[0].message,
            "Press Ctrl+Alt+Space, then type a word you can see on screen."
        );
        assert!(w[KEY_STEP as usize].card);
        assert_eq!(w[1].buttons[0].id, "connect");
        assert_eq!(w[1].buttons[1].id, "later");
        assert_eq!(w[3].target_selector, "#pill, #intent");
        assert_eq!(w[3].buttons[0].label, "Got it");
        for chord in ["Ctrl+Alt+Space", "", "  "] {
            for s in tour(chord) {
                assert!(!s.message.contains("  "), "{}", s.message);
                assert!(!s.message.contains("Press ,"), "{}", s.message);
            }
        }
    }

    #[test]
    fn steps_apply_by_whether_a_key_exists() {
        let w = tour("Ctrl+Alt+Space");
        assert_eq!(first_applicable(&w, 1, false), Some(1));
        assert_eq!(first_applicable(&w, 2, false), Some(3));
        assert_eq!(first_applicable(&w, 1, true), Some(2));
        assert_eq!(first_applicable(&w, 4, true), None);
        assert_eq!(position(&w, 3, false), (2, 3));
        assert_eq!(position(&w, 2, true), (1, 3));
    }

    #[test]
    fn watchdog_restores_click_through() {
        let bubble = Some(Rect::new(100.0, 100.0, 200.0, 80.0));
        let long = WATCHDOG + Duration::from_millis(1);
        let away = Some((10.0, 10.0));
        assert!(watchdog_due(long, away, bubble));
        assert!(watchdog_due(long, away, None));
        assert!(!watchdog_due(long, Some((150.0, 120.0)), bubble));
        assert!(!watchdog_due(Duration::from_secs(1), away, bubble));
        assert!(!watchdog_due(long, None, bubble));
    }
}
