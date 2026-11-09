//! The platform lane's smoke labels, listed in `tools/smoke-labels/platform.txt`.
//! `selftest/mod.rs` documents the probe API, with a worked example.
//!
//! Semantics come from main.js's runSmoke and lib/smoke-v2*.js, as worked out
//! in `.claude/rust-rewrite/audit/parity-ipc.md` section 3.

use std::fmt::Display;
use std::fs;
use std::process::{Child, Command, Stdio};
use std::ptr::{null, null_mut};
use std::time::Duration;

use serde_json::{json, Map, Value};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut};
use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows_sys::Win32::UI::Shell::GetCurrentProcessExplicitAppUserModelID;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    PostMessageW, WM_CLOSE, WM_KEYDOWN,
};

use crate::host::menu;
use crate::selftest::{Cx, Input};
use crate::state::{shared, Mode};
use crate::tray::{self, Manual};
use crate::windows::{self, flags, OVERLAY};
use crate::{autostart, crash, history, hotkeys, paths, store, updater};

const SECOND: Duration = Duration::from_secs(1);
const AUMID: &str = "com.aevalbuddy.app";
const MAIN_CHORD: &str = "Ctrl+Alt+Space";
const ACTIONS: [&str; 5] = [
    "captureQuery",
    "captureRegion",
    "findOnScreen",
    "agentTask",
    "toggleVisibility",
];
/// Seconds before asking the feed again (main.js:3295).
const FEED_PAUSES: [u64; 2] = [10, 30];
const FEED_TIMEOUT: Duration = Duration::from_secs(45);

pub fn run(cx: &mut Cx) {
    // The foreground check goes first, before anything else takes focus.
    single_instance(cx);
    cx.must("tray", tray::is_ready(&cx.app), "no way back in");
    let aumid = process_aumid();
    cx.must("aumid", aumid == AUMID, aumid);
    shortcut_labels(cx);
    global_keys(cx);
    escape_suspend(cx);
    capture_mode(cx);
    shortcut_set(cx);
    tray_data(cx);
    tray_click(cx);
    history_labels(cx);
    crash_labels(cx);
    context_menus(cx);
    context_menu_editable(cx);
    launch_at_login(cx);
    navigation_blocked(cx);
    update_labels(cx);
    // Last: a second copy of the app comes up and takes the screen for a
    // moment, which the other checks should not have to share.
    quit_on_last_close(cx);
}

/// Prints the expected value as Electron did, or fails with what it got.
fn expect(cx: &Cx, label: &str, got: impl Display, want: &str) {
    let got = got.to_string();
    if got == want {
        cx.log(label, got);
    } else {
        cx.must(label, false, format!("{got}, want {want}"));
    }
}

fn is_panel(cx: &Cx) -> bool {
    cx.wait_for(3 * SECOND, || shared(&cx.app).lock().mode == Mode::Panel)
}

fn open_bar(cx: &Cx) -> bool {
    windows::open_bar(&cx.app, None, None);
    is_panel(cx)
}

fn close_bar(cx: &Cx) {
    windows::collapse_panel(&cx.app);
    cx.wait_for(3 * SECOND, || shared(&cx.app).lock().mode == Mode::Pill);
}

// ------------------------------------------------------------- lifecycle

fn process_aumid() -> String {
    let mut text = null_mut();
    // SAFETY: an out pointer to a live local; on success it holds a
    // NUL-terminated string this call owns until CoTaskMemFree.
    unsafe {
        if GetCurrentProcessExplicitAppUserModelID(&mut text) != 0 || text.is_null() {
            return String::new();
        }
        let len = (0..).take_while(|i| *text.add(*i) != 0).count();
        let id = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
        CoTaskMemFree(text.cast_const().cast());
        id
    }
}

/// Another copy of this exe, with no self-test and nothing of its output
/// reaching the smoke log.
fn this_app() -> Result<Command, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = Command::new(exe);
    command
        .env_remove("AEVAL_SMOKE")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Ok(command)
}

/// Waits for `child` to end and answers its exit code; kills it past `timeout`.
fn wait_exit(cx: &Cx, child: &mut Child, timeout: Duration) -> Option<i32> {
    let mut code = None;
    let ended = cx.wait_for(timeout, || match child.try_wait() {
        Ok(Some(status)) => {
            code = status.code();
            true
        }
        Ok(None) => false,
        Err(_) => true,
    });
    if !ended {
        let _ = child.kill();
        let _ = child.wait();
    }
    code
}

/// LIFE-01, B-012: a second launch on the same profile leaves, and the
/// running copy's bar comes forward with keyboard focus.
fn single_instance(cx: &Cx) {
    let spawned = this_app().and_then(|mut c| c.spawn().map_err(|e| e.to_string()));
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => return cx.must("singleInstance", false, e),
    };
    let code = wait_exit(cx, &mut child, 20 * SECOND);
    let panel = is_panel(cx);
    let front = cx.wait_for(2 * SECOND, || {
        flags::foreground_label().as_deref() == Some(OVERLAY)
    });
    let ok = code == Some(0) && panel && front;
    let why = format!("exit {code:?}, panel {panel}, foreground {front}");
    cx.must("singleInstance", ok, why);
    close_bar(cx);
}

struct Found {
    pid: u32,
    windows: Vec<HWND>,
}

/// Collects the top-level windows of one process that carry the app's title.
///
/// SAFETY: `data` is the `Found` that `app_windows` passes, alive for the
/// whole EnumWindows call, and `hwnd` comes from Windows.
unsafe extern "system" fn collect(hwnd: HWND, data: LPARAM) -> BOOL {
    let found = &mut *(data as *mut Found);
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    let mut title = [0u16; 64];
    let len = GetWindowTextW(hwnd, title.as_mut_ptr(), 64).max(0) as usize;
    if pid == found.pid && String::from_utf16_lossy(&title[..len]).starts_with("AevalBuddy") {
        found.windows.push(hwnd);
    }
    TRUE
}

fn app_windows(pid: u32) -> Vec<HWND> {
    let mut found = Found {
        pid,
        windows: Vec::new(),
    };
    // SAFETY: `collect` only uses `found`, which outlives this call.
    unsafe { EnumWindows(Some(collect), &mut found as *mut Found as LPARAM) };
    found.windows
}

/// LIFE-09: closing every window ends the process cleanly. A second copy on
/// its own profile, so this run keeps its windows.
fn quit_on_last_close(cx: &Cx) {
    let dir = std::env::temp_dir().join(format!("aeval-lastclose-{}", std::process::id()));
    let spawned = this_app().and_then(|mut c| {
        c.env("AEVAL_USERDATA", &dir)
            .env("AEVAL_OFFLINE", "1")
            .spawn()
            .map_err(|e| e.to_string())
    });
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => return cx.must("quitOnLastClose", false, e),
    };
    let pid = child.id();
    let up = cx.wait_for(40 * SECOND, || !app_windows(pid).is_empty());
    cx.sleep(3000);
    let mut code = None;
    let ended = cx.wait_for(30 * SECOND, || {
        for hwnd in app_windows(pid) {
            // SAFETY: a plain message post; a window that has gone just
            // makes it fail.
            unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
        }
        cx.sleep(250);
        match child.try_wait() {
            Ok(Some(status)) => {
                code = status.code();
                true
            }
            _ => false,
        }
    });
    if !ended {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = fs::remove_dir_all(&dir);
    let why = format!("windows up {up}, ended {ended}, exit {code:?}");
    cx.must("quitOnLastClose", up && ended && code == Some(0), why);
}

// --------------------------------------------------------------- hotkeys

fn chord_held(cx: &Cx, chord: &str) -> bool {
    hotkeys::parse(chord).is_ok_and(|(mods, code)| {
        cx.app
            .global_shortcut()
            .is_registered(Shortcut::new(Some(mods), code))
    })
}

fn key_held(cx: &Cx, code: Code) -> bool {
    cx.app
        .global_shortcut()
        .is_registered(Shortcut::new(None, code))
}

fn shortcut_labels(cx: &Cx) {
    let reg = hotkeys::active(&cx.app);
    for action in ["captureQuery", "captureRegion", "toggleVisibility"] {
        cx.log(&format!("shortcut.{action}"), &reg[action]);
    }
    let line = ACTIONS.map(|a| format!("{a}:{}", reg[a])).join(",");
    cx.log("shortcuts", line);
    let keys = Value::Object(store::get().get_shortcuts());
    let others_empty = ACTIONS[1..]
        .iter()
        .all(|a| keys[*a] == "" && reg[*a].is_null());
    let one = keys["captureQuery"] == MAIN_CHORD && reg["captureQuery"] == true && others_empty;
    cx.must("shortcutsOneChord", one, format!("{keys} {reg}"));
}

fn global_keys(cx: &Cx) {
    let idle = key_held(cx, Code::Escape);
    open_bar(cx);
    let enter = key_held(cx, Code::Enter);
    let bar = key_held(cx, Code::Escape);
    close_bar(cx);
    cx.must("noGlobalEnter", !enter, "Enter is registered globally");
    let why = format!("bar={bar} idle={idle}");
    cx.must("noGlobalEscWithBar", !bar && !idle, why);
}

fn in_hud(cx: &Cx) -> bool {
    shared(&cx.app).lock().mode == Mode::Hud
}

/// Enters the layout editor, which holds the global Escape.
fn hud_with_escape(cx: &Cx) -> bool {
    if !in_hud(cx) {
        windows::enter_hud(&cx.app);
    }
    cx.wait_for(3 * SECOND, || key_held(cx, Code::Escape))
}

/// KEY-09, B-011: two injected Escapes that overlap never reach our global
/// Escape, it comes back only after the last, and with it back the same key
/// does reach us (so the zero means something).
fn escape_suspend(cx: &Cx) {
    let app = &cx.app;
    if !hud_with_escape(cx) {
        windows::exit_hud(app);
        return cx.must("escapeSuspend", false, "Escape not held in the editor");
    }
    let before = hotkeys::escape_fired();
    hotkeys::suspend_escape(app);
    let off = !key_held(cx, Code::Escape);
    let first = cx.input(Input::Key("Escape"));
    // The second key starts before the first has resumed.
    hotkeys::suspend_escape(app);
    hotkeys::resume_escape(app);
    let still_off = !key_held(cx, Code::Escape);
    let second = cx.input(Input::Key("Escape"));
    cx.sleep(60);
    hotkeys::resume_escape(app);
    cx.sleep(200);
    let fired = hotkeys::escape_fired() - before;
    // The page may have taken the keys and left the editor; back in, the
    // last resume must have given Escape back.
    let back = hud_with_escape(cx);
    let control = hotkeys::escape_fired();
    let sent = cx.input(Input::Key("Escape"));
    let reached = cx.wait_for(3 * SECOND, || hotkeys::escape_fired() > control);
    let left = cx.wait_for(3 * SECOND, || !in_hud(cx));
    if !left {
        windows::exit_hud(app);
    }
    let ok = off
        && still_off
        && first.is_ok()
        && second.is_ok()
        && fired == 0
        && back
        && sent.is_ok()
        && reached;
    let why = format!(
        "off {off}, still off {still_off}, fired {fired}, back {back}, control {reached}, \
         keys {first:?} {second:?} {sent:?}"
    );
    cx.must("escapeSuspend", ok, why);
}

/// KEY-11: recording a combo suspends every global shortcut.
fn capture_mode(cx: &Cx) {
    let before = chord_held(cx, MAIN_CHORD);
    hotkeys::set_capture_mode(&cx.app, true);
    let let_go = cx.wait_for(2 * SECOND, || !chord_held(cx, MAIN_CHORD));
    hotkeys::set_capture_mode(&cx.app, false);
    let back = cx.wait_for(2 * SECOND, || chord_held(cx, MAIN_CHORD));
    let why = format!("before {before}, let go {let_go}, back {back}");
    cx.must("captureMode", before && let_go && back, why);
}

/// KEY-12, KEY-13.
fn shortcut_set(cx: &Cx) {
    let app = &cx.app;
    let chord = "Ctrl+Alt+Shift+K";
    let unknown = hotkeys::set(app, "noSuchAction", "Ctrl+Alt+K");
    let no_modifier = hotkeys::set(app, "captureRegion", "K");
    let clash = hotkeys::set(app, "captureRegion", MAIN_CHORD);
    let clash_text = clash["error"].as_str().unwrap_or_default();
    let good = hotkeys::set(app, "captureRegion", chord);
    let held = chord_held(cx, chord);
    let ok = unknown["error"] == "Unknown action."
        && no_modifier["ok"] == false
        && clash_text.starts_with("Already used by")
        && good["ok"] == true
        && good["active"]["captureRegion"] == true
        && held;
    let why = format!("{unknown} {no_modifier} {clash} {good} held {held}");
    cx.must("shortcutSet", ok, why);

    let reset = hotkeys::reset(app);
    let released = !chord_held(cx, chord);
    let ok = reset["ok"] == true
        && reset["shortcuts"]["captureRegion"] == ""
        && reset["active"]["captureRegion"].is_null()
        && reset["active"]["captureQuery"] == true
        && released;
    let why = format!("{reset} released {released}");
    cx.must("shortcutReset", ok, why);
}

// ------------------------------------------------------------------ tray

fn tray_input<'a>(
    update: &'a Value,
    agent_running: bool,
    shortcuts: &'a Map<String, Value>,
    manual: &'a Manual,
    now: u64,
) -> tray::Input<'a> {
    tray::Input {
        update,
        agent_running,
        hidden: false,
        shortcuts,
        manual,
        now,
    }
}

fn tray_data(cx: &Cx) {
    let s = store::default_shortcuts();
    let none = Manual::Idle;
    let plain = json!({ "status": "idle" });
    let labels = |u: &Value, on| tray::labels(&tray_input(u, on, &s, &none, 0));

    let idle = labels(&plain, false);
    let running = labels(&plain, true);
    let update = json!({ "status": "ready", "version": "9.9.9" });
    let ready = labels(&update, false);
    let base = [
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
    ];
    let with = |first: &str| {
        let mut all = vec![first.to_string(), "-".to_string()];
        all.extend(base.iter().map(|l| l.to_string()));
        all
    };
    let ok = idle == base
        && running == with("Stop the task")
        && ready == with("Restart to update to 9.9.9");
    let why = format!("{} // {running:?} // {ready:?}", idle.join("|"));
    cx.must("trayOrder", ok, why);

    // TRAY-04: shown beside the label, never registered.
    let mut chords = Map::new();
    chords.insert("captureQuery".into(), json!(MAIN_CHORD));
    chords.insert("captureRegion".into(), json!("Ctrl+Alt+Shift+J"));
    let items = tray::template(&tray_input(&plain, true, &chords, &none, 0));
    let accel = |id: &str| {
        let item = items.iter().find(|i| i.id == id);
        item.and_then(|i| i.accelerator.clone())
    };
    let shown = accel("stop").as_deref() == Some("Esc")
        && accel("capture").as_deref() == Some(MAIN_CHORD)
        && accel("region").as_deref() == Some("Ctrl+Alt+Shift+J")
        && accel("find").is_none();
    let registered = chord_held(cx, "Ctrl+Alt+Shift+J");
    let why = format!("{items:?} registered {registered}");
    cx.must("trayAccel", shown && !registered, why);

    let checking = Manual::Checking;
    let failed = Manual::Failed;
    let current = Manual::Current {
        until: 500,
        version: "1.1.1".into(),
    };
    let check = |update: &Value, manual: &Manual, now| {
        let items = tray::template(&tray_input(update, false, &s, manual, now));
        let row = items.into_iter().find(|i| i.id == "check");
        row.map(|i| (i.label, i.enabled))
    };
    let down = json!({ "status": "downloading", "version": "1.2.0", "progress": 0.426 });
    let states = [
        (check(&plain, &checking, 0), "Checking…", false),
        (check(&down, &none, 0), "Downloading 1.2.0… 42%", false),
        (check(&plain, &current, 100), "Up to date (1.1.1)", false),
        (check(&plain, &current, 900), "Check for updates", true),
        (check(&plain, &failed, 0), "Update failed, try again", true),
        (check(&plain, &none, 0), "Check for updates", true),
    ];
    let wrong: Vec<String> = states
        .iter()
        .filter(|(got, label, on)| *got != Some((label.to_string(), *on)))
        .map(|(got, label, _)| format!("{got:?} for {label}"))
        .collect();
    cx.must("trayCheckStates", wrong.is_empty(), wrong.join("; "));

    let tips = [
        tray::tooltip(&update, true, &s),
        tray::tooltip(&plain, true, &s),
        tray::tooltip(&plain, false, &s),
        tray::tooltip(&plain, false, &Map::new()),
    ];
    let want = [
        "AevalBuddy · 9.9.9 is ready, restart to update",
        "AevalBuddy is using your computer · Esc to stop",
        "AevalBuddy · Ctrl+Alt+Space",
        "AevalBuddy",
    ];
    cx.must("trayTooltip", tips == want, tips.join(" | "));
}

/// TRAY-02, D-07: one double click on the icon opens the bar once.
fn tray_click(cx: &Cx) {
    let before = tray::click_opens();
    tray::deliver_double_click(&cx.app);
    let panel = is_panel(cx);
    let opens = tray::click_opens() - before;
    close_bar(cx);
    let why = format!("{opens} opens, panel {panel}");
    cx.must("trayClick", opens == 1 && panel, why);
}

// --------------------------------------------------------------- history

fn turn(role: &str, text: &str) -> Value {
    json!({ "role": role, "text": text })
}

fn history_labels(cx: &Cx) {
    let h = history::get();
    h.set_enabled(true);
    h.clear();
    let mut chat = vec![
        turn("user", "why is the build red"),
        turn("assistant", "A peer dependency is missing."),
    ];
    h.save("smoke-1", "test", &chat);
    let other = [turn("user", "reword this paragraph")];
    h.save("smoke-2", "test", &other);
    // The same id twice is an edit of one conversation, not a second row.
    chat.push(turn("user", "and the warning under it?"));
    h.save("smoke-1", "test", &chat);
    let kept = h
        .entry("smoke-1")
        .and_then(|e| e["messages"].as_array().map(Vec::len));
    let saved = format!("{}:{}", h.count(), kept.unwrap_or(0));
    expect(cx, "historySaved", saved, "2:3");
    let rows = h.list("");
    let title = rows.first().map_or(Value::Null, |r| r["title"].clone());
    let named = title == "reword this paragraph" || title == "why is the build red";
    cx.must("historyTitle", named, &title);
    // Search reaches the bodies, not only the titles.
    let found = h.list("peer dependency").len();
    let none = h.list("nothing here at all").len();
    expect(cx, "historySearch", format!("{found}:{none}"), "1:0");
    h.remove("smoke-2");
    expect(cx, "historyRemove", h.count(), "1");
    // Off means no new writes, and does not delete what is already there.
    h.set_enabled(false);
    h.save("smoke-3", "test", &[turn("user", "nope")]);
    let written = if h.entry("smoke-3").is_some() {
        "wrote"
    } else {
        "skipped"
    };
    let line = format!("{}:{written}", h.count());
    expect(cx, "historyRespectsSwitch", line, "1:skipped");
    h.set_enabled(store::get().get_save_history());
    h.clear();
    expect(cx, "historyCleared", h.count(), "0");

    // LIFE-11: quitting writes now, without waiting for the 400 ms throttle.
    // Pending writes settle first, so the file can only have it from the
    // flush.
    cx.sleep(600);
    h.set_enabled(true);
    h.save("smoke-flush", "test", &[turn("user", "flushed on quit")]);
    h.flush_now();
    let file = paths::user_data_dir().join("history.json");
    let text = fs::read_to_string(&file).unwrap_or_default();
    cx.must(
        "historyFlushOnQuit",
        text.contains("smoke-flush"),
        file.display(),
    );
    h.set_enabled(store::get().get_save_history());
    h.clear();
    h.flush_now();
}

// ---------------------------------------------------------------- crashes

fn log_has(marker: &str) -> bool {
    crash::file()
        .and_then(|p| fs::read_to_string(p).ok())
        .is_some_and(|text| text.contains(marker))
}

fn deliberate_panic(message: &str) {
    panic!("{message}");
}

fn crash_labels(cx: &Cx) {
    const TASK: &str = "smoke: deliberate task panic";
    tauri::async_runtime::spawn(async { deliberate_panic(TASK) });
    let logged = cx.wait_for(3 * SECOND, || log_has(TASK));
    cx.must("crashLogged", logged, "task panic not in errors.log");

    // B-013: stopped on the main thread, logged, and the app keeps running.
    const MAIN: &str = "smoke: deliberate main-thread panic";
    let stopped = cx.on_main(|_| crash::guard("smoke", || deliberate_panic(MAIN)));
    let alive = cx.on_main(|_| 1);
    let written = log_has(MAIN);
    let ok = stopped == Some(None) && alive == Some(1) && written;
    let why = format!("guard {stopped:?}, alive {alive:?}, logged {written}");
    cx.must("crashMainThread", ok, why);
}

// ------------------------------------------------------------------ menus

fn context_menus(cx: &Cx) {
    let labels = |p: Value| menu::item_labels(&p);
    let answer = labels(json!({ "kind": "answer", "text": "a", "question": "q" }));
    let hit = labels(json!({ "kind": "hit", "text": "a", "index": 0 }));
    let ended = labels(json!({ "kind": "row", "text": "a", "ended": true, "task": "t" }));
    let open = labels(json!({ "kind": "row", "text": "a", "ended": false, "task": "t" }));
    let other = labels(json!({ "kind": "other" }));
    let ok = answer == ["Copy", "Find this on screen", "Do it"]
        && hit == ["Go to", "Copy text", "Ask about this"]
        && ended == ["Copy step", "Retry from here"]
        && open == ["Copy step"]
        && other.is_empty();
    let why = format!("{answer:?} {hit:?} {ended:?} {open:?} {other:?}");
    cx.must("contextMenus", ok, why);
}

/// The popup menu of this process on screen, if any. `#32768` is the class
/// of every Windows menu window.
fn open_menu() -> Option<HWND> {
    let class: Vec<u16> = "#32768\0".encode_utf16().collect();
    let mut after: HWND = null_mut();
    loop {
        // SAFETY: `class` is NUL-terminated and alive; the handles come from
        // Windows and are only passed back to it.
        unsafe {
            after = FindWindowExW(null_mut(), after, class.as_ptr(), null());
            if after.is_null() {
                return None;
            }
            let mut pid = 0;
            GetWindowThreadProcessId(after, &mut pid);
            if pid == GetCurrentProcessId() && IsWindowVisible(after) != 0 {
                return Some(after);
            }
        }
    }
}

/// Closes an open menu from this thread. Its modal loop runs inside one
/// event-loop callback, so nothing sent to the main thread runs until the
/// menu ends and `EndMenu` could never get there; an Escape posted to the
/// menu window reaches the loop directly.
fn dismiss_menu(cx: &Cx) -> bool {
    let closed = cx.wait_for(3 * SECOND, || match open_menu() {
        Some(menu) => {
            // SAFETY: a plain message post; a menu that has gone just makes
            // it fail.
            unsafe { PostMessageW(menu, WM_KEYDOWN, usize::from(VK_ESCAPE), 0) };
            false
        }
        None => true,
    });
    if closed {
        return true;
    }
    // Never leave a modal menu up: every later probe would wait behind it.
    let _ = cx.input(Input::Key("Escape"));
    cx.wait_for(2 * SECOND, || open_menu().is_none())
}

/// Runs a script that right-clicks by event, waits for the native menu, then
/// closes it. The scripts wait 50 ms first, so `eval` has its answer before
/// the menu's modal loop starts.
fn menu_for(cx: &Cx, script: &str) -> Result<bool, String> {
    let sent = cx.eval(OVERLAY, script);
    let shown = cx.wait_for(3 * SECOND, || open_menu().is_some());
    let gone = dismiss_menu(cx);
    sent.map(|_| shown && gone)
}

/// WIN-14, B-077: a text field and a selection each get a native menu from
/// the bridge, with the templates main.js used.
fn context_menu_editable(cx: &Cx) {
    let data = menu::edit_template("edit") == ["cut", "copy", "paste", "-", "selectAll"]
        && menu::edit_template("selection") == ["copy", "selectAll"];
    let opened = open_bar(cx);
    let field = menu_for(
        cx,
        "(() => { setTimeout(() => { document.getElementById('query-input').dispatchEvent(\
         new MouseEvent('contextmenu', { bubbles: true, cancelable: true })); }, 50); \
         return true; })()",
    );
    let selection = menu_for(
        cx,
        "(() => { setTimeout(() => { const s = document.createElement('span'); \
         s.textContent = 'smoke selection'; s.style.userSelect = 'text'; \
         document.body.appendChild(s); getSelection().selectAllChildren(s); \
         s.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true })); \
         setTimeout(() => { getSelection().removeAllRanges(); s.remove(); }, 0); }, 50); \
         return true; })()",
    );
    close_bar(cx);
    let ok = data && opened && field == Ok(true) && selection == Ok(true);
    let why = format!("template {data}, bar {opened}, field {field:?}, selection {selection:?}");
    cx.must("contextMenuEditable", ok, why);
}

// ---------------------------------------------------------------- system

/// SYS-03, B-081: on the Settings path the Run value appears and is marked
/// enabled; off removes it.
fn launch_at_login(cx: &Cx) {
    if !autostart::live() {
        return cx.log("launchAtLogin", "skipped (dev build)");
    }
    let s = store::get();
    let was = s.get_launch_at_login();
    s.set_launch_at_login(false);
    autostart::apply();
    let off = autostart::registered();
    s.set_launch_at_login(true);
    autostart::apply();
    let on = autostart::registered();
    let approved = autostart::approved();
    s.set_launch_at_login(was);
    autostart::apply();
    let want = std::env::current_exe()
        .ok()
        .map(|e| autostart::run_value(&e));
    let ok = off.is_none()
        && on.is_some()
        && on == want
        && approved.as_deref() == Some(&autostart::APPROVED_ON[..]);
    let why = format!("off {off:?}, on {on:?}, want {want:?}, approved {approved:?}");
    cx.must("launchAtLogin", ok, why);
}

/// B-078: a page cannot navigate off the app.
fn navigation_blocked(cx: &Cx) {
    let before = cx.eval(OVERLAY, "location.href");
    let sent = cx.eval(
        OVERLAY,
        "(() => { location.href = 'https://example.com/'; return true; })()",
    );
    cx.sleep(1000);
    let after = cx.eval(OVERLAY, "location.href");
    let ok = sent.is_ok() && before.is_ok() && before == after;
    let why = format!("{before:?} {sent:?} {after:?}");
    cx.must("navigationBlocked", ok, why);
}

// ---------------------------------------------------------------- updater

/// The raw cause of the last failed check; the state has only the sentence
/// written for people.
fn last_cause() -> String {
    let file = paths::user_data_dir().join("updater.log");
    let Ok(text) = fs::read_to_string(file) else {
        return "no updater.log".into();
    };
    let last = text
        .lines()
        .rfind(|l| l.contains(" warn ") || l.contains(" error "));
    last.unwrap_or("-").chars().take(400).collect()
}

/// A 5xx from the feed, or a check that never settled: asked again. The log
/// line reads `server (attempt 1): 502 502 Bad Gateway for <url>`.
fn transient(update: &Value) -> bool {
    if update["status"] == "timeout" {
        return true;
    }
    let again = regex::Regex::new(r"(?i)\): 5\d\d\b|timed out|ETIMEDOUT");
    update["status"] == "retrying" && again.is_ok_and(|r| r.is_match(&last_cause()))
}

fn check_once(cx: &Cx) -> Value {
    let app = cx.app.clone();
    cx.block_on(async move {
        match tokio::time::timeout(FEED_TIMEOUT, updater::check(&app)).await {
            Ok(state) => state,
            Err(_) => json!({ "status": "timeout", "problem": "" }),
        }
    })
}

fn or_dash(v: &Value) -> &str {
    v.as_str().filter(|s| !s.is_empty()).unwrap_or("-")
}

/// UPD-05, UPD-10, UPD-12, B-088 (main.js:3244-3320).
fn update_labels(cx: &Cx) {
    let feed = updater::feed_config();
    let ok = feed["provider"] == "github"
        && feed["owner"] == "aevalmere"
        && feed["repo"] == "aevalbuddy"
        && feed["private"] == false;
    cx.must("updateConfig", ok, &feed);

    let kinds = [
        updater::classify("net::ERR_INTERNET_DISCONNECTED", None),
        updater::classify("429 Too Many Requests", Some(429)),
        updater::classify("sha512 checksum mismatch, expected a, got b", None),
        updater::classify("HttpError: 502 Bad Gateway", None),
    ];
    let ok = kinds == ["offline", "limited", "corrupt", "server"];
    cx.must("updateClassify", ok, kinds.join(","));

    let notes = updater::notes_snippet(
        "<h2>What&#39;s new</h2><ul><li>Faster <b>area</b> capture</li>\
         <li>Fix &amp; tidy</li></ul><p>Install</p>",
    );
    let ok = notes == "Faster area capture · Fix & tidy";
    cx.must("updateNotes", ok, notes);

    let override_of = |feed: &'static str| {
        let env = move |key: &str| match key {
            "AEVAL_UPDATE_FEED" => Some(feed.to_string()),
            "AEVAL_SMOKE" => Some("1".to_string()),
            _ => None,
        };
        updater::feed_override_from(env, true)
    };
    let remote = override_of("http://evil.example/feed");
    let creds = override_of("https://user:pw@example.com/feed");
    // A good feed is accepted, so the two empties above mean something.
    let good = override_of("https://example.com/feed");
    let ok = remote.is_empty() && creds.is_empty() && good == "https://example.com/feed/";
    cx.must("feedOverrideStrict", ok, format!("{remote} {creds} {good}"));

    let mut attempts = Vec::new();
    let mut retries = 0;
    let state = loop {
        let state = check_once(cx);
        attempts.push(state["status"].as_str().unwrap_or_default().to_string());
        if retries >= FEED_PAUSES.len() || !transient(&state) {
            break state;
        }
        cx.log(&format!("update.retry{}", retries + 1), last_cause());
        cx.sleep(FEED_PAUSES[retries] * 1000);
        retries += 1;
    };
    let status = state["status"].as_str().unwrap_or_default();
    let problem = state["problem"].as_str().unwrap_or_default();
    cx.log("update.attempts", attempts.join(","));
    cx.log("update.status", status);
    cx.log("update.version", or_dash(&state["version"]));
    cx.log("update.problem", or_dash(&state["problem"]));
    let expected = std::env::var("AEVAL_UPDATE_EXPECT")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "current,downloading,ready".to_string());
    let ok = expected == "any" || expected.split(',').any(|s| s == status);
    cx.must("updateFeed", ok, format!("{status}: {problem}"));
    let quiet = regex::Regex::new(r"(?i)error|exception|stack|http");
    let loud = quiet.is_ok_and(|r| r.is_match(problem));
    cx.must("updateQuiet", !loud, problem);
    if status == "retrying" || status == "error" {
        cx.log("update.cause", last_cause());
    }
}
