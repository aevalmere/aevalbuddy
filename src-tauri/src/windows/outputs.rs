//! Free-floating output windows (REQUESTS.md 8-11, PARITY NEW-01..07).
//!
//! Each answer lives in its own `output-<n>` window with its own
//! conversation. Rust places a new one 12 px off the bar's glass and shows
//! it; the page globs itself out of the bar (it asks `output:bar` where the
//! bar is) and plays the close glob before `output:close` destroys it. One
//! hidden, loaded window is always kept warm so the next answer opens with
//! no page load.
//!
//! The bar's follow-ups go to the focused output; the bar's `chat:new`
//! detaches it so the next question opens a new window. ARCHITECTURE.md
//! sections 9 and 10.

use serde_json::{json, Value};
use tauri::AppHandle;

use super::layout::{self, Anchor};
use super::{
    app_driven, bar_rect, create_page, emit, flags, hwnd, mark_app, motion, overlay_state, screen,
    send_overlay_state, stage, window, Meta, Resize,
};
use crate::state::{now_ms, shared, Chat, Output, OutputKind, Rect};
use crate::store;

pub const PAGE: &str = "renderer/output.html";

pub fn is_output(label: &str) -> bool {
    label.starts_with("output-")
}

fn meta() -> Meta {
    Meta {
        margin: stage::MARGIN,
        resize: Resize::All,
        min: (store::MIN_W as f64, layout::OUTPUT_MIN_HEIGHT),
        max: (store::MAX_W as f64, 0.0),
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
    let at = bar_rect(app);
    let outer = stage::inflate(at, stage::MARGIN);
    match create_page(app, &label, PAGE, outer, meta()) {
        Ok(_) => Ok(label),
        Err(e) => {
            shared(app).lock().outputs.remove(&label);
            Err(e)
        }
    }
}

/// Keeps one hidden, unused output window around.
pub fn create_spare(app: &AppHandle) -> tauri::Result<()> {
    let have = shared(app)
        .lock()
        .outputs
        .values()
        .any(|o| !o.active && o.chat.messages.is_empty() && o.kind == OutputKind::Chat);
    if !have {
        create(app)?;
    }
    Ok(())
}

fn spare(app: &AppHandle) -> Option<String> {
    let found = shared(app)
        .lock()
        .outputs
        .values()
        .find(|o| !o.active && o.chat.messages.is_empty() && o.kind == OutputKind::Chat)
        .map(|o| o.label.clone());
    found.or_else(|| create(app).ok())
}

/// The output the next answer goes to: the focused one if it is still open,
/// else a fresh window (which becomes focused). Agent b calls this when a
/// question is submitted.
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
    let label = spare(app)?;
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

/// The focused output's label, if any.
pub fn focused(app: &AppHandle) -> Option<String> {
    shared(app).lock().focused_output.clone()
}

/// Runs `f` on an output's state (agent b: messages, region, busy text).
pub fn with<R>(app: &AppHandle, label: &str, f: impl FnOnce(&mut Output) -> R) -> Option<R> {
    let st = shared(app);
    let mut s = st.lock();
    s.outputs.get_mut(label).map(f)
}

fn size_for(o: &Output) -> (f64, f64) {
    let wa = screen::work_area();
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
    let Some(o) = shared(app).lock().outputs.get(label).cloned() else {
        return;
    };
    if shared(app).lock().hidden {
        with(app, label, |o| o.active = true);
        return;
    }
    let size = size_for(&o);
    let wa = screen::work_area();
    let bar = bar_rect(app);
    let to = match o.rect {
        Some(r) => Rect::new(r.x, r.y, size.0, size.1),
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
    });
    flags::set_meta(h, |m| m.resize = Resize::All);
    mark_app(label);
    if !flags::is_visible(h) {
        motion::stop(h);
        motion::set_glass(h, to);
        flags::show_inactive(h);
        flags::pin_on_top(h);
    } else {
        motion::animate(h, None, to, layout::T_OUT_GROW, None);
    }
    emit(app, label, "overlay:state", overlay_state(app, Some(label)));
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
    let (_, want_h) = size_for(&o);
    if (have.height - want_h).abs() < 1.0 {
        return;
    }
    let wa = screen::work_area();
    let bar = bar_rect(app);
    let grow_up = have.y + have.height / 2.0 < bar.y + bar.height / 2.0;
    let mut to = Rect::new(have.x, have.y, have.width, want_h);
    if grow_up {
        to.y = have.y + have.height - want_h;
    }
    to.y = layout::clamp(to.y, wa.y, (wa.y + wa.height - want_h).max(wa.y));
    with(app, label, |o| o.rect = Some(to));
    mark_app(label);
    motion::animate(h, None, to, layout::T_OUT_GROW, None);
}

/// Hides every output (the bar collapsed). Conversations stay; the next
/// open brings back the ones that have content.
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

/// `output:close` (ARCHITECTURE.md 10.1): the page has played its close
/// glob, so the window goes at once with its conversation (history already
/// has it). A query streaming into it stops; follow-ups go to the most
/// recently focused output still open.
pub fn close(app: &AppHandle, label: &str) {
    let busy_here = {
        let st = shared(app);
        let mut s = st.lock();
        let busy_here = s.busy && s.focused_output.as_deref() == Some(label);
        s.outputs.remove(label);
        if s.focused_output.as_deref() == Some(label) {
            s.focused_output = s
                .outputs
                .values()
                .filter(|o| o.active)
                .max_by_key(|o| o.touched)
                .map(|o| o.label.clone());
        }
        busy_here
    };
    if busy_here && !shared(app).call(app, |h| h.abort_query.as_ref()) {
        shared(app).lock().busy = false;
    }
    if let Some(w) = window(app, label) {
        if let Some(h) = flags::hwnd_of(&w) {
            motion::stop(h);
        }
        let _ = w.destroy();
    }
    send_overlay_state(app);
}

/// `chat:new` from an output window: that window starts over; no new
/// window opens (ARCHITECTURE.md 10.3).
pub fn reset(app: &AppHandle, label: &str) {
    let busy_here = {
        let st = shared(app);
        let s = st.lock();
        s.busy && s.focused_output.as_deref() == Some(label)
    };
    if busy_here && !shared(app).call(app, |h| h.abort_query.as_ref()) {
        shared(app).lock().busy = false;
    }
    let found = with(app, label, |o| {
        o.chat = Chat::fresh();
        o.user_height = 0.0;
        o.content_height = layout::DEFAULT_OUTPUT_HEIGHT;
        o.kind = OutputKind::Chat;
    });
    if found.is_none() {
        return;
    }
    store::get().set_output_height(0.0);
    shared(app).lock().focused_output = Some(label.to_string());
    emit(app, label, "chat:reset", json!({}));
    send_overlay_state(app);
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
    let Some(label) = existing.or_else(|| spare(app)) else {
        return false;
    };
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

/// The window got focus: follow-ups now go to it.
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
    }
}

/// The page loaded: replay its conversation and state.
pub fn on_loaded(app: &AppHandle, label: &str) {
    let Some(o) = with(app, label, |o| {
        o.ready = true;
        o.clone()
    }) else {
        return;
    };
    emit(app, label, "view:set", json!({ "view": o.view }));
    emit(app, label, "chat:sync", chat_payload(&o));
    emit(app, label, "overlay:state", overlay_state(app, Some(label)));
}

/// `chat:sync` for one window: the whole transcript, as main.js sent it.
pub fn chat_payload(o: &Output) -> Value {
    json!({ "id": o.chat.id, "messages": o.chat.messages })
}

/// A drag on the window's edge ended: that height is now the user's.
pub fn on_user_resize(app: &AppHandle, label: &str) {
    if app_driven(app, label) {
        return;
    }
    let Some(h) = hwnd(app, label) else {
        return;
    };
    let r = motion::glass(h);
    with(app, label, |o| {
        o.user_height = r.height;
        o.rect = Some(r);
    });
    store::get().set_output_width(r.width);
    store::get().set_output_height(r.height);
    emit(app, label, "overlay:state", overlay_state(app, Some(label)));
}

/// A title-bar drag ended: the window stays where it was dropped.
pub fn on_user_move(app: &AppHandle, label: &str) {
    if app_driven(app, label) {
        return;
    }
    let Some(h) = hwnd(app, label) else {
        return;
    };
    let r = motion::glass(h);
    with(app, label, |o| o.rect = Some(r));
    let pos = layout::pos_from_rect(screen::work_area(), r);
    store::get().set_output_pos(&pos.to_value());
}

pub fn on_destroyed(app: &AppHandle, label: &str) {
    {
        let st = shared(app);
        let mut s = st.lock();
        s.outputs.remove(label);
        if s.focused_output.as_deref() == Some(label) {
            s.focused_output = None;
        }
    }
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = create_spare(&a);
        send_overlay_state(&a);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
