//! Window flags and behaviour the Tauri builder does not cover
//! (ARCHITECTURE.md D10, PARITY WIN-01..03, WIN-12, WIN-13, NEW-07).
//!
//! - Tool window (off the taskbar and Alt-Tab), topmost re-assertion,
//!   WDA_EXCLUDEFROMCAPTURE on every show, show without activating,
//!   focusable on/off through WS_EX_NOACTIVATE.
//! - A window subclass that answers WM_NCHITTEST with resize codes on a band
//!   around the glass (the stage margin is click-through, so without this a
//!   resize grab would fall through to the app underneath), and
//!   WM_GETMINMAXINFO from size limits kept here, so limits and bounds
//!   changes never race through the event loop.
//! - Click-through with forwarding: Tauri's `set_ignore_cursor_events` has no
//!   `forward`, so a 60 Hz cursor poll runs while any window is click-through
//!   and hands the page the `mousemove` / `mouseleave` it used to get. It
//!   sleeps at 10 Hz when nothing is click-through.
//!
//! Never SetLayeredWindowAttributes or opacity (WIN-15).

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::Mutex;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tauri::{AppHandle, Manager, WebviewWindow};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindow, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
    IsWindow, IsWindowVisible, SetForegroundWindow, SetWindowDisplayAffinity, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, GWL_EXSTYLE, GW_HWNDNEXT, HWND_TOPMOST, MINMAXINFO, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE,
    SW_SHOWNOACTIVATE, WM_GETMINMAXINFO, WM_NCDESTROY, WM_NCHITTEST, WM_POWERBROADCAST,
    WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use crate::state::Rect;

const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;
const PBT_APMRESUMEAUTOMATIC: usize = 0x12;
const SUBCLASS_ID: usize = 0xAEBA;
/// Resize band around the glass, DIPs.
pub const BAND: f64 = 8.0;

/// Which edges of a window can be dragged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Resize {
    #[default]
    None,
    /// The open bar: width only.
    Horizontal,
    /// Output windows, Settings, History: all eight.
    All,
}

/// What the subclass needs about one window. Sizes are glass DIPs.
#[derive(Clone, Copy, Debug, Default)]
pub struct Meta {
    pub margin: f64,
    pub resize: Resize,
    pub min: (f64, f64),
    /// 0 = no limit on that axis.
    pub max: (f64, f64),
}

fn metas() -> &'static Mutex<HashMap<isize, Meta>> {
    static M: OnceLock<Mutex<HashMap<isize, Meta>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

fn labels() -> &'static Mutex<HashMap<String, isize>> {
    static L: OnceLock<Mutex<HashMap<String, isize>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The window's HWND as an integer (raw pointers are not Send).
pub fn hwnd_of(w: &WebviewWindow) -> Option<isize> {
    if let Some(h) = labels().lock().get(w.label()) {
        return Some(*h);
    }
    let handle = w.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

pub fn hwnd_for(label: &str) -> Option<isize> {
    labels().lock().get(label).copied()
}

fn h(x: isize) -> HWND {
    x as HWND
}

/// Registers a window: tool style, protection, subclass. Runs the Win32
/// parts on the main thread, which owns the window (SetWindowSubclass
/// cannot cross threads).
pub fn adopt(app: &AppHandle, w: &WebviewWindow, meta: Meta) {
    let Some(hwnd) = hwnd_of(w) else {
        return;
    };
    labels().lock().insert(w.label().to_string(), hwnd);
    metas().lock().insert(hwnd, meta);
    let _ = app.run_on_main_thread(move || {
        set_tool_window(hwnd);
        protect(hwnd);
        // SAFETY: the window belongs to this (main) thread; the proc removes
        // itself on WM_NCDESTROY.
        unsafe {
            SetWindowSubclass(h(hwnd), Some(subclass), SUBCLASS_ID, 0);
        }
    });
}

/// The labels registered to `hwnd`, taken out of `map`. A label already
/// registered to a newer window stays (B-092).
fn drop_handle(map: &mut HashMap<String, isize>, hwnd: isize) -> Vec<String> {
    let gone: Vec<String> = map
        .iter()
        .filter(|(_, v)| **v == hwnd)
        .map(|(l, _)| l.clone())
        .collect();
    map.retain(|_, v| *v != hwnd);
    gone
}

/// The window behind `hwnd` is being destroyed (its subclass sees
/// WM_NCDESTROY): everything kept for that handle goes, and nothing kept
/// for a window that reopened under the same label at once (B-092, B-093).
pub fn forget_hwnd(hwnd: isize) {
    let gone = drop_handle(&mut labels().lock(), hwnd);
    metas().lock().remove(&hwnd);
    {
        let mut c = clicks().lock();
        for label in gone {
            c.remove(&label);
        }
    }
    super::motion::forget(hwnd);
}

pub fn set_meta(hwnd: isize, f: impl FnOnce(&mut Meta)) {
    if let Some(m) = metas().lock().get_mut(&hwnd) {
        f(m);
    }
}

pub fn meta(hwnd: isize) -> Meta {
    metas().lock().get(&hwnd).copied().unwrap_or_default()
}

/// Sets glass size limits (WIN-13: never the resizable flag).
pub fn set_limits(hwnd: isize, min: (f64, f64), max: (f64, f64)) {
    set_meta(hwnd, |m| {
        m.min = min;
        m.max = max;
    });
}

fn ex_style(hwnd: isize) -> isize {
    // SAFETY: reading a style of a live window.
    unsafe { GetWindowLongPtrW(h(hwnd), GWL_EXSTYLE) }
}

fn set_ex_style(hwnd: isize, style: isize) {
    // SAFETY: as above; FRAMECHANGED makes the new style take.
    unsafe {
        SetWindowLongPtrW(h(hwnd), GWL_EXSTYLE, style);
        SetWindowPos(
            h(hwnd),
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// WIN-03: WS_EX_TOOLWINDOW, which is what Electron's `type: 'toolbar'` set.
pub fn set_tool_window(hwnd: isize) {
    let s = ex_style(hwnd);
    let want = (s | WS_EX_TOOLWINDOW as isize) & !(WS_EX_APPWINDOW as isize);
    if want != s {
        set_ex_style(hwnd, want);
    }
}

pub fn is_tool_window(hwnd: isize) -> bool {
    ex_style(hwnd) & WS_EX_TOOLWINDOW as isize != 0
}

/// Electron's setFocusable: WS_EX_NOACTIVATE off or on.
pub fn set_focusable(hwnd: isize, focusable: bool) {
    let s = ex_style(hwnd);
    let want = if focusable {
        s & !(WS_EX_NOACTIVATE as isize)
    } else {
        s | WS_EX_NOACTIVATE as isize
    };
    if want != s {
        set_ex_style(hwnd, want);
    }
}

/// LIFE-18: WDA_EXCLUDEFROMCAPTURE needs build 19041.
pub fn can_hide_from_capture() -> bool {
    win_build() >= 19041
}

pub fn win_build() -> u32 {
    static B: OnceLock<u32> = OnceLock::new();
    *B.get_or_init(|| {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
            .and_then(|k| k.get_value::<String, _>("CurrentBuildNumber"))
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    })
}

/// WIN-02, re-applied on every show (Windows can drop it while hidden).
pub fn protect(hwnd: isize) {
    if !can_hide_from_capture() || std::env::var_os("AEVAL_NO_PROTECT").is_some() {
        return;
    }
    // SAFETY: a live window owned by this process.
    unsafe {
        SetWindowDisplayAffinity(h(hwnd), WDA_EXCLUDEFROMCAPTURE);
    }
}

/// WIN-01: topmost again (Windows drops it after focus changes).
pub fn pin_on_top(hwnd: isize) {
    // SAFETY: z-order only.
    unsafe {
        SetWindowPos(
            h(hwnd),
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

pub fn is_visible(hwnd: isize) -> bool {
    // SAFETY: a query.
    unsafe { IsWindowVisible(h(hwnd)) != 0 }
}

/// Whether the window is maximized (B-024).
pub fn is_maximized(hwnd: isize) -> bool {
    // SAFETY: a query.
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsZoomed(h(hwnd)) != 0 }
}

/// Shows without taking focus from the app in front (showInactive).
pub fn show_inactive(hwnd: isize) {
    protect(hwnd);
    // SAFETY: shows a window we own.
    unsafe {
        ShowWindow(h(hwnd), SW_SHOWNOACTIVATE);
    }
    pin_on_top(hwnd);
}

/// Hides through Tauri and Win32 both, so the two never disagree about a
/// window shown with `show_inactive`.
pub fn hide(w: &WebviewWindow) {
    let _ = w.hide();
    if let Some(hwnd) = hwnd_of(w) {
        // SAFETY: hides a window we own.
        unsafe {
            ShowWindow(h(hwnd), SW_HIDE);
        }
    }
}

/// show() + focus(): Tauri's show (keeps its visibility flag right), then the
/// foreground and the webview's keyboard focus.
pub fn show_focused(w: &WebviewWindow) {
    if let Some(hwnd) = hwnd_of(w) {
        protect(hwnd);
    }
    let _ = w.show();
    let _ = w.set_focus();
    if let Some(hwnd) = hwnd_of(w) {
        // SAFETY: we were just handed foreground rights by a hotkey or click.
        unsafe {
            SetForegroundWindow(h(hwnd));
        }
        pin_on_top(hwnd);
    }
}

/// The app that was in front when the bar last took focus.
static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

fn foreground() -> isize {
    // SAFETY: a query.
    unsafe { GetForegroundWindow() as isize }
}

fn is_ours(hwnd: isize) -> bool {
    let mut pid = 0u32;
    // SAFETY: an out-parameter; a stale handle leaves it 0.
    unsafe { GetWindowThreadProcessId(h(hwnd), &mut pid) };
    pid == std::process::id()
}

/// Notes the app in front before the bar takes focus (POS-09's blur half),
/// unless that is one of our windows.
pub fn remember_foreground() {
    let fg = foreground();
    if fg != 0 && !is_ours(fg) {
        PREV_FOREGROUND.store(fg, Ordering::Relaxed);
    }
}

/// The label of our window that has the foreground, if one does.
pub fn foreground_label() -> Option<String> {
    let fg = foreground();
    if fg == 0 {
        return None;
    }
    labels()
        .lock()
        .iter()
        .find(|(_, v)| **v == fg)
        .map(|(l, _)| l.clone())
}

/// The first visible app window below `from` in z-order that is not ours
/// and not a tool window, or 0.
fn next_app_window(from: isize) -> isize {
    let mut cur = from;
    for _ in 0..512 {
        // SAFETY: walks the z-order; a stale handle ends the walk with 0.
        cur = unsafe { GetWindow(h(cur), GW_HWNDNEXT) as isize };
        if cur == 0 {
            return 0;
        }
        let style = ex_style(cur);
        let skip = style & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) as isize != 0;
        if is_visible(cur) && !skip && !is_ours(cur) {
            return cur;
        }
    }
    0
}

/// Gives the foreground back when `hwnd` has it: to the app that was in
/// front when the bar opened if it is still there, else to the next app
/// window below. Collapsing the bar and Esc in an output (`output:blur`)
/// call it, so the person keeps typing where they were (D-01).
pub fn blur(hwnd: isize) {
    if foreground() != hwnd {
        return;
    }
    let prev = PREV_FOREGROUND.load(Ordering::Relaxed);
    // SAFETY: a query on a handle that may be stale.
    let alive = prev != 0 && unsafe { IsWindow(h(prev)) } != 0;
    let to = if alive && is_visible(prev) && !is_ours(prev) {
        prev
    } else {
        next_app_window(hwnd)
    };
    if to != 0 {
        // SAFETY: we hold the foreground, so we may hand it on.
        unsafe {
            SetForegroundWindow(h(to));
        }
    }
}

/// The window's outer rect, physical pixels.
pub fn rect_px(hwnd: isize) -> Rect {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: an out-parameter.
    unsafe { GetWindowRect(h(hwnd), &mut r) };
    Rect::new(
        f64::from(r.left),
        f64::from(r.top),
        f64::from(r.right - r.left),
        f64::from(r.bottom - r.top),
    )
}

/// Moves and sizes in one call, without activating or reordering.
pub fn set_rect_px(hwnd: isize, r: Rect) {
    // SAFETY: a live window we own.
    unsafe {
        SetWindowPos(
            h(hwnd),
            std::ptr::null_mut(),
            r.x as i32,
            r.y as i32,
            r.width.max(1.0) as i32,
            r.height.max(1.0) as i32,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        );
    }
}

pub fn scale_of(hwnd: isize) -> f64 {
    // SAFETY: a query.
    let dpi = unsafe { GetDpiForWindow(h(hwnd)) };
    if dpi == 0 {
        1.0
    } else {
        f64::from(dpi) / 96.0
    }
}

/// NEW-07: the resize code for a point (physical pixels), or None for the
/// client area. `win` is the window's outer rect.
pub fn hit_test(win: Rect, margin: f64, band: f64, resize: Resize, x: f64, y: f64) -> Option<u32> {
    if resize == Resize::None {
        return None;
    }
    let g = Rect::new(
        win.x + margin,
        win.y + margin,
        win.width - 2.0 * margin,
        win.height - 2.0 * margin,
    );
    // Half the band reaches inside the glass; a frameless window with no
    // margin keeps the whole band inside.
    let inner = if margin > 0.0 { band / 2.0 } else { band };
    let near = |p: f64, edge: f64| p >= edge - band && p <= edge + inner;
    let near_far = |p: f64, edge: f64| p >= edge - inner && p <= edge + band;
    let in_x = x >= g.x - band && x <= g.x + g.width + band;
    let in_y = y >= g.y - band && y <= g.y + g.height + band;
    let left = in_y && near(x, g.x);
    let right = in_y && near_far(x, g.x + g.width);
    if resize == Resize::Horizontal {
        let strict_y = y >= g.y && y <= g.y + g.height;
        return match (left && strict_y, right && strict_y) {
            (true, _) => Some(10),
            (_, true) => Some(11),
            _ => None,
        };
    }
    let top = in_x && near(y, g.y);
    let bottom = in_x && near_far(y, g.y + g.height);
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(13),
        (_, true, true, _) => Some(14),
        (true, _, _, true) => Some(16),
        (_, true, _, true) => Some(17),
        (true, ..) => Some(10),
        (_, true, ..) => Some(11),
        (_, _, true, _) => Some(12),
        (_, _, _, true) => Some(15),
        _ => None,
    }
}

fn band_hit(hwnd: isize, x: f64, y: f64) -> Option<u32> {
    let m = meta(hwnd);
    let s = scale_of(hwnd);
    hit_test(rect_px(hwnd), m.margin * s, BAND * s, m.resize, x, y)
}

/// B-090: a panic in the handling never unwinds into Win32; the message
/// then gets the default handling.
unsafe extern "system" fn subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    let answered = catch_unwind(AssertUnwindSafe(|| on_message(hwnd, msg, wparam, lparam)));
    match answered {
        Ok(Some(r)) => r,
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

/// The subclass's own handling: Some(result) when it answers the message,
/// None for the default handling.
unsafe fn on_message(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    let key = hwnd as isize;
    match msg {
        WM_NCHITTEST => {
            let x = f64::from((lparam & 0xFFFF) as u16 as i16);
            let y = f64::from(((lparam >> 16) & 0xFFFF) as u16 as i16);
            if let Some(code) = band_hit(key, x, y) {
                return Some(code as LRESULT);
            }
            let m = meta(key);
            if m.margin > 0.0 {
                // The rest of a margined window is client area: the default
                // border hit-test would make the invisible outer edge resize.
                return Some(1);
            }
        }
        WM_GETMINMAXINFO => {
            let r = DefSubclassProc(hwnd, msg, wparam, lparam);
            let m = meta(key);
            let s = scale_of(key);
            let info = &mut *(lparam as *mut MINMAXINFO);
            let px = |v: f64| (super::stage::limit(v, m.margin) * s).round() as i32;
            if m.min.0 > 0.0 {
                info.ptMinTrackSize.x = px(m.min.0);
            }
            if m.min.1 > 0.0 {
                info.ptMinTrackSize.y = px(m.min.1);
            }
            if m.max.0 > 0.0 {
                info.ptMaxTrackSize.x = px(m.max.0);
            }
            if m.max.1 > 0.0 {
                info.ptMaxTrackSize.y = px(m.max.1);
            }
            return Some(r);
        }
        WM_POWERBROADCAST if wparam == PBT_APMRESUMEAUTOMATIC => {
            crate::updater::on_resume();
        }
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID);
            forget_hwnd(key);
        }
        _ => {}
    }
    None
}

// ------------------------------------------------------------ click-through

#[derive(Clone, Copy, Debug, Default)]
struct Click {
    requested: bool,
    effective: bool,
    inside: bool,
    last: (i32, i32),
}

fn clicks() -> &'static Mutex<HashMap<String, Click>> {
    static C: OnceLock<Mutex<HashMap<String, Click>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `setIgnoreMouseEvents(ignore, { forward: true })`. While the pointer is on
/// a resize band the window keeps taking the mouse, so the grab works.
pub fn set_click_through(w: &WebviewWindow, ignore: bool) {
    let label = w.label().to_string();
    let in_band = ignore
        && hwnd_of(w).is_some_and(|hw| {
            let (x, y) = super::screen::cursor_px();
            band_hit(hw, x, y).is_some()
        });
    let effective = ignore && !in_band;
    let changed = {
        let mut map = clicks().lock();
        let c = map.entry(label).or_default();
        c.requested = ignore;
        let changed = c.effective != effective;
        c.effective = effective;
        changed
    };
    if changed || !ignore {
        let _ = w.set_ignore_cursor_events(effective);
    }
}

pub fn is_click_through(label: &str) -> bool {
    clicks().lock().get(label).is_some_and(|c| c.effective)
}

fn mouse_down() -> bool {
    // SAFETY: a query.
    (unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON)) } as u16 & 0x8000) != 0
}

/// The poll runs at 60 Hz while the pointer is within this many DIPs of a
/// click-through window, else at 4 Hz (B-091).
const NEAR: f64 = 64.0;

/// Whether (x, y) is within `pad` of `r`.
pub fn near(r: Rect, x: f64, y: f64, pad: f64) -> bool {
    x >= r.x - pad && x <= r.x + r.width + pad && y >= r.y - pad && y <= r.y + r.height + pad
}

/// What the poll may apply to `label` now: the new value when the window
/// still asks for click-through and the value changes, else None. The poll
/// works from a snapshot; a `set_click_through(false)` since then wins.
fn poll_apply(map: &mut HashMap<String, Click>, label: &str, effective: bool) -> Option<bool> {
    let c = map.get_mut(label)?;
    if !c.requested || c.effective == effective {
        return None;
    }
    c.effective = effective;
    Some(effective)
}

/// Starts the forwarding poll (once, at boot).
pub fn start_poll(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || loop {
        let any = clicks().lock().values().any(|c| c.requested);
        let close = any && poll_once(&app);
        std::thread::sleep(Duration::from_millis(if close { 16 } else { 250 }));
    });
}

/// One look at the pointer. Returns whether it is near a click-through
/// window, so the next look comes soon.
fn poll_once(app: &AppHandle) -> bool {
    let mut close = false;
    let (cx, cy) = super::screen::cursor_px();
    let down = mouse_down();
    let snapshot: Vec<(String, Click)> = clicks()
        .lock()
        .iter()
        .filter(|(_, c)| c.requested)
        .map(|(l, c)| (l.clone(), *c))
        .collect();
    for (label, mut c) in snapshot {
        let Some(hwnd) = hwnd_for(&label) else {
            continue;
        };
        if !is_visible(hwnd) {
            continue;
        }
        let Some(w) = app.get_webview_window(&label) else {
            continue;
        };
        let r = rect_px(hwnd);
        let inside = r.contains(cx, cy);
        let s = scale_of(hwnd);
        close |= near(r, cx, cy, NEAR * s);
        if !down {
            let effective = band_hit(hwnd, cx, cy).is_none();
            if effective != c.effective {
                c.effective = effective;
                // Applied on the main thread after a fresh look at the
                // request, so a stale snapshot never turns it back on.
                let (a, l) = (app.clone(), label.clone());
                let _ = app.run_on_main_thread(move || {
                    let apply = poll_apply(&mut clicks().lock(), &l, effective);
                    if let (Some(v), Some(w)) = (apply, a.get_webview_window(&l)) {
                        let _ = w.set_ignore_cursor_events(v);
                    }
                });
            }
        }
        if c.effective {
            let px = (cx as i32, cy as i32);
            if inside && px != c.last {
                let x = (cx - r.x) / s;
                let y = (cy - r.y) / s;
                let _ = w.eval(format!(
                    "document.dispatchEvent(new MouseEvent('mousemove',\
                     {{clientX:{x},clientY:{y},bubbles:true}}))"
                ));
            } else if c.inside && !inside {
                let _ = w.eval("document.dispatchEvent(new MouseEvent('mouseleave'))");
            }
            c.last = px;
        }
        c.inside = inside;
        if let Some(slot) = clicks().lock().get_mut(&label) {
            slot.inside = c.inside;
            slot.last = c.last;
        }
    }
    close
}

#[cfg(test)]
mod tests {
    use super::*;

    // An output window: glass 420x200 at (120, 120) inside a 20 px margin.
    fn win() -> Rect {
        Rect::new(100.0, 100.0, 460.0, 240.0)
    }

    #[test]
    fn edges_and_corners_on_the_margin() {
        let t = |x, y| hit_test(win(), 20.0, 8.0, Resize::All, x, y);
        assert_eq!(t(115.0, 200.0), Some(10)); // left, on the margin
        assert_eq!(t(123.0, 200.0), Some(10)); // left, just inside the glass
        assert_eq!(t(545.0, 200.0), Some(11));
        assert_eq!(t(300.0, 114.0), Some(12));
        assert_eq!(t(300.0, 325.0), Some(15));
        assert_eq!(t(116.0, 116.0), Some(13));
        assert_eq!(t(544.0, 116.0), Some(14));
        assert_eq!(t(116.0, 324.0), Some(16));
        assert_eq!(t(544.0, 324.0), Some(17));
        assert_eq!(t(300.0, 200.0), None); // the glass
        assert_eq!(t(102.0, 200.0), None); // outer margin stays click-through
    }

    #[test]
    fn bar_resizes_sideways_only() {
        let t = |x, y| hit_test(win(), 20.0, 8.0, Resize::Horizontal, x, y);
        assert_eq!(t(115.0, 200.0), Some(10));
        assert_eq!(t(545.0, 200.0), Some(11));
        assert_eq!(t(300.0, 114.0), None);
        assert_eq!(t(116.0, 116.0), None);
        assert_eq!(hit_test(win(), 20.0, 8.0, Resize::None, 115.0, 200.0), None);
    }

    #[test]
    fn stale_snapshot_does_not_reapply_ignore() {
        let mut map: HashMap<String, Click> = HashMap::new();
        let on = Click {
            requested: true,
            ..Click::default()
        };
        map.insert("overlay".into(), on);
        // The poll's snapshot said click-through; it was turned off since.
        map.get_mut("overlay").unwrap().requested = false;
        assert_eq!(poll_apply(&mut map, "overlay", true), None);
        assert!(!map["overlay"].effective);
        // Still asked for: the change applies, once.
        map.get_mut("overlay").unwrap().requested = true;
        assert_eq!(poll_apply(&mut map, "overlay", true), Some(true));
        assert_eq!(poll_apply(&mut map, "overlay", true), None);
        assert_eq!(poll_apply(&mut map, "gone", true), None);
    }

    #[test]
    fn poll_is_fast_only_near_a_window() {
        let r = Rect::new(100.0, 100.0, 400.0, 200.0);
        assert!(near(r, 300.0, 200.0, 64.0));
        assert!(near(r, 40.0, 100.0, 64.0));
        assert!(!near(r, 30.0, 100.0, 64.0));
        assert!(!near(r, 300.0, 370.0, 64.0));
    }

    #[test]
    fn forget_by_hwnd_keeps_new_window() {
        let mut map: HashMap<String, isize> = HashMap::new();
        // Settings closed (handle 1) and opened again at once (handle 2).
        map.insert("settings".into(), 2);
        map.insert("overlay".into(), 3);
        assert!(drop_handle(&mut map, 1).is_empty());
        assert_eq!(map.get("settings"), Some(&2));
        assert_eq!(drop_handle(&mut map, 2), vec!["settings".to_string()]);
        assert_eq!(map.get("overlay"), Some(&3));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn frameless_window_band_is_inside() {
        let w = Rect::new(0.0, 0.0, 600.0, 400.0);
        let t = |x, y| hit_test(w, 0.0, 8.0, Resize::All, x, y);
        assert_eq!(t(3.0, 200.0), Some(10));
        assert_eq!(t(597.0, 397.0), Some(17));
        assert_eq!(t(300.0, 200.0), None);
    }
}
