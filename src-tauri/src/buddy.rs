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

#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::capture::{self, screen, Rect};
use crate::desktop::win;

pub const LABEL: &str = "buddy";
pub const POLL_MS: u64 = 150;
const FORWARD_MS: u64 = 16;

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
    pub find_on_screen: &'a str,
    pub agent_task: &'a str,
}

/// The first-run tour (TOUR-01): four steps, each skippable, each moving on
/// when the thing it asks for actually happens.
pub fn walkthrough(k: &Chords<'_>) -> Vec<Step> {
    vec![
        Step {
            win: "settings".into(),
            needs: "settings-key".into(),
            target_selector: "#api-key-input".into(),
            click_scope: "#sec-key".into(),
            message: "Paste an OpenRouter API key here and press Save. A free key works.".into(),
            advance_on: "key-saved".into(),
            ..Default::default()
        },
        Step {
            win: "overlay".into(),
            target_selector: "#pill, #query-input".into(),
            click_scope: "#pill, #panel".into(),
            message: format!(
                "Press {} and ask anything about your screen.",
                k.capture_query
            ),
            advance_on: "first-answer".into(),
            ..Default::default()
        },
        Step {
            win: "overlay".into(),
            target_selector: "#pill, #query-input".into(),
            click_scope: "#pill, #panel".into(),
            message: format!("Press {} and type a word you can see.", k.find_on_screen),
            advance_on: "find-hit".into(),
            ..Default::default()
        },
        Step {
            card: true,
            message: format!(
                "Press {} and say what to do. Agent asks before anything risky, and Esc stops it.",
                k.agent_task
            ),
            buttons: vec![
                Button {
                    id: "try".into(),
                    label: "Try it".into(),
                    primary: true,
                },
                Button {
                    id: "later".into(),
                    label: "Later".into(),
                    primary: false,
                },
            ],
            ..Default::default()
        },
    ]
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

pub fn is_agent() -> bool {
    with(|s| s.active && s.mode == Mode::Agent)
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
    if let Some(w) = window() {
        win::place(&w, display.phys);
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
                .on_page_load(|_, p| {
                    if matches!(p.event(), tauri::webview::PageLoadEvent::Finished) {
                        let queued = with(|s| {
                            s.ready = true;
                            s.queued.take()
                        });
                        if let Some(q) = queued {
                            send_show(q);
                        }
                    }
                })
                .build();
        if let Ok(w) = built {
            let raw = capture::hwnd_of(&w);
            win::tool_window(raw, true);
            win::place(&w, phys);
            let _ = w.set_ignore_cursor_events(true);
            win::show_inactive(raw);
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

fn destroy_window() {
    if let Some(w) = window() {
        let _ = w.destroy();
    }
    with(|s| {
        s.ready = false;
        s.queued = None;
    });
}

/// Replays the pointer into the page while the window ignores the mouse.
fn start_forwarding() {
    std::thread::spawn(|| {
        let mut last: Option<(i32, i32)> = None;
        let mut inside = false;
        loop {
            std::thread::sleep(Duration::from_millis(FORWARD_MS));
            let Some(w) = window() else { return };
            let (ignoring, phys, scale) = with(|s| (s.ignoring, s.phys, s.scale));
            if !ignoring {
                last = None;
                continue;
            }
            let Some((x, y)) = screen::cursor_phys() else {
                continue;
            };
            if last == Some((x, y)) {
                continue;
            }
            last = Some((x, y));
            let now_inside = phys.contains(x, y);
            if now_inside {
                let cx = (x - phys.x) as f64 / scale;
                let cy = (y - phys.y) as f64 / scale;
                let _ = w.eval(format!(
                    "document.dispatchEvent(new MouseEvent('mousemove',{{clientX:{cx},clientY:{cy},bubbles:true}}))"
                ));
            } else if inside {
                let _ = w.eval("document.dispatchEvent(new MouseEvent('mouseleave'))");
            }
            inside = now_inside;
        }
    });
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
    tokio::time::sleep(Duration::from_millis(260)).await;
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
        destroy_window();
    }
}

/// The tour's Skip: the next step, or done after the last.
pub fn skip() {
    let (active, mode, index, len) = with(|s| (s.active, s.mode, s.index, s.steps.len() as i64));
    if !active || mode != Mode::Steps {
        return;
    }
    if index + 1 < len {
        go_to(index + 1);
    } else {
        complete();
    }
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
    let (step, index, count, b) = with(|s| {
        let st = s
            .steps
            .get(s.index.max(0) as usize)
            .cloned()
            .unwrap_or_default();
        (st, s.index, s.steps.len(), s.bounds)
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
    let buttons: Vec<Value> = step
        .buttons
        .iter()
        .map(|x| {
            let mut v = json!({ "id": x.id, "label": x.label });
            if x.primary {
                v["primary"] = json!(true);
            }
            v
        })
        .collect();
    send_show(json!({
        "kind": "card",
        "x": ((b.x + 16.0).max(anchor.x + anchor.width - 380.0) - b.x).round(),
        "y": (anchor.y + anchor.height + 12.0 - b.y).round(),
        "message": step.message,
        "buttons": buttons,
        "stepIndex": index,
        "stepCount": count,
    }));
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

fn go_to(i: i64) {
    let step = with(|s| {
        s.index = i;
        s.last_sent = None;
        s.steps.get(i as usize).cloned()
    });
    let Some(step) = step else { return };
    (deps().ensure_step_prereq)(&step);
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
    let (b, msg, index, count) = with(|s| {
        let m = s
            .steps
            .get(s.index.max(0) as usize)
            .map(|st| st.message.clone())
            .unwrap_or_default();
        (s.bounds, m, s.index, s.steps.len())
    });
    let x = (f("cx") - b.x).round();
    let y = (f("cy") - b.y).round();
    let (tw, th) = (f("w").round(), f("h").round());
    let key = format!("{x}|{y}|{tw}|{th}|{msg}");
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
    send_show(json!({
        "kind": "step",
        "x": x,
        "y": y,
        "targetW": tw,
        "targetH": th,
        "message": msg,
        "skippable": true,
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
/// callout.
pub fn set_click_through(ignore: bool) {
    if let Some(w) = window() {
        with(|s| s.ignoring = ignore);
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
    let (mode, step, index, len) = with(|s| {
        (
            s.mode,
            s.steps.get(s.index.max(0) as usize).cloned(),
            s.index,
            s.steps.len() as i64,
        )
    });
    if mode != Mode::Steps {
        return;
    }
    let Some(step) = step else { return };
    if event == step.advance_on {
        if index + 1 < len {
            go_to(index + 1);
        } else {
            complete();
        }
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
            destroy_window();
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
    destroy_window();
    (deps().on_cancel)();
}

/// `buddy:card`: skip the card; `try` also opens the bar in Agent.
pub fn card(id: &str) {
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

    #[test]
    fn tour_has_four_steps_with_live_chords() {
        let w = walkthrough(&Chords {
            capture_query: "Ctrl+Alt+Space",
            find_on_screen: "Ctrl+Alt+F",
            agent_task: "Ctrl+Alt+A",
        });
        assert_eq!(w.len(), 4);
        assert_eq!(w[0].target_selector, "#api-key-input");
        assert_eq!(w[0].click_scope, "#sec-key");
        assert_eq!(w[0].advance_on, "key-saved");
        assert_eq!(w[1].advance_on, "first-answer");
        assert!(w[1].message.contains("Ctrl+Alt+Space"));
        assert_eq!(w[2].advance_on, "find-hit");
        assert!(w[3].card);
        assert_eq!(w[3].buttons[0].id, "try");
        assert!(w[3]
            .message
            .starts_with("Press Ctrl+Alt+A and say what to do."));
    }
}
