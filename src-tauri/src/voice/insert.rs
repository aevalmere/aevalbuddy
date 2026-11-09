//! Putting the words where the person is typing, in any app. The clipboard
//! paste is the reliable path (one Ctrl+V reaches editors, browsers,
//! terminals and chat apps alike, where per-character input is slow and
//! some apps drop or reorder it), so it goes first: what was on the
//! clipboard is saved, the text pasted, and the old contents put back. The
//! dictated text is marked so Windows clipboard history and cloud sync skip
//! it. When the clipboard is busy, the text is typed as Unicode keystrokes
//! instead.

use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::DataExchange as dx;
use windows_sys::Win32::System::Memory as mem;
use windows_sys::Win32::UI::Input::KeyboardAndMouse as km;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

/// `dwExtraInfo` on every key this module sends, so the push-to-talk hook
/// knows them for ours.
pub const OWN_INPUT: usize = 0xAE7A_B0D1;

/// How long the target gets to read the clipboard before it is put back.
const PASTE_SETTLE: Duration = Duration::from_millis(450);

const CF_UNICODETEXT: u32 = 13;
const VK_CONTROL: u16 = 0x11;
const VK_V: u16 = 0x56;
const VK_RETURN: u16 = 0x0D;
/// An unassigned key: pressing it while Win is down stops the Start menu
/// opening when Win is let go (the AutoHotkey "menu mask").
const VK_MASK: u16 = 0xE8;

/// How the text went in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Paste,
    Typed,
}

fn key(vk: u16, scan: u16, flags: u32) -> km::INPUT {
    km::INPUT {
        r#type: km::INPUT_KEYBOARD,
        Anonymous: km::INPUT_0 {
            ki: km::KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: OWN_INPUT,
            },
        },
    }
}

fn tap(vk: u16) -> [km::INPUT; 2] {
    // SAFETY: a plain lookup.
    let scan = unsafe { km::MapVirtualKeyW(vk as u32, km::MAPVK_VK_TO_VSC) } as u16;
    [key(vk, scan, 0), key(vk, scan, km::KEYEVENTF_KEYUP)]
}

fn send(events: &[km::INPUT]) -> Result<(), String> {
    if events.is_empty() {
        return Ok(());
    }
    let size = std::mem::size_of::<km::INPUT>() as i32;
    // SAFETY: a live slice of initialised INPUT structs.
    let n = unsafe { km::SendInput(events.len() as u32, events.as_ptr(), size) };
    if n as usize == events.len() {
        Ok(())
    } else {
        Err(
            "Windows did not take the keystrokes. An app running as administrator may have focus."
                .into(),
        )
    }
}

/// The keystrokes that type `text`: each UTF-16 unit as a Unicode key down
/// and up, and each line break as Enter.
pub fn unicode_inputs(text: &str) -> Vec<km::INPUT> {
    let mut out = Vec::new();
    let normal = text.replace("\r\n", "\n").replace('\r', "\n");
    for ch in normal.chars() {
        if ch == '\n' {
            out.extend(tap(VK_RETURN));
            continue;
        }
        let mut buf = [0u16; 2];
        for unit in ch.encode_utf16(&mut buf) {
            out.push(key(0, *unit, km::KEYEVENTF_UNICODE));
            out.push(key(0, *unit, km::KEYEVENTF_UNICODE | km::KEYEVENTF_KEYUP));
        }
    }
    out
}

/// Stops the Start menu from opening when a held Win key is let go.
pub fn mask_start_menu() {
    let _ = send(&tap(VK_MASK));
}

fn down(vk: i32) -> bool {
    // SAFETY: a plain query.
    (unsafe { km::GetAsyncKeyState(vk) } as u16 & 0x8000) != 0
}

/// Waits until Ctrl, Alt, Shift and Win are all up, so Ctrl+V is not read
/// as Win+Ctrl+V; gives up after `max`.
fn wait_for_hands_off(max: Duration) {
    let end = Instant::now() + max;
    while [0x10, 0x11, 0x12, 0x5B, 0x5C].iter().any(|vk| down(*vk)) && Instant::now() < end {
        thread::sleep(Duration::from_millis(15));
    }
}

/// Types `text` into the window that has the keyboard focus.
pub fn insert(text: &str) -> Result<Method, String> {
    if text.is_empty() {
        return Ok(Method::Paste);
    }
    wait_for_hands_off(Duration::from_millis(1500));
    match paste(text) {
        Ok(()) => Ok(Method::Paste),
        Err(_) => send(&unicode_inputs(text)).map(|()| Method::Typed),
    }
}

fn paste(text: &str) -> Result<(), String> {
    let owner = Owner::new();
    let saved = Clipboard::open(owner.0)?.save();
    {
        let c = Clipboard::open(owner.0)?;
        c.replace(&[(CF_UNICODETEXT, utf16z(text))])?;
    }
    // SAFETY: a plain query.
    let ours = unsafe { dx::GetClipboardSequenceNumber() };
    let mut ctrl_v = Vec::new();
    // SAFETY: plain lookups.
    let (sc, sv) = unsafe {
        (
            km::MapVirtualKeyW(VK_CONTROL as u32, km::MAPVK_VK_TO_VSC) as u16,
            km::MapVirtualKeyW(VK_V as u32, km::MAPVK_VK_TO_VSC) as u16,
        )
    };
    ctrl_v.push(key(VK_CONTROL, sc, 0));
    ctrl_v.push(key(VK_V, sv, 0));
    ctrl_v.push(key(VK_V, sv, km::KEYEVENTF_KEYUP));
    ctrl_v.push(key(VK_CONTROL, sc, km::KEYEVENTF_KEYUP));
    let sent = send(&ctrl_v);
    thread::sleep(PASTE_SETTLE);
    // Put the old contents back unless something else has written since.
    // SAFETY: a plain query.
    if unsafe { dx::GetClipboardSequenceNumber() } == ours {
        if let Ok(c) = Clipboard::open(owner.0) {
            let _ = c.replace(&saved);
        }
    }
    sent
}

fn utf16z(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// A message-only window to own the clipboard: SetClipboardData fails for a
/// clipboard opened without an owner.
struct Owner(HWND);

impl Owner {
    fn new() -> Owner {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // SAFETY: a message-only STATIC window, destroyed in Drop.
        let hwnd = unsafe {
            wm::CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                0,
                0,
                wm::HWND_MESSAGE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        Owner(hwnd)
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: our own window.
            unsafe { wm::DestroyWindow(self.0) };
        }
    }
}

/// An open clipboard, closed on drop.
pub struct Clipboard;

/// Formats held as GDI or metafile handles rather than memory: they cannot
/// be copied byte for byte, and Windows makes most of them again from the
/// memory formats (CF_DIB gives CF_BITMAP).
fn handle_format(f: u32) -> bool {
    matches!(f, 2 | 3 | 9 | 14 | 0x80 | 0x82 | 0x83 | 0x8E) || (0x300..=0x3FF).contains(&f)
}

/// The most saved and put back: anything bigger stays as the paste left it
/// rather than holding a copy of a huge image in memory.
const SAVE_MAX: usize = 64 << 20;

fn register(name: &str) -> u32 {
    let w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: a NUL-terminated name.
    unsafe { dx::RegisterClipboardFormatW(w.as_ptr()) }
}

impl Clipboard {
    /// Opens it, retrying for up to half a second while another app holds it.
    pub fn open(owner: HWND) -> Result<Clipboard, String> {
        for _ in 0..25 {
            // SAFETY: a plain call; closed in Drop.
            if unsafe { dx::OpenClipboard(owner) } != 0 {
                return Ok(Clipboard);
            }
            thread::sleep(Duration::from_millis(20));
        }
        Err("The clipboard is busy.".into())
    }

    /// Every memory-held format, copied.
    pub fn save(&self) -> Vec<(u32, Vec<u8>)> {
        let mut out = Vec::new();
        let mut total = 0usize;
        let mut f = 0u32;
        loop {
            // SAFETY: the clipboard is open.
            f = unsafe { dx::EnumClipboardFormats(f) };
            if f == 0 {
                break;
            }
            if handle_format(f) {
                continue;
            }
            // SAFETY: as above; the handle belongs to the clipboard.
            let h = unsafe { dx::GetClipboardData(f) };
            if h.is_null() {
                continue;
            }
            // SAFETY: a memory-held format's handle is an HGLOBAL.
            let size = unsafe { mem::GlobalSize(h) };
            if size == 0 || total + size > SAVE_MAX {
                continue;
            }
            // SAFETY: locked for the copy, unlocked after.
            let p = unsafe { mem::GlobalLock(h) } as *const u8;
            if p.is_null() {
                continue;
            }
            // SAFETY: `size` bytes are readable at `p` while locked.
            let bytes = unsafe { std::slice::from_raw_parts(p, size) }.to_vec();
            // SAFETY: matched with the lock above.
            unsafe { mem::GlobalUnlock(h) };
            total += size;
            out.push((f, bytes));
        }
        out
    }

    /// Empties it and puts `items` there, marked as nothing for clipboard
    /// history or cloud sync to keep.
    pub fn replace(&self, items: &[(u32, Vec<u8>)]) -> Result<(), String> {
        // SAFETY: the clipboard is open.
        if unsafe { dx::EmptyClipboard() } == 0 {
            return Err("The clipboard could not be emptied.".into());
        }
        let no = 0u32.to_le_bytes().to_vec();
        let marks = [
            (
                register("ExcludeClipboardContentFromMonitorProcessing"),
                no.clone(),
            ),
            (register("CanIncludeInClipboardHistory"), no.clone()),
            (register("CanUploadToCloudClipboard"), no),
        ];
        for (f, bytes) in items.iter().chain(marks.iter()) {
            if *f == 0 {
                continue;
            }
            // SAFETY: a fresh movable block, filled while locked, then
            // handed to the clipboard, which owns it from then on.
            unsafe {
                let h = mem::GlobalAlloc(mem::GMEM_MOVEABLE, bytes.len().max(1));
                if h.is_null() {
                    return Err("Out of memory for the clipboard.".into());
                }
                let p = mem::GlobalLock(h) as *mut u8;
                if p.is_null() {
                    windows_sys::Win32::Foundation::GlobalFree(h);
                    continue;
                }
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
                mem::GlobalUnlock(h);
                if dx::SetClipboardData(*f, h).is_null() {
                    windows_sys::Win32::Foundation::GlobalFree(h);
                }
            }
        }
        Ok(())
    }

    /// The clipboard's text, if it holds any.
    pub fn text(&self) -> Option<String> {
        // SAFETY: the clipboard is open; the handle belongs to it.
        unsafe {
            let h = dx::GetClipboardData(CF_UNICODETEXT);
            if h.is_null() {
                return None;
            }
            let p = mem::GlobalLock(h) as *const u16;
            if p.is_null() {
                return None;
            }
            let units = mem::GlobalSize(h) / 2;
            let s = std::slice::from_raw_parts(p, units);
            let end = s.iter().position(|u| *u == 0).unwrap_or(units);
            let text = String::from_utf16_lossy(&s[..end]);
            mem::GlobalUnlock(h);
            Some(text)
        }
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        // SAFETY: opened in `open`.
        unsafe { dx::CloseClipboard() };
    }
}

/// Puts `text` on the clipboard (for the smoke probe's before and after).
pub fn set_clipboard_text(text: &str) -> Result<(), String> {
    let owner = Owner::new();
    let c = Clipboard::open(owner.0)?;
    // SAFETY: the clipboard is open.
    if unsafe { dx::EmptyClipboard() } == 0 {
        return Err("The clipboard could not be emptied.".into());
    }
    let bytes = utf16z(text);
    // SAFETY: as in `replace`.
    unsafe {
        let h = mem::GlobalAlloc(mem::GMEM_MOVEABLE, bytes.len());
        let p = mem::GlobalLock(h) as *mut u8;
        if p.is_null() {
            return Err("Out of memory for the clipboard.".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        mem::GlobalUnlock(h);
        dx::SetClipboardData(CF_UNICODETEXT, h);
    }
    drop(c);
    Ok(())
}

/// The clipboard's text now.
pub fn clipboard_text() -> Option<String> {
    let owner = Owner::new();
    Clipboard::open(owner.0).ok()?.text()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(events: &[km::INPUT]) -> Vec<(u16, u16, bool)> {
        events
            .iter()
            // SAFETY: every event built here is a keyboard event.
            .map(|e| unsafe {
                (
                    e.Anonymous.ki.wVk,
                    e.Anonymous.ki.wScan,
                    e.Anonymous.ki.dwFlags & km::KEYEVENTF_KEYUP != 0,
                )
            })
            .collect()
    }

    #[test]
    fn typing_sends_each_unit_down_and_up_and_lines_as_enter() {
        let ev = unicode_inputs("a\u{1F600}\r\nb");
        let u = units(&ev);
        // a: 2, the emoji is a surrogate pair: 4, Enter: 2, b: 2.
        assert_eq!(ev.len(), 10);
        assert_eq!(u[0], (0, 'a' as u16, false));
        assert_eq!(u[1], (0, 'a' as u16, true));
        assert_eq!((u[2].1, u[4].1), (0xD83D, 0xDE00));
        assert_eq!(u[6].0, VK_RETURN);
        assert!(ev
            .iter()
            .all(|e| unsafe { e.Anonymous.ki.dwExtraInfo } == OWN_INPUT));
        assert!(unicode_inputs("").is_empty());
    }

    #[test]
    fn handle_formats_are_not_copied() {
        assert!(handle_format(2) && handle_format(14) && handle_format(0x305));
        assert!(!handle_format(CF_UNICODETEXT) && !handle_format(8) && !handle_format(0xC0FF));
        let z = utf16z("hi");
        assert_eq!(z, vec![b'h', 0, b'i', 0, 0, 0]);
    }
}
