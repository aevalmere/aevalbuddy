//! Displays in Electron's terms: an id, DIP bounds, a scale factor, plus the
//! physical rectangle the capture backends and the desktop helper work in.
//!
//! DIP bounds follow Chromium's layout (B-005, D-09), the one WebView2 uses
//! for `window.screenX` and every page coordinate: the primary keeps its
//! origin, and every other display is placed edge to edge with the
//! neighbour it touches physically, so no two DIP rectangles overlap and a
//! DIP rect names one display. `dip_origins` is that layout, and
//! `windows/screen.rs` uses the same function for window placement. The
//! function names follow Electron's `screen` API so callers do not change.

use std::cmp::Ordering;
use std::collections::VecDeque;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFO,
    MONITORINFOEXW, MONITOR_DEFAULTTONULL,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, GetWindowThreadProcessId,
};

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

    /// Squared distance from a point to this rect (0 inside).
    fn distance_to(&self, px: i32, py: i32) -> i64 {
        let dx = (self.x - px).max(0).max(px - (self.x + self.width - 1)) as i64;
        let dy = (self.y - py).max(0).max(py - (self.y + self.height - 1)) as i64;
        dx * dx + dy * dy
    }

    /// Squared gap between two rects (0 when they touch or overlap).
    fn gap_to(&self, o: &PxRect) -> i64 {
        let dx = (o.x - (self.x + self.width))
            .max(self.x - (o.x + o.width))
            .max(0) as i64;
        let dy = (o.y - (self.y + self.height))
            .max(self.y - (o.y + o.height))
            .max(0) as i64;
        dx * dx + dy * dy
    }
}

/// A monitor as Windows reports it, before any DIP layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub hmonitor: isize,
    /// `\\.\DISPLAY1`: what DXGI outputs and GDI call the monitor.
    pub device: String,
    pub primary: bool,
    /// Effective DPI (96 is 100%).
    pub dpi: u32,
    pub phys: PxRect,
    pub work: PxRect,
}

impl Monitor {
    pub fn scale(&self) -> f64 {
        if self.dpi == 0 {
            1.0
        } else {
            self.dpi as f64 / 96.0
        }
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
    /// Physical pixels of the work area.
    #[serde(skip)]
    pub work_phys: PxRect,
    /// The HMONITOR (0 for a display built by hand).
    #[serde(skip)]
    pub hmonitor: isize,
}

impl Display {
    /// One display on its own: DIP origin at its physical origin over its
    /// scale. `with_layout` places it among the others.
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
            work_phys: work,
            hmonitor: 0,
        }
    }

    fn from_monitor(m: &Monitor) -> Display {
        let mut d = Display::from_phys(&m.device, m.primary, m.dpi, m.phys, m.work);
        d.hmonitor = m.hmonitor;
        d
    }

    /// Moves the DIP bounds (and the work area with them) to `(x, y)`.
    fn place_at(&mut self, (x, y): (f64, f64)) {
        let s = self.scale_factor;
        let (p, w) = (self.phys, self.work_phys);
        self.bounds = Rect::new(
            x,
            y,
            (p.width as f64 / s).round(),
            (p.height as f64 / s).round(),
        );
        self.work_area = Rect::new(
            x + ((w.x - p.x) as f64 / s).round(),
            y + ((w.y - p.y) as f64 / s).round(),
            (w.width as f64 / s).round(),
            (w.height as f64 / s).round(),
        );
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

    /// Physical point on this display to DIP.
    pub fn point_to_dip(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let s = self.scale_factor;
        (
            self.bounds.x + (x - self.phys.x as f64) / s,
            self.bounds.y + (y - self.phys.y as f64) / s,
        )
    }

    /// DIP rect on this display to physical pixels.
    // (c)'s geometry API; index.js exported it.
    #[allow(dead_code)]
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

// ------------------------------------------------------------ the layout

/// What the layout needs to know about one display.
#[derive(Clone, Copy, Debug)]
pub struct Place<'a> {
    pub name: &'a str,
    pub primary: bool,
    pub scale: f64,
    pub phys: PxRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Right,
    Left,
    Bottom,
    Top,
}

/// The side of `a` that `b` touches physically: a shared edge, with their
/// extents along it overlapping or meeting (Chromium's DisplayInfosTouch).
fn touching(a: &PxRect, b: &PxRect) -> Option<Side> {
    let (ar, ab) = (a.x + a.width, a.y + a.height);
    let (br, bb) = (b.x + b.width, b.y + b.height);
    let along_x = a.x.max(b.x) <= ar.min(br);
    let along_y = a.y.max(b.y) <= ab.min(bb);
    if b.y == ab && along_x {
        Some(Side::Bottom)
    } else if b.x == ar && along_y {
        Some(Side::Right)
    } else if bb == a.y && along_x {
        Some(Side::Top)
    } else if br == a.x && along_y {
        Some(Side::Left)
    } else {
        None
    }
}

/// A display's DIP extent for placing its neighbours. Rounded up, so a
/// neighbour never starts inside it whatever rounding its own size gets.
fn dip_len(len: i32, scale: f64) -> f64 {
    (len as f64 / scale).ceil()
}

fn dip_size(p: &Place<'_>) -> (f64, f64) {
    (
        dip_len(p.phys.width, p.scale),
        dip_len(p.phys.height, p.scale),
    )
}

/// Chromium's ScaleOffset: how far along the shared edge `current` starts,
/// in DIPs. Measured on the parent when it starts after the parent's start,
/// on itself when it starts before.
fn edge_offset(parent_begin: i32, current_begin: i32, parent_scale: f64, own_scale: f64) -> f64 {
    if current_begin >= parent_begin {
        ((current_begin - parent_begin) as f64 / parent_scale).floor()
    } else {
        -((parent_begin - current_begin) as f64 / own_scale).floor()
    }
}

/// B-005: the DIP origin of each display, in input order. The primary keeps
/// its origin; the others are placed edge to edge with the display they
/// touch physically, walking out from the primary (nearest first, then by
/// device name, so enumeration order never changes the answer). A display
/// that touches none keeps its physical origin over its scale. Any overlap
/// left after that is pushed out along the side the display was placed on.
pub fn dip_origins(list: &[Place<'_>]) -> Vec<(f64, f64)> {
    let n = list.len();
    if n == 0 {
        return Vec::new();
    }
    let size = |i: usize| dip_size(&list[i]);
    let p = list.iter().position(|d| d.primary).unwrap_or(0);
    let pp = list[p];
    let mut origin: Vec<(f64, f64)> = list
        .iter()
        .map(|d| (d.phys.x as f64 / d.scale, d.phys.y as f64 / d.scale))
        .collect();
    let mut side: Vec<Option<Side>> = vec![None; n];
    let mut order = vec![p];
    let mut rest: Vec<usize> = (0..n).filter(|&i| i != p).collect();
    rest.sort_by_key(|&i| (pp.phys.gap_to(&list[i].phys), list[i].name));
    let mut queue = VecDeque::from([p]);
    while let Some(parent) = queue.pop_front() {
        let pa = list[parent];
        let (px, py) = origin[parent];
        let (pw, ph) = size(parent);
        let mut i = 0;
        while i < rest.len() {
            let c = rest[i];
            let cu = list[c];
            let Some(s) = touching(&pa.phys, &cu.phys) else {
                i += 1;
                continue;
            };
            let (cw, ch) = size(c);
            let along_y = edge_offset(pa.phys.y, cu.phys.y, pa.scale, cu.scale);
            let along_x = edge_offset(pa.phys.x, cu.phys.x, pa.scale, cu.scale);
            origin[c] = match s {
                Side::Right => (px + pw, py + along_y),
                Side::Left => (px - cw, py + along_y),
                Side::Bottom => (px + along_x, py + ph),
                Side::Top => (px + along_x, py - ch),
            };
            side[c] = Some(s);
            order.push(c);
            queue.push_back(c);
            rest.remove(i);
        }
    }
    order.extend(rest);
    // Each push moves a display past one earlier display's far edge, in one
    // direction, so it needs at most one push per earlier display.
    for (k, &c) in order.iter().enumerate().skip(1) {
        let (cw, ch) = size(c);
        for _ in 0..k {
            let me = Rect::new(origin[c].0, origin[c].1, cw, ch);
            let mut hit = None;
            for &o in &order[..k] {
                let (ow, oh) = size(o);
                if me.overlap(&Rect::new(origin[o].0, origin[o].1, ow, oh)) > 0.0 {
                    hit = Some(o);
                    break;
                }
            }
            let Some(o) = hit else { break };
            let (ow, oh) = size(o);
            let (ox, oy) = origin[o];
            origin[c] = match side[c] {
                Some(Side::Left) => (ox - cw, origin[c].1),
                Some(Side::Top) => (origin[c].0, oy - ch),
                Some(Side::Bottom) => (origin[c].0, oy + oh),
                _ => (ox + ow, origin[c].1),
            };
        }
    }
    origin
}

/// Places displays built with `from_phys` in the canonical DIP layout.
pub fn with_layout(mut list: Vec<Display>) -> Vec<Display> {
    let origins = {
        let places: Vec<Place<'_>> = list
            .iter()
            .map(|d| Place {
                name: &d.device,
                primary: d.primary,
                scale: d.scale_factor,
                phys: d.phys,
            })
            .collect();
        dip_origins(&places)
    };
    for (d, o) in list.iter_mut().zip(origins) {
        d.place_at(o);
    }
    list
}

// ------------------------------------------------------------- enumeration

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
    let out = &mut *(data.0 as *mut Vec<Monitor>);
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
    out.push(Monitor {
        hmonitor: mon.0 as isize,
        device: wide(&info.szDevice),
        primary: mi.dwFlags & 1 != 0,
        dpi: dx,
        phys: px(mi.rcMonitor),
        work: px(mi.rcWork),
    });
    BOOL(1)
}

/// Every monitor as Windows reports it, primary first; empty when the
/// session gives none.
pub fn monitors() -> Vec<Monitor> {
    let mut out: Vec<Monitor> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut out as *mut Vec<Monitor> as isize),
        );
    }
    out.sort_by_key(|m| !m.primary);
    out
}

/// Every display, primary first, in the canonical DIP layout.
pub fn all_displays() -> Vec<Display> {
    let mut out: Vec<Display> = monitors().iter().map(Display::from_monitor).collect();
    if out.is_empty() {
        let r = PxRect {
            x: 0,
            y: 0,
            width: 1024,
            height: 768,
        };
        out.push(Display::from_phys(r"\\.\DISPLAY1", true, 96, r, r));
    }
    with_layout(out)
}

pub fn primary(list: &[Display]) -> Display {
    list.iter()
        .find(|d| d.primary)
        .or_else(|| list.first())
        .cloned()
        .unwrap_or_else(|| all_displays()[0].clone())
}

/// On equal overlap: the primary, then the display holding the centre.
fn tie_rank(d: &Display, cx: f64, cy: f64) -> (bool, bool) {
    (d.primary, d.bounds.contains(cx, cy))
}

/// Electron's `screen.getDisplayMatching`: most overlap, else nearest. Ties
/// go to the primary, then to the display the rect's centre is in; the first
/// display wins any tie left.
pub fn matching(list: &[Display], r: &Rect) -> Display {
    let (cx, cy) = (r.x + r.width / 2.0, r.y + r.height / 2.0);
    let mut best: Option<(&Display, f64)> = None;
    for d in list {
        let a = d.bounds.overlap(r);
        if a <= 0.0 {
            continue;
        }
        let better = match best {
            None => true,
            Some((b, ba)) => match a.total_cmp(&ba) {
                Ordering::Greater => true,
                Ordering::Equal => tie_rank(d, cx, cy) > tie_rank(b, cx, cy),
                Ordering::Less => false,
            },
        };
        if better {
            best = Some((d, a));
        }
    }
    match best {
        Some((d, _)) => d.clone(),
        None => nearest_point(list, cx, cy),
    }
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

/// The display holding most of a physical rect, else the one holding its
/// origin, else the primary. Ties as in `matching`.
pub fn matching_phys(list: &[Display], r: &PxRect) -> Display {
    let (cx, cy) = (r.x + r.width / 2, r.y + r.height / 2);
    let rank = |d: &Display| (d.primary, d.phys.contains(cx, cy));
    let mut best: Option<(&Display, i64)> = None;
    for d in list {
        let a = d.phys.overlap(r);
        if a <= 0 {
            continue;
        }
        let better = match best {
            None => true,
            Some((b, ba)) => a > ba || (a == ba && rank(d) > rank(b)),
        };
        if better {
            best = Some((d, a));
        }
    }
    best.map(|(d, _)| d.clone())
        .or_else(|| list.iter().find(|d| d.phys.contains(r.x, r.y)).cloned())
        .unwrap_or_else(|| primary(list))
}

/// The pointer in physical pixels, if the session lets us read it.
pub fn cursor_phys() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p).ok()? };
    Some((p.x, p.y))
}

// ------------------------------------------------------ the active display

/// The HMONITOR of the foreground window, unless there is none or it is one
/// of ours (the bar has focus while a question is typed).
pub fn foreground_monitor() -> Option<isize> {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        let _ = GetWindowThreadProcessId(fg, Some(&mut pid));
        if pid == 0 || pid == std::process::id() {
            return None;
        }
        let m = MonitorFromWindow(fg, MONITOR_DEFAULTTONULL);
        if m.0.is_null() {
            None
        } else {
            Some(m.0 as isize)
        }
    }
}

/// D-09: which display is active, as an index into `phys`. The monitor of
/// the foreground window when that window is not ours (`foreground`), else
/// the display under the pointer, else the one nearest the pointer, else
/// the primary. One rule for capture and for window placement.
pub fn pick_active(
    phys: &[PxRect],
    primary: usize,
    foreground: Option<usize>,
    cursor: Option<(i32, i32)>,
) -> usize {
    if let Some(i) = foreground.filter(|&i| i < phys.len()) {
        return i;
    }
    if let Some((x, y)) = cursor {
        if let Some(i) = phys.iter().position(|r| r.contains(x, y)) {
            return i;
        }
        let gap = |i: &usize| phys[*i].distance_to(x, y);
        if let Some(i) = (0..phys.len()).min_by_key(gap) {
            return i;
        }
    }
    primary.min(phys.len().saturating_sub(1))
}

/// The screen the user is working on (D-09, CAP-01): see `pick_active`.
pub fn active_display(list: &[Display]) -> Display {
    let phys: Vec<PxRect> = list.iter().map(|d| d.phys).collect();
    let fg = foreground_monitor().and_then(|m| list.iter().position(|d| d.hmonitor == m));
    let p = list.iter().position(|d| d.primary).unwrap_or(0);
    list.get(pick_active(&phys, p, fg, cursor_phys()))
        .cloned()
        .unwrap_or_else(|| primary(list))
}

/// The display `latch` recorded (`Display::id`): the one the person was on
/// when they called the bar up, kept while the bar has focus.
static LATCHED: Mutex<Option<u32>> = Mutex::new(None);

/// Records the display placement and capture keep to (K-10's latch).
pub fn latch(id: u32) {
    *LATCHED.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
}

pub fn latched_id() -> Option<u32> {
    *LATCHED.lock().unwrap_or_else(|e| e.into_inner())
}

/// The latched display as it is now; the active display when nothing was
/// latched or that display is gone.
pub fn latched_display(list: &[Display]) -> Display {
    find_latched(list, latched_id()).unwrap_or_else(|| active_display(list))
}

fn find_latched(list: &[Display], id: Option<u32>) -> Option<Display> {
    let id = id?;
    list.iter().find(|d| d.id == id).cloned()
}

/// Physical rect to DIP (`screen.screenToDipRect(null, rect)`).
pub fn screen_to_dip_rect(list: &[Display], r: PxRect) -> Rect {
    matching_phys(list, &r).rect_to_dip(r)
}

/// DIP rect to physical (`screen.dipToScreenRect(null, rect)`).
// (c)'s geometry API; index.js exported it.
#[allow(dead_code)]
pub fn dip_to_screen_rect(list: &[Display], r: Rect) -> PxRect {
    matching(list, &r).rect_to_phys(r)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn pr(x: i32, y: i32, w: i32, h: i32) -> PxRect {
        PxRect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn desk() -> Vec<Display> {
        with_layout(vec![
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
        ])
    }

    /// B-005's case: a 100% primary with a 150% display to its right. Under
    /// the old origin/scale rule their DIP ranges overlapped at 1280..1920.
    pub fn mixed() -> Vec<Display> {
        with_layout(vec![
            Display::from_phys(
                r"\\.\DISPLAY1",
                true,
                96,
                pr(0, 0, 1920, 1080),
                pr(0, 0, 1920, 1040),
            ),
            Display::from_phys(
                r"\\.\DISPLAY2",
                false,
                144,
                pr(1920, 0, 2560, 1440),
                pr(1920, 0, 2560, 1380),
            ),
        ])
    }

    fn no_overlaps(list: &[Display]) -> bool {
        list.iter().enumerate().all(|(i, a)| {
            list.iter()
                .skip(i + 1)
                .all(|b| a.bounds.overlap(&b.bounds) == 0.0)
        })
    }

    #[test]
    fn overlapping_mixed_dpi_layout_is_adjacent() {
        let d = mixed();
        assert_eq!(d[0].bounds, Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(d[1].bounds, Rect::new(1920.0, 0.0, 1707.0, 960.0));
        assert_eq!(d[1].work_area, Rect::new(1920.0, 0.0, 1707.0, 920.0));
        // They touch at x=1920 and share no area.
        assert_eq!(d[0].bounds.x + d[0].bounds.width, d[1].bounds.x);
        assert!(no_overlaps(&d));
    }

    #[test]
    fn rect_in_old_overlap_maps_to_primary() {
        let d = mixed();
        let r = Rect::new(1500.0, 200.0, 300.0, 150.0);
        assert_eq!(matching(&d, &r).device, r"\\.\DISPLAY1");
        assert_eq!(dip_to_screen_rect(&d, r), pr(1500, 200, 300, 150));
        // The same spot seen from the physical side round-trips.
        assert_eq!(screen_to_dip_rect(&d, pr(1500, 200, 300, 150)), r);
        // And the secondary's own first DIPs are its own.
        let s = Rect::new(1930.0, 10.0, 100.0, 100.0);
        assert_eq!(matching(&d, &s).device, r"\\.\DISPLAY2");
        assert_eq!(dip_to_screen_rect(&d, s), pr(1935, 15, 150, 150));
    }

    #[test]
    fn layouts_never_overlap_whatever_the_scales() {
        // Left of the primary at 150%, below it at 125%, and a third display
        // to the right of the second one (an L), at mixed scales.
        let d = with_layout(vec![
            Display::from_phys(
                r"\\.\DISPLAY1",
                true,
                96,
                pr(0, 0, 1920, 1080),
                pr(0, 0, 1920, 1080),
            ),
            Display::from_phys(
                r"\\.\DISPLAY2",
                false,
                144,
                pr(-2560, 0, 2560, 1440),
                pr(-2560, 0, 2560, 1440),
            ),
            Display::from_phys(
                r"\\.\DISPLAY3",
                false,
                120,
                pr(0, 1080, 1920, 1080),
                pr(0, 1080, 1920, 1080),
            ),
            Display::from_phys(
                r"\\.\DISPLAY4",
                false,
                192,
                pr(1920, 1080, 3840, 2160),
                pr(1920, 1080, 3840, 2160),
            ),
        ]);
        assert!(no_overlaps(&d), "{d:?}");
        assert_eq!(d[1].bounds, Rect::new(-1707.0, 0.0, 1707.0, 960.0));
        assert_eq!(d[2].bounds, Rect::new(0.0, 1080.0, 1536.0, 864.0));
        // It meets the primary at a corner, which Chromium counts as
        // touching: below the primary, at its physical offset.
        assert_eq!(d[3].bounds.x, 1920.0);
        assert_eq!(d[3].bounds.y, 1080.0);
    }

    #[test]
    fn layout_ignores_enumeration_order() {
        let mut d = mixed();
        d.reverse();
        let d = with_layout(d);
        assert_eq!(d[1].bounds, Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(d[0].bounds, Rect::new(1920.0, 0.0, 1707.0, 960.0));
    }

    #[test]
    fn offsets_along_the_edge_follow_chromium() {
        // A 200% display to the right, starting 400 px lower: the offset is
        // measured on the parent (100%), so 400 DIP.
        let two = |y: i32| {
            with_layout(vec![
                Display::from_phys(
                    r"\\.\DISPLAY1",
                    true,
                    96,
                    pr(0, 0, 1920, 1080),
                    pr(0, 0, 1920, 1080),
                ),
                Display::from_phys(
                    r"\\.\DISPLAY2",
                    false,
                    192,
                    pr(1920, y, 3840, 2160),
                    pr(1920, y, 3840, 2160),
                ),
            ])
        };
        let d = two(400);
        assert_eq!((d[1].bounds.x, d[1].bounds.y), (1920.0, 400.0));
        // Starting 400 px higher: measured on itself (200%), so -200 DIP.
        let d = two(-400);
        assert_eq!((d[1].bounds.x, d[1].bounds.y), (1920.0, -200.0));
    }

    #[test]
    fn dip_bounds_divide_by_scale() {
        let d = desk();
        assert_eq!(d[0].bounds, Rect::new(0.0, 0.0, 1280.0, 720.0));
        assert_eq!(d[0].scale_factor, 1.5);
        // Edge to edge with the 150% primary, not at 1920 / 1.
        assert_eq!(d[1].bounds, Rect::new(1280.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn matching_picks_most_overlap_by_id_not_order() {
        let d = desk();
        let r = Rect::new(1270.0, 10.0, 300.0, 100.0);
        assert_eq!(matching(&d, &r).device, r"\\.\DISPLAY2");
        let r = Rect::new(100.0, 100.0, 10.0, 10.0);
        assert_eq!(matching(&d, &r).device, r"\\.\DISPLAY1");
        // Off every display: the nearest one.
        let r = Rect::new(5000.0, 10.0, 10.0, 10.0);
        assert_eq!(matching(&d, &r).device, r"\\.\DISPLAY2");
    }

    #[test]
    fn ties_go_to_the_primary_then_the_centre() {
        let d = mixed();
        // Straddles the edge with equal area on both sides.
        let r = Rect::new(1820.0, 100.0, 200.0, 100.0);
        assert_eq!(matching(&d, &r).device, r"\\.\DISPLAY1");
        let mut swapped = d.clone();
        swapped.reverse();
        assert_eq!(matching(&swapped, &r).device, r"\\.\DISPLAY1");
        let p = pr(1820, 100, 200, 100);
        assert_eq!(matching_phys(&swapped, &p).device, r"\\.\DISPLAY1");
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
    fn physical_points_map_through_their_own_display() {
        let d = desk();
        assert_eq!(d[0].point_to_dip((300.0, 150.0)), (200.0, 100.0));
        assert_eq!(d[1].point_to_dip((1930.0, 5.0)), (1290.0, 5.0));
    }

    #[test]
    fn active_prefers_foreground_monitor() {
        let phys = [pr(0, 0, 1920, 1080), pr(1920, 0, 2560, 1440)];
        // The foreground window's monitor wins over the pointer.
        assert_eq!(pick_active(&phys, 0, Some(1), Some((100, 100))), 1);
        assert_eq!(pick_active(&phys, 0, Some(0), Some((2000, 100))), 0);
        // Our own window in front (no foreground): the pointer's display.
        assert_eq!(pick_active(&phys, 0, None, Some((2000, 100))), 1);
        // A pointer in a gap: the nearest display.
        assert_eq!(pick_active(&phys, 0, None, Some((5000, 100))), 1);
        assert_eq!(pick_active(&phys, 0, None, Some((-50, 100))), 0);
        // Nothing to go on: the primary.
        assert_eq!(pick_active(&phys, 0, None, None), 0);
        assert_eq!(pick_active(&phys, 1, Some(7), None), 1);
    }

    #[test]
    fn latch_finds_its_display_only_while_it_exists() {
        let d = mixed();
        let got = find_latched(&d, Some(d[1].id)).map(|x| x.device);
        assert_eq!(got.as_deref(), Some(r"\\.\DISPLAY2"));
        assert_eq!(find_latched(&d, Some(12345)), None);
        assert_eq!(find_latched(&d, None), None);
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
