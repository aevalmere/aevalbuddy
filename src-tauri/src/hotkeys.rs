//! Global shortcuts (PARITY KEY-01..15, ARCHITECTURE.md D7).
//!
//! Chords are stored in Electron's accelerator syntax and parsed here into
//! RegisterHotKey modifiers and a key, never rewritten. Escape is global only
//! while it has to work wherever focus is (agent running, tour or pointer,
//! area selector, layout editor); Enter never is.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

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

fn register(app: &AppHandle, acc: &str, action: &str) -> bool {
    let Ok((mods, code)) = parse(acc) else {
        return false;
    };
    let sc = Shortcut::new(Some(mods), code);
    if app.global_shortcut().register(sc).is_err() {
        return false;
    }
    bound().lock().insert(sc.id(), action.to_string());
    true
}

/// main.js applyRegistrations: re-derives every global shortcut from state.
pub fn apply(app: &AppHandle) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    bound().lock().clear();
    let st = shared(app);
    let (capture_mode, want_escape) = {
        let mut s = st.lock();
        s.escape_registered = false;
        let want = !s.escape_suspended
            && (s.agent.running || s.buddy_active || s.region_active || s.mode == Mode::Hud);
        (s.capture_mode, want)
    };
    if capture_mode {
        return;
    }
    let store = store::get();
    let mut results = BTreeMap::new();
    for action in ACTIONS {
        let acc = store.shortcut(action);
        let r = if acc.is_empty() {
            None
        } else {
            Some(register(app, &acc, action))
        };
        results.insert(action.to_string(), r);
    }
    st.lock().hotkeys = results;
    crate::tray::refresh(app);
    if want_escape {
        let ok = register(app, "Escape", ESCAPE);
        st.lock().escape_registered = ok;
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

/// KEY-09: while the engine injects a synthetic Escape, ours lets go.
pub fn suspend_escape(app: &AppHandle) {
    let registered = {
        let st = shared(app);
        let mut s = st.lock();
        s.escape_suspended = true;
        std::mem::replace(&mut s.escape_registered, false)
    };
    if registered {
        if let Ok((m, c)) = parse("Escape") {
            let _ = app.global_shortcut().unregister(Shortcut::new(Some(m), c));
        }
    }
}

/// Retakes Escape 60 ms after the injected key.
pub fn resume_escape(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        shared(&app).lock().escape_suspended = false;
        apply(&app);
    });
}

/// `shortcuts:capture-mode` (KEY-11).
pub fn set_capture_mode(app: &AppHandle, on: bool) {
    shared(app).lock().capture_mode = on;
    apply(app);
}

/// `shortcuts:set` (KEY-12).
pub fn set(app: &AppHandle, action: &str, accelerator: &str) -> Value {
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
    let clash = store
        .get_shortcuts()
        .into_iter()
        .find(|(other, acc)| other != action && acc.as_str() == Some(accelerator));
    if let Some((other, _)) = clash {
        let label = action_label(&other).unwrap_or("another action");
        return json!({
            "ok": false,
            "error": format!("Already used by \"{label}\". Pick a different combo."),
        });
    }
    // Probe with everything of ours unregistered, so the probe only fails
    // when another application owns the combo.
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    bound().lock().clear();
    shared(app).lock().escape_registered = false;
    let available = match parse(accelerator) {
        Ok((m, c)) => {
            let sc = Shortcut::new(Some(m), c);
            let ok = gs.register(sc).is_ok();
            if ok {
                let _ = gs.unregister(sc);
            }
            ok
        }
        Err(_) => false,
    };
    if !available {
        apply(app);
        return json!({ "ok": false, "error": "That shortcut is already in use - try another." });
    }
    store.set_shortcut(action, accelerator);
    apply(app);
    json!({ "ok": true, "shortcuts": store.get_shortcuts(), "active": active(app) })
}

/// `shortcuts:reset` (KEY-13).
pub fn reset(app: &AppHandle) -> Value {
    let shortcuts = store::get().reset_shortcuts();
    apply(app);
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
}
