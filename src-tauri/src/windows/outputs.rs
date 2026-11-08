//! Free-floating output windows (REQUESTS.md 8-11, PARITY NEW-01..07).
//!
//! Each answer lives in its own `output-<n>` window with its own
//! conversation. Rust places a new one 12 px off the bar's glass and shows
//! it; the page globs itself out of the bar (it asks `output:bar` where the
//! bar is) and plays the close glob before `output:close` destroys it. One
//! hidden, loaded window is always kept warm so the next answer opens with
//! no page load.
//!
//! The bar's follow-ups go to the target (the focused output); the bar's
//! `chat:new` detaches it so the next question opens a new window. Find
//! results and task trails open a window of their own (`fresh`). Each
//! window owns its Ask: closing or resetting one stops only its own
//! (`AppState::queries`, K-03). ARCHITECTURE.md sections 9 and 10.

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::AppHandle;

use super::layout::{self, Anchor};
use super::{
    app_driven, bar_rect, create_page, drop_held, emit, flags, hold_events, hwnd, mark_app, motion,
    overlay_state, release_held, screen, send_now, send_overlay_state, stage, take_held, window,
    Meta, Resize, OVERLAY,
};
use crate::state::{now_ms, shared, AppState, Chat, Output, OutputKind, Rect};
use crate::store;

pub const PAGE: &str = "renderer/output.html";
/// D-32: a seventh window closes the least recently touched idle one.
pub const MAX_OPEN: usize = 6;
/// D-31: how long "Start over" can be undone.
pub const UNDO_MS: u64 = 10_000;

pub fn is_output(label: &str) -> bool {
    label.starts_with("output-")
}

/// No maximum: a drag or a maximize may take the whole work area (B-024);
/// `store::MAX_W` caps only the width a new window starts at.
pub(super) fn meta() -> Meta {
    Meta {
        margin: stage::MARGIN,
        resize: Resize::All,
        min: (store::MIN_W as f64, layout::OUTPUT_MIN_HEIGHT),
        max: (0.0, 0.0),
    }
}

/// Creates a hidden output window with a fresh conversation; returns its
/// label. It sits on the bar until it is shown, so the glob starts there.
pub fn create(app: &AppHandle) -> tauri::Result<String> {
    let label = {
        let st = shared(app);
        let mut s = st.lock();
        let n = s.next_output;
        s.next_output += 1;
        let label = format!("output-{n}");
        s.outputs.insert(
            label.clone(),
            Output {
                label: label.clone(),
                chat: Chat::fresh(),
                view: "chat".into(),
                content_height: layout::DEFAULT_OUTPUT_HEIGHT,
                ..Output::default()
            },
        );
        label
    };
    // Events sent before the page says hello wait for it (B-019).
    hold_events(&label);
    let at = bar_rect(app);
    let outer = stage::inflate(at, stage::MARGIN);
    match create_page(app, &label, PAGE, outer, meta()) {
        Ok(_) => Ok(label),
        Err(e) => {
            shared(app).lock().outputs.remove(&label);
            drop_held(&label);
            Err(e)
        }
    }
}

/// Hidden, empty and showing a conversation: free to take the next answer.
fn is_idle(o: &Output) -> bool {
    !o.active && o.chat.messages.is_empty() && o.kind == OutputKind::Chat
}

/// Keeps one hidden, unused output window around.
pub fn create_spare(app: &AppHandle) -> tauri::Result<()> {
    let have = shared(app).lock().outputs.values().any(is_idle);
    if !have {
        create(app)?;
    }
    Ok(())
}

/// The idle window to reuse, a loaded one first, never `except`.
pub fn pick_spare(outputs: &BTreeMap<String, Output>, except: Option<&str>) -> Option<String> {
    let free = |o: &&Output| is_idle(o) && except != Some(o.label.as_str());
    outputs
        .values()
        .filter(free)
        .find(|o| o.ready)
        .or_else(|| outputs.values().find(free))
        .map(|o| o.label.clone())
}

fn spare(app: &AppHandle, except: Option<&str>) -> Option<String> {
    let found = pick_spare(&shared(app).lock().outputs, except);
    found.or_else(|| create(app).ok())
}

/// A spare (or new) window becomes the target and is shown.
fn open_spare(app: &AppHandle, except: Option<&str>) -> Option<String> {
    let label = spare(app, except)?;
    {
        let st = shared(app);
        let mut s = st.lock();
        s.focused_output = Some(label.clone());
        if let Some(o) = s.outputs.get_mut(&label) {
            o.touched = now_ms();
        }
    }
    // Shown and placed before the first event reaches it: the page globs
    // itself in on `chat:user` / `query:start` / `agent:begin`.
    show(app, &label);
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = create_spare(&a);
    });
    Some(label)
}

/// The output the next answer goes to: the target if it is still open,
/// else a fresh window (which becomes the target). The Ask calls this when
/// a question is submitted.
pub fn target(app: &AppHandle) -> Option<String> {
    let focused = {
        let st = shared(app);
        let s = st.lock();
        s.focused_output
            .clone()
            .filter(|l| s.outputs.get(l).map(|o| o.active).unwrap_or(false))
    };
    if focused.is_some() {
        return focused;
    }
    open_spare(app, None)
}

/// K-04: a window for a Find result or a task trail. Always a spare or a
/// new window, never the conversation the bar is continuing. It becomes
/// the target, so follow-ups typed while it is in front go to it.
pub fn fresh(app: &AppHandle) -> Option<String> {
    let except = focused(app);
    open_spare(app, except.as_deref())
}

/// The focused output's label, if any.
pub fn focused(app: &AppHandle) -> Option<String> {
    shared(app).lock().focused_output.clone()
}

/// Runs `f` on an output's state (the Ask: messages, region, busy text).
pub fn with<R>(app: &AppHandle, label: &str, f: impl FnOnce(&mut Output) -> R) -> Option<R> {
    let st = shared(app);
    let mut s = st.lock();
    s.outputs.get_mut(label).map(f)
}

/// B-025: the work area of the display the window is on.
fn own_area(h: isize) -> Rect {
    screen::matching_px(flags::rect_px(h)).work_area
}

fn size_for(o: &Output, wa: Rect) -> (f64, f64) {
    let w = o
        .rect
        .map(|r| r.width)
        .unwrap_or(store::get().get_output_width() as f64);
    (
        w,
        layout::output_height(wa, o.content_height, o.user_height),
    )
}

fn open_rects(app: &AppHandle, except: &str) -> Vec<Rect> {
    shared(app)
        .lock()
        .outputs
        .values()
        .filter(|o| o.active && o.label != except)
        .filter_map(|o| o.rect)
        .collect()
}

/// Shows an output (main.js showOutput). A new one opens 12 px off the bar
/// (layout::place_output); later shows put it back where it was. The page
/// draws the glob itself (ARCHITECTURE.md 10.3), so the window only moves.
pub fn show(app: &AppHandle, label: &str) {
    let Some(h) = hwnd(app, label) else {
        return;
    };
    let Some(mut o) = shared(app).lock().outputs.get(label).cloned() else {
        return;
    };
    let opening = !o.active;
    let stored = store::get().get_output_height();
    o.user_height = seeded_height(o.user_height, o.placed, stored);
    let user = o.user_height;
    if shared(app).lock().hidden {
        with(app, label, |o| {
            o.active = true;
            o.user_height = user;
        });
        if opening {
            enforce_cap(app, label);
        }
        return;
    }
    // B-025: a placed window stays on its own display; a new one opens on
    // the display the person is on.
    let display = if o.rect.is_some() {
        screen::matching_px(flags::rect_px(h))
    } else {
        screen::latched()
    };
    let wa = display.work_area;
    let size = size_for(&o, wa);
    let bar = bar_rect(app);
    let to = match o.rect {
        Some(r) => layout::clamp_to(wa, Rect::new(r.x, r.y, size.0, size.1)),
        None => {
            let anchor = Anchor::from_value(&store::get().get_overlay_pos());
            layout::place_output(wa, bar, &anchor, size, &open_rects(app, label))
        }
    };
    with(app, label, |o| {
        o.active = true;
        o.placed = true;
        o.rect = Some(to);
        o.touched = now_ms();
        o.user_height = user;
    });
    flags::set_meta(h, |m| m.resize = Resize::All);
    mark_app(label);
    // A maximized window stays maximized; `to` is where it goes back to.
    let maxed = flags::is_maximized(h);
    if !flags::is_visible(h) {
        motion::stop(h);
        if !maxed {
            motion::set_glass_on(h, to, display);
        }
        flags::show_inactive(h);
        flags::pin_on_top(h);
    } else if !maxed {
        motion::animate(h, None, to, layout::T_OUT_GROW, None);
    }
    emit(app, label, "overlay:state", overlay_state(app, Some(label)));
    if opening {
        enforce_cap(app, label);
    }
    send_meta(app);
}

/// IPC-05: the transcript's height changed. A window whose bottom edge is
/// nearer the bar grows upward so it never covers it.
pub fn resize(app: &AppHandle, label: &str, height: f64) {
    let Some(o) = with(app, label, |o| {
        o.content_height = height;
        o.clone()
    }) else {
        return;
    };
    if !o.active || o.user_height > 0.0 {
        return;
    }
    let Some(h) = hwnd(app, label) else {
        return;
    };
    if !flags::is_visible(h) {
        return;
    }
    let have = motion::glass(h);
    let wa = own_area(h);
    let (_, want_h) = size_for(&o, wa);
    if (have.height - want_h).abs() < 1.0 {
        return;
    }
    let bar = bar_rect(app);
    let grow_up = grow_of(have, bar) == "up";
    let mut to = Rect::new(have.x, have.y, have.width, want_h);
    if grow_up {
        to.y = have.y + have.height - want_h;
    }
    to.y = layout::clamp(to.y, wa.y, (wa.y + wa.height - want_h).max(wa.y));
    with(app, label, |o| o.rect = Some(to));
    mark_app(label);
    motion::animate(h, None, to, layout::T_OUT_GROW, None);
}

/// `output:glass {top, height}`: where the visible sheet sits inside the
/// glass, in glass DIPs. The page sends it at most once a frame.
pub fn set_sheet(app: &AppHandle, label: &str, top: f64, height: f64) {
    let sheet = (top.max(0.0), height.max(0.0));
    with(app, label, |o| o.sheet = Some(sheet));
}

/// Hides every output: the layout editor shows the bar alone.
/// Conversations stay; the next open brings back the ones with content.
pub fn hide_all(app: &AppHandle) {
    let labels: Vec<String> = shared(app)
        .lock()
        .outputs
        .values_mut()
        .filter(|o| o.active)
        .map(|o| {
            o.active = false;
            o.label.clone()
        })
        .collect();
    for l in labels {
        hide_window(app, &l);
    }
}

fn hide_window(app: &AppHandle, label: &str) {
    if let Some(w) = window(app, label) {
        if let Some(h) = flags::hwnd_of(&w) {
            motion::stop(h);
        }
        flags::hide(&w);
    }
}

/// The visibility chord: hide the windows, remember which were open.
pub fn hide_windows(app: &AppHandle) {
    let labels: Vec<String> = shared(app)
        .lock()
        .outputs
        .values()
        .filter(|o| o.active)
        .map(|o| o.label.clone())
        .collect();
    for l in labels {
        hide_window(app, &l);
    }
}

pub fn show_active_windows(app: &AppHandle) {
    let labels: Vec<String> = shared(app)
        .lock()
        .outputs
        .values()
        .filter(|o| o.active)
        .map(|o| o.label.clone())
        .collect();
    for l in labels {
        show(app, &l);
    }
}

/// The bar opened: conversations with content come back (main.js
/// triggerCapture's `if (chat.length) showOutput()`).
pub fn show_with_chat(app: &AppHandle) {
    let labels: Vec<String> = shared(app)
        .lock()
        .outputs
        .values()
        .filter(|o| !o.chat.messages.is_empty())
        .map(|o| o.label.clone())
        .collect();
    for l in labels {
        show(app, &l);
    }
}

/// The agent's trail with no agent module hooked: the focused output.
pub fn show_trail(app: &AppHandle, _focus: bool) {
    let label = shared(app)
        .lock()
        .outputs
        .values()
        .find(|o| o.kind == OutputKind::Trail)
        .map(|o| o.label.clone());
    if let Some(l) = label.or_else(|| focused(app)) {
        show(app, &l);
    }
}

/// Whether an Ask is streaming into `label`. Until the Ask reports per
/// window (K-03), its single busy flag counts for the target window.
fn running_in(app: &AppHandle, label: &str) -> bool {
    let st = shared(app);
    let s = st.lock();
    s.query_running(label)
        || (s.queries.is_empty() && s.busy && s.focused_output.as_deref() == Some(label))
}

/// Stops the Ask streaming into `label`, and only that one.
fn abort_in(app: &AppHandle, label: &str) {
    let st = shared(app);
    if st.call_str(app, label, |h| h.abort_query_for.as_ref()) {
        return;
    }
    // No per-window abort: the single Ask is this window's (running_in).
    if !st.call(app, |h| h.abort_query.as_ref()) {
        st.lock().busy = false;
    }
}

/// `output:close` (ARCHITECTURE.md 10.1): the page has played its close
/// glob, so the window goes at once with its conversation (history already
/// has it). Only its own Ask stops; follow-ups go to the most recently
/// focused output still open.
pub fn close(app: &AppHandle, label: &str) {
    if running_in(app, label) {
        abort_in(app, label);
    }
    {
        let st = shared(app);
        let mut s = st.lock();
        s.outputs.remove(label);
        s.query_forget(label);
        if s.focused_output.as_deref() == Some(label) {
            s.focused_output = s
                .outputs
                .values()
                .filter(|o| o.active)
                .max_by_key(|o| o.touched)
                .map(|o| o.label.clone());
        }
    }
    if let Some(w) = window(app, label) {
        if let Some(h) = flags::hwnd_of(&w) {
            motion::stop(h);
        }
        let _ = w.destroy();
    }
    send_overlay_state(app);
}

/// K-04: every window with a conversation closes (the tray's "Close all
/// answer windows", `output:close-all`); each stops only its own Ask. The
/// warm spare stays.
pub fn close_all(app: &AppHandle) {
    let labels: Vec<String> = shared(app)
        .lock()
        .outputs
        .values()
        .filter(|o| o.active || !o.chat.messages.is_empty())
        .map(|o| o.label.clone())
        .collect();
    for l in labels {
        close(app, &l);
    }
}

/// `chat:new` from an output window: that window starts over and no new
/// window opens (ARCHITECTURE.md 10.3). The old conversation can come back
/// for 10 s (`output:undo-reset`), and the picked area is dropped (IPC-18).
///
/// `chat:reset {undoMs}`: how long Undo is offered, 0 when there was
/// nothing to undo.
pub fn reset(app: &AppHandle, label: &str) {
    if running_in(app, label) {
        abort_in(app, label);
    }
    let until = now_ms() + UNDO_MS;
    let found = with(app, label, |o| {
        let old = std::mem::replace(&mut o.chat, Chat::fresh());
        o.undo = (!old.messages.is_empty()).then_some((old, until));
        o.user_height = 0.0;
        o.content_height = layout::DEFAULT_OUTPUT_HEIGHT;
        o.kind = OutputKind::Chat;
        o.undo.is_some()
    });
    let Some(undoable) = found else {
        return;
    };
    crate::region::clear_active();
    store::get().set_output_height(0.0);
    shared(app).lock().focused_output = Some(label.to_string());
    let undo_ms = if undoable { UNDO_MS } else { 0 };
    emit(app, label, "chat:reset", json!({ "undoMs": undo_ms }));
    send_overlay_state(app);
}

/// Puts back the conversation "Start over" replaced, if its 10 s are not
/// up and nothing new was asked since.
pub fn restore(o: &mut Output, now: u64) -> bool {
    match o.undo.take() {
        Some((chat, until)) if now <= until && o.chat.messages.is_empty() => {
            o.chat = chat;
            true
        }
        _ => false,
    }
}

/// `output:undo-reset`: restores and replays the conversation.
pub fn undo_reset(app: &AppHandle, label: &str) -> bool {
    let now = now_ms();
    let back = with(app, label, |o| restore(o, now).then(|| o.clone()));
    let Some(Some(o)) = back else {
        return false;
    };
    emit(app, label, "chat:sync", chat_payload(&o));
    send_overlay_state(app);
    true
}

/// main.js openConversation: a saved conversation reopens in its own
/// window (or the one already showing it), about the whole screen again.
pub fn open_conversation(app: &AppHandle, entry: &Value) -> bool {
    let Some(id) = entry.get("id").and_then(Value::as_str) else {
        return false;
    };
    let messages: Vec<Value> = entry
        .get("messages")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|m| {
                    let mut out = json!({ "role": m["role"], "text": m["text"] });
                    if let Some(k) = m.get("kind").filter(|k| !k.is_null()) {
                        out = json!({ "role": m["role"], "kind": k, "text": m["text"] });
                    }
                    out
                })
                .collect()
        })
        .unwrap_or_default();
    let existing = shared(app)
        .lock()
        .outputs
        .values()
        .find(|o| o.chat.id == id)
        .map(|o| o.label.clone());
    let Some(label) = existing.or_else(|| spare(app, None)) else {
        return false;
    };
    if running_in(app, &label) {
        abort_in(app, &label);
    }
    let o = with(app, &label, |o| {
        o.chat.id = id.to_string();
        o.chat.messages = messages;
        o.chat.region = None;
        o.chat.last_question.clear();
        o.user_height = 0.0;
        o.content_height = layout::DEFAULT_OUTPUT_HEIGHT;
        o.clone()
    });
    shared(app).lock().focused_output = Some(label.clone());
    if let Some(o) = o {
        if o.ready {
            emit(app, &label, "chat:sync", chat_payload(&o));
        }
    }
    show(app, &label);
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = create_spare(&a);
    });
    true
}

/// The next question opens a new window.
pub fn detach_focus(app: &AppHandle) {
    shared(app).lock().focused_output = None;
}

/// The window got focus: follow-ups now go to it, and it is in front.
pub fn touch(app: &AppHandle, label: &str) {
    let changed = {
        let st = shared(app);
        let mut s = st.lock();
        let active = s.outputs.get(label).map(|o| o.active).unwrap_or(false);
        if let Some(o) = s.outputs.get_mut(label) {
            o.touched = now_ms();
        }
        if active && s.focused_output.as_deref() != Some(label) {
            s.focused_output = Some(label.to_string());
            true
        } else {
            false
        }
    };
    if changed {
        send_overlay_state(app);
    } else {
        send_meta(app);
    }
}

/// Visible outputs, most recently touched first.
pub fn visible_order(s: &AppState) -> Vec<String> {
    if s.hidden {
        return Vec::new();
    }
    let mut open: Vec<&Output> = s.outputs.values().filter(|o| o.active).collect();
    open.sort_by_key(|o| (Reverse(o.touched), o.label.clone()));
    open.into_iter().map(|o| o.label.clone()).collect()
}

/// The next target when cycling by `dir` (1 or -1) from `current`.
pub fn next_in_cycle(order: &[String], current: Option<&str>, dir: i64) -> Option<String> {
    if order.is_empty() {
        return None;
    }
    let n = order.len() as i64;
    let i = match current.and_then(|c| order.iter().position(|l| l == c)) {
        Some(at) => (at as i64 + dir.signum()).rem_euclid(n),
        None => 0,
    };
    order.get(i as usize).cloned()
}

/// `output:cycle-target {dir}`: the target moves through the visible
/// outputs in `touched` order. Returns the new target.
pub fn cycle_target(app: &AppHandle, dir: i64) -> Option<String> {
    let next = {
        let st = shared(app);
        let mut s = st.lock();
        let order = visible_order(&s);
        let next = next_in_cycle(&order, s.focused_output.as_deref(), dir)?;
        s.focused_output = Some(next.clone());
        next
    };
    send_overlay_state(app);
    Some(next)
}

/// `chat`, `find` or `trail`.
pub fn kind_of(o: &Output) -> &'static str {
    if o.kind == OutputKind::Trail {
        "trail"
    } else if o.view == "find" {
        "find"
    } else {
        "chat"
    }
}

/// What a window with no question yet is called.
fn untitled(kind: &str) -> &'static str {
    match kind {
        "find" => "Find",
        "trail" => "Task",
        _ => "New conversation",
    }
}

/// The window's title: its first question, else what it shows.
pub fn title_of(o: &Output) -> String {
    o.chat
        .messages
        .iter()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["text"].as_str())
        .map(crate::history::first_line)
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| untitled(kind_of(o)).to_string())
}

/// K-05: `overlay:state.target` (`{label, title, kind}` of the window the
/// bar's next question continues, or null) and `visibleOutputs`.
pub fn target_fields(s: &AppState) -> (Value, usize) {
    let target = s
        .focused_output
        .as_ref()
        .and_then(|l| s.outputs.get(l))
        .filter(|o| o.active)
        .map(|o| json!({ "label": o.label, "title": title_of(o), "kind": kind_of(o) }))
        .unwrap_or(Value::Null);
    (target, visible_order(s).len())
}

/// `output:last-answer`: the text of the window's last answer, '' if none.
pub fn last_answer_of(messages: &[Value]) -> String {
    messages
        .iter()
        .rev()
        .find(|m| m["role"] == "assistant" && m["kind"] != "trail")
        .and_then(|m| m["text"].as_str())
        .unwrap_or_default()
        .to_string()
}

pub fn last_answer(app: &AppHandle, label: &str) -> String {
    let text = with(app, label, |o| last_answer_of(&o.chat.messages));
    text.unwrap_or_default()
}

/// Which way the sheet grows: away from the bar, so it never covers it.
pub fn grow_of(r: Rect, bar: Rect) -> &'static str {
    if r.y + r.height / 2.0 < bar.y + bar.height / 2.0 {
        "up"
    } else {
        "down"
    }
}

/// K-06: `output:meta {grow, front, target}` for every output. `grow` is
/// `down` or `up`; `front` is true for the most recently touched open
/// window; `target` for the window the bar's next question continues.
pub fn metas(s: &AppState, bar: Rect) -> Vec<(String, Value)> {
    let front = s
        .outputs
        .values()
        .filter(|o| o.active)
        .max_by_key(|o| o.touched)
        .map(|o| o.label.as_str());
    let target = s.focused_output.as_deref();
    s.outputs
        .values()
        .map(|o| {
            let me = Some(o.label.as_str());
            let meta = json!({
                "grow": o.rect.map_or("down", |r| grow_of(r, bar)),
                "front": o.active && front == me,
                "target": o.active && target == me,
            });
            (o.label.clone(), meta)
        })
        .collect()
}

fn sent_meta() -> &'static Mutex<HashMap<String, Value>> {
    static M: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Sends `output:meta` to each output whose values changed since the last.
pub fn send_meta(app: &AppHandle) {
    let bar = bar_rect(app);
    let all = metas(&shared(app).lock(), bar);
    for (label, meta) in all {
        let last = sent_meta().lock().insert(label.clone(), meta.clone());
        if last.as_ref() != Some(&meta) {
            emit(app, &label, "output:meta", meta);
        }
    }
}

/// The open window to close when a new one would make seven: the least
/// recently touched one with no Ask streaming into it, never `keep`.
pub fn pick_to_close(s: &AppState, keep: &str) -> Option<String> {
    let open = s.outputs.values().filter(|o| o.active).count();
    if open <= MAX_OPEN {
        return None;
    }
    // Until the Ask reports per window, its busy flag belongs to the target.
    let legacy_running = if s.queries.is_empty() && s.busy {
        s.focused_output.as_deref()
    } else {
        None
    };
    s.outputs
        .values()
        .filter(|o| o.active && o.label != keep && !s.query_running(&o.label))
        .filter(|o| legacy_running != Some(o.label.as_str()))
        .min_by_key(|o| o.touched)
        .map(|o| o.label.clone())
}

/// D-32: at most six windows. The one that goes is said on the bar
/// (`bar:note {text}`).
fn enforce_cap(app: &AppHandle, keep: &str) {
    let victim = {
        let st = shared(app);
        let s = st.lock();
        pick_to_close(&s, keep).map(|l| {
            let title = s.outputs.get(&l).map(title_of).unwrap_or_default();
            (l, title)
        })
    };
    let Some((label, title)) = victim else {
        return;
    };
    close(app, &label);
    let text = if store::get().get_save_history() {
        format!("Closed \"{title}\". It is in History.")
    } else {
        format!("Closed \"{title}\".")
    };
    emit(app, OVERLAY, "bar:note", json!({ "text": text }));
}

/// The page loaded: replay its conversation and state.
pub fn on_loaded(app: &AppHandle, label: &str) {
    let Some(o) = with(app, label, |o| {
        o.ready = true;
        o.clone()
    }) else {
        return;
    };
    send_now(app, label, "view:set", json!({ "view": o.view }));
    send_now(app, label, "chat:sync", chat_payload(&o));
    send_now(app, label, "overlay:state", overlay_state(app, Some(label)));
    sent_meta().lock().remove(label);
    send_meta(app);
}

/// The page said hello, so its listeners are in place: its state goes,
/// then what was sent to it while it loaded, in order (B-019). What was
/// held is taken before the transcript is copied, so every held event's
/// effect on the transcript is in that `chat:sync`.
pub fn on_hello(app: &AppHandle, label: &str) {
    let first = take_held(label);
    on_loaded(app, label);
    if let Some(events) = first {
        for (channel, payload) in replayable(events) {
            send_now(app, label, &channel, payload);
        }
        release_held(app, label);
    }
}

/// What a page that has just had `chat:sync` still needs of the events
/// held while it loaded, in order. State that `on_loaded` sends again goes,
/// and so does `chat:user` (the question is in the transcript). An Ask that
/// ended with `query:done` is in the transcript too, so all of its events
/// go; one that failed keeps its start and its error.
pub fn replayable(events: Vec<(String, Value)>) -> Vec<(String, Value)> {
    const RESENT: [&str; 7] = [
        "chat:sync",
        "chat:reset",
        "chat:user",
        "view:set",
        "overlay:state",
        "output:meta",
        "ui:text-scale",
    ];
    let gen_of = |p: &Value| p.get("gen").and_then(Value::as_u64);
    let ended = |name: &str| -> Vec<Option<u64>> {
        events
            .iter()
            .filter(|(c, _)| c == name)
            .map(|(_, p)| gen_of(p))
            .collect()
    };
    let done = ended("query:done");
    let failed = ended("query:error");
    events
        .into_iter()
        .filter(|(c, p)| {
            if RESENT.contains(&c.as_str()) {
                return false;
            }
            if !c.starts_with("query:") {
                return true;
            }
            let gen = gen_of(p);
            if done.contains(&gen) {
                return false;
            }
            !failed.contains(&gen) || c == "query:start" || c == "query:error"
        })
        .collect()
}

/// B-098: a window that has not been placed yet starts at the height the
/// person last dragged one to; 0 follows the transcript.
pub fn seeded_height(user: f64, placed: bool, stored: i64) -> f64 {
    if user > 0.0 || placed {
        user
    } else {
        stored.max(0) as f64
    }
}

/// `chat:sync` for one window: the whole transcript, as main.js sent it.
pub fn chat_payload(o: &Output) -> Value {
    json!({ "id": o.chat.id, "messages": o.chat.messages })
}

/// A drag (or `window:nudge`) put the window at `r`: it stays there, and a
/// new height is the person's.
pub fn keep_user_rect(app: &AppHandle, label: &str, r: Rect, resized: bool) {
    with(app, label, |o| {
        o.rect = Some(r);
        if resized {
            o.user_height = r.height;
        }
    });
    if resized {
        store::get().set_output_width(r.width);
        store::get().set_output_height(r.height);
        emit(app, label, "overlay:state", overlay_state(app, Some(label)));
    } else {
        // Relative to the display it was dropped on (B-025).
        let wa = hwnd(app, label).map_or_else(super::bar_area, own_area);
        let pos = layout::pos_from_rect(wa, r);
        store::get().set_output_pos(&pos.to_value());
    }
    send_meta(app);
}

/// A drag on the window's edge ended: that height is now the user's. A
/// maximized window keeps the rect it will go back to (B-024).
pub fn on_user_resize(app: &AppHandle, label: &str) {
    if app_driven(app, label) {
        return;
    }
    let Some(h) = hwnd(app, label) else {
        return;
    };
    if !flags::is_maximized(h) {
        keep_user_rect(app, label, motion::glass(h), true);
    }
}

/// A title-bar drag ended: the window stays where it was dropped.
pub fn on_user_move(app: &AppHandle, label: &str) {
    if app_driven(app, label) {
        return;
    }
    let Some(h) = hwnd(app, label) else {
        return;
    };
    if !flags::is_maximized(h) {
        keep_user_rect(app, label, motion::glass(h), false);
    }
}

/// B-025: the displays changed. Every open output stays inside the work
/// area of the display it is on now; one mid-drag is the person's to place.
pub fn clamp_all(app: &AppHandle) {
    if flags::mouse_down() {
        return;
    }
    let labels: Vec<String> = shared(app)
        .lock()
        .outputs
        .values()
        .filter(|o| o.active)
        .map(|o| o.label.clone())
        .collect();
    for l in labels {
        let Some(h) = hwnd(app, &l) else {
            continue;
        };
        if flags::is_maximized(h) {
            continue;
        }
        let r = motion::glass(h);
        let to = layout::clamp_to(own_area(h), r);
        if to != r {
            mark_app(&l);
            motion::stop(h);
            motion::set_glass(h, to);
            with(app, &l, |o| o.rect = Some(to));
        }
    }
}

/// B-090: the browser process died and took this window's page with it.
/// The same label gets a new window where the old one was. What is sent
/// meanwhile waits for the new page's hello (`windows::on_destroyed` held
/// it), which sends the conversation again.
pub fn rebuild(app: &AppHandle, label: &str) {
    let Some(o) = with(app, label, |o| {
        o.ready = false;
        o.clone()
    }) else {
        drop_held(label);
        return;
    };
    let at = o.rect.unwrap_or_else(|| bar_rect(app));
    let outer = stage::inflate(at, stage::MARGIN);
    if let Err(e) = create_page(app, label, PAGE, outer, meta()) {
        crate::crash::write("renderer", &format!("{label} not rebuilt: {e}"));
        drop_held(label);
        on_destroyed(app, label);
        return;
    }
    if o.active {
        show(app, label);
    }
}

pub fn on_destroyed(app: &AppHandle, label: &str) {
    {
        let st = shared(app);
        let mut s = st.lock();
        s.outputs.remove(label);
        s.query_forget(label);
        if s.focused_output.as_deref() == Some(label) {
            s.focused_output = None;
        }
    }
    sent_meta().lock().remove(label);
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = create_spare(&a);
        send_overlay_state(&a);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(label: &str, active: bool, touched: u64) -> Output {
        Output {
            label: label.into(),
            active,
            touched,
            view: "chat".into(),
            ..Output::default()
        }
    }

    fn with_chat(mut o: Output, question: &str) -> Output {
        o.chat
            .messages
            .push(json!({ "role": "user", "text": question }));
        o
    }

    fn state(list: Vec<Output>) -> AppState {
        let mut s = AppState::default();
        for o in list {
            s.outputs.insert(o.label.clone(), o);
        }
        s
    }

    #[test]
    fn output_labels() {
        assert!(is_output("output-1"));
        assert!(is_output("output-12"));
        assert!(!is_output("overlay"));
    }

    #[test]
    fn chat_sync_sends_the_whole_transcript() {
        let mut o = Output::default();
        for i in 0..40 {
            o.chat
                .messages
                .push(json!({ "role": "user", "text": i.to_string() }));
        }
        let p = chat_payload(&o);
        let m = p["messages"].as_array().unwrap();
        assert_eq!(m.len(), 40);
        assert_eq!(m.last().unwrap()["text"], "39");
    }

    #[test]
    fn fresh_never_returns_the_focused_chat() {
        // The focused window was emptied and hidden: idle, but the bar is
        // still continuing it.
        let mut s = state(vec![out("output-1", false, 9), out("output-2", false, 1)]);
        s.focused_output = Some("output-1".into());
        assert_eq!(
            pick_spare(&s.outputs, s.focused_output.as_deref()),
            Some("output-2".into())
        );
        // A loaded spare goes first.
        s.outputs.get_mut("output-2").unwrap().ready = false;
        s.outputs.insert("output-3".into(), {
            let mut o = out("output-3", false, 0);
            o.ready = true;
            o
        });
        assert_eq!(
            pick_spare(&s.outputs, Some("output-1")),
            Some("output-3".into())
        );
        // Open windows and windows with a conversation are never spares.
        let busy = state(vec![
            out("output-1", true, 1),
            with_chat(out("output-2", false, 1), "hi"),
        ]);
        assert_eq!(pick_spare(&busy.outputs, None), None);
    }

    #[test]
    fn close_of_other_window_keeps_running_query() {
        let mut s = state(vec![out("output-1", true, 1), out("output-2", true, 2)]);
        s.query_started("output-1", 7);
        s.focused_output = Some("output-2".into());
        // Closing output-2 forgets only its own entry.
        assert!(!s.query_running("output-2"));
        s.outputs.remove("output-2");
        s.query_forget("output-2");
        assert!(s.query_running("output-1"));
        assert!(s.busy);
    }

    #[test]
    fn titles_and_kinds() {
        let mut o = out("output-1", true, 1);
        assert_eq!(title_of(&o), "New conversation");
        o.view = "find".into();
        assert_eq!((kind_of(&o), title_of(&o).as_str()), ("find", "Find"));
        o.kind = OutputKind::Trail;
        assert_eq!((kind_of(&o), title_of(&o).as_str()), ("trail", "Task"));
        let o = with_chat(out("output-2", true, 1), "  open   notepad ");
        assert_eq!(title_of(&o), "open notepad");
    }

    #[test]
    fn cycling_follows_touch_order_and_wraps() {
        let s = state(vec![
            out("output-1", true, 30),
            out("output-2", true, 10),
            out("output-3", true, 20),
            out("output-4", false, 99),
        ]);
        let order = visible_order(&s);
        assert_eq!(order, vec!["output-1", "output-3", "output-2"]);
        let next = |c: &str, d: i64| next_in_cycle(&order, Some(c), d).unwrap();
        assert_eq!(next("output-1", 1), "output-3");
        assert_eq!(next("output-2", 1), "output-1");
        assert_eq!(next("output-1", -1), "output-2");
        assert_eq!(next_in_cycle(&order, None, 1).unwrap(), "output-1");
        assert_eq!(next_in_cycle(&[], Some("output-1"), 1), None);
    }

    #[test]
    fn front_follows_touch() {
        let mut s = state(vec![out("output-1", true, 5), out("output-2", true, 9)]);
        s.focused_output = Some("output-1".into());
        s.outputs.get_mut("output-1").unwrap().rect = Some(Rect::new(0.0, 0.0, 400.0, 100.0));
        let bar = Rect::new(0.0, 300.0, 400.0, 76.0);
        let m: BTreeMap<String, Value> = metas(&s, bar).into_iter().collect();
        assert_eq!(m["output-2"]["front"], true);
        assert_eq!(m["output-1"]["front"], false);
        assert_eq!(m["output-1"]["target"], true);
        assert_eq!(m["output-2"]["target"], false);
        // Above the bar: the sheet grows up, away from it.
        assert_eq!(m["output-1"]["grow"], "up");
        assert_eq!(m["output-2"]["grow"], "down");
        s.outputs.get_mut("output-1").unwrap().touched = 20;
        let m: BTreeMap<String, Value> = metas(&s, bar).into_iter().collect();
        assert_eq!(m["output-1"]["front"], true);
        assert_eq!(m["output-2"]["front"], false);
    }

    #[test]
    fn seventh_closes_least_recent_idle() {
        let mut list: Vec<Output> = (1..=7)
            .map(|i| out(&format!("output-{i}"), true, i))
            .collect();
        list.push(out("output-8", false, 0));
        let mut s = state(list);
        // output-1 is the oldest but is still answering.
        s.query_started("output-1", 1);
        assert_eq!(pick_to_close(&s, "output-7"), Some("output-2".into()));
        // Six open: nothing closes.
        s.outputs.get_mut("output-3").unwrap().active = false;
        assert_eq!(pick_to_close(&s, "output-7"), None);
    }

    #[test]
    fn close_all_and_last_answer() {
        let messages = vec![
            json!({ "role": "user", "text": "q1" }),
            json!({ "role": "assistant", "text": "a1" }),
            json!({ "role": "user", "text": "q2" }),
            json!({ "role": "assistant", "text": "a2" }),
            json!({ "role": "assistant", "kind": "trail", "text": "steps" }),
        ];
        assert_eq!(last_answer_of(&messages), "a2");
        assert_eq!(last_answer_of(&messages[..1]), "");
    }

    #[test]
    fn new_window_uses_stored_height() {
        assert_eq!(seeded_height(0.0, false, 480), 480.0);
        assert_eq!(seeded_height(0.0, false, 0), 0.0);
        // A height of its own wins.
        assert_eq!(seeded_height(300.0, false, 480), 300.0);
        // A window already placed (and since Start over: 0) keeps
        // following its transcript.
        assert_eq!(seeded_height(0.0, true, 480), 0.0);
    }

    #[test]
    fn undo_reset_within_10s() {
        let mut o = with_chat(out("output-1", true, 1), "old question");
        let old = std::mem::replace(&mut o.chat, Chat::fresh());
        o.undo = Some((old, 1_000 + UNDO_MS));
        assert!(restore(&mut o, 1_000 + UNDO_MS));
        assert_eq!(o.chat.messages.len(), 1);
        assert!(o.undo.is_none());
        // Too late.
        let old = std::mem::replace(&mut o.chat, Chat::fresh());
        o.undo = Some((old, 1_000));
        assert!(!restore(&mut o, 1_001));
        assert!(o.chat.messages.is_empty());
        // Something new was asked since: the new conversation stays.
        let mut o = with_chat(out("output-1", true, 1), "old");
        let old = std::mem::replace(&mut o.chat, Chat::fresh());
        o.undo = Some((old, u64::MAX));
        o.chat
            .messages
            .push(json!({ "role": "user", "text": "new" }));
        assert!(!restore(&mut o, 0));
        assert_eq!(o.chat.messages[0]["text"], "new");
    }
}
