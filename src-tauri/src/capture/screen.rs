//! Displays in Electron's terms: an id, DIP bounds, a scale factor, plus the
//! physical rectangle the capture backends and the desktop helper work in.
//!
//! DIP origin of a monitor is its physical origin divided by its own scale,
//! the rule Tauri uses for logical positions, so a DIP rect from a page and
//! a DIP rect computed here agree on every monitor. On a single monitor (and
//! any uniform-DPI layout) this is exactly Electron's mapping. Agent (a)'s
//! `windows/screen.rs` may replace this module; the function names follow
//! Electron's `screen` API so callers do not change.

use serde::{Deserialize, Serialize};
use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// A DIP rectangle, `{x, y, width, height}` as Electron writes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A physical-pixel rectangle on the virtual screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PxRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn overlap(&self, o: &Rect) -> f64 {
        let w = (self.x + self.width).min(o.x + o.width) - self.x.max(o.x);
        let h = (self.y + self.height).min(o.y + o.height) - self.y.max(o.y);
        if w > 0.0 && h > 0.0 {
            w * h
        } else {
            0.0
        }
    }

    fn distance_to(&self, px: f64, py: f64) -> f64 {
        let dx = (self.x - px).max(0.0).max(px - (self.x + self.width));
        let dy = (self.y - py).max(0.0).max(py - (self.y + self.height));
        dx * dx + dy * dy
    }

    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.width && py < self.y + self.height
    }
}

impl PxRect {
    fn overlap(&self, o: &PxRect) -> i64 {
        let w = (self.x + self.width).min(o.x + o.width) - self.x.max(o.x);
        let h = (self.y + self.height).min(o.y + o.height) - self.y.max(o.y);
        if w > 0 && h > 0 {
            w as i64 * h as i64
        } else {
            0
        }
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.width && py < self.y + self.height
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Display {
    pub id: u32,
    /// `\\.\DISPLAY1`: what DXGI outputs and GDI call the monitor.
    pub device: String,
    pub primary: bool,
    pub scale_factor: f64,
    /// DIP bounds, as Electron's `display.bounds`.
    pub bounds: Rect,
    pub work_area: Rect,
    /// Physical pixels.
    pub phys: PxRect,
}

impl Display {
    pub fn from_phys(device: &str, primary: bool, dpi: u32, phys: PxRect, work: PxRect) -> Display {
        let scale = if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 };
        Display {
            id: display_id(device),
            device: device.to_string(),
            primary,
            scale_factor: scale,
            bounds: to_dip(phys, scale),
            work_area: to_dip(work, scale),
            phys,
        }
    }

    /// Physical rect inside this display to DIP.
    pub fn rect_to_dip(&self, r: PxRect) -> Rect {
        let s = self.scale_factor;
        Rect::new(
            (self.bounds.x + (r.x - self.phys.x) as f64 / s).round(),
            (self.bounds.y + (r.y - self.phys.y) as f64 / s).round(),
            (r.width as f64 / s).round(),
            (r.height as f64 / s).round(),
        )
    }

    /// DIP rect on this display to physical pixels.
    pub fn rect_to_phys(&self, r: Rect) -> PxRect {
        let s = self.scale_factor;
        PxRect {
            x: (self.phys.x as f64 + (r.x - self.bounds.x) * s).round() as i32,
            y: (self.phys.y as f64 + (r.y - self.bounds.y) * s).round() as i32,
            width: (r.width * s).round() as i32,
            height: (r.height * s).round() as i32,
        }
    }
}

fn to_dip(r: PxRect, s: f64) -> Rect {
    Rect::new(
        (r.x as f64 / s).round(),
        (r.y as f64 / s).round(),
        (r.width as f64 / s).round(),
        (r.height as f64 / s).round(),
    )
}

/// A stable id from the device name (FNV-1a), since Windows has no numeric
/// display id of its own that survives a reboot.
pub fn display_id(device: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in device.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h & 0x7fff_ffff
}

fn px(r: RECT) -> PxRect {
    PxRect {
        x: r.left,
        y: r.top,
        width: r.right - r.left,
        height: r.bottom - r.top,
    }
}

pub fn wide(s: &[u16]) -> String {
    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..end])
}

unsafe extern "system" fn collect(mon: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    let out = &mut *(data.0 as *mut Vec<Display>);
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if !GetMonitorInfoW(mon, &mut info.monitorInfo as *mut MONITORINFO).as_bool() {
        return BOOL(1);
    }
    let (mut dx, mut dy) = (96u32, 96u32);
    if GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy).is_err() {
        dx = 96;
    }
    let mi = info.monitorInfo;
    out.push(Display::from_phys(
        &wide(&info.szDevice),
        mi.dwFlags & 1 != 0,
        dx,
        px(mi.rcMonitor),
        px(mi.rcWork),
    ));
    BOOL(1)
}

/// Every display, primary first.
pub fn all_displays() -> Vec<Display> {
    let mut out: Vec<Display> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut out as *mut Vec<Display> as isize),
        );
    }
    out.sort_by_key(|d| !d.primary);
    if out.is_empty() {
        out.push(Display::from_phys(
            r"\\.\DISPLAY1",
            true,
            96,
            PxRect {
                x: 0,
                y: 0,
                width: 1024,
                height: 768,
            },
            PxRect {
                x: 0,
                y: 0,
                width: 1024,
                height: 768,
            },
        ));
    }
    out
}

pub fn primary(list: &[Display]) -> Display {
    list.iter()
        .find(|d| d.primary)
        .or_else(|| list.first())
        .cloned()
        .unwrap_or_else(|| all_displays()[0].clone())
}

/// Electron's `screen.getDisplayMatching`: most overlap, else nearest.
pub fn matching(list: &[Display], r: &Rect) -> Display {
    let best = list
        .iter()
        .map(|d| (d, d.bounds.overlap(r)))
        .filter(|(_, a)| *a > 0.0)
        .max_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((d, _)) = best {
        return d.clone();
    }
    nearest_point(list, r.x + r.width / 2.0, r.y + r.height / 2.0)
}

/// Electron's `screen.getDisplayNearestPoint` (DIP).
pub fn nearest_point(list: &[Display], x: f64, y: f64) -> Display {
    list.iter()
        .min_by(|a, b| {
            a.bounds
                .distance_to(x, y)
                .total_cmp(&b.bounds.distance_to(x, y))
        })
        .cloned()
        .unwrap_or_else(|| primary(list))
}

/// The display holding most of a physical rect, else the primary.
pub fn matching_phys(list: &[Display], r: &PxRect) -> Display {
    list.iter()
        .map(|d| (d, d.phys.overlap(r)))
        .filter(|(_, a)| *a > 0)
        .max_by_key(|(_, a)| *a)
        .map(|(d, _)| d.clone())
        .or_else(|| list.iter().find(|d| d.phys.contains(r.x, r.y)).cloned())
        .unwrap_or_else(|| primary(list))
}

/// The pointer in physical pixels, if the session lets us read it.
pub fn cursor_phys() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p).ok()? };
    Some((p.x, p.y))
}

/// The screen the user is working on: the one under the pointer, falling
/// back to the primary on a locked or switching session (CAP-01).
pub fn active_display(list: &[Display]) -> Display {
    match cursor_phys() {
        Some((x, y)) => list
            .iter()
            .find(|d| d.phys.contains(x, y))
            .cloned()
            .unwrap_or_else(|| primary(list)),
        None => primary(list),
    }
}

/// Physical rect to DIP (`screen.screenToDipRect(null, rect)`).
pub fn screen_to_dip_rect(list: &[Display], r: PxRect) -> Rect {
    matching_phys(list, &r).rect_to_dip(r)
}

/// DIP rect to physical (`screen.dipToScreenRect(null, rect)`).
pub fn dip_to_screen_rect(list: &[Display], r: Rect) -> PxRect {
    matching(list, &r).rect_to_phys(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pr(x: i32, y: i32, w: i32, h: i32) -> PxRect {
        PxRect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn desk() -> Vec<Display> {
        vec![
            Display::from_phys(
                r"\\.\DISPLAY1",
                true,
                144,
                pr(0, 0, 1920, 1080),
                pr(0, 0, 1920, 1040),
            ),
            Display::from_phys(
                r"\\.\DISPLAY2",
                false,
                96,
                pr(1920, 0, 1920, 1080),
                pr(1920, 0, 1920, 1080),
            ),
        ]
    }

    #[test]
    fn dip_bounds_divide_by_scale() {
        let d = desk();
        assert_eq!(d[0].bounds, Rect::new(0.0, 0.0, 1280.0, 720.0));
        assert_eq!(d[0].scale_factor, 1.5);
        assert_eq!(d[1].bounds, Rect::new(1920.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn matching_picks_most_overlap_by_id_not_order() {
        let d = desk();
        let r = Rect::new(1900.0, 10.0, 300.0, 100.0);
        assert_eq!(matching(&d, &r).device, r"\\.\DISPLAY2");
        let r = Rect::new(100.0, 100.0, 10.0, 10.0);
        assert_eq!(matching(&d, &r).device, r"\\.\DISPLAY1");
        // Off every display: the nearest one.
        let r = Rect::new(5000.0, 10.0, 10.0, 10.0);
        assert_eq!(matching(&d, &r).device, r"\\.\DISPLAY2");
    }

    #[test]
    fn dip_round_trip_on_a_scaled_display() {
        let d = desk();
        let p = pr(300, 150, 600, 300);
        let dip = screen_to_dip_rect(&d, p);
        assert_eq!(dip, Rect::new(200.0, 100.0, 400.0, 200.0));
        assert_eq!(dip_to_screen_rect(&d, dip), p);
        let p2 = pr(2000, 100, 50, 50);
        assert_eq!(dip_to_screen_rect(&d, screen_to_dip_rect(&d, p2)), p2);
    }

    #[test]
    fn display_ids_are_stable_and_distinct() {
        assert_eq!(display_id(r"\\.\DISPLAY1"), display_id(r"\\.\DISPLAY1"));
        assert_ne!(display_id(r"\\.\DISPLAY1"), display_id(r"\\.\DISPLAY2"));
    }

    #[test]
    fn primary_comes_first_and_is_the_fallback() {
        let mut d = desk();
        d.reverse();
        assert_eq!(primary(&d).device, r"\\.\DISPLAY1");
    }
}
