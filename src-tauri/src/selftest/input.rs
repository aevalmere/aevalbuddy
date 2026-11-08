//! Real input for probes: `SendInput` at physical pixels, the way a person's
//! mouse and keyboard reach the windows (K-30). Whatever is under the pointer,
//! or has keyboard focus, gets it: ours or not.

use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::Input::KeyboardAndMouse as km;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// One gesture. Points are physical pixels on the virtual screen.
// The lanes' probes use every variant; the harness's own probe uses Move.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum Input<'a> {
    /// Moves the pointer there.
    Move(i32, i32),
    /// Left button down and up there.
    Click(i32, i32),
    /// Two left clicks there, inside the double-click time.
    DoubleClick(i32, i32),
    /// Right button down and up there.
    RightClick(i32, i32),
    /// Left button down at `from`, moved in steps, released at `to`.
    Drag { from: (i32, i32), to: (i32, i32) },
    /// `notches` wheel notches with the pointer at `at`: positive scrolls up
    /// (away from the person), negative down.
    Wheel { at: (i32, i32), notches: i32 },
    /// A chord, pressed in order and released in reverse: `"Escape"`,
    /// `"Enter"`, `"Ctrl+Shift+K"`, `"Alt+Space"`, `"Ctrl+="`, `"Ctrl++"`,
    /// `"Alt+Down"`, `"F6"`.
    Key(&'a str),
    /// Typed as Unicode keystrokes into whatever has focus.
    Text(&'a str),
}

/// Time for the target to see one event before the next.
const STEP: Duration = Duration::from_millis(30);
const WHEEL_DELTA: i32 = 120;
const LEFT: (u32, u32) = (km::MOUSEEVENTF_LEFTDOWN, km::MOUSEEVENTF_LEFTUP);
const RIGHT: (u32, u32) = (km::MOUSEEVENTF_RIGHTDOWN, km::MOUSEEVENTF_RIGHTUP);

/// Sends `gesture`. An error when Windows took fewer events than were sent
/// (a window of higher integrity has focus, or the desktop is locked), or
/// when a chord names a key this table does not know.
pub fn send(gesture: Input<'_>) -> Result<(), String> {
    match gesture {
        Input::Move(x, y) => move_to(x, y),
        Input::Click(x, y) => move_to(x, y).and_then(|()| click(LEFT, 1)),
        Input::DoubleClick(x, y) => move_to(x, y).and_then(|()| click(LEFT, 2)),
        Input::RightClick(x, y) => move_to(x, y).and_then(|()| click(RIGHT, 1)),
        Input::Drag { from, to } => drag(from, to),
        Input::Wheel { at, notches } => {
            move_to(at.0, at.1)?;
            let data = (notches * WHEEL_DELTA) as u32;
            inject(&[mouse(0, 0, km::MOUSEEVENTF_WHEEL, data)])?;
            thread::sleep(STEP);
            Ok(())
        }
        Input::Key(chord) => key(chord),
        Input::Text(text) => type_text(text),
    }
}

/// Where the pointer is, in physical pixels.
pub fn cursor() -> (i32, i32) {
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: `p` is a valid place for the point.
    unsafe { wm::GetCursorPos(&mut p) };
    (p.x, p.y)
}

fn mouse(dx: i32, dy: i32, flags: u32, data: u32) -> km::INPUT {
    km::INPUT {
        r#type: km::INPUT_MOUSE,
        Anonymous: km::INPUT_0 {
            mi: km::MOUSEINPUT {
                dx,
                dy,
                mouseData: data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn keybd(vk: u16, scan: u16, flags: u32) -> km::INPUT {
    km::INPUT {
        r#type: km::INPUT_KEYBOARD,
        Anonymous: km::INPUT_0 {
            ki: km::KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn inject(events: &[km::INPUT]) -> Result<(), String> {
    let size = std::mem::size_of::<km::INPUT>() as i32;
    // SAFETY: `events` is a live slice of initialised INPUT structs.
    let n = unsafe { km::SendInput(events.len() as u32, events.as_ptr(), size) };
    if n as usize == events.len() {
        Ok(())
    } else {
        Err(format!("SendInput took {n} of {} events", events.len()))
    }
}

fn move_to(x: i32, y: i32) -> Result<(), String> {
    // SAFETY: plain Win32 calls, no pointers.
    let (vx, vy, vw, vh) = unsafe {
        (
            wm::GetSystemMetrics(wm::SM_XVIRTUALSCREEN),
            wm::GetSystemMetrics(wm::SM_YVIRTUALSCREEN),
            wm::GetSystemMetrics(wm::SM_CXVIRTUALSCREEN).max(2),
            wm::GetSystemMetrics(wm::SM_CYVIRTUALSCREEN).max(2),
        )
    };
    let nx = ((x - vx) as i64 * 65535 / (vw as i64 - 1)) as i32;
    let ny = ((y - vy) as i64 * 65535 / (vh as i64 - 1)) as i32;
    let flags = km::MOUSEEVENTF_MOVE | km::MOUSEEVENTF_ABSOLUTE | km::MOUSEEVENTF_VIRTUALDESK;
    inject(&[mouse(nx, ny, flags, 0)])?;
    // Normalised coordinates can land a pixel off; finish on the exact one.
    // SAFETY: as above.
    unsafe { wm::SetCursorPos(x, y) };
    thread::sleep(STEP);
    Ok(())
}

fn click((down, up): (u32, u32), times: usize) -> Result<(), String> {
    let mut events = Vec::new();
    for _ in 0..times {
        events.push(mouse(0, 0, down, 0));
        events.push(mouse(0, 0, up, 0));
    }
    inject(&events)?;
    thread::sleep(STEP);
    Ok(())
}

fn drag(from: (i32, i32), to: (i32, i32)) -> Result<(), String> {
    move_to(from.0, from.1)?;
    inject(&[mouse(0, 0, km::MOUSEEVENTF_LEFTDOWN, 0)])?;
    thread::sleep(STEP);
    // Twelve steps: past any drag threshold, and slow enough for a window's
    // move loop to follow.
    let steps = 12;
    let mut moved = Ok(());
    for i in 1..=steps {
        moved = move_to(
            from.0 + (to.0 - from.0) * i / steps,
            from.1 + (to.1 - from.1) * i / steps,
        );
        if moved.is_err() {
            break;
        }
    }
    // The button comes up even after a failed move, or Windows would think
    // it is still held.
    let up = inject(&[mouse(0, 0, km::MOUSEEVENTF_LEFTUP, 0)]);
    thread::sleep(STEP);
    moved.and(up)
}

fn key(chord: &str) -> Result<(), String> {
    let keys = parse_chord(chord)?;
    let ext = |e: bool| if e { km::KEYEVENTF_EXTENDEDKEY } else { 0 };
    let press = |&(vk, e): &(u16, bool)| keybd(vk, 0, ext(e));
    let release = |&(vk, e): &(u16, bool)| keybd(vk, 0, ext(e) | km::KEYEVENTF_KEYUP);
    let mut events: Vec<km::INPUT> = keys.iter().map(press).collect();
    events.extend(keys.iter().rev().map(release));
    inject(&events)?;
    thread::sleep(STEP);
    Ok(())
}

fn type_text(text: &str) -> Result<(), String> {
    let mut events = Vec::new();
    for unit in text.encode_utf16() {
        events.push(keybd(0, unit, km::KEYEVENTF_UNICODE));
        events.push(keybd(0, unit, km::KEYEVENTF_UNICODE | km::KEYEVENTF_KEYUP));
    }
    inject(&events)?;
    thread::sleep(STEP);
    Ok(())
}

/// A chord's keys in press order, modifiers first: the virtual-key code and
/// whether it is an extended key.
fn parse_chord(chord: &str) -> Result<Vec<(u16, bool)>, String> {
    let (mods, last) = match chord.strip_suffix("++") {
        Some(rest) => (rest, "+"),
        None => chord.rsplit_once('+').unwrap_or(("", chord)),
    };
    let mut keys = Vec::new();
    for name in mods.split('+').filter(|m| !m.is_empty()).chain([last]) {
        keys.push(vk(name).ok_or_else(|| format!("no key {name:?}"))?);
    }
    Ok(keys)
}

fn vk(name: &str) -> Option<(u16, bool)> {
    let plain = |code: u16| Some((code, false));
    let extended = |code: u16| Some((code, true));
    match name.trim().to_ascii_lowercase().as_str() {
        "ctrl" | "control" => plain(0x11),
        "alt" => plain(0x12),
        "shift" => plain(0x10),
        "win" | "meta" | "super" => extended(0x5B),
        "enter" | "return" => plain(0x0D),
        "escape" | "esc" => plain(0x1B),
        "space" => plain(0x20),
        "tab" => plain(0x09),
        "backspace" => plain(0x08),
        "+" | "=" | "plus" => plain(0xBB),
        "-" | "minus" => plain(0xBD),
        "," | "comma" => plain(0xBC),
        "." | "period" => plain(0xBE),
        "/" | "slash" => plain(0xBF),
        "delete" | "del" => extended(0x2E),
        "insert" => extended(0x2D),
        "home" => extended(0x24),
        "end" => extended(0x23),
        "pageup" => extended(0x21),
        "pagedown" => extended(0x22),
        "left" | "arrowleft" => extended(0x25),
        "up" | "arrowup" => extended(0x26),
        "right" | "arrowright" => extended(0x27),
        "down" | "arrowdown" => extended(0x28),
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => {
            plain(s.as_bytes()[0].to_ascii_uppercase() as u16)
        }
        s => {
            let n: u16 = s.strip_prefix('f')?.parse().ok()?;
            (1..=24).contains(&n).then_some((0x6F + n, false))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(chord: &str) -> Vec<u16> {
        parse_chord(chord).unwrap().iter().map(|k| k.0).collect()
    }

    #[test]
    fn chords_press_modifiers_first() {
        assert_eq!(codes("Ctrl+Shift+K"), [0x11, 0x10, 0x4B]);
        assert_eq!(codes("Escape"), [0x1B]);
        assert_eq!(codes("Ctrl++"), [0x11, 0xBB]);
        assert_eq!(codes("Ctrl+="), [0x11, 0xBB]);
        assert_eq!(codes("ctrl+1"), [0x11, 0x31]);
        assert_eq!(codes("F6"), [0x75]);
        assert_eq!(parse_chord("Alt+Down").unwrap()[1], (0x28, true));
        assert!(parse_chord("Ctrl+Nope").is_err());
        assert!(parse_chord("F25").is_err());
        assert!(parse_chord("").is_err());
    }
}
