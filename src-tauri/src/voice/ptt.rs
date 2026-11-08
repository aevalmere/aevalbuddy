//! Push to talk: hold the dictation chord (Ctrl+Win unless changed) in any
//! app, speak, let go. RegisterHotKey cannot see a chord of modifiers alone
//! or a key being let go, so this is a low-level keyboard hook on a thread
//! of its own. It only watches: nothing is swallowed except the chord's own
//! key (when it has one), Space while the chord is held (which keeps
//! listening hands-free, as in Wispr Flow) and Esc while listening (cancel).
//! Another key pressed with the chord (Ctrl+Win+D, Ctrl+Win+Right) is that
//! shortcut, so listening stops without a word.
//!
//! The chord is written in the store's accelerator syntax: `Ctrl+Super`,
//! `Ctrl+Alt+Space`, `Alt+Shift+D`.

use std::sync::mpsc;
use std::sync::OnceLock;

use parking_lot::Mutex;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

use super::insert::OWN_INPUT;

pub const DEFAULT: &str = "Ctrl+Super";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

/// A hold-to-talk chord: modifiers, and at most one other key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub mods: Mods,
    pub key: Option<u16>,
}

fn key_code(name: &str) -> Option<u16> {
    let n = name.to_ascii_lowercase();
    match n.as_str() {
        "space" => Some(0x20),
        "tab" => Some(0x09),
        "capslock" => Some(0x14),
        "insert" => Some(0x2D),
        "home" => Some(0x24),
        "end" => Some(0x23),
        "pageup" => Some(0x21),
        "pagedown" => Some(0x22),
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => {
            Some(s.as_bytes()[0].to_ascii_uppercase() as u16)
        }
        s => {
            let f: u16 = s.strip_prefix('f')?.parse().ok()?;
            (1..=24).contains(&f).then_some(0x6F + f)
        }
    }
}

fn key_name(vk: u16) -> String {
    match vk {
        0x20 => "Space".into(),
        0x09 => "Tab".into(),
        0x14 => "CapsLock".into(),
        0x2D => "Insert".into(),
        0x24 => "Home".into(),
        0x23 => "End".into(),
        0x21 => "PageUp".into(),
        0x22 => "PageDown".into(),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        v => char::from_u32(v as u32).map(String::from).unwrap_or_default(),
    }
}

/// Parses an accelerator. A chord of modifiers alone needs two of them, so
/// one held key never starts listening.
pub fn parse(acc: &str) -> Result<Chord, String> {
    let mut mods = Mods::default();
    let mut key = None;
    for part in acc.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "commandorcontrol" | "cmdorctrl" => mods.ctrl = true,
            "alt" | "option" | "altgr" => mods.alt = true,
            "shift" => mods.shift = true,
            "super" | "win" | "meta" | "windows" => mods.win = true,
            _ => {
                if key.is_some() {
                    return Err("Use one key with the modifiers.".into());
                }
                key = Some(key_code(part).ok_or_else(|| format!("{part} can't be part of the dictation shortcut."))?);
            }
        }
    }
    let n = [mods.ctrl, mods.alt, mods.shift, mods.win].iter().filter(|m| **m).count();
    match (n, key) {
        (0, _) => Err("Add Ctrl, Alt, Shift or Win to the shortcut.".into()),
        (1, None) => Err("Hold two modifiers, like Ctrl+Win, or add a key.".into()),
        (1, Some(_)) if mods.shift => Err("Shift and one key would type a capital. Add Ctrl, Alt or Win.".into()),
        _ => Ok(Chord { mods, key }),
    }
}

/// The store spelling: `Ctrl+Alt+Shift+Super+Key`.
pub fn accelerator(c: &Chord) -> String {
    let mut parts: Vec<String> = Vec::new();
    let m = c.mods;
    for (on, name) in [(m.ctrl, "Ctrl"), (m.alt, "Alt"), (m.shift, "Shift"), (m.win, "Super")] {
        if on {
            parts.push(name.into());
        }
    }
    if let Some(k) = c.key {
        parts.push(key_name(k));
    }
    parts.join("+")
}

/// How people read it: `Ctrl+Win`.
pub fn label(acc: &str) -> String {
    match parse(acc) {
        Ok(c) => accelerator(&c).replace("Super", "Win"),
        Err(_) => acc.to_string(),
    }
}

/// What the hook tells the dictation session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The chord went down: start listening.
    Start,
    /// Space while holding: keep listening after the chord is let go.
    Latch,
    /// The chord was let go (or pressed again when latched): transcribe.
    Finish,
    /// Esc: stop and drop what was heard.
    Cancel,
    /// Another key joined the chord: it was some other shortcut.
    Abort,
    /// Win is down with another modifier: keep the Start menu shut.
    Mask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Held,
    /// Hands-free; `released` once the chord has been let go.
    Latched { released: bool },
    /// Waits for every key of the chord to be up before it can start again.
    Spent,
}

const VK_SPACE: u16 = 0x20;
const VK_ESCAPE: u16 = 0x1B;

/// The chord's state, fed every key the person presses. Pure, so the tests
/// drive it with plain key codes.
pub struct Machine {
    chord: Option<Chord>,
    mods: Mods,
    key_down: bool,
    phase: Phase,
    /// Settings is recording a new shortcut: watch, never start.
    pub paused: bool,
}

impl Machine {
    pub fn new(chord: Option<Chord>) -> Machine {
        Machine {
            chord,
            mods: Mods::default(),
            key_down: false,
            phase: Phase::Idle,
            paused: false,
        }
    }

    pub fn set_chord(&mut self, chord: Option<Chord>) {
        self.chord = chord;
        self.phase = Phase::Idle;
    }

    fn set_mod(&mut self, vk: u16, down: bool) -> bool {
        let m = &mut self.mods;
        match vk {
            0x10 | 0xA0 | 0xA1 => m.shift = down,
            0x11 | 0xA2 | 0xA3 => m.ctrl = down,
            0x12 | 0xA4 | 0xA5 => m.alt = down,
            0x5B | 0x5C => m.win = down,
            _ => return false,
        }
        true
    }

    /// Takes the other modifiers from `phys`; the one `vk` names is about
    /// to change and is left to [`Machine::on_key`].
    pub fn resync(&mut self, phys: Mods, vk: u16) {
        let mine = |codes: &[u16]| codes.contains(&vk);
        if !mine(&[0x10, 0xA0, 0xA1]) {
            self.mods.shift = phys.shift;
        }
        if !mine(&[0x11, 0xA2, 0xA3]) {
            self.mods.ctrl = phys.ctrl;
        }
        if !mine(&[0x12, 0xA4, 0xA5]) {
            self.mods.alt = phys.alt;
        }
        if !mine(&[0x5B, 0x5C]) {
            self.mods.win = phys.win;
        }
    }

    fn chord_held(&self, c: &Chord) -> bool {
        self.mods == c.mods && (c.key.is_none() || self.key_down)
    }

    fn chord_part_up(&self, c: &Chord) -> bool {
        let m = self.mods;
        let want = c.mods;
        (want.ctrl && !m.ctrl)
            || (want.alt && !m.alt)
            || (want.shift && !m.shift)
            || (want.win && !m.win)
            || (c.key.is_some() && !self.key_down)
    }

    fn all_up(&self, c: &Chord) -> bool {
        let m = self.mods;
        let want = c.mods;
        !(want.ctrl && m.ctrl || want.alt && m.alt || want.shift && m.shift || want.win && m.win || self.key_down)
    }

    /// One key event. Returns what to tell the session and whether the key
    /// is swallowed.
    pub fn on_key(&mut self, vk: u16, down: bool) -> (Option<Event>, bool) {
        let is_mod = self.set_mod(vk, down);
        let Some(c) = self.chord else {
            return (None, false);
        };
        let is_key = c.key == Some(vk);
        if is_key {
            self.key_down = down;
        }
        if self.paused {
            let mask = down && self.mods.win && (self.mods.ctrl || self.mods.alt || self.mods.shift);
            return (mask.then_some(Event::Mask), false);
        }
        match (self.phase, down) {
            (Phase::Idle, true) => {
                if self.chord_held(&c) {
                    self.phase = Phase::Held;
                    return (Some(Event::Start), is_key);
                }
                (None, false)
            }
            (Phase::Held, true) => {
                if is_key {
                    // The chord's key repeating.
                    return (None, true);
                }
                if is_mod && c.mods == self.mods {
                    return (None, false);
                }
                match vk {
                    VK_SPACE => {
                        self.phase = Phase::Latched { released: false };
                        (Some(Event::Latch), true)
                    }
                    VK_ESCAPE => {
                        self.phase = Phase::Spent;
                        (Some(Event::Cancel), true)
                    }
                    _ => {
                        self.phase = Phase::Spent;
                        (Some(Event::Abort), false)
                    }
                }
            }
            (Phase::Held, false) => {
                if self.chord_part_up(&c) {
                    self.phase = if self.all_up(&c) { Phase::Idle } else { Phase::Spent };
                    return (Some(Event::Finish), is_key);
                }
                (None, false)
            }
            (Phase::Latched { released }, true) => {
                if vk == VK_ESCAPE {
                    self.phase = Phase::Spent;
                    return (Some(Event::Cancel), true);
                }
                if released && self.chord_held(&c) {
                    self.phase = Phase::Spent;
                    return (Some(Event::Finish), is_key);
                }
                // Space repeating while the chord is still held.
                (None, vk == VK_SPACE && !released)
            }
            (Phase::Latched { released: false }, false) => {
                if self.all_up(&c) {
                    self.phase = Phase::Latched { released: true };
                }
                (None, is_key || vk == VK_SPACE)
            }
            (Phase::Latched { .. }, false) => (None, false),
            (Phase::Spent, _) => {
                if self.all_up(&c) {
                    self.phase = Phase::Idle;
                }
                (None, is_key && !down)
            }
        }
    }

    /// The session ended from elsewhere (the bar, a timeout): start over.
    pub fn reset(&mut self) {
        self.phase = match self.phase {
            Phase::Idle => Phase::Idle,
            _ => Phase::Spent,
        };
        if let Some(c) = self.chord {
            if self.all_up(&c) {
                self.phase = Phase::Idle;
            }
        }
    }
}

static MACHINE: Mutex<Option<Machine>> = parking_lot::const_mutex(None);
static EVENTS: OnceLock<Mutex<mpsc::Sender<Event>>> = OnceLock::new();

/// Sets the chord (None turns dictation off).
pub fn set_chord(chord: Option<Chord>) {
    if let Some(m) = MACHINE.lock().as_mut() {
        m.set_chord(chord);
    }
}

/// While Settings records a shortcut, the hook only watches.
pub fn set_paused(on: bool) {
    if let Some(m) = MACHINE.lock().as_mut() {
        m.paused = on;
    }
}

pub fn reset() {
    if let Some(m) = MACHINE.lock().as_mut() {
        m.reset();
    }
}

/// The modifiers as Windows has them now. A key let go where the hook could
/// not see it (on the lock screen, after Win+L) would otherwise stay down.
fn physical_mods() -> Mods {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    // SAFETY: plain queries.
    let down = |vk: i32| (unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000) != 0;
    Mods {
        ctrl: down(0x11),
        alt: down(0x12),
        shift: down(0x10),
        win: down(0x5B) || down(0x5C),
    }
}

unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == wm::HC_ACTION as i32 {
        // SAFETY: for WH_KEYBOARD_LL, lparam points at a KBDLLHOOKSTRUCT.
        let k = unsafe { &*(lparam as *const wm::KBDLLHOOKSTRUCT) };
        let msg = wparam as u32;
        let down = msg == wm::WM_KEYDOWN || msg == wm::WM_SYSKEYDOWN;
        let up = msg == wm::WM_KEYUP || msg == wm::WM_SYSKEYUP;
        if (down || up) && k.dwExtraInfo != OWN_INPUT {
            let vk = k.vkCode as u16;
            let phys = physical_mods();
            let (event, swallow) = match MACHINE.lock().as_mut() {
                Some(m) => {
                    m.resync(phys, vk);
                    m.on_key(vk, down)
                }
                None => (None, false),
            };
            if let (Some(e), Some(tx)) = (event, EVENTS.get()) {
                let _ = tx.lock().send(e);
            }
            if swallow {
                return 1;
            }
        }
    }
    // SAFETY: passing the event on, as every hook must.
    unsafe { wm::CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

/// Installs the hook on its own thread and hands its events to `on_event`
/// on another, so the hook returns at once.
pub fn start(chord: Option<Chord>, on_event: impl Fn(Event) + Send + 'static) {
    let (tx, rx) = mpsc::channel::<Event>();
    if EVENTS.set(Mutex::new(tx)).is_err() {
        return;
    }
    *MACHINE.lock() = Some(Machine::new(chord));
    let _ = std::thread::Builder::new().name("voice-ptt-events".into()).spawn(move || {
        for e in rx {
            on_event(e);
        }
    });
    let _ = std::thread::Builder::new().name("voice-ptt".into()).spawn(|| {
        // SAFETY: a low-level hook for this module, kept for the life of the
        // app; this thread pumps the messages it needs.
        unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            let h = wm::SetWindowsHookExW(wm::WH_KEYBOARD_LL, Some(hook), module, 0);
            if h.is_null() {
                crate::crash::write("voice-ptt", "SetWindowsHookExW failed");
                return;
            }
            let mut msg: wm::MSG = std::mem::zeroed();
            while wm::GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
            wm::UnhookWindowsHookEx(h);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: u16 = 0xA2;
    const WIN: u16 = 0x5B;
    const D: u16 = 0x44;

    fn feed(m: &mut Machine, keys: &[(u16, bool)]) -> Vec<Event> {
        keys.iter().filter_map(|(k, d)| m.on_key(*k, *d).0).collect()
    }

    #[test]
    fn accelerators_parse_and_print() {
        let c = parse(DEFAULT).unwrap();
        assert_eq!((c.mods.ctrl, c.mods.win, c.key), (true, true, None));
        assert_eq!(label(DEFAULT), "Ctrl+Win");
        assert_eq!(accelerator(&parse("win+control").unwrap()), "Ctrl+Super");
        assert_eq!(accelerator(&parse("Ctrl+Alt+space").unwrap()), "Ctrl+Alt+Space");
        assert_eq!(parse("Alt+F9").unwrap().key, Some(0x78));
        assert!(parse("Ctrl").is_err());
        assert!(parse("Shift+A").is_err());
        assert!(parse("Ctrl+A+B").is_err());
        assert!(parse("Ctrl+Enter").is_err());
    }

    #[test]
    fn hold_then_release_dictates_and_a_shortcut_does_not() {
        let mut m = Machine::new(parse(DEFAULT).ok());
        // Either order of the two modifiers starts it; letting go finishes.
        assert_eq!(feed(&mut m, &[(CTRL, true), (WIN, true), (WIN, true)]), [Event::Start]);
        assert_eq!(feed(&mut m, &[(WIN, false), (CTRL, false)]), [Event::Finish]);
        assert_eq!(feed(&mut m, &[(WIN, true), (CTRL, true), (CTRL, false), (WIN, false)]), [Event::Start, Event::Finish]);
        // Ctrl+Win+D is the desktop shortcut: no words, no restart until up.
        assert_eq!(feed(&mut m, &[(CTRL, true), (WIN, true), (D, true), (D, false)]), [Event::Start, Event::Abort]);
        assert_eq!(feed(&mut m, &[(WIN, false), (WIN, true), (CTRL, false)]), []);
        // Ctrl+Shift+Win is a different chord.
        assert_eq!(feed(&mut m, &[(WIN, false), (CTRL, true), (0xA0, true), (WIN, true)]), []);
        let mut off = Machine::new(None);
        assert_eq!(feed(&mut off, &[(CTRL, true), (WIN, true)]), []);
    }

    #[test]
    fn a_key_let_go_out_of_sight_does_not_stick() {
        let mut m = Machine::new(parse(DEFAULT).ok());
        // Win went down, then the lock screen took the key up.
        assert_eq!(m.on_key(WIN, true), (None, false));
        m.resync(Mods::default(), CTRL);
        assert_eq!(m.on_key(CTRL, true), (None, false));
    }

    #[test]
    fn space_keeps_listening_until_the_chord_comes_again_and_esc_cancels() {
        let mut m = Machine::new(parse(DEFAULT).ok());
        assert_eq!(m.on_key(CTRL, true), (None, false));
        assert_eq!(m.on_key(WIN, true), (Some(Event::Start), false));
        assert_eq!(m.on_key(VK_SPACE, true), (Some(Event::Latch), true));
        assert_eq!(m.on_key(VK_SPACE, false), (None, true));
        assert_eq!(feed(&mut m, &[(WIN, false), (CTRL, false), (0x41, true), (0x41, false)]), []);
        assert_eq!(feed(&mut m, &[(CTRL, true), (WIN, true)]), [Event::Finish]);
        assert_eq!(feed(&mut m, &[(WIN, false), (CTRL, false)]), []);
        assert_eq!(feed(&mut m, &[(CTRL, true), (WIN, true)]), [Event::Start]);
        assert_eq!(m.on_key(VK_ESCAPE, true), (Some(Event::Cancel), true));
        assert_eq!(feed(&mut m, &[(WIN, false), (CTRL, false), (CTRL, true), (WIN, true)]), [Event::Start]);
    }

    #[test]
    fn a_chord_with_a_key_swallows_it_and_pausing_only_masks() {
        let mut m = Machine::new(parse("Ctrl+Alt+Space").ok());
        assert_eq!(m.on_key(CTRL, true), (None, false));
        assert_eq!(m.on_key(0xA4, true), (None, false));
        assert_eq!(m.on_key(VK_SPACE, true), (Some(Event::Start), true));
        assert_eq!(m.on_key(VK_SPACE, true), (None, true));
        assert_eq!(m.on_key(VK_SPACE, false), (Some(Event::Finish), true));
        let mut p = Machine::new(parse(DEFAULT).ok());
        p.paused = true;
        assert_eq!(feed(&mut p, &[(CTRL, true), (WIN, true)]), [Event::Mask]);
    }
}
