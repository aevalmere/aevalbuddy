//! Watching the coding agents on this PC (Claude Code, Codex, Gemini CLI,
//! Copilot CLI, Cursor, Windsurf, OpenCode): the PC stays awake while any of
//! them works, and when one finishes or needs you a card comes out of the
//! bar saying which. Clicking it, or Alt+Tab while it shows, jumps to that
//! agent's window. Ported from Aevalsistant.
//!
//! Nothing here runs until the person turns it on: connecting agents edits
//! their config files (`specs`), so it is theirs to decide. Hook calls
//! arrive from `hook` over a named pipe; `sessions` keeps them; this
//! file wires them to the power request, the card window and Settings.

mod chats;
pub mod hook;
pub mod ipc;
mod native;
mod power;
pub mod sessions;
pub mod specs;

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use self::sessions::{CardKind, CardRequest, HookEvent, Session, SessionBook, TranscriptProbe};
use crate::capture::{self, PxRect};
use crate::desktop::win;
use crate::state::Rect;

/// The card's window.
pub const LABEL: &str = "agents";
/// The card is 420 wide; the window adds room for its shadow.
const CARD_WIDTH: f64 = 452.0;
/// Between the bar and the card.
const GAP: f64 = 2.0;
/// The countdown runs only while the person is at the keyboard.
const AWAY_AFTER_MS: u64 = 20_000;
const TICK_MS: u64 = 250;
/// Rows under the head; the rest are counted.
const MAX_ROWS: usize = 6;
/// Sessions the tray menu lists; the rest are counted.
const TRAY_ROWS: usize = 8;
const TAIL_BYTES: u64 = 256 * 1024;
/// How long after the last agent stops a PC with its lid shut goes to sleep.
const LID_SLEEP_AFTER_MS: u64 = 60_000;

/// `agents` in aevalbuddy.json.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    /// Watch coding agents at all. Off until the person turns it on.
    pub enabled: bool,
    /// Agent ids switched off one by one.
    pub agents_off: Vec<String>,
    /// Also watch the Claude and ChatGPT desktop apps (`chats`).
    pub chats: bool,
    pub keep_awake: bool,
    pub display_on: bool,
    pub lid_awake: bool,
    /// `scheme|ac|dc` while the lid override is in effect, so a crash is
    /// repaired on the next start.
    pub lid_saved: String,
    pub notify_done: bool,
    pub notify_background_done: bool,
    pub notify_needs_you: bool,
    pub notify_background_needs_you: bool,
    pub sound: bool,
    pub expand_on_hover: bool,
    pub alt_tab: bool,
    /// 0 short, 1 normal, 2 long.
    pub card_time: u8,
    /// List subagents and child sessions under their session.
    pub show_children: bool,
    /// The one-time offer to turn this on has been shown.
    pub offered: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            enabled: false,
            agents_off: Vec::new(),
            chats: false,
            keep_awake: true,
            display_on: false,
            lid_awake: false,
            lid_saved: String::new(),
            notify_done: true,
            notify_background_done: false,
            notify_needs_you: true,
            notify_background_needs_you: true,
            sound: true,
            expand_on_hover: true,
            alt_tab: true,
            card_time: 1,
            show_children: true,
            offered: false,
        }
    }
}

impl Prefs {
    /// Whether a card should show for this request. Background cards also
    /// need their parent setting on.
    pub fn wants(&self, r: &CardRequest) -> bool {
        match r.kind {
            CardKind::Info => true,
            CardKind::NeedsYou => {
                self.notify_needs_you && (!r.background || self.notify_background_needs_you)
            }
            CardKind::Done => self.notify_done && (!r.background || self.notify_background_done),
        }
    }

    /// How long a card stays up while you are at the keyboard.
    pub fn card_ms(&self, kind: CardKind) -> u64 {
        let normal: u64 = match kind {
            CardKind::NeedsYou => 10_000,
            CardKind::Info => 5_000,
            CardKind::Done => 8_000,
        };
        match self.card_time {
            0 => normal * 6 / 10,
            2 => normal * 7 / 4,
            _ => normal,
        }
    }
}

/// `now` with the fields of `patch` Settings may change.
fn merged(now: &Prefs, patch: &Value) -> Prefs {
    let (Ok(Value::Object(mut all)), Some(patch)) = (serde_json::to_value(now), patch.as_object())
    else {
        return now.clone();
    };
    for (k, v) in patch {
        // These two are the app's own bookkeeping.
        if k != "lidSaved" && k != "offered" && all.contains_key(k) {
            all.insert(k.clone(), v.clone());
        }
    }
    serde_json::from_value(Value::Object(all)).unwrap_or_else(|_| now.clone())
}

pub fn prefs() -> Prefs {
    crate::store::get()
        .raw("agents")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn save_prefs(p: &Prefs) {
    crate::store::get().set_raw("agents", json!(p));
}

/// A button on a card the app itself shows.
type Action = (&'static str, &'static str);

struct Showing {
    card: CardRequest,
    actions: Vec<Action>,
    elapsed: u64,
    total: u64,
    /// The page has measured it and the window is up.
    placed: bool,
}

struct State {
    book: SessionBook,
    awake: power::Awake,
    showing: Option<Showing>,
    hover: bool,
    /// `Prefs::alt_tab`, kept here so the keyboard hook never reads the
    /// store.
    alt_tab: bool,
    /// Why an agent's hooks could not be written, by agent id.
    errors: HashMap<&'static str, String>,
    /// The session rows the tray menu was last asked to show.
    tray: (Vec<TrayRow>, usize),
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| {
        Mutex::new(State {
            book: SessionBook::default(),
            awake: power::Awake::new(),
            showing: None,
            hover: false,
            alt_tab: true,
            errors: HashMap::new(),
            tray: (Vec::new(), 0),
        })
    })
}

static APP: OnceLock<AppHandle> = OnceLock::new();

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A test profile or a CI check never touches the real agents' hooks, and
/// never shares the pipe with the real app.
fn out_of_bounds() -> bool {
    [
        "AEVAL_USERDATA",
        "AEVAL_SMOKE",
        "AEVAL_SHOT",
        "AEVAL_CAPTURE_CHECK",
        "AEVAL_DESKTOP_CHECK",
    ]
    .iter()
    .any(|k| std::env::var_os(k).is_some())
}

/// Called once from the app's setup.
pub fn setup(app: &AppHandle) {
    if out_of_bounds() || APP.set(app.clone()).is_err() {
        return;
    }
    let p = prefs();
    state().lock().alt_tab = p.alt_tab;
    // A lid override left behind by a crash or a power cut.
    if !p.lid_saved.is_empty() && power::restore_lid(&p.lid_saved) {
        save_prefs(&Prefs {
            lid_saved: String::new(),
            ..p.clone()
        });
    }
    if power::has_lid() {
        power::watch_lid();
    }
    tauri::async_runtime::spawn(serve());
    let _ = std::thread::Builder::new()
        .name("agents-tick".into())
        .spawn(tick_loop);
    chats::set(p.enabled && p.chats);
    if p.enabled {
        sync_soon();
    } else if !p.offered {
        offer_later();
    }
}

// -------------------------------------------------------------------- pipe

fn pipe(first: bool) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    tokio::net::windows::named_pipe::ServerOptions::new()
        .first_pipe_instance(first)
        .create(hook::pipe_name())
}

/// Takes hook calls for the life of the app. Each connection is one
/// envelope, read on its own task, so a slow client never holds the next.
async fn serve() {
    let mut server = match pipe(true) {
        Ok(s) => s,
        Err(e) => {
            crate::crash::write("agents-pipe", &e.to_string());
            return;
        }
    };
    loop {
        let connected = server.connect().await.is_ok();
        let next = loop {
            match pipe(false) {
                Ok(s) => break s,
                Err(_) => tokio::time::sleep(Duration::from_secs(1)).await,
            }
        };
        let client = std::mem::replace(&mut server, next);
        if connected {
            tauri::async_runtime::spawn(read_one(client));
        }
    }
}

async fn read_one(mut client: tokio::net::windows::named_pipe::NamedPipeServer) {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::new();
    let read = async {
        let mut chunk = [0u8; 16 * 1024];
        // The client reads its stdin before it writes, then closes.
        while let Ok(n) = client.read(&mut chunk).await {
            if n == 0 || buf.len() > (8 << 20) {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
    };
    let _ = tokio::time::timeout(Duration::from_secs(10), read).await;
    let text = String::from_utf8_lossy(&buf).into_owned();
    let _ = tauri::async_runtime::spawn_blocking(move || {
        let _ = crate::crash::guard("agents-event", || on_envelope(&text));
    })
    .await;
}

fn on_envelope(text: &str) {
    if let Some(e) = HookEvent::from_envelope(text) {
        on_event(&e);
    }
}

/// One hook event: the session book, the power request, and a card when the
/// event calls for one.
fn on_event(e: &HookEvent) {
    let p = prefs();
    // A chat sample that was under way when chat watching was switched off.
    if !p.enabled || (e.agent == "chat" && !p.chats) {
        return;
    }
    let request = state().lock().book.apply(e, now_ms());
    refresh_power(&p);
    match request {
        Some(mut card) if p.wants(&card) => {
            if card.wants_snippet {
                card.detail = last_reply(card.session_id.as_deref());
            }
            show_card(card, Vec::new(), &p);
        }
        _ => refresh_card(&p),
    }
    sync_tray();
}

// ------------------------------------------------------------------- chats

/// Whether a coding agent's hooks already report a busy session in this
/// window, so the chat watcher does not list it a second time.
fn hook_busy(hwnd: i64) -> bool {
    state().lock().book.hook_busy_in(hwnd)
}

/// Chat watching ended, by the setting or by itself: its sessions go, and
/// Settings hears why.
fn chats_stopped() {
    let had_any = state().lock().book.forget_agent("chat");
    if had_any {
        let p = prefs();
        refresh_power(&p);
        refresh_card(&p);
        sync_tray();
    }
    if let Some(app) = APP.get() {
        crate::windows::emit(app, crate::windows::SETTINGS, "agents:state", snapshot());
    }
}

// -------------------------------------------------------------- transcript

fn modified_ms(path: &str) -> Option<u64> {
    let at = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(at.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64)
}

/// The whole lines in the last 256 KB of a transcript.
fn tail(path: &str) -> Vec<String> {
    let read = || -> std::io::Result<(Vec<u8>, u64)> {
        let mut f = std::fs::File::open(path)?;
        let len = f.metadata()?.len();
        let start = len.saturating_sub(TAIL_BYTES);
        f.seek(SeekFrom::Start(start))?;
        let mut buf = Vec::new();
        f.take(TAIL_BYTES).read_to_end(&mut buf)?;
        Ok((buf, start))
    };
    match read() {
        Ok((buf, start)) => sessions::tail_lines(&buf, start),
        Err(_) => Vec::new(),
    }
}

/// What a session last said, for a card whose hook did not carry it.
fn last_reply(session: Option<&str>) -> String {
    let path = session
        .and_then(|id| state().lock().book.get(id).map(|s| s.transcript.clone()))
        .unwrap_or_default();
    let reply = if path.is_empty() {
        String::new()
    } else {
        sessions::clean_snippet(&sessions::last_assistant_text(&tail(&path)))
    };
    if reply.is_empty() {
        "Ready for your next prompt.".to_string()
    } else {
        reply
    }
}

fn probe(s: &Session) -> Option<TranscriptProbe> {
    if s.agent != "claude" || s.transcript.is_empty() {
        return None;
    }
    let modified = modified_ms(&s.transcript)?;
    // The tail is read only when the file has been written since last time.
    let ends_in_interrupt =
        modified > s.transcript_seen && sessions::ends_in_interrupt(&tail(&s.transcript));
    Some(TranscriptProbe {
        modified,
        ends_in_interrupt,
    })
}

// ------------------------------------------------------------------- power

/// The power request and the lid override follow whether anything is busy.
fn refresh_power(p: &Prefs) {
    let busy = {
        let mut st = state().lock();
        let busy = p.enabled && st.book.busy_count() > 0;
        let keep = busy && p.keep_awake;
        st.awake.set(keep, keep && p.display_on);
        busy
    };
    let want_lid = busy && p.keep_awake && p.lid_awake;
    if want_lid {
        // Work again: a sleep that was waiting for its minute is off.
        power::cancel_lid_sleep();
    }
    if want_lid && p.lid_saved.is_empty() {
        if !power::has_lid() {
            return;
        }
        match power::override_lid() {
            Ok(saved) => save_prefs(&Prefs {
                lid_saved: saved,
                ..prefs()
            }),
            Err(why) => crate::crash::write("agents-lid", &why),
        }
    } else if !want_lid && !p.lid_saved.is_empty() && power::restore_lid(&p.lid_saved) {
        save_prefs(&Prefs {
            lid_saved: String::new(),
            ..prefs()
        });
        // The plan's own lid action only runs when the lid next moves. With
        // the lid already shut, the app carries it out a minute from now.
        let action = power::saved_action(&p.lid_saved, power::on_battery());
        if let Some(how) = power::lid_plan(action, busy, power::lid_shut()) {
            sleep_later(how);
        }
    }
}

/// A minute after the last agent stopped with the lid shut: the sleep or
/// hibernate that closing the lid would have been. Checked again when the
/// minute is up, since the lid may have opened or an agent started.
fn sleep_later(how: power::LidSleep) {
    let token = power::plan_lid_sleep();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(LID_SLEEP_AFTER_MS));
        let _ = crate::crash::guard("agents-lid-sleep", || {
            let p = prefs();
            let check = power::LidCheck {
                on: p.enabled && p.keep_awake && p.lid_awake,
                busy: state().lock().book.busy_count() > 0,
                shut: power::lid_shut(),
                planned: power::lid_sleep_planned(token),
                in_use: native::idle_ms() < LID_SLEEP_AFTER_MS,
            };
            if power::lid_sleep_due(check) && !power::suspend(how) {
                crate::crash::write("agents-lid", "Windows refused to sleep.");
            }
        });
    });
}

// -------------------------------------------------------------------- card

/// The rows under the card's head, and how many more there are.
fn rows(book: &SessionBook, head: Option<&str>, nest: bool, now: u64) -> (Vec<Value>, usize) {
    let all = book.tree(head, nest);
    let more = all.len().saturating_sub(MAX_ROWS);
    let list = all
        .iter()
        .take(MAX_ROWS)
        .filter_map(|row| {
            let s = book.get(&row.session_id)?;
            let sub = row
                .subagent_id
                .as_ref()
                .and_then(|id| s.subagents.iter().find(|(k, _)| k == id));
            Some(match sub {
                Some((id, a)) => json!({
                    "session": s.id,
                    "subagent": id,
                    "depth": row.depth,
                    "name": Some(a.kind.as_str()).filter(|k| !k.is_empty()).unwrap_or("Subagent"),
                    "place": "",
                    "state": "working",
                    "text": sessions::subagent_text(a, now),
                    "canJump": false,
                }),
                None => json!({
                    "session": s.id,
                    "depth": row.depth,
                    "name": s.project(),
                    "place": s.place(),
                    "state": sessions::state_of(s).as_str(),
                    "text": sessions::status_text(s, now),
                    "canJump": native::is_window(s.hwnd),
                }),
            })
        })
        .collect();
    (list, more)
}

/// `agents:card`: everything the page draws.
fn payload(st: &State, p: &Prefs, from: &str) -> Value {
    let Some(showing) = &st.showing else {
        return Value::Null;
    };
    let c = &showing.card;
    let (list, more) = rows(&st.book, c.session_id.as_deref(), p.show_children, now_ms());
    let can_jump = native::is_window(c.hwnd);
    let actions: Vec<Value> = showing
        .actions
        .iter()
        .map(|(id, label)| json!({ "id": id, "label": label }))
        .collect();
    json!({
        "kind": c.kind.as_str(),
        "title": c.title,
        "place": c.host,
        "detail": c.detail,
        "session": c.session_id,
        "canJump": can_jump,
        "altTab": p.alt_tab && can_jump,
        "expand": p.expand_on_hover,
        "rows": list,
        "more": more,
        "actions": actions,
        "from": from,
    })
}

fn window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window(LABEL) {
        return Some(w);
    }
    let built =
        WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("renderer/agents.html".into()))
            .title("AevalBuddy agents")
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
            .build();
    match built {
        Ok(w) => {
            let raw = capture::hwnd_of(&w);
            // Never takes the keyboard, and stays out of screenshots.
            win::tool_window(raw, true);
            capture::exclude_window(raw);
            crate::crash::guard_webview(&w, |app, label| {
                if let Some(w) = app.get_webview_window(label) {
                    let _ = w.destroy();
                }
            });
            Some(w)
        }
        Err(e) => {
            crate::crash::write("agents-card", &e.to_string());
            None
        }
    }
}

/// Puts `card` up, in place of any card showing now.
fn show_card(card: CardRequest, actions: Vec<Action>, p: &Prefs) {
    let Some(app) = APP.get() else { return };
    let kind = card.kind;
    let total = if actions.is_empty() {
        p.card_ms(kind)
    } else {
        // A question from the app waits longer for its answer.
        20_000
    };
    {
        let mut st = state().lock();
        st.hover = false;
        st.showing = Some(Showing {
            card,
            actions,
            elapsed: 0,
            total,
            placed: false,
        });
    }
    if p.sound && kind != CardKind::Info && !native::do_not_disturb() {
        native::chime(kind == CardKind::NeedsYou);
    }
    let a = app.clone();
    let p = p.clone();
    let _ = app.run_on_main_thread(move || {
        let _ = crate::crash::guard("agents-show", || {
            if window(&a).is_some() {
                send_card(&a, &p);
            }
        });
    });
}

/// Sends the card showing now to the page (which answers with its height).
fn send_card(app: &AppHandle, p: &Prefs) {
    let (from, _) = spot(app, 0.0);
    let data = payload(&state().lock(), p, from);
    if !data.is_null() {
        crate::windows::emit(app, LABEL, "agents:card", data);
    }
}

/// The rows changed under a card that is up.
fn refresh_card(p: &Prefs) {
    let Some(app) = APP.get() else { return };
    let placed = state().lock().showing.as_ref().is_some_and(|s| s.placed);
    if placed {
        send_card(app, p);
    }
}

/// Where a card `height` tall goes: under the bar, centred on it, or above
/// it when there is no room below. Also which edge it grows out of.
fn spot(app: &AppHandle, height: f64) -> (&'static str, Rect) {
    let area = crate::windows::bar_area();
    if !crate::windows::bar_on_screen(app) {
        return ("top", place_alone(area, height));
    }
    let bar = crate::windows::bar_rect(app);
    place_card(bar, area, height)
}

/// With no bar on screen (the button is off and the bar is closed) the card
/// hangs from the top of the screen, in the middle.
fn place_alone(area: Rect, height: f64) -> Rect {
    let width = CARD_WIDTH.min(area.width);
    let x = area.x + ((area.width - width) / 2.0).round();
    Rect::new(x, area.y, width, height.min(area.height))
}

fn place_card(bar: Rect, area: Rect, height: f64) -> (&'static str, Rect) {
    let width = CARD_WIDTH.min(area.width);
    let left = bar.x + bar.width / 2.0 - width / 2.0;
    let x = left.clamp(area.x, (area.x + area.width - width).max(area.x));
    let below = bar.y + bar.height + GAP;
    if below + height <= area.y + area.height {
        ("top", Rect::new(x, below, width, height))
    } else {
        let y = (bar.y - GAP - height).max(area.y);
        ("bottom", Rect::new(x, y, width, height))
    }
}

/// `agents:size`: the page measured the card. The window takes that size
/// beside the bar and shows, without taking focus.
fn page_size(app: &AppHandle, p: &Value) -> Value {
    let height = p
        .get("height")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .clamp(40.0, 640.0);
    let Some(w) = app.get_webview_window(LABEL) else {
        return Value::Null;
    };
    if state().lock().showing.is_none() {
        return Value::Null;
    }
    let (from, at) = spot(app, height);
    let px = crate::windows::screen::dip_to_px(at);
    let rect = PxRect {
        x: px.x.round() as i32,
        y: px.y.round() as i32,
        width: px.width.round() as i32,
        height: px.height.round() as i32,
    };
    win::place(&w, rect);
    let raw = capture::hwnd_of(&w);
    win::show_inactive(raw);
    win::raise(raw);
    if let Some(s) = state().lock().showing.as_mut() {
        s.placed = true;
    }
    json!({ "from": from })
}

/// How long the window waits for the page to say `agents:hidden` before it
/// goes anyway. The fold takes 260 ms and two frames; this only runs out
/// when the page cannot answer.
const HIDE_WAIT_MS: u64 = 600;

/// `agents:hidden`: the page has folded the card and painted the frame
/// without it, so the window can go with nothing on it.
fn page_hidden(app: &AppHandle) -> Value {
    // Unless another card came up meanwhile.
    if state().lock().showing.is_none() {
        if let Some(w) = app.get_webview_window(LABEL) {
            win::hide(capture::hwnd_of(&w));
        }
    }
    json!({ "ok": true })
}

/// Takes the card down. The page folds it first and says when it is gone
/// (`agents:hidden`); the window hides then.
fn hide_card() {
    let Some(app) = APP.get() else { return };
    {
        let mut st = state().lock();
        if st.showing.take().is_none() {
            return;
        }
        st.hover = false;
    }
    crate::windows::emit(app, LABEL, "agents:hide", Value::Null);
    let a = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(HIDE_WAIT_MS));
        // Unless another card came up meanwhile.
        if state().lock().showing.is_some() {
            return;
        }
        let a2 = a.clone();
        let _ = a.run_on_main_thread(move || {
            if let Some(w) = a2.get_webview_window(LABEL) {
                win::hide(capture::hwnd_of(&w));
            }
        });
    });
}

/// The window a session lives in.
fn window_of(session: &str) -> i64 {
    let st = state().lock();
    st.book
        .get(session)
        .map(|s| s.hwnd)
        .or_else(|| {
            let c = &st.showing.as_ref()?.card;
            (c.session_id.as_deref() == Some(session)).then_some(c.hwnd)
        })
        .unwrap_or(0)
}

/// Brings a session's window to the front, as a click on its row in the
/// card does. False when the window is gone.
pub fn jump(session: &str) -> bool {
    native::focus(window_of(session))
}

/// `agents:open`: a row or the head was clicked.
fn open(p: &Value) -> Value {
    let hwnd = match p.get("session").and_then(Value::as_str) {
        Some(id) => window_of(id),
        None => state().lock().showing.as_ref().map_or(0, |s| s.card.hwnd),
    };
    let ok = native::focus(hwnd);
    hide_card();
    json!({ "ok": ok })
}

/// Alt+Tab while a card with a window behind it is up: jump there. Called
/// from the keyboard hook, so it never waits; true swallows the key.
pub fn alt_tab() -> bool {
    let Some(st) = state().try_lock() else {
        return false;
    };
    let hwnd = match &st.showing {
        Some(s) if s.placed && st.alt_tab => s.card.hwnd,
        _ => 0,
    };
    drop(st);
    if !native::is_window(hwnd) {
        return false;
    }
    std::thread::spawn(move || {
        native::focus(hwnd);
        hide_card();
    });
    true
}

/// The card's countdown, the session sweep and the hook re-check.
fn tick_loop() {
    let mut ticks: u64 = 0;
    loop {
        std::thread::sleep(Duration::from_millis(TICK_MS));
        ticks += 1;
        let _ = crate::crash::guard("agents-tick", || {
            let expired = {
                let mut st = state().lock();
                let hover = st.hover;
                match st.showing.as_mut() {
                    Some(s) if s.placed => {
                        if !hover && native::idle_ms() < AWAY_AFTER_MS {
                            s.elapsed += TICK_MS;
                        }
                        s.elapsed >= s.total
                    }
                    _ => false,
                }
            };
            if expired {
                hide_card();
            }
            // Every 5 seconds.
            if ticks % 20 == 0 {
                sweep();
                // The minutes in a row's text move on even when nothing
                // else does.
                sync_tray();
            }
            // Every 10 minutes: agents installed since, or hooks another
            // tool rewrote.
            if ticks % 2400 == 0 {
                let p = prefs();
                if p.enabled {
                    sync_hooks(&p);
                }
            }
        });
    }
}

fn sweep() {
    let p = prefs();
    if !p.enabled {
        return;
    }
    let changed = {
        let mut st = state().lock();
        if st.book.is_empty() {
            return;
        }
        st.book.sweep(
            now_ms(),
            |s| native::is_alive(s.pid, s.pid_start),
            probe,
            |s, id| s.subagent_transcript(id).and_then(|t| modified_ms(&t)),
        )
    };
    if changed {
        refresh_power(&p);
        refresh_card(&p);
    }
}

// -------------------------------------------------------------------- tray

/// One session as the tray menu lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayRow {
    pub session: String,
    /// The project and where it runs: "site · VS Code".
    pub label: String,
    /// "needs you", "working 12m", "done 3m ago".
    pub status: String,
    /// Its window is still there to jump to.
    pub can_jump: bool,
}

/// The first sessions in the card's order (needs you, then done, then
/// working), and how many more there are. With `nest`, what a session
/// started sits under it, set in; its row goes to that session's window.
fn tray_list(
    book: &SessionBook,
    nest: bool,
    now: u64,
    has_window: impl Fn(i64) -> bool,
) -> (Vec<TrayRow>, usize) {
    let all = book.tree(None, nest);
    let more = all.len().saturating_sub(TRAY_ROWS);
    let list = all
        .iter()
        .take(TRAY_ROWS)
        .filter_map(|row| {
            let s = book.get(&row.session_id)?;
            let sub = row
                .subagent_id
                .as_ref()
                .and_then(|id| s.subagents.iter().find(|(k, _)| k == id));
            let (name, status) = match sub {
                Some((_, a)) => {
                    let kind = if a.kind.is_empty() {
                        "Subagent"
                    } else {
                        a.kind.as_str()
                    };
                    (kind.to_string(), sessions::subagent_text(a, now))
                }
                None => {
                    let place = s.place();
                    let name = if place.is_empty() {
                        s.project()
                    } else {
                        format!("{} \u{b7} {place}", s.project())
                    };
                    (name, sessions::status_text(s, now))
                }
            };
            let label = if row.depth > 0 {
                format!("\u{2003}\u{2003}{name}")
            } else {
                name
            };
            Some(TrayRow {
                session: s.id.clone(),
                label,
                status,
                can_jump: has_window(s.hwnd),
            })
        })
        .collect();
    (list, more)
}

/// The sessions the tray menu lists. Empty while agent watching is off.
pub fn tray_rows() -> (Vec<TrayRow>, usize) {
    let p = prefs();
    if !p.enabled {
        return (Vec::new(), 0);
    }
    tray_list(
        &state().lock().book,
        p.show_children,
        now_ms(),
        native::is_window,
    )
}

/// Has the tray menu redrawn when its session rows are no longer the ones
/// it was last asked to show.
fn sync_tray() {
    let Some(app) = APP.get() else { return };
    let rows = tray_rows();
    {
        let mut st = state().lock();
        if st.tray == rows {
            return;
        }
        st.tray = rows;
    }
    crate::tray::refresh(app);
}

// ------------------------------------------------------------------- hooks

fn profile() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}

/// `<exe> --hook`, with the exe path in a form every shell takes.
fn hook_base() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let exe = exe.to_string_lossy();
    let exe = exe.strip_prefix(r"\\?\").unwrap_or(&exe);
    Some(format!(
        "{} --hook",
        specs::hook_path(exe, native::short_path)
    ))
}

/// Writes `text` over `file` in one step, so a crash never leaves half a
/// settings file.
fn replace_file(file: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = file.as_os_str().to_owned();
    tmp.push(".aevalbuddy.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, file)
}

/// Adds (`want`) or removes this app's hooks for one agent under `home`.
/// `Ok(true)` when they are in place afterwards. An agent whose config
/// folder does not exist is not installed, and is left alone.
fn sync_one(a: &specs::AgentSpec, home: &Path, base: &str, want: bool) -> Result<bool, String> {
    if !home.join(a.dir).is_dir() {
        return Ok(false);
    }
    let file = home.join(a.file);
    let text = std::fs::read_to_string(&file).ok();
    let cmd = specs::command_for(a, base);
    if want && specs::is_installed(a, text.as_deref(), &cmd) {
        return Ok(true);
    }
    if !want && text.is_none() {
        return Ok(false);
    }
    let (next, changed) = specs::apply(a, text.as_deref(), want.then_some(cmd.as_str()))?;
    if !changed {
        return Ok(want);
    }
    // The person's file, as it was before this app first touched it.
    if let (Some(old), true) = (&text, a.layout != specs::Layout::Own) {
        let mut bak = file.as_os_str().to_owned();
        bak.push(".aevalbuddy.bak");
        if !Path::new(&bak).exists() {
            std::fs::write(&bak, old).map_err(|e| e.to_string())?;
        }
    }
    match next {
        Some(new) => replace_file(&file, &new).map_err(|e| e.to_string())?,
        None => std::fs::remove_file(&file).map_err(|e| e.to_string())?,
    }
    Ok(want)
}

/// Brings every agent's hooks in line with the settings.
fn sync_hooks(p: &Prefs) {
    let (Some(home), Some(base)) = (profile(), hook_base()) else {
        return;
    };
    let mut errors = HashMap::new();
    for a in &specs::ALL {
        let want = p.enabled && !p.agents_off.iter().any(|id| id == a.id);
        if let Err(why) = sync_one(a, &home, &base, want) {
            errors.insert(a.id, why);
        }
    }
    state().lock().errors = errors;
    if let Some(app) = APP.get() {
        crate::windows::emit(app, crate::windows::SETTINGS, "agents:state", snapshot());
    }
}

fn sync_soon() {
    std::thread::spawn(|| {
        let _ = crate::crash::guard("agents-hooks", || sync_hooks(&prefs()));
    });
}

// ---------------------------------------------------------------- settings

/// `agents:get`: everything Settings' Agents page shows.
fn snapshot() -> Value {
    let p = prefs();
    let home = profile();
    let base = hook_base().unwrap_or_default();
    let (errors, busy, awake) = {
        let st = state().lock();
        (st.errors.clone(), st.book.busy_count(), st.awake.holding())
    };
    let agents: Vec<Value> = specs::ALL
        .iter()
        .map(|a| {
            let found = home.as_ref().is_some_and(|h| h.join(a.dir).is_dir());
            let text = home
                .as_ref()
                .filter(|_| found)
                .and_then(|h| std::fs::read_to_string(h.join(a.file)).ok());
            let cmd = specs::command_for(a, &base);
            json!({
                "id": a.id,
                "name": a.name,
                "found": found,
                "on": !p.agents_off.iter().any(|id| id == a.id),
                "connected": specs::is_installed(a, text.as_deref(), &cmd),
                "note": a.note,
                "error": errors.get(a.id),
            })
        })
        .collect();
    // Aevalsistant does the same job from the tray; with both running, each
    // shows its own card.
    let other = std::env::var_os("LOCALAPPDATA")
        .map(|d| {
            PathBuf::from(d)
                .join("Aevalsistant")
                .join("Aevalsistant.exe")
        })
        .is_some_and(|exe| exe.is_file());
    json!({
        "prefs": p,
        "agents": agents,
        "aevalsistant": other,
        "chatProblem": chats::problem(),
        "hasLid": power::has_lid(),
        "working": busy,
        "awake": awake.0,
        "screenOn": awake.1,
    })
}

/// `agents:set`: one or more settings changed. Applied at once.
fn set(patch: &Value) -> Value {
    let before = prefs();
    let after = merged(&before, patch);
    if after != before {
        save_prefs(&after);
        state().lock().alt_tab = after.alt_tab;
        if !after.enabled {
            // Nothing is watched any more: let the PC sleep, drop the card.
            state().lock().book = SessionBook::default();
            hide_card();
        }
        refresh_power(&after);
        if after.enabled != before.enabled || after.agents_off != before.agents_off {
            sync_hooks(&after);
        }
        let chats_on = after.enabled && after.chats;
        if chats_on != (before.enabled && before.chats) {
            chats::set(chats_on);
            if !chats_on {
                chats_stopped();
            }
        }
        sync_tray();
    }
    snapshot()
}

fn info(title: &str, detail: &str) -> CardRequest {
    CardRequest {
        session_id: None,
        hwnd: 0,
        host: String::new(),
        kind: CardKind::Info,
        title: title.to_string(),
        detail: detail.to_string(),
        wants_snippet: false,
        background: false,
    }
}

/// `agents:test`: a card as a finished agent's would look.
fn test_card() -> Value {
    let mut card = info(
        "rover finished",
        "This is what you see when an agent finishes. Click it to jump to that window.",
    );
    card.kind = CardKind::Done;
    card.host = "Terminal".to_string();
    show_card(card, Vec::new(), &prefs());
    json!({ "ok": true })
}

/// Once, a while after the first start with this in the app: the offer to
/// turn agent watching on.
fn offer_later() {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(90));
        let _ = crate::crash::guard("agents-offer", || {
            let p = prefs();
            if p.enabled || p.offered {
                return;
            }
            // Only where there is an agent to watch.
            let any = profile().is_some_and(|h| specs::ALL.iter().any(|a| h.join(a.dir).is_dir()));
            if !any {
                return;
            }
            save_prefs(&Prefs {
                offered: true,
                ..p.clone()
            });
            let card = info(
                "Watch your coding agents?",
                "AevalBuddy can keep this PC awake while Claude Code, Codex and the others work, and tell you when one finishes or needs you.",
            );
            let actions = vec![("enable", "Turn on"), ("later", "Not now")];
            show_card(card, actions, &p);
        });
    });
}

/// `agents:act`: a button on one of the app's own cards.
fn act(p: &Value) -> Value {
    let id = p.get("id").and_then(Value::as_str).unwrap_or_default();
    hide_card();
    if id == "enable" {
        set(&json!({ "enabled": true }));
        let done = info(
            "Agent watching is on",
            "Restart any agent sessions that are open now. Settings, Agents has the rest.",
        );
        show_card(done, Vec::new(), &prefs());
    }
    json!({ "ok": true })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(kind: CardKind, background: bool) -> CardRequest {
        CardRequest {
            background,
            kind,
            ..info("t", "d")
        }
    }

    #[test]
    fn background_cards_follow_their_own_switches() {
        let p = Prefs::default();
        assert!(!p.enabled, "off until the person turns it on");
        assert!(p.wants(&request(CardKind::Done, false)));
        assert!(!p.wants(&request(CardKind::Done, true)));
        assert!(p.wants(&request(CardKind::NeedsYou, true)));
        assert!(p.wants(&request(CardKind::Info, true)));
        let quiet = Prefs {
            notify_needs_you: false,
            ..Prefs::default()
        };
        assert!(!quiet.wants(&request(CardKind::NeedsYou, false)));
        assert!(!quiet.wants(&request(CardKind::NeedsYou, true)));
        assert_eq!(p.card_ms(CardKind::Done), 8_000);
        let short = Prefs {
            card_time: 0,
            ..Prefs::default()
        };
        assert_eq!(short.card_ms(CardKind::NeedsYou), 6_000);
        let long = Prefs {
            card_time: 2,
            ..Prefs::default()
        };
        assert_eq!(long.card_ms(CardKind::Done), 14_000);
    }

    #[test]
    fn settings_merge_field_by_field() {
        let now = Prefs::default();
        let next = merged(
            &now,
            &json!({ "enabled": true, "displayOn": true, "cardTime": 2 }),
        );
        assert!(next.enabled && next.display_on && next.keep_awake);
        assert_eq!(next.card_time, 2);
        // The page cannot write the app's own bookkeeping, a field that
        // does not exist, or a value of the wrong kind.
        let odd = json!({ "lidSaved": "x|1|2", "offered": true, "nope": 1 });
        assert_eq!(merged(&next, &odd), next);
        assert_eq!(merged(&next, &json!({ "enabled": "yes" })), next);
        assert_eq!(merged(&next, &json!(3)), next);
        // Stored settings from before a field existed read with defaults.
        let old: Prefs = serde_json::from_value(json!({ "enabled": true })).unwrap();
        assert!(old.enabled && old.notify_done && old.alt_tab);
        assert!(!old.chats, "chat watching is off until it is asked for");
        assert!(merged(&old, &json!({ "chats": true })).chats);
    }

    #[test]
    fn the_card_sits_under_the_bar_or_above_it() {
        let area = Rect::new(0.0, 0.0, 1920.0, 1040.0);
        let bar = Rect::new(660.0, 20.0, 600.0, 56.0);
        let (from, at) = place_card(bar, area, 120.0);
        assert_eq!(from, "top");
        assert_eq!((at.x, at.y, at.width), (734.0, 78.0, 452.0));
        // A bar at the bottom of the screen: the card goes above it.
        let low = Rect::new(660.0, 960.0, 600.0, 56.0);
        let (from, at) = place_card(low, area, 120.0);
        assert_eq!(from, "bottom");
        assert_eq!(at.y + at.height, 958.0);
        // Never off the side of the work area.
        let edge = Rect::new(0.0, 20.0, 56.0, 56.0);
        assert_eq!(place_card(edge, area, 120.0).1.x, 0.0);
        let alone = place_alone(area, 120.0);
        assert_eq!(alone.y, area.y);
        assert_eq!(alone.x + alone.width / 2.0, area.x + area.width / 2.0);
    }

    #[test]
    fn rows_list_the_other_sessions() {
        let mut book = SessionBook::default();
        let ev = |name: &str, id: &str| HookEvent {
            name: name.into(),
            session_id: id.into(),
            agent: "claude".into(),
            cwd: format!("C:\\code\\{id}"),
            host: "VS Code".into(),
            ..HookEvent::default()
        };
        for i in 0..9 {
            book.apply(&ev("UserPromptSubmit", &format!("s{i}")), 1_000);
        }
        book.apply(&ev("Stop", "s0"), 2_000);
        let (list, more) = rows(&book, Some("s0"), true, 2_000 + 5 * 60_000);
        assert_eq!((list.len(), more), (6, 2));
        assert_eq!(list[0]["state"], "working");
        assert_eq!(list[0]["text"], "working 5m");
        assert_eq!(list[0]["place"], "VS Code");
        assert_eq!(list[0]["canJump"], false);
        assert!(
            list.iter().all(|r| r["session"] != "s0"),
            "the head is above"
        );
    }

    #[test]
    fn the_tray_lists_sessions_that_need_you_first() {
        let mut book = SessionBook::default();
        let ev = |name: &str, id: &str| HookEvent {
            name: name.into(),
            session_id: id.into(),
            agent: "claude".into(),
            cwd: format!("C:\\code\\{id}"),
            host: "VS Code".into(),
            ..HookEvent::default()
        };
        assert_eq!(tray_list(&book, true, 1_000, |_| true), (Vec::new(), 0));
        for i in 0..10 {
            book.apply(&ev("UserPromptSubmit", &format!("s{i}")), 1_000);
        }
        book.apply(&ev("Stop", "s3"), 2_000);
        let mut ask = ev("Notification", "s7");
        ask.notification_type = "permission_prompt".into();
        ask.hwnd = 7;
        book.apply(&ask, 3_000);
        let later = 3_000 + 5 * 60_000;
        let (list, more) = tray_list(&book, false, later, |hwnd| hwnd == 7);
        assert_eq!((list.len(), more), (TRAY_ROWS, 2));
        assert_eq!(
            list[0],
            TrayRow {
                session: "s7".into(),
                label: "s7 \u{b7} VS Code".into(),
                status: "needs you".into(),
                can_jump: true,
            }
        );
        assert_eq!(
            (list[1].session.as_str(), list[1].status.as_str()),
            ("s3", "done 5m ago")
        );
        assert_eq!(list[2].status, "working 5m");
        assert!(
            list[1..].iter().all(|r| !r.can_jump),
            "no window, nowhere to jump"
        );
        // With subagents listed, each sits under its session, set in.
        let mut sub = ev("SubagentStart", "s7");
        sub.agent_id = "x1".into();
        sub.agent_type = "Explore".into();
        book.apply(&sub, 3_000);
        let (nested, more) = tray_list(&book, true, later, |_| false);
        assert_eq!(more, 3);
        assert_eq!(nested[1].label, "\u{2003}\u{2003}Explore");
        assert_eq!(
            (nested[1].session.as_str(), nested[1].status.as_str()),
            ("s7", "running 5m")
        );
    }

    #[test]
    fn hooks_sync_into_a_profile_and_back_out() {
        let home = std::env::temp_dir().join(format!("aeval-agents-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        let theirs = r#"{"model":"opus"}"#;
        std::fs::write(home.join(".claude").join("settings.json"), theirs).unwrap();
        let base = "C:/Apps/AevalBuddy/AevalBuddy.exe --hook";
        let claude = specs::get("claude");
        // Codex is not installed here: nothing is created for it.
        assert_eq!(sync_one(specs::get("codex"), &home, base, true), Ok(false));
        assert!(!home.join(".codex").exists());
        assert_eq!(sync_one(claude, &home, base, true), Ok(true));
        let file = home.join(".claude").join("settings.json");
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(specs::is_installed(claude, Some(&text), base));
        let bak = home.join(".claude").join("settings.json.aevalbuddy.bak");
        assert_eq!(std::fs::read_to_string(&bak).unwrap(), theirs);
        // Already in place: the file is not rewritten.
        let stamp = std::fs::metadata(&file).unwrap().modified().unwrap();
        assert_eq!(sync_one(claude, &home, base, true), Ok(true));
        assert_eq!(std::fs::metadata(&file).unwrap().modified().unwrap(), stamp);
        assert_eq!(sync_one(claude, &home, base, false), Ok(false));
        let clean = std::fs::read_to_string(&file).unwrap();
        assert!(!clean.contains("--hook") && clean.contains("opus"));
        // A file that is not JSON is reported and left as it is.
        std::fs::write(&file, "{ nope").unwrap();
        assert!(sync_one(claude, &home, base, true).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{ nope");
        let _ = std::fs::remove_dir_all(&home);
    }
}
