//! The tray icon: the way back in when the overlay is hidden (PARITY
//! TRAY-01..08). The menu is built from data (`template`), so the smoke run
//! can check its order in every state without opening it.
//!
//! The menu is made once and then changed in place (B-080): `refresh` works
//! out the rows on the calling thread and posts the edit to the main thread,
//! which touches only the rows whose text or state changed. The menu is never
//! swapped, so one that is open stays open.

use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use std::sync::OnceLock;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use parking_lot::Mutex;
use serde_json::{Map, Value};
use tauri::image::Image;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;

use crate::agents::TrayRow;
use crate::state::shared;

const TRAY_ID: &str = "main";

/// The buddy's arrow, 16 px and 32 px PNGs (from lib/tray.js).
const ICON_16: &str = concat!(
    "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAA40lEQVR42mNgQAInz98NYKAEbF5w4P/ujafmnzx/V4AsA5a1",
    "rv/f5tz2f/PCA+dPnr9rQJLmk+fvKoAMaLJqAOMVnZvenzx/N4EUAxwWVq+EGwDCc4sW/z95/i5xXgIZMCV+OooBINwfNvH/",
    "kSPXCHsJ5FxsBoAwKFx2bzyF30snz99twGUAsiEkGwASm5Y6u6DJqsGBkBfmwwyYnb/wfrdvD5gNok+ev9tPTCCuB2lssmpI",
    "QI/SrUsO3Sc2KuHRBUqRaNFJesJa1bcFHI3bVx0DGaBActIGRdvJ83f3k5Qi6Q4AIKPkKV/EBQ0AAAAASUVORK5CYII=",
);

/// The 32 px arrow area-resampled to 20 px (125 % scale).
const ICON_20: &str = concat!(
    "iVBORw0KGgoAAAANSUhEUgAAABQAAAAUCAYAAACNiR0NAAACEElEQVR42q3UTWgTURAA4KhY8IeKKF4EL3oQz57WUqhI0SoI",
    "Fb0JIrkoeNCCevCw2XRrk1jXFVJoJRKI+TExFks3m0ha2+wPO7Q6/tA/4zvoRRRUEFFDpU+fuhKEJNttB+byDt/O23kzHk9V",
    "GOacTytN84Bkg2clIpfQwoPnot+LyuNbJszvAiSrlgWqKUMNHAxQ6bhMh6PjCEgOAJI17sG0+YiBAsfTwKHAYlpS3uv6TBcg",
    "2ewKLGStcm/71d8gS7GtmyZ6hr7qxmwSkGxZEgZI1ilx7bWN2enf56P93kE6Xnz2FJC0LwXcoSS0t/+DdoZPD9B82nxjTZXP",
    "ApKNTsDduYT+rhbIMng4xJq1YE2VBwDJ9kZg60hs4lM9kGXPfpGmgsMLuj7zApDsrAd23JPVz41AGx2JlSgg2VsPPJWRc99q",
    "IdePSTTSdWdS4PhLAsd7BY7f1OjKZzKyWqmqohI6EvoHSidkOpZ/MglI1jrtMp+RFBt4Fb2c5FnF3a3+P8+nxUez/YWPgKTF",
    "Keh9EBl7KbaJowLH73k4BM0To8+T1472LdpV3jwZ/mFa81cAyWonYBN7OpELsaaqs87bF+NfbJBNUS5l5AFJs6tRZP9LvWsU",
    "gx3Bv425QXVjdg6QbHO9MLTSdGcha32I++/TfMas/PqI6LgxNapcz2YYkPQBkvOAZOuKLF+2F5e9cN3ETzvdpnm7PLtoAAAA",
    "AElFTkSuQmCC",
);

/// The 32 px arrow area-resampled to 24 px (150 % scale).
const ICON_24: &str = concat!(
    "iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACbElEQVR42uXVTWgTQRQA4IgiCB4kBxEF8VDw4kE8eFi1tbWo",
    "uVTp2Z8ieJEe/EEQBN1uskkX86MmVm0a06a2tUnUbtqkgTZNk+w2mZY4hzY/tgwiVUSkIAYKBcPTkaYkBU3WxpMD77BzmG/f",
    "vLdvVaqihTDZJ8kZuxzPXkCYbFNVe1FgqHM8/EIQl+TE21sIk13VBg64TSMZQSPAs5t936Lh2QGEyf5qAgc9Fv87wwk9aI+2",
    "ga3lCQQGpA8IkyaEyfZqAIe9D0e/UIBj2F9hbraAzxn+GJ+eZxEmuzcK1Hmto3l9A78G0DCeMcFLazAnx7MiwmTPRgBNP/+6",
    "5PBC6I5xtC4QnZxLIUzO/lWXIUyu/A4ohOPGcxj3zSwmkgt3FdcFYXK1HKCr5cB6vgOC3vjyz+uyI0z2KgHM5YA1qE4Lw92T",
    "IMez9MqaESZbywJTaL7bdcddEUCj/aQB+vlXEIumlhAmDWWBWDQl0juuFKAhnBZgTJwG2uLlASmNHdd7//zWp9qhs9X5SdAI",
    "To5hLRzDtnAMW1mxJSmdWg8YGvUlz/TDG/PNLCJMDiluU0nOLBcD95qM70VH6CstaGGPP64Dz/1AHmHSqhyQ0tB1zQXaI23f",
    "OYbt4Ri2BmFiWZ8VnVETwTdTigE5kXV6HgQ8fL2O4xhWvdq6Nf6+WK54PvH1Ohh2RVYUj3OEyQ6EyU7bxcdbivdDgaTv0aWn",
    "JVn03B6knXOuWmO8dtA08lm/mgWtidg1QQFjtQB1yJ/sdpv9+Y7LduhlPRCNzK1U1PsVAptoFonkwlAkPAtyIptDmNgQJpv/",
    "xX9bTUP136wf1AQ1I6UbPpgAAAAASUVORK5CYII=",
);

const ICON_32: &str = concat!(
    "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAABsUlEQVR42u2Xv2rDMBDG/Qh+hD6CZ0+GzqWeujbP0LVQhKFZ",
    "mjYUCpmK+wcK9VAPKRRKIBmagKAgOmUSfgQ/gsoXJBBBDrUsK0sFt0TD/XLf3XdyEBgOZZxQxnPKeBjs4wBg+rAQlHFGGY/2",
    "AnB3OhHjk1vxtVzXlPGBb4A5ALKYiOHhUMym38KrJDqAgnibfPqTZBtAxeP5KyD6lwRJTAAI/L5crfuVhDK+aUATAAJ3si/6",
    "kQQATcn1vpCjCklSl8mjvwCoKG7eAYEYuwJI2gAg7s+eVV9AkgPvAKovFrOf7pJgxGwAtkzLXhK5iKwAVEjTspMEAPgXXQBU",
    "X1hVAgbzUaw6A1wdjRRA1NqG2wKgAUfH11UWk7kWifUeaAsguz935QOsCaDJntWScmbDJoCni6KE2WDUTOMn9U6dAMDjtQR1",
    "FpOBvKvQ3TtGL3dWAXSxTB7pJgW9m+RxIgOMQ+36LCah4b5qeitIF0yCPg+MCuU2Abxclu624g6AEKU2NaN6pHh5tuMdoENA",
    "FjkJxAfApgoYS4Boy6f2+XRPKeOlTIyonD7NWlbD/+fb/3F1fgFvqYMQkYvkrgAAAABJRU5ErkJggg==",
);

/// Every size, smallest first: 100, 125, 150 and 200 % of 16 px.
const ICONS: [(u32, &str); 4] = [(16, ICON_16), (20, ICON_20), (24, ICON_24), (32, ICON_32)];

/// One menu row. `id` is what a click reports; a separator has id `-`.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: &'static str,
    pub label: String,
    /// Shown beside the label, never registered (TRAY-04).
    pub accelerator: Option<String>,
    pub enabled: bool,
}

impl Item {
    fn new(id: &'static str, label: impl Into<String>, accelerator: Option<String>) -> Self {
        Item {
            id,
            label: label.into(),
            accelerator: accelerator.filter(|a| !a.is_empty()),
            enabled: true,
        }
    }

    fn disabled(id: &'static str, label: impl Into<String>) -> Self {
        Item {
            enabled: false,
            ..Item::new(id, label, None)
        }
    }

    fn sep() -> Self {
        Item::disabled("-", "-")
    }

    pub fn is_separator(&self) -> bool {
        self.id == "-"
    }
}

/// The tray's own Check for updates answer (TRAY-05).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Manual {
    #[default]
    Idle,
    Checking,
    /// Up to date, shown until the given time (ms).
    Current {
        until: u64,
        version: String,
    },
    Failed,
}

fn manual() -> &'static Mutex<Manual> {
    static M: OnceLock<Mutex<Manual>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(Manual::Idle))
}

/// Everything the menu depends on.
pub struct Input<'a> {
    pub update: &'a Value,
    pub agent_running: bool,
    pub hidden: bool,
    pub shortcuts: &'a Map<String, Value>,
    pub manual: &'a Manual,
    pub now: u64,
}

fn check_item(u: &Value, m: &Manual, now: u64) -> Item {
    let status = u["status"].as_str().unwrap_or("");
    let version = u["version"].as_str().unwrap_or("");
    if *m == Manual::Checking {
        return Item::disabled("check", "Checking…");
    }
    if status == "downloading" && !version.is_empty() {
        let pct = (u["progress"].as_f64().unwrap_or(0.0) * 100.0).floor();
        return Item::disabled("check", format!("Downloading {version}… {pct}%"));
    }
    if let Manual::Current { until, version } = m {
        if now < *until {
            return Item::disabled("check", format!("Up to date ({version})"));
        }
    }
    if *m == Manual::Failed {
        return Item::new("check", "Update failed, try again", None);
    }
    Item::new("check", "Check for updates", None)
}

/// The menu as data, in SPEC-v2 section 4.4's order (TRAY-03), with the
/// tasks-and-conversations wording and Close all answer windows (C-83..C-85).
pub fn template(i: &Input) -> Vec<Item> {
    let key = |a: &str| i.shortcuts.get(a).and_then(Value::as_str).map(String::from);
    let u = i.update;
    let mut out = Vec::new();
    if u["status"] == "ready" && u["restartPending"] == true {
        out.push(Item::disabled(
            "restarting",
            "Restarting after this answer…",
        ));
        out.push(Item::sep());
    } else if u["status"] == "ready" {
        let v = u["version"].as_str().unwrap_or("");
        out.push(Item::new(
            "restart",
            format!("Restart to update to {v}"),
            None,
        ));
        out.push(Item::sep());
    }
    if i.agent_running {
        out.push(Item::new("stop", "Stop the task", Some("Esc".into())));
        out.push(Item::sep());
    }
    out.push(Item::new(
        "capture",
        "Ask, find, or do…",
        key("captureQuery"),
    ));
    out.push(Item::new(
        "region",
        "Ask about an area…",
        key("captureRegion"),
    ));
    out.push(Item::new("find", "Find on screen…", key("findOnScreen")));
    out.push(Item::new("agent", "Do a task…", key("agentTask")));
    out.push(Item::sep());
    let toggle = if i.hidden {
        "Show the overlay"
    } else {
        "Hide the overlay"
    };
    out.push(Item::new("toggle", toggle, key("toggleVisibility")));
    out.push(Item::new("new", "New conversation", None));
    out.push(Item::new("close", "Close all answer windows", None));
    out.push(Item::new("history", "History…", None));
    out.push(Item::sep());
    out.push(Item::new("settings", "Settings…", None));
    out.push(Item::new("help", "Help and feedback…", None));
    out.push(check_item(u, i.manual, i.now));
    out.push(Item::sep());
    out.push(Item::new("quit", "Quit AevalBuddy", None));
    out
}

/// The ids of the rows that list coding-agent sessions. A click reports the
/// row, and `Live::sessions` says which session it shows.
const SESSION_IDS: [&str; 8] = [
    "session-0",
    "session-1",
    "session-2",
    "session-3",
    "session-4",
    "session-5",
    "session-6",
    "session-7",
];

/// `items` with the coding agents' sessions above the bar's own rows: one
/// row each, with its state where a chord would be, then how many more
/// there are. A row whose window is gone is greyed out.
fn with_sessions(mut items: Vec<Item>, rows: &[TrayRow], more: usize) -> Vec<Item> {
    if rows.is_empty() {
        return items;
    }
    let at = items.iter().position(|it| it.id == "capture").unwrap_or(0);
    let mut block: Vec<Item> = rows
        .iter()
        .zip(SESSION_IDS)
        .map(|(row, id)| {
            // A menu reads a single & as "underline the next letter".
            let label = row.label.replace('&', "&&");
            Item {
                enabled: row.can_jump,
                ..Item::new(id, label, Some(row.status.clone()))
            }
        })
        .collect();
    if more > 0 {
        block.push(Item::disabled("sessions-more", format!("{more} more")));
    }
    block.push(Item::sep());
    let rest = items.split_off(at);
    items.extend(block);
    items.extend(rest);
    items
}

/// Which session row an id names.
fn slot_of(id: &str) -> Option<usize> {
    SESSION_IDS.iter().position(|s| *s == id)
}

/// Labels only, separators as `-`, for the order check.
pub fn labels(i: &Input) -> Vec<String> {
    template(i).into_iter().map(|it| it.label).collect()
}

/// TRAY-06.
pub fn tooltip(update: &Value, agent_running: bool, shortcuts: &Map<String, Value>) -> String {
    if update["status"] == "ready" {
        let v = update["version"].as_str().unwrap_or("");
        return format!("AevalBuddy · {v} is ready, restart to update");
    }
    if agent_running {
        return "AevalBuddy is using your computer · Esc to stop".into();
    }
    match shortcuts.get("captureQuery").and_then(Value::as_str) {
        Some(c) if !c.is_empty() => format!("AevalBuddy · {c}"),
        _ => "AevalBuddy".into(),
    }
}

/// The row's text. The chord follows a tab, which Windows right-aligns as
/// accelerator text; nothing is registered for it (TRAY-04).
fn text(item: &Item) -> String {
    match &item.accelerator {
        Some(a) => format!("{}\t{a}", item.label),
        None => item.label.clone(),
    }
}

/// One edit to the live menu.
#[derive(Debug, PartialEq)]
enum Change {
    /// The rows themselves differ (an update is ready, a task started): they
    /// are replaced inside the same menu.
    Rows,
    Text(usize, String),
    Enabled(usize, bool),
}

/// The edits that turn `old` into `new`; none for a row that is the same.
fn diff(old: &[Item], new: &[Item]) -> Vec<Change> {
    let same_rows = old.len() == new.len() && old.iter().zip(new).all(|(a, b)| a.id == b.id);
    if !same_rows {
        return vec![Change::Rows];
    }
    let mut out = Vec::new();
    for (i, (a, b)) in old.iter().zip(new).enumerate() {
        if text(a) != text(b) {
            out.push(Change::Text(i, text(b)));
        }
        if a.enabled != b.enabled {
            out.push(Change::Enabled(i, b.enabled));
        }
    }
    out
}

/// Only the download percentage moved; shown at most once a second.
fn progress_only(new: &[Item], changes: &[Change]) -> bool {
    match changes {
        [Change::Text(i, _)] => new[*i].id == "check" && new[*i].label.starts_with("Downloading"),
        _ => false,
    }
}

/// The smallest icon at least 16 px at this scale, so Windows only ever
/// scales it down (TRAY-01).
fn icon_size(scale: f64) -> u32 {
    let want = 16.0 * scale - 0.01;
    ICONS
        .iter()
        .map(|(size, _)| *size)
        .find(|size| f64::from(*size) >= want)
        .unwrap_or(32)
}

fn icon(size: u32) -> Option<Image<'static>> {
    let (_, b64) = ICONS.iter().find(|(s, _)| *s == size)?;
    let bytes = B64.decode(b64).ok()?;
    Image::from_bytes(&bytes).ok().map(|i| i.to_owned())
}

fn screen_icon_size() -> u32 {
    icon_size(crate::windows::screen::primary().scale)
}

fn current_input_parts(app: &AppHandle) -> (Value, bool, bool, Map<String, Value>) {
    crate::state::refresh(app);
    let (running, hidden) = {
        let st = shared(app);
        let s = st.lock();
        (s.agent.running, s.hidden)
    };
    (
        crate::updater::get(),
        running,
        hidden,
        crate::store::get().get_shortcuts(),
    )
}

/// The rows and tooltip for the state right now, and the session behind
/// each session row.
fn snapshot(app: &AppHandle) -> (Vec<Item>, String, Vec<String>) {
    let (update, agent_running, hidden, shortcuts) = current_input_parts(app);
    let m = manual().lock().clone();
    let input = Input {
        update: &update,
        agent_running,
        hidden,
        shortcuts: &shortcuts,
        manual: &m,
        now: crate::state::now_ms(),
    };
    let (rows, more) = crate::agents::tray_rows();
    (
        with_sessions(template(&input), &rows, more),
        tooltip(&update, agent_running, &shortcuts),
        rows.into_iter().map(|row| row.session).collect(),
    )
}

/// A live row: the menu item for a label, or a separator.
enum Handle {
    Row(MenuItem<Wry>),
    Sep(PredefinedMenuItem<Wry>),
}

impl Handle {
    fn item(&self) -> &dyn IsMenuItem<Wry> {
        match self {
            Handle::Row(row) => row,
            Handle::Sep(sep) => sep,
        }
    }
}

/// What the tray shows, with the handles that change it in place.
struct Live {
    menu: Menu<Wry>,
    items: Vec<Item>,
    handles: Vec<Handle>,
    /// The session each session row shows, in row order.
    sessions: Vec<String>,
    tip: String,
    icon: u32,
    /// When the download percentage last changed on screen (ms).
    progress_at: u64,
}

fn live() -> &'static Mutex<Option<Live>> {
    static L: OnceLock<Mutex<Option<Live>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(None))
}

fn make(app: &AppHandle, item: &Item) -> tauri::Result<Handle> {
    if item.is_separator() {
        return Ok(Handle::Sep(PredefinedMenuItem::separator(app)?));
    }
    let row = MenuItem::with_id(app, item.id, text(item), item.enabled, None::<&str>)?;
    Ok(Handle::Row(row))
}

/// Replaces every row inside the same menu: never `set_menu`, which would
/// swap out a menu the person has open.
fn fill(app: &AppHandle, live: &mut Live, items: Vec<Item>) -> tauri::Result<()> {
    for h in live.handles.drain(..) {
        let _ = live.menu.remove(h.item());
    }
    live.items = Vec::new();
    for item in &items {
        let h = make(app, item)?;
        live.menu.append(h.item())?;
        live.handles.push(h);
    }
    live.items = items;
    Ok(())
}

/// Main thread: applies one snapshot, touching only what changed.
fn apply(app: &AppHandle, items: Vec<Item>, tip: String, sessions: Vec<String>) {
    let mut guard = live().lock();
    let Some(live) = guard.as_mut() else {
        return;
    };
    live.sessions = sessions;
    let changes = diff(&live.items, &items);
    let now = crate::state::now_ms();
    let progress = progress_only(&items, &changes);
    if !progress || now >= live.progress_at + 1000 {
        if progress {
            live.progress_at = now;
        }
        if changes.contains(&Change::Rows) {
            if let Err(e) = fill(app, live, items) {
                crate::crash::write("tray", &e.to_string());
            }
        } else {
            for change in changes {
                match change {
                    Change::Text(i, t) => {
                        if let Handle::Row(row) = &live.handles[i] {
                            let _ = row.set_text(t);
                        }
                    }
                    Change::Enabled(i, on) => {
                        if let Handle::Row(row) = &live.handles[i] {
                            let _ = row.set_enabled(on);
                        }
                    }
                    Change::Rows => {}
                }
            }
            live.items = items;
        }
    }
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    if tip != live.tip {
        let _ = tray.set_tooltip(Some(&tip));
        live.tip = tip;
    }
    let size = screen_icon_size();
    if size != live.icon {
        if let Some(img) = icon(size) {
            let _ = tray.set_icon(Some(img));
            live.icon = size;
        }
    }
}

/// Creates the tray (boot, before hotkeys).
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let (items, tip, sessions) = snapshot(app);
    let menu = Menu::new(app)?;
    let mut state = Live {
        menu: menu.clone(),
        items: Vec::new(),
        handles: Vec::new(),
        sessions,
        tip: tip.clone(),
        icon: screen_icon_size(),
        progress_at: 0,
    };
    fill(app, &mut state, items)?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip(tip)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            crate::crash::guard("tray.menu", || on_menu(app, event.id().as_ref()));
        })
        .on_tray_icon_event(|tray, event| {
            crate::crash::guard("tray.click", || on_icon(tray, &event));
        });
    if let Some(img) = icon(state.icon) {
        builder = builder.icon(img);
    }
    builder.build(app)?;
    *live().lock() = Some(state);
    Ok(())
}

/// Whether a click at `now` opens: not when it is the second click of a
/// double click (within the system double-click time of the last open).
fn opens(last: &AtomicU64, now: u64, double_click_ms: u64) -> bool {
    let prev = last.load(SeqCst);
    if prev != 0 && now.saturating_sub(prev) < double_click_ms {
        return false;
    }
    last.store(now, SeqCst);
    true
}

/// A left click opens the bar (or the trail while a task runs). Windows
/// reports a double click as two clicks and a double click, so only the
/// first click of it opens (D-07); Electron opened on all three.
fn on_icon(tray: &tauri::tray::TrayIcon, event: &TrayIconEvent) {
    static LAST_OPEN: AtomicU64 = AtomicU64::new(0);
    let click = matches!(
        event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
    );
    if !click {
        return;
    }
    // SAFETY: GetDoubleClickTime takes no arguments and only reads a setting.
    let double_click_ms = u64::from(unsafe { GetDoubleClickTime() });
    if opens(&LAST_OPEN, crate::state::now_ms(), double_click_ms) {
        CLICK_OPENS.fetch_add(1, SeqCst);
        let app = tray.app_handle().clone();
        tauri::async_runtime::spawn(async move {
            crate::windows::trigger_or_trail(&app);
        });
    }
}

/// How many clicks have opened the bar (smoke `trayClick`).
static CLICK_OPENS: AtomicU64 = AtomicU64::new(0);

pub fn click_opens() -> u64 {
    CLICK_OPENS.load(SeqCst)
}

/// Delivers what Windows sends for one double click on the icon, in order:
/// a click, the double click, and a second click (smoke `trayClick`).
pub fn deliver_double_click(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let id = tray.id().clone();
    let position = tauri::PhysicalPosition::new(0.0, 0.0);
    let rect = || tauri::Rect {
        position: tauri::Position::Physical(tauri::PhysicalPosition::new(0, 0)),
        size: tauri::Size::Physical(tauri::PhysicalSize::new(0, 0)),
    };
    let click = TrayIconEvent::Click {
        id: id.clone(),
        position,
        rect: rect(),
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
    };
    let double = TrayIconEvent::DoubleClick {
        id,
        position,
        rect: rect(),
        button: MouseButton::Left,
    };
    for event in [&click, &double, &click] {
        on_icon(&tray, event);
    }
}

/// Whether the tray came up (smoke `tray`).
pub fn is_ready(app: &AppHandle) -> bool {
    app.tray_by_id(TRAY_ID).is_some()
}

/// Brings menu, tooltip and icon up to date; called whenever a label could
/// change. Never blocks: the edit runs on the main thread (inline when this
/// is the main thread).
pub fn refresh(app: &AppHandle) {
    static TAKEN: AtomicU64 = AtomicU64::new(0);
    static SHOWN: AtomicU64 = AtomicU64::new(0);
    if live().lock().is_none() {
        return;
    }
    let seq = TAKEN.fetch_add(1, SeqCst) + 1;
    let (items, tip, sessions) = snapshot(app);
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        // A snapshot older than one already shown is stale.
        if SHOWN.fetch_max(seq, SeqCst) > seq {
            return;
        }
        crate::crash::guard("tray.refresh", || apply(&handle, items, tip, sessions));
    });
}

pub fn destroy(app: &AppHandle) {
    *live().lock() = None;
    let _ = app.remove_tray_by_id(TRAY_ID);
}

/// After a check that found nothing, "Up to date" shows for 10 s. Each check
/// clears only its own: a second check inside the 10 s is not cut short.
fn run_check(app: &AppHandle) {
    *manual().lock() = Manual::Checking;
    refresh(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let u = crate::updater::check(&app).await;
        let next = match u["status"].as_str().unwrap_or("") {
            "current" => Manual::Current {
                until: crate::state::now_ms() + 10_000,
                version: u["current"].as_str().unwrap_or("").to_string(),
            },
            "downloading" | "ready" | "dev" => Manual::Idle,
            _ => Manual::Failed,
        };
        let mine = next.clone();
        *manual().lock() = next;
        refresh(&app);
        if matches!(mine, Manual::Current { .. }) {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            let cleared = expire(&mut manual().lock(), &mine);
            if cleared {
                refresh(&app);
            }
        }
    });
}

/// Back to idle only if `m` is still the answer this check set.
fn expire(m: &mut Manual, mine: &Manual) -> bool {
    if *m != *mine {
        return false;
    }
    *m = Manual::Idle;
    true
}

fn on_menu(app: &AppHandle, id: &str) {
    let app = app.clone();
    let id = id.to_string();
    tauri::async_runtime::spawn(async move {
        use crate::windows as w;
        match id.as_str() {
            "restart" => {
                crate::updater::restart(&app);
            }
            "stop" => {
                shared(&app).call(&app, |h| h.stop_agent.as_ref());
            }
            "capture" => w::open_bar(&app, None, None),
            "region" => {
                shared(&app).call(&app, |h| h.start_region.as_ref());
            }
            "find" => w::open_bar(&app, Some("find"), None),
            "agent" => w::open_bar(&app, Some("agent"), None),
            "toggle" => w::toggle_visibility(&app),
            "new" => w::new_chat(&app),
            "close" => w::outputs::close_all(&app),
            "history" => w::open_history(&app),
            "settings" => w::open_settings(&app, None),
            "help" => {
                w::open_help(&app);
            }
            "check" => run_check(&app),
            "quit" => app.exit(0),
            other => {
                if let Some(session) = session_of(other) {
                    crate::agents::jump(&session);
                }
            }
        }
    });
}

/// The session behind a clicked session row.
fn session_of(id: &str) -> Option<String> {
    let slot = slot_of(id)?;
    live().lock().as_ref()?.sessions.get(slot).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn shortcuts() -> Map<String, Value> {
        crate::store::default_shortcuts()
    }

    fn items_for(update: Value, running: bool, hidden: bool, m: Manual) -> Vec<Item> {
        let s = shortcuts();
        template(&Input {
            update: &update,
            agent_running: running,
            hidden,
            shortcuts: &s,
            manual: &m,
            now: 1000,
        })
    }

    fn labels_for(update: Value, running: bool, hidden: bool, m: Manual) -> Vec<String> {
        let items = items_for(update, running, hidden, m);
        items.into_iter().map(|it| it.label).collect()
    }

    #[test]
    fn idle_order() {
        let l = labels_for(json!({"status": "idle"}), false, false, Manual::Idle);
        assert_eq!(
            l,
            vec![
                "Ask, find, or do…",
                "Ask about an area…",
                "Find on screen…",
                "Do a task…",
                "-",
                "Hide the overlay",
                "New conversation",
                "Close all answer windows",
                "History…",
                "-",
                "Settings…",
                "Help and feedback…",
                "Check for updates",
                "-",
                "Quit AevalBuddy",
            ]
        );
    }

    #[test]
    fn ready_running_hidden() {
        let u = json!({"status": "ready", "version": "1.2.0"});
        let l = labels_for(u, true, true, Manual::Idle);
        assert_eq!(
            &l[..4],
            ["Restart to update to 1.2.0", "-", "Stop the task", "-"]
        );
        assert!(l.contains(&"Show the overlay".to_string()));
        let pending = json!({"status": "ready", "version": "1.2.0", "restartPending": true});
        let l = labels_for(pending, false, false, Manual::Idle);
        assert_eq!(l[0], "Restarting after this answer…");
    }

    #[test]
    fn check_states() {
        let u = json!({"status": "downloading", "version": "1.2.0", "progress": 0.426});
        let s = shortcuts();
        let row = |u: &Value, m: Manual, now| check_item(u, &m, now).label;
        assert_eq!(row(&json!({}), Manual::Checking, 0), "Checking…");
        assert_eq!(row(&u, Manual::Idle, 0), "Downloading 1.2.0… 42%");
        let cur = Manual::Current {
            until: 500,
            version: "1.1.1".into(),
        };
        assert_eq!(row(&json!({}), cur.clone(), 100), "Up to date (1.1.1)");
        assert_eq!(row(&json!({}), cur, 900), "Check for updates");
        assert_eq!(
            row(&json!({}), Manual::Failed, 0),
            "Update failed, try again"
        );
        let items = template(&Input {
            update: &json!({}),
            agent_running: true,
            hidden: false,
            shortcuts: &s,
            manual: &Manual::Idle,
            now: 0,
        });
        assert_eq!(items[0].accelerator.as_deref(), Some("Esc"));
        assert_eq!(items[2].accelerator.as_deref(), Some("Ctrl+Alt+Space"));
        assert_eq!(items[3].accelerator, None);
    }

    #[test]
    fn refresh_only_touches_changed_items() {
        let idle = json!({"status": "idle"});
        let base = items_for(idle.clone(), false, false, Manual::Idle);
        // Nothing changed: no edits at all.
        assert!(diff(&base, &base).is_empty());
        // A manual check: only the check row, text and state.
        let checking = items_for(idle.clone(), false, false, Manual::Checking);
        let i = base.iter().position(|it| it.id == "check").unwrap();
        assert_eq!(
            diff(&base, &checking),
            [
                Change::Text(i, "Checking…".into()),
                Change::Enabled(i, false),
            ]
        );
        // Hiding the overlay: only the toggle's text.
        let hidden = items_for(idle.clone(), false, true, Manual::Idle);
        let t = base.iter().position(|it| it.id == "toggle").unwrap();
        assert_eq!(
            diff(&base, &hidden),
            [Change::Text(t, "Show the overlay".into())]
        );
        // A task starting adds rows: replaced in place, not swapped.
        let running = items_for(idle, true, false, Manual::Idle);
        assert_eq!(diff(&base, &running), [Change::Rows]);
    }

    fn session(id: &str, label: &str, status: &str, can_jump: bool) -> TrayRow {
        TrayRow {
            session: id.into(),
            label: label.into(),
            status: status.into(),
            can_jump,
        }
    }

    #[test]
    fn sessions_lead_the_menu() {
        let base = items_for(json!({"status": "idle"}), true, false, Manual::Idle);
        // No sessions, or agent watching off: the menu as it always was.
        assert_eq!(with_sessions(base.clone(), &[], 0), base);
        let rows = [
            session("a", "site · Codex", "needs you", true),
            session("b", "R&D · VS Code", "done 3m ago", false),
        ];
        let items = with_sessions(base.clone(), &rows, 3);
        let labels: Vec<&str> = items.iter().map(|it| it.label.as_str()).collect();
        assert_eq!(
            &labels[..7],
            [
                "Stop the task",
                "-",
                "site · Codex",
                "R&&D · VS Code",
                "3 more",
                "-",
                "Ask, find, or do…",
            ]
        );
        assert_eq!(text(&items[2]), "site · Codex\tneeds you");
        assert_eq!((items[2].id, items[3].id), ("session-0", "session-1"));
        // A session whose window is gone cannot be clicked, nor the count.
        assert!(items[2].enabled && !items[3].enabled && !items[4].enabled);
        assert_eq!(items.len(), base.len() + 4);
        assert_eq!(slot_of("session-1"), Some(1));
        assert_eq!(slot_of("settings"), None);
        // A minute passing changes one row's text in place.
        let later = [
            rows[0].clone(),
            session("b", "R&D · VS Code", "done 4m ago", false),
        ];
        assert_eq!(
            diff(&items, &with_sessions(base.clone(), &later, 3)),
            [Change::Text(3, "R&&D · VS Code\tdone 4m ago".into())]
        );
        // A session ending replaces the rows inside the same menu.
        let fewer = with_sessions(base, &rows[..1], 0);
        assert_eq!(diff(&items, &fewer), [Change::Rows]);
        // More sessions than rows: the extra ones are only counted.
        let many: Vec<TrayRow> = (0..10)
            .map(|i| session(&format!("s{i}"), "x", "working", true))
            .collect();
        let listed = with_sessions(Vec::new(), &many, 2);
        assert_eq!(listed.len(), SESSION_IDS.len() + 2);
        assert_eq!(listed[7].id, "session-7");
        assert_eq!(listed[8].label, "2 more");
    }

    #[test]
    fn progress_is_throttled_but_other_edits_are_not() {
        let at = |p: f64| json!({"status": "downloading", "version": "1.2.0", "progress": p});
        let a = items_for(at(0.10), false, false, Manual::Idle);
        let b = items_for(at(0.11), false, false, Manual::Idle);
        assert!(progress_only(&b, &diff(&a, &b)));
        let hidden = items_for(at(0.10), false, true, Manual::Idle);
        assert!(!progress_only(&hidden, &diff(&a, &hidden)));
        let idle = items_for(json!({}), false, false, Manual::Idle);
        assert!(!progress_only(&a, &diff(&idle, &a)));
    }

    #[test]
    fn double_click_opens_once() {
        let last = AtomicU64::new(0);
        assert!(opens(&last, 10_000, 500));
        // The second click of the same double click.
        assert!(!opens(&last, 10_200, 500));
        assert!(opens(&last, 11_000, 500));
    }

    #[test]
    fn second_check_keeps_its_ten_seconds() {
        let first = Manual::Current {
            until: 10_000,
            version: "1.1.1".into(),
        };
        let second = Manual::Current {
            until: 16_000,
            version: "1.1.1".into(),
        };
        let mut m = second.clone();
        // The first check's timer fires while the second's answer shows.
        assert!(!expire(&mut m, &first));
        assert_eq!(m, second);
        assert!(expire(&mut m, &second));
        assert_eq!(m, Manual::Idle);
    }

    #[test]
    fn icon_matches_scale() {
        assert_eq!(icon_size(1.0), 16);
        assert_eq!(icon_size(1.25), 20);
        assert_eq!(icon_size(1.5), 24);
        assert_eq!(icon_size(1.75), 32);
        assert_eq!(icon_size(2.0), 32);
        assert_eq!(icon_size(3.0), 32);
    }

    #[test]
    fn tooltips() {
        let s = shortcuts();
        let ready = json!({"status": "ready", "version": "2.0.0"});
        assert_eq!(
            tooltip(&ready, true, &s),
            "AevalBuddy · 2.0.0 is ready, restart to update"
        );
        assert_eq!(
            tooltip(&json!({}), true, &s),
            "AevalBuddy is using your computer · Esc to stop"
        );
        assert_eq!(
            tooltip(&json!({}), false, &s),
            "AevalBuddy · Ctrl+Alt+Space"
        );
        assert_eq!(tooltip(&json!({}), false, &Map::new()), "AevalBuddy");
    }

    #[test]
    fn icons_decode() {
        for (size, b64) in ICONS {
            let bytes = B64.decode(b64).unwrap();
            assert_eq!(&bytes[1..4], b"PNG");
            let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
            assert_eq!(w, size);
        }
    }
}
