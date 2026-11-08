//! Displays in Electron's units (ARCHITECTURE.md D11 `screen.*`, PARITY
//! POS-24). Every stored anchor, offset and width is in DIPs; Win32 speaks
//! physical pixels. The primary display's DIP origin is (0, 0) and each other
//! display's DIP origin is its physical origin over its own scale, which is
//! what Chromium does for side-by-side layouts.

use std::ptr;

use windows_sys::Win32::Foundation::{BOOL, LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

use crate::state::Rect;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Display {
    pub id: isize,
    pub primary: bool,
    pub scale: f64,
    /// Physical pixels.
    pub bounds_px: Rect,
    pub work_px: Rect,
    /// DIPs.
    pub bounds: Rect,
    pub work_area: Rect,
}

impl Display {
    fn from_px(id: isize, primary: bool, scale: f64, bounds_px: Rect, work_px: Rect) -> Self {
        let ox = bounds_px.x / scale;
        let oy = bounds_px.y / scale;
        let to_dip = |r: Rect| {
            Rect::new(
                ox + (r.x - bounds_px.x) / scale,
                oy + (r.y - bounds_px.y) / scale,
                r.width / scale,
                r.height / scale,
            )
        };
        Display {
            id,
            primary,
            scale,
            bounds_px,
            work_px,
            bounds: to_dip(bounds_px),
            work_area: to_dip(work_px),
        }
    }

    pub fn dip_to_px(&self, r: Rect) -> Rect {
        let s = self.scale;
        Rect::new(
            (self.bounds_px.x + (r.x - self.bounds.x) * s).round(),
            (self.bounds_px.y + (r.y - self.bounds.y) * s).round(),
            (r.width * s).round(),
            (r.height * s).round(),
        )
    }

    pub fn px_to_dip(&self, r: Rect) -> Rect {
        let s = self.scale;
        Rect::new(
            self.bounds.x + (r.x - self.bounds_px.x) / s,
            self.bounds.y + (r.y - self.bounds_px.y) / s,
            r.width / s,
            r.height / s,
        )
    }
}

fn rect(r: &RECT) -> Rect {
    Rect::new(
        f64::from(r.left),
        f64::from(r.top),
        f64::from(r.right - r.left),
        f64::from(r.bottom - r.top),
    )
}

/// LIFE-03: `AEVAL_SHOT_SCALE` forces the scale the window maths use.
fn forced_scale() -> Option<f64> {
    let v: f64 = std::env::var("AEVAL_SHOT_SCALE").ok()?.parse().ok()?;
    (v > 0.0).then_some(v)
}

unsafe extern "system" fn collect(mon: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    let out = &mut *(data as *mut Vec<Display>);
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(mon, &mut info) == 0 {
        return 1;
    }
    let (mut dx, mut dy) = (96u32, 96u32);
    let scale = if GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) == 0 {
        f64::from(dx) / 96.0
    } else {
        1.0
    };
    let scale = forced_scale().unwrap_or(scale);
    let primary = info.dwFlags & 1 != 0;
    out.push(Display::from_px(
        mon as isize,
        primary,
        scale,
        rect(&info.rcMonitor),
        rect(&info.rcWork),
    ));
    1
}

/// Every display, primary first.
pub fn all() -> Vec<Display> {
    let mut out: Vec<Display> = Vec::new();
    // SAFETY: the callback only writes into `out`, which outlives the call.
    unsafe {
        EnumDisplayMonitors(
            ptr::null_mut(),
            ptr::null(),
            Some(collect),
            &mut out as *mut Vec<Display> as LPARAM,
        );
    }
    out.sort_by_key(|d| !d.primary);
    if out.is_empty() {
        let r = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        out.push(Display::from_px(0, true, 1.0, r, r));
    }
    out
}

pub fn primary() -> Display {
    all()[0]
}

/// The primary display's work area, DIPs (main.js workArea()).
pub fn work_area() -> Rect {
    primary().work_area
}

/// The display a DIP rect mostly sits on (getDisplayMatching).
pub fn matching(r: Rect) -> Display {
    let list = all();
    list.iter()
        .copied()
        .max_by(|a, b| r.overlap(&a.bounds).total_cmp(&r.overlap(&b.bounds)))
        .filter(|d| r.overlap(&d.bounds) > 0.0)
        .unwrap_or_else(|| nearest(r.center(), &list))
}

fn nearest((x, y): (f64, f64), list: &[Display]) -> Display {
    let dist = |d: &Display| {
        let (cx, cy) = d.bounds.center();
        (cx - x).hypot(cy - y)
    };
    list.iter()
        .copied()
        .min_by(|a, b| dist(a).total_cmp(&dist(b)))
        .unwrap_or_else(primary)
}

/// The display holding a physical rect.
pub fn matching_px(r: Rect) -> Display {
    let list = all();
    list.iter()
        .copied()
        .max_by(|a, b| r.overlap(&a.bounds_px).total_cmp(&r.overlap(&b.bounds_px)))
        .filter(|d| r.overlap(&d.bounds_px) > 0.0)
        .unwrap_or_else(primary)
}

/// The pointer, physical pixels.
pub fn cursor_px() -> (f64, f64) {
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: an out-parameter.
    unsafe { GetCursorPos(&mut p) };
    (f64::from(p.x), f64::from(p.y))
}

/// The display under the pointer (main.js activeDisplay()).
pub fn active() -> Display {
    let (x, y) = cursor_px();
    let list = all();
    list.iter()
        .copied()
        .find(|d| d.bounds_px.contains(x, y))
        .unwrap_or_else(primary)
}

/// Physical point to DIPs (screen.screenToDipPoint).
pub fn px_to_dip_point(x: f64, y: f64) -> (f64, f64) {
    let d = matching_px(Rect::new(x, y, 1.0, 1.0));
    let r = d.px_to_dip(Rect::new(x, y, 0.0, 0.0));
    (r.x, r.y)
}

pub fn dip_to_px(r: Rect) -> Rect {
    matching(r).dip_to_px(r)
}

pub fn px_to_dip(r: Rect) -> Rect {
    matching_px(r).px_to_dip(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_at_150_percent() {
        let b = Rect::new(0.0, 0.0, 2880.0, 1800.0);
        let w = Rect::new(0.0, 0.0, 2880.0, 1728.0);
        let d = Display::from_px(1, true, 1.5, b, w);
        assert_eq!(d.work_area, Rect::new(0.0, 0.0, 1920.0, 1152.0));
        let px = d.dip_to_px(Rect::new(100.0, 50.0, 420.0, 76.0));
        assert_eq!(px, Rect::new(150.0, 75.0, 630.0, 114.0));
        assert_eq!(d.px_to_dip(px), Rect::new(100.0, 50.0, 420.0, 76.0));
    }

    #[test]
    fn secondary_display_origin() {
        let b = Rect::new(2560.0, 0.0, 1920.0, 1080.0);
        let d = Display::from_px(2, false, 1.0, b, b);
        assert_eq!(d.bounds.x, 2560.0);
        let d2 = Display::from_px(3, false, 2.0, b, b);
        assert_eq!(d2.bounds.x, 1280.0);
        assert_eq!(d2.dip_to_px(Rect::new(1290.0, 5.0, 10.0, 10.0)).x, 2580.0);
    }

    #[test]
    fn enumerates_at_least_one() {
        let list = all();
        assert!(!list.is_empty());
        assert!(list[0].primary || list.len() == 1);
    }
}
