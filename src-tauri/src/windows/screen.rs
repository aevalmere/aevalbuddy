//! Displays in Electron's units (ARCHITECTURE.md D11 `screen.*`, PARITY
//! POS-24). Every stored anchor, offset and width is in DIPs; Win32 speaks
//! physical pixels. The DIP layout is Chromium's, built by
//! `capture::screen::dip_origins` (B-005): the primary keeps (0, 0) and every
//! other display sits edge to edge with the one it touches, so DIP rects
//! never overlap and a DIP rect names one display. Capture uses the same
//! layout and the same active-display rule (D-09).

use std::cmp::Ordering;

use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

use crate::capture::screen as cap;
use crate::state::Rect;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Display {
    /// The HMONITOR.
    pub id: isize,
    /// Stable across display changes: `capture::screen::display_id` of the
    /// device name, the same number as the capture side's `Display::id`.
    pub key: u32,
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
    /// One display on its own: DIP origin at its physical origin over its
    /// scale. `build` places it among the others.
    fn from_px(id: isize, primary: bool, scale: f64, bounds_px: Rect, work_px: Rect) -> Self {
        let mut d = Display {
            id,
            key: 0,
            primary,
            scale,
            bounds_px,
            work_px,
            bounds: Rect::default(),
            work_area: Rect::default(),
        };
        d.place_at((bounds_px.x / scale, bounds_px.y / scale));
        d
    }

    /// Puts the DIP bounds (and the work area with them) at `(ox, oy)`.
    fn place_at(&mut self, (ox, oy): (f64, f64)) {
        let (s, b) = (self.scale, self.bounds_px);
        let to_dip = |r: Rect| {
            Rect::new(
                ox + (r.x - b.x) / s,
                oy + (r.y - b.y) / s,
                r.width / s,
                r.height / s,
            )
        };
        self.bounds = to_dip(self.bounds_px);
        self.work_area = to_dip(self.work_px);
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

fn rect(r: cap::PxRect) -> Rect {
    Rect::new(
        f64::from(r.x),
        f64::from(r.y),
        f64::from(r.width),
        f64::from(r.height),
    )
}

/// LIFE-03, K-11: `AEVAL_SHOT_SCALE` forces the scale the window maths use.
pub fn forced_scale() -> Option<f64> {
    let v: f64 = std::env::var("AEVAL_SHOT_SCALE").ok()?.parse().ok()?;
    (v > 0.0).then_some(v)
}

/// The displays for a set of monitors, in the canonical layout, primary
/// first. `forced` replaces every monitor's scale (LIFE-03).
fn build(mons: &[cap::Monitor], forced: Option<f64>) -> Vec<Display> {
    let scales: Vec<f64> = mons
        .iter()
        .map(|m| forced.unwrap_or_else(|| m.scale()))
        .collect();
    let places: Vec<cap::Place<'_>> = mons
        .iter()
        .zip(&scales)
        .map(|(m, &scale)| cap::Place {
            name: &m.device,
            primary: m.primary,
            scale,
            phys: m.phys,
        })
        .collect();
    let origins = cap::dip_origins(&places);
    let mut out: Vec<Display> = mons
        .iter()
        .zip(scales)
        .zip(origins)
        .map(|((m, scale), o)| {
            let (b, w) = (rect(m.phys), rect(m.work));
            let mut d = Display::from_px(m.hmonitor, m.primary, scale, b, w);
            d.key = cap::display_id(&m.device);
            d.place_at(o);
            d
        })
        .collect();
    out.sort_by_key(|d| !d.primary);
    out
}

/// Every display, primary first.
pub fn all() -> Vec<Display> {
    let mut out = build(&cap::monitors(), forced_scale());
    if out.is_empty() {
        let r = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let s = forced_scale().unwrap_or(1.0);
        out.push(Display::from_px(0, true, s, r, r));
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

/// The display a DIP rect mostly sits on (getDisplayMatching). Ties go to
/// the primary, then to the display holding the rect's centre.
pub fn matching(r: Rect) -> Display {
    matching_in(&all(), r)
}

/// The display among `list` whose `area` overlaps `r` most, ties as in
/// `matching`.
fn most_overlap(list: &[Display], r: Rect, area: impl Fn(&Display) -> Rect) -> Option<Display> {
    let (cx, cy) = r.center();
    let rank = |d: &Display| (d.primary, area(d).contains(cx, cy));
    let mut best: Option<(Display, f64)> = None;
    for d in list {
        let a = r.overlap(&area(d));
        if a <= 0.0 {
            continue;
        }
        let better = match best {
            None => true,
            Some((b, ba)) => match a.total_cmp(&ba) {
                Ordering::Greater => true,
                Ordering::Equal => rank(d) > rank(&b),
                Ordering::Less => false,
            },
        };
        if better {
            best = Some((*d, a));
        }
    }
    best.map(|(d, _)| d)
}

fn matching_in(list: &[Display], r: Rect) -> Display {
    let best = most_overlap(list, r, |d| d.bounds);
    best.unwrap_or_else(|| nearest(r.center(), list))
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
    matching_px_in(&all(), r)
}

fn matching_px_in(list: &[Display], r: Rect) -> Display {
    let best = most_overlap(list, r, |d| d.bounds_px);
    best.unwrap_or_else(primary)
}

/// The pointer, physical pixels.
pub fn cursor_px() -> (f64, f64) {
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: an out-parameter.
    unsafe { GetCursorPos(&mut p) };
    (f64::from(p.x), f64::from(p.y))
}

/// The pointer, when the session lets us read it.
fn cursor_now() -> Option<(i32, i32)> {
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: an out-parameter.
    let ok = unsafe { GetCursorPos(&mut p) } != 0;
    ok.then_some((p.x, p.y))
}

fn active_in(list: &[Display]) -> Display {
    let phys: Vec<cap::PxRect> = list
        .iter()
        .map(|d| cap::PxRect {
            x: d.bounds_px.x as i32,
            y: d.bounds_px.y as i32,
            width: d.bounds_px.width as i32,
            height: d.bounds_px.height as i32,
        })
        .collect();
    let fg = cap::foreground_monitor().and_then(|m| list.iter().position(|d| d.id == m));
    let p = list.iter().position(|d| d.primary).unwrap_or(0);
    list.get(cap::pick_active(&phys, p, fg, cursor_now()))
        .copied()
        .unwrap_or_else(primary)
}

/// K-10 (D-09): the display the person is using. The monitor of the
/// foreground window when that window is not ours, else the display under
/// the pointer, else the nearest one to it. Capture uses the same rule.
pub fn active() -> Display {
    active_in(&all())
}

/// K-10: the active display, recorded until the next latch. Capture reads
/// the same latch, so a question is about the screen the bar was opened on.
pub fn latch_active() -> Display {
    let d = active();
    cap::latch(d.key);
    d
}

/// K-10: the display `latch_active` recorded, as it is now; the live active
/// display when nothing was latched or that display is gone.
pub fn latched() -> Display {
    let list = all();
    cap::latched_id()
        .and_then(|k| list.iter().copied().find(|d| d.key == k))
        .unwrap_or_else(|| active_in(&list))
}

/// K-10: a display's work area in DIPs, read live; the active display's when
/// that display is gone.
pub fn work_area_of(display: Display) -> Rect {
    let list = all();
    list.iter()
        .find(|d| d.key == display.key)
        .map(|d| d.work_area)
        .unwrap_or_else(|| active_in(&list).work_area)
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
    use crate::capture::screen::tests::pr;

    fn mon(device: &str, primary: bool, dpi: u32, phys: cap::PxRect) -> cap::Monitor {
        cap::Monitor {
            hmonitor: cap::display_id(device) as isize,
            device: device.to_string(),
            primary,
            dpi,
            phys,
            work: phys,
        }
    }

    /// B-005's case: a 100% primary, a 150% display to its right.
    fn mixed() -> Vec<Display> {
        let mons = [
            mon(r"\\.\DISPLAY1", true, 96, pr(0, 0, 1920, 1080)),
            mon(r"\\.\DISPLAY2", false, 144, pr(1920, 0, 2560, 1440)),
        ];
        build(&mons, None)
    }

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
    fn overlapping_mixed_dpi_layout_is_adjacent() {
        let d = mixed();
        assert_eq!(d[0].bounds, Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(d[1].bounds.x, 1920.0);
        assert_eq!(d[1].bounds.y, 0.0);
        assert_eq!(d[0].bounds.overlap(&d[1].bounds), 0.0);
        // The same layout as the capture side, display for display.
        let c = cap::tests::mixed();
        assert_eq!(d[1].bounds.x, c[1].bounds.x);
        assert_eq!(d[1].bounds.y, c[1].bounds.y);
        assert_eq!(d[1].key, c[1].id);
    }

    #[test]
    fn rect_in_old_overlap_maps_to_primary() {
        let d = mixed();
        let r = Rect::new(1500.0, 200.0, 300.0, 150.0);
        let m = matching_in(&d, r);
        assert!(m.primary);
        assert_eq!(m.dip_to_px(r), r);
        // Read back from the physical side through the same display.
        let p = matching_px_in(&d, r);
        assert!(p.primary);
        assert_eq!(p.px_to_dip(r), r);
        // The secondary's first DIPs are its own.
        let sr = Rect::new(1930.0, 10.0, 100.0, 100.0);
        let s = matching_in(&d, sr);
        assert!(!s.primary);
        assert_eq!(s.dip_to_px(sr).x, 1935.0);
    }

    #[test]
    fn forced_scale_applies_to_every_display() {
        let one = [mon(r"\\.\DISPLAY1", true, 96, pr(0, 0, 1920, 1080))];
        let d = build(&one, Some(1.5));
        assert_eq!(d[0].scale, 1.5);
        assert_eq!(d[0].bounds, Rect::new(0.0, 0.0, 1280.0, 720.0));
    }

    #[test]
    fn latch_reads_the_display_it_recorded() {
        let d = latch_active();
        assert_eq!(latched().key, d.key);
        assert_eq!(work_area_of(d), d.work_area);
    }

    #[test]
    fn enumerates_at_least_one() {
        let list = all();
        assert!(!list.is_empty());
        assert!(list[0].primary || list.len() == 1);
    }
}
