//! Global shortcuts (PARITY KEY-01..15, ARCHITECTURE.md D7).
//!
//! Chords are stored in Electron's accelerator syntax and parsed here into
//! RegisterHotKey modifiers and a key, never rewritten. Escape is global only
//! while it has to work wherever focus is (agent running, tour or pointer,
//! area selector, layout editor); Enter never is.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU32, Ordering::SeqCst};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};

use crate::state::{shared, Mode};
use crate::store::{self, ACTIONS};

/// KEY-15.
pub fn action_label(action: &str) -> Option<&'static str> {
    Some(match action {
        "captureQuery" => "Ask, find, or do",
        "captureRegion" => "Ask about an area",
        "findOnScreen" => "Find on screen",
        "agentTask" => "Do a task",
        "toggleVisibility" => "Hide or show the overlay",
        _ => return None,
    })
}

/// Parses an Electron accelerator (`Ctrl+Alt+Space`, `CommandOrControl+A`,
/// `Super+Shift+F5`, `Ctrl+Plus`, `Alt+!`...) for Windows.
pub fn parse(accelerator: &str) -> Result<(Modifiers, Code), String> {
    let acc = accelerator.trim();
    if acc.is_empty() {
        return Err("empty accelerator".into());
    }
    let mut tokens: Vec<String> = acc.split('+').map(|t| t.trim().to_string()).collect();
    // "Ctrl++" names the plus key.
    if acc.ends_with("++") {
        tokens.truncate(tokens.len().saturating_sub(2));
        tokens.push("Plus".into());
    }
    let mut mods = Modifiers::empty();
    let mut key: Option<(Modifiers, Code)> = None;
    for t in tokens {
        if t.is_empty() {
            return Err(format!("empty part in {acc}"));
        }
        if let Some(m) = modifier(&t) {
            mods |= m;
            continue;
        }
        if key.is_some() {
            return Err(format!("more than one key in {acc}"));
        }
        key = Some(key_code(&t).ok_or_else(|| format!("unknown key {t}"))?);
    }
    let (shift, code) = key.ok_or_else(|| format!("no key in {acc}"))?;
    Ok((mods | shift, code))
}

fn modifier(t: &str) -> Option<Modifiers> {
    Some(match t.to_ascii_lowercase().as_str() {
        "ctrl" | "control" | "commandorcontrol" | "cmdorctrl" => Modifiers::CONTROL,
        "alt" | "option" => Modifiers::ALT,
        // AltGr is Ctrl+Alt on Windows.
        "altgr" => Modifiers::CONTROL | Modifiers::ALT,
        "shift" => Modifiers::SHIFT,
        "super" | "meta" | "command" | "cmd" => Modifiers::SUPER,
        _ => return None,
    })
}

/// A key name, with the Shift a shifted character implies.
fn key_code(t: &str) -> Option<(Modifiers, Code)> {
    let none = Modifiers::empty();
    let shift = Modifiers::SHIFT;
    let lower = t.to_ascii_lowercase();
    if t.chars().count() == 1 {
        let c = t.chars().next()?;
        if c.is_ascii_alphabetic() {
            return letter(c.to_ascii_uppercase()).map(|k| (none, k));
        }
        if c.is_ascii_digit() {
            return digit(c).map(|k| (none, k));
        }
        let shifted_digit = ")!@#$%^&*(".find(c);
        if let Some(i) = shifted_digit {
            return digit(char::from(b'0' + i as u8)).map(|k| (shift, k));
        }
        return Some(match c {
            ';' => (none, Code::Semicolon),
            ':' => (shift, Code::Semicolon),
            '=' => (none, Code::Equal),
            ',' => (none, Code::Comma),
            '<' => (shift, Code::Comma),
            '-' => (none, Code::Minus),
            '_' => (shift, Code::Minus),
            '.' => (none, Code::Period),
            '>' => (shift, Code::Period),
            '/' => (none, Code::Slash),
            '?' => (shift, Code::Slash),
            '`' => (none, Code::Backquote),
            '~' => (shift, Code::Backquote),
            '[' => (none, Code::BracketLeft),
            '{' => (shift, Code::BracketLeft),
            ']' => (none, Code::BracketRight),
            '}' => (shift, Code::BracketRight),
            '\\' => (none, Code::Backslash),
            '|' => (shift, Code::Backslash),
            '\'' => (none, Code::Quote),
            '"' => (shift, Code::Quote),
            _ => return None,
        });
    }
    if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
        return function_key(n).map(|k| (none, k));
    }
    if let Some(n) = lower.strip_prefix("num").and_then(|n| n.parse::<u8>().ok()) {
        return numpad(n).map(|k| (none, k));
    }
    Some(match lower.as_str() {
        "plus" => (shift, Code::Equal),
        "space" => (none, Code::Space),
        "tab" => (none, Code::Tab),
        "capslock" => (none, Code::CapsLock),
        "numlock" => (none, Code::NumLock),
        "scrolllock" => (none, Code::ScrollLock),
        "backspace" => (none, Code::Backspace),
        "delete" => (none, Code::Delete),
        "insert" => (none, Code::Insert),
        "return" | "enter" => (none, Code::Enter),
        "up" => (none, Code::ArrowUp),
        "down" => (none, Code::ArrowDown),
        "left" => (none, Code::ArrowLeft),
        "right" => (none, Code::ArrowRight),
        "home" => (none, Code::Home),
        "end" => (none, Code::End),
        "pageup" => (none, Code::PageUp),
        "pagedown" => (none, Code::PageDown),
        "escape" | "esc" => (none, Code::Escape),
        "volumeup" => (none, Code::AudioVolumeUp),
        "volumedown" => (none, Code::AudioVolumeDown),
        "volumemute" => (none, Code::AudioVolumeMute),
        "medianexttrack" => (none, Code::MediaTrackNext),
        "mediaprevioustrack" => (none, Code::MediaTrackPrevious),
        "mediastop" => (none, Code::MediaStop),
        "mediaplaypause" => (none, Code::MediaPlayPause),
        "printscreen" => (none, Code::PrintScreen),
        "numdec" => (none, Code::NumpadDecimal),
        "numadd" => (none, Code::NumpadAdd),
        "numsub" => (none, Code::NumpadSubtract),
        "nummult" => (none, Code::NumpadMultiply),
        "numdiv" => (none, Code::NumpadDivide),
        _ => return None,
    })
}

fn letter(c: char) -> Option<Code> {
    const KEYS: [Code; 26] = [
        Code::KeyA,
        Code::KeyB,
        Code::KeyC,
        Code::KeyD,
        Code::KeyE,
        Code::KeyF,
        Code::KeyG,
        Code::KeyH,
        Code::KeyI,
        Code::KeyJ,
        Code::KeyK,
        Code::KeyL,
        Code::KeyM,
        Code::KeyN,
        Code::KeyO,
        Code::KeyP,
        Code::KeyQ,
        Code::KeyR,
        Code::KeyS,
        Code::KeyT,
        Code::KeyU,
        Code::KeyV,
        Code::KeyW,
        Code::KeyX,
        Code::KeyY,
        Code::KeyZ,
    ];
    KEYS.get((c as u8).checked_sub(b'A')? as usize).copied()
}

fn digit(c: char) -> Option<Code> {
    const KEYS: [Code; 10] = [
        Code::Digit0,
        Code::Digit1,
        Code::Digit2,
        Code::Digit3,
        Code::Digit4,
        Code::Digit5,
        Code::Digit6,
        Code::Digit7,
        Code::Digit8,
        Code::Digit9,
    ];
    KEYS.get((c as u8).checked_sub(b'0')? as usize).copied()
}

fn numpad(n: u8) -> Option<Code> {
    const KEYS: [Code; 10] = [
        Code::Numpad0,
        Code::Numpad1,
        Code::Numpad2,
        Code::Numpad3,
        Code::Numpad4,
        Code::Numpad5,
        Code::Numpad6,
        Code::Numpad7,
        Code::Numpad8,
        Code::Numpad9,
    ];
    KEYS.get(n as usize).copied()
}

fn function_key(n: u8) -> Option<Code> {
    const KEYS: [Code; 24] = [
        Code::F1,
        Code::F2,
        Code::F3,
        Code::F4,
        Code::F5,
        Code::F6,
        Code::F7,
        Code::F8,
        Code::F9,
        Code::F10,
        Code::F11,
        Code::F12,
        Code::F13,
        Code::F14,
        Code::F15,
        Code::F16,
        Code::F17,
        Code::F18,
        Code::F19,
        Code::F20,
        Code::F21,
        Code::F22,
        Code::F23,
        Code::F24,
    ];
    KEYS.get((n as usize).checked_sub(1)?).copied()
}

/// main.js validAccelerator: a leading Ctrl/Alt/Super (not Shift alone) and
/// a final part that is not itself a modifier.
pub fn valid_accelerator(acc: &str) -> bool {
    if acc.trim().is_empty() {
        return false;
    }
    let lead = [
        "Ctrl",
        "Control",
        "CommandOrControl",
        "Alt",
        "AltGr",
        "Super",
        "Meta",
    ];
    if !lead.iter().any(|m| acc.starts_with(&format!("{m}+"))) {
        return false;
    }
    let key = acc.rsplit('+').next().unwrap_or("");
    let mods = [
        "Ctrl",
        "Control",
        "CommandOrControl",
        "Alt",
        "AltGr",
        "Shift",
        "Super",
        "Meta",
    ];
    !key.is_empty() && !mods.contains(&key)
}

/// Registered shortcut id -> action.
fn bound() -> &'static Mutex<HashMap<u32, String>> {
    static B: OnceLock<Mutex<HashMap<u32, String>>> = OnceLock::new();
    B.get_or_init(|| Mutex::new(HashMap::new()))
}

const ESCAPE: &str = "escape";
const ESCAPE_CHORD: &str = "Escape";
/// How long a caller off the main thread waits for its answer.
const MAIN_WAIT: Duration = Duration::from_secs(5);
/// U48: offered when the chord asked for is taken.
const ALTERNATIVES: [&str; 3] = ["Ctrl+Alt+A", "Super+Shift+Space", "Ctrl+Shift+Space"];
/// C-56.
const TAKEN: &str = "Another app already uses that shortcut. Try another.";
const NOT_CHANGED: &str = "The shortcut was not changed. Try again.";

/// Registration, as `reconcile` needs it: the plugin in the app, a fake in
/// the tests.
trait Keys {
    fn register(&mut self, acc: &str, action: &str) -> bool;
    fn unregister(&mut self, acc: &str);
    fn unregister_all(&mut self);
}

struct Plugin<'a>(&'a AppHandle);

impl Keys for Plugin<'_> {
    fn register(&mut self, acc: &str, action: &str) -> bool {
        let Ok((mods, code)) = parse(acc) else {
            return false;
        };
        let sc = Shortcut::new(Some(mods), code);
        if self.0.global_shortcut().register(sc).is_err() {
            return false;
        }
        bound().lock().insert(sc.id(), action.to_string());
        true
    }

    fn unregister(&mut self, acc: &str) {
        if let Ok((mods, code)) = parse(acc) {
            let sc = Shortcut::new(Some(mods), code);
            let _ = self.0.global_shortcut().unregister(sc);
            bound().lock().remove(&sc.id());
        }
    }

    fn unregister_all(&mut self) {
        let _ = self.0.global_shortcut().unregister_all();
        bound().lock().clear();
    }
}

/// What is registered now. Only the main thread touches it.
#[derive(Default)]
struct Registered {
    /// `(action, chord)` as last applied; None after anything unregistered
    /// everything behind its back.
    chords: Option<Vec<(String, String)>>,
    escape: bool,
}

fn registered() -> &'static Mutex<Registered> {
    static R: OnceLock<Mutex<Registered>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Brings the registrations to `chords` plus Escape when `escape_wanted` says
/// so, touching nothing that is already right: an unchanged set registers
/// nothing, an Escape-only change registers only Escape, and a chord that
/// failed is retried alone. Returns whether `results` changed.
/// `escape_wanted` is read after the chords, just before Escape is
/// registered.
fn reconcile(
    keys: &mut dyn Keys,
    reg: &mut Registered,
    chords: Vec<(String, String)>,
    results: &mut BTreeMap<String, Option<bool>>,
    escape_wanted: impl FnOnce() -> bool,
) -> bool {
    let mut changed = false;
    if reg.chords.as_ref() != Some(&chords) {
        keys.unregister_all();
        reg.escape = false;
        for (action, acc) in &chords {
            let r = (!acc.is_empty()).then(|| keys.register(acc, action));
            results.insert(action.clone(), r);
        }
        reg.chords = Some(chords);
        changed = true;
    } else {
        for (action, acc) in &chords {
            if results.get(action) == Some(&Some(false)) && keys.register(acc, action) {
                results.insert(action.clone(), Some(true));
                changed = true;
            }
        }
    }
    let want = escape_wanted();
    if want && !reg.escape {
        reg.escape = keys.register(ESCAPE_CHORD, ESCAPE);
    } else if !want && reg.escape {
        keys.unregister(ESCAPE_CHORD);
        reg.escape = false;
    }
    changed
}

/// KEY-09: each synthetic key that carries Escape suspends ours; it comes
/// back when the last one is done, so two keys 30 ms apart cannot let the
/// first one's resume retake Escape under the second.
struct Depth(AtomicU32);

impl Depth {
    /// True when this call took Escape away (0 to 1).
    fn suspend(&self) -> bool {
        self.0.fetch_add(1, SeqCst) == 0
    }

    /// True when this call gave it back (1 to 0). A resume with no suspend
    /// is ignored.
    fn resume(&self) -> bool {
        // A compare-exchange loop: `fetch_update` is deprecated on the CI
        // toolchain and its replacement is newer than the crate's MSRV.
        let mut d = self.0.load(SeqCst);
        while d > 0 {
            match self.0.compare_exchange_weak(d, d - 1, SeqCst, SeqCst) {
                Ok(_) => return d == 1,
                Err(now) => d = now,
            }
        }
        false
    }

    fn suspended(&self) -> bool {
        self.0.load(SeqCst) > 0
    }
}

static ESCAPE_DEPTH: Depth = Depth(AtomicU32::new(0));

/// Runs `f` on the main thread (inline when already there; Tauri runs a
/// main-thread task in place) and waits for its answer.
fn on_main_wait<R: Send + 'static>(
    app: &AppHandle,
    f: impl FnOnce(&AppHandle) -> R + Send + 'static,
) -> Option<R> {
    let (tx, rx) = mpsc::channel();
    let a = app.clone();
    let task = move || {
        if let Some(r) = crate::crash::guard("hotkeys", || f(&a)) {
            let _ = tx.send(r);
        }
    };
    if let Err(e) = app.run_on_main_thread(task) {
        eprintln!("[aevalbuddy] shortcuts: not run on the main thread: {e}");
        return None;
    }
    rx.recv_timeout(MAIN_WAIT).ok()
}

/// main.js applyRegistrations: re-derives every global shortcut from state.
/// Every call runs on the main thread, one after another, so two applies
/// never interleave their plugin calls (KEY-09, LIFE-06).
pub fn apply(app: &AppHandle) {
    let a = app.clone();
    let task = move || {
        crate::crash::guard("hotkeys.apply", || apply_now(&a));
    };
    if let Err(e) = app.run_on_main_thread(task) {
        eprintln!("[aevalbuddy] shortcuts: not applied: {e}");
    }
}

fn want_escape(app: &AppHandle) -> bool {
    if ESCAPE_DEPTH.suspended() {
        return false;
    }
    let st = shared(app);
    let s = st.lock();
    let busy = s.agent.running || s.buddy_active || s.region_active || s.mode == Mode::Hud;
    busy && !s.capture_mode
}

/// On the main thread.
fn apply_now(app: &AppHandle) {
    crate::state::refresh(app);
    let st = shared(app);
    let capture_mode = st.lock().capture_mode;
    // Recording a new chord (KEY-11): nothing of ours may fire.
    let chords: Vec<(String, String)> = if capture_mode {
        Vec::new()
    } else {
        let store = store::get();
        ACTIONS
            .iter()
            .map(|a| (a.to_string(), store.shortcut(a)))
            .collect()
    };
    let mut results = st.lock().hotkeys.clone();
    let mut keys = Plugin(app);
    let wanted = || want_escape(app);
    let mut reg = registered().lock();
    let changed = reconcile(&mut keys, &mut reg, chords, &mut results, wanted);
    let escape = reg.escape;
    drop(reg);
    {
        let mut s = st.lock();
        s.escape_registered = escape;
        if changed {
            s.hotkeys = results;
        }
    }
    if changed {
        crate::tray::refresh(app);
    }
}

/// After the boot apply: each chord another app holds, as main.js logged it.
pub fn report_failures(app: &AppHandle) {
    let failed: Vec<String> = shared(app)
        .lock()
        .hotkeys
        .iter()
        .filter(|(_, r)| **r == Some(false))
        .map(|(a, _)| a.clone())
        .collect();
    let store = store::get();
    for action in failed {
        let acc = store.shortcut(&action);
        eprintln!("[aevalbuddy] shortcut {acc} ({action}) could not be registered");
    }
}

/// The result map as `settings:get.active` / `shortcuts:set.active` give it.
pub fn active(app: &AppHandle) -> Value {
    let s = shared(app);
    let s = s.lock();
    let mut m = serde_json::Map::new();
    for a in ACTIONS {
        let v = match s.hotkeys.get(a).copied().flatten() {
            Some(b) => json!(b),
            None if store::get().shortcut(a).is_empty() => Value::Null,
            None => json!(false),
        };
        m.insert(a.into(), v);
    }
    Value::Object(m)
}

/// Whether the main chord failed (for `overlay:state.hotkeyFailed`).
pub fn main_chord_failed(app: &AppHandle) -> bool {
    shared(app)
        .lock()
        .hotkeys
        .get("captureQuery")
        .copied()
        .flatten()
        == Some(false)
}

/// The plugin's handler, wired in lib.rs.
pub fn on_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let action = bound().lock().get(&shortcut.id()).cloned();
    let Some(action) = action else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::windows::on_hotkey(&app, &action);
    });
}

/// KEY-09: while the engine injects a synthetic Escape, ours lets go. Waits
/// until Escape is unregistered, since the key goes out when this returns.
pub fn suspend_escape(app: &AppHandle) {
    if !ESCAPE_DEPTH.suspend() {
        return;
    }
    on_main_wait(app, |a| {
        let mut reg = registered().lock();
        if reg.escape {
            Plugin(a).unregister(ESCAPE_CHORD);
            reg.escape = false;
        }
        drop(reg);
        shared(a).lock().escape_registered = false;
    });
}

/// After the injected key (the desktop client has already waited 60 ms):
/// the last resume retakes Escape if it is still wanted.
pub fn resume_escape(app: &AppHandle) {
    if ESCAPE_DEPTH.resume() {
        apply(app);
    }
}

/// `shortcuts:capture-mode` (KEY-11).
pub fn set_capture_mode(app: &AppHandle, on: bool) {
    shared(app).lock().capture_mode = on;
    apply(app);
}

/// Whether two accelerators name the same keys.
fn same_chord(a: &str, b: &str) -> bool {
    matches!((parse(a), parse(b)), (Ok(x), Ok(y)) if x == y)
}

/// U48: the suggestions no other app holds (per `probe`) and none of ours
/// (`ours`, which includes the chord just refused) uses.
fn free_alternatives(ours: &[String], mut probe: impl FnMut(&str) -> bool) -> Vec<String> {
    ALTERNATIVES
        .iter()
        .filter(|c| !ours.iter().any(|o| same_chord(o, c)))
        .filter(|c| probe(c))
        .map(|c| c.to_string())
        .collect()
}

/// Whether another app holds `acc`. Only meaningful with ours unregistered.
fn probe(app: &AppHandle, acc: &str) -> bool {
    let Ok((m, c)) = parse(acc) else {
        return false;
    };
    let gs = app.global_shortcut();
    let sc = Shortcut::new(Some(m), c);
    let ok = gs.register(sc).is_ok();
    if ok {
        let _ = gs.unregister(sc);
    }
    ok
}

/// `shortcuts:set` (KEY-12).
pub fn set(app: &AppHandle, action: &str, accelerator: &str) -> Value {
    let (action, accelerator) = (action.to_string(), accelerator.to_string());
    match on_main_wait(app, move |a| set_now(a, &action, &accelerator)) {
        Some(answer) => answer,
        None => json!({ "ok": false, "error": NOT_CHANGED }),
    }
}

fn set_now(app: &AppHandle, action: &str, accelerator: &str) -> Value {
    if action_label(action).is_none() {
        return json!({ "ok": false, "error": "Unknown action." });
    }
    if !valid_accelerator(accelerator) {
        return json!({
            "ok": false,
            "error": "Include Ctrl, Alt, or Win plus a key, for example Ctrl+Shift+A.",
        });
    }
    let store = store::get();
    let shortcuts = store.get_shortcuts();
    let clash = shortcuts
        .iter()
        .find(|(other, acc)| other.as_str() != action && acc.as_str() == Some(accelerator));
    if let Some((other, _)) = clash {
        let label = action_label(other).unwrap_or("another action");
        return json!({
            "ok": false,
            "error": format!("Already used by \"{label}\". Pick a different combo."),
        });
    }
    // Probe with everything of ours unregistered, so the probe only fails
    // when another application owns the combo.
    Plugin(app).unregister_all();
    *registered().lock() = Registered::default();
    shared(app).lock().escape_registered = false;
    if !probe(app, accelerator) {
        let mut ours: Vec<String> = shortcuts
            .values()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect();
        ours.push(accelerator.to_string());
        let free = free_alternatives(&ours, |c| probe(app, c));
        apply_now(app);
        return json!({ "ok": false, "error": TAKEN, "free": free });
    }
    store.set_shortcut(action, accelerator);
    apply_now(app);
    json!({ "ok": true, "shortcuts": store.get_shortcuts(), "active": active(app) })
}

/// `shortcuts:reset` (KEY-13).
pub fn reset(app: &AppHandle) -> Value {
    let shortcuts = store::get().reset_shortcuts();
    on_main_wait(app, apply_now);
    json!({ "ok": true, "shortcuts": shortcuts, "active": active(app) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> (Modifiers, Code) {
        parse(s).unwrap_or_else(|e| panic!("{s}: {e}"))
    }

    #[test]
    fn electron_forms() {
        let ca = Modifiers::CONTROL | Modifiers::ALT;
        assert_eq!(p("Ctrl+Alt+Space"), (ca, Code::Space));
        assert_eq!(p("CommandOrControl+Shift+A").1, Code::KeyA);
        assert_eq!(
            p("CmdOrCtrl+Shift+a").0,
            Modifiers::CONTROL | Modifiers::SHIFT
        );
        assert_eq!(p("Control+F5"), (Modifiers::CONTROL, Code::F5));
        assert_eq!(p("Super+F24").1, Code::F24);
        assert_eq!(p("Meta+Up"), (Modifiers::SUPER, Code::ArrowUp));
        assert_eq!(p("AltGr+E"), (ca, Code::KeyE));
        assert_eq!(p("Option+Return").1, Code::Enter);
        assert_eq!(
            p("Ctrl+Plus"),
            (Modifiers::CONTROL | Modifiers::SHIFT, Code::Equal)
        );
        assert_eq!(
            p("Ctrl++"),
            (Modifiers::CONTROL | Modifiers::SHIFT, Code::Equal)
        );
        assert_eq!(
            p("Alt+!"),
            (Modifiers::ALT | Modifiers::SHIFT, Code::Digit1)
        );
        assert_eq!(p("Ctrl+/"), (Modifiers::CONTROL, Code::Slash));
        assert_eq!(p("Ctrl+num7").1, Code::Numpad7);
        assert_eq!(p("Ctrl+numadd").1, Code::NumpadAdd);
        assert_eq!(p("Ctrl+Esc").1, Code::Escape);
        assert_eq!(p("Escape"), (Modifiers::empty(), Code::Escape));
        assert_eq!(p("ctrl+shift+pagedown").1, Code::PageDown);
        assert_eq!(p("Ctrl+0").1, Code::Digit0);
    }

    #[test]
    fn unparseable_is_an_error_not_a_panic() {
        for bad in [
            "", "Ctrl+", "Ctrl+Foo", "Ctrl+A+B", "Ctrl+F25", "Ctrl+F0", "Hyper+A",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn validation_matches_main_js() {
        assert!(valid_accelerator("Ctrl+Shift+A"));
        assert!(valid_accelerator("Super+K"));
        assert!(!valid_accelerator("Shift+A"));
        assert!(!valid_accelerator("Ctrl+Shift"));
        assert!(!valid_accelerator("A"));
        assert!(!valid_accelerator("  "));
        assert!(!valid_accelerator("Ctrl+"));
        for c in ALTERNATIVES {
            assert!(valid_accelerator(c), "{c}");
            assert!(parse(c).is_ok(), "{c}");
        }
    }

    #[test]
    fn labels() {
        assert_eq!(action_label("captureQuery"), Some("Ask, find, or do"));
        assert_eq!(
            action_label("toggleVisibility"),
            Some("Hide or show the overlay")
        );
        assert_eq!(action_label("nope"), None);
        for a in ACTIONS {
            assert!(action_label(a).is_some());
        }
    }

    #[derive(Default)]
    struct Fake {
        taken: Vec<&'static str>,
        on: Vec<String>,
        calls: Vec<String>,
    }

    impl Keys for Fake {
        fn register(&mut self, acc: &str, _action: &str) -> bool {
            self.calls.push(format!("+{acc}"));
            let ok = !self.taken.contains(&acc);
            if ok {
                self.on.push(acc.to_string());
            }
            ok
        }

        fn unregister(&mut self, acc: &str) {
            self.calls.push(format!("-{acc}"));
            self.on.retain(|a| a != acc);
        }

        fn unregister_all(&mut self) {
            self.calls.push("-all".into());
            self.on.clear();
        }
    }

    #[derive(Default)]
    struct Rig {
        keys: Fake,
        reg: Registered,
        results: BTreeMap<String, Option<bool>>,
    }

    impl Rig {
        fn run(&mut self, chords: Vec<(String, String)>, escape: bool) -> bool {
            self.keys.calls.clear();
            let Rig { keys, reg, results } = self;
            reconcile(keys, reg, chords, results, || escape)
        }

        fn apply(&mut self, chord: &str, escape: bool) -> bool {
            let chords = vec![
                ("captureQuery".to_string(), chord.to_string()),
                ("findOnScreen".to_string(), String::new()),
            ];
            self.run(chords, escape)
        }
    }

    #[test]
    fn apply_skips_unchanged_set() {
        let mut rig = Rig::default();
        assert!(rig.apply("Ctrl+Alt+Space", false));
        assert_eq!(rig.keys.calls, ["-all", "+Ctrl+Alt+Space"]);
        assert_eq!(rig.results["captureQuery"], Some(true));
        assert_eq!(rig.results["findOnScreen"], None);
        // Opening the bar again changes nothing, so nothing is touched.
        assert!(!rig.apply("Ctrl+Alt+Space", false));
        assert!(rig.keys.calls.is_empty());
        // Escape alone comes and goes.
        assert!(!rig.apply("Ctrl+Alt+Space", true));
        assert_eq!(rig.keys.calls, ["+Escape"]);
        assert!(!rig.apply("Ctrl+Alt+Space", true));
        assert!(rig.keys.calls.is_empty());
        assert!(!rig.apply("Ctrl+Alt+Space", false));
        assert_eq!(rig.keys.calls, ["-Escape"]);
        // A new chord re-registers the set.
        assert!(rig.apply("Ctrl+Shift+K", true));
        assert_eq!(rig.keys.calls, ["-all", "+Ctrl+Shift+K", "+Escape"]);
        assert_eq!(rig.keys.on, ["Ctrl+Shift+K", "Escape"]);
    }

    #[test]
    fn a_taken_chord_is_retried_alone() {
        let mut rig = Rig::default();
        rig.keys.taken = vec!["Ctrl+Alt+Space"];
        assert!(rig.apply("Ctrl+Alt+Space", false));
        assert_eq!(rig.results["captureQuery"], Some(false));
        assert!(!rig.apply("Ctrl+Alt+Space", false));
        assert_eq!(rig.keys.calls, ["+Ctrl+Alt+Space"]);
        rig.keys.taken.clear();
        assert!(rig.apply("Ctrl+Alt+Space", false));
        assert_eq!(rig.keys.calls, ["+Ctrl+Alt+Space"]);
        assert_eq!(rig.results["captureQuery"], Some(true));
    }

    #[test]
    fn capture_mode_lets_go_and_keeps_results() {
        let mut rig = Rig::default();
        rig.apply("Ctrl+Alt+Space", true);
        assert!(rig.run(Vec::new(), false));
        assert_eq!(rig.keys.calls, ["-all"]);
        assert!(rig.keys.on.is_empty());
        assert_eq!(rig.results["captureQuery"], Some(true));
        assert!(rig.apply("Ctrl+Alt+Space", false));
        assert_eq!(rig.keys.on, ["Ctrl+Alt+Space"]);
    }

    #[test]
    fn escape_depth_counter() {
        let depth = Depth(AtomicU32::new(0));
        let mut rig = Rig::default();
        rig.apply("Ctrl+Alt+Space", !depth.suspended());
        assert!(rig.reg.escape);
        // Two synthetic keys overlap.
        assert!(depth.suspend());
        assert!(!depth.suspend());
        rig.apply("Ctrl+Alt+Space", !depth.suspended());
        assert!(!rig.reg.escape);
        // The first one finishes: Escape stays off.
        assert!(!depth.resume());
        assert!(depth.suspended());
        rig.apply("Ctrl+Alt+Space", !depth.suspended());
        assert!(!rig.reg.escape);
        // The second one finishes: Escape comes back.
        assert!(depth.resume());
        rig.apply("Ctrl+Alt+Space", !depth.suspended());
        assert!(rig.reg.escape);
        // A resume with no suspend never goes below zero.
        assert!(!depth.resume());
        assert!(!depth.suspended());
    }

    #[test]
    fn taken_returns_free_alternatives() {
        let ours = vec!["Ctrl+Alt+Space".to_string(), "Control+Alt+A".to_string()];
        let mut probed = Vec::new();
        let free = free_alternatives(&ours, |c| {
            probed.push(c.to_string());
            c != "Super+Shift+Space"
        });
        assert_eq!(free, ["Ctrl+Shift+Space"]);
        // Ctrl+Alt+A is already one of ours, so it is not offered or probed.
        assert_eq!(probed, ["Super+Shift+Space", "Ctrl+Shift+Space"]);
        assert!(free_alternatives(&ours, |_| false).is_empty());
        assert_eq!(
            TAKEN,
            "Another app already uses that shortcut. Try another."
        );
    }
}
