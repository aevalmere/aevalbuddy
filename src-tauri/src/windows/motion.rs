//! Window-level motion (lib/motion.js, PARITY POS-22). One curve,
//! `cubic-bezier(0.32, 0.72, 0, 1)` (DESIGN.md's --ease-glide), 60 Hz
//! SetWindowPos, one animation per window; a new one takes over from where
//! the last had reached. Reduced motion jumps to the end. Bounds only, never
//! opacity (WIN-15).

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION,
};

use super::{flags, screen, stage};
use crate::state::Rect;

const FRAME: Duration = Duration::from_millis(16);

/// A cubic-bezier easing solved the way a browser does it.
#[derive(Clone, Copy, Debug)]
pub struct Bezier {
    ax: f64,
    bx: f64,
    cx: f64,
    ay: f64,
    by: f64,
    cy: f64,
}

impl Bezier {
    pub const fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Bezier {
            ax: 3.0 * x1 - 3.0 * x2 + 1.0,
            bx: 3.0 * x2 - 6.0 * x1,
            cx: 3.0 * x1,
            ay: 3.0 * y1 - 3.0 * y2 + 1.0,
            by: 3.0 * y2 - 6.0 * y1,
            cy: 3.0 * y1,
        }
    }

    pub fn at(&self, x: f64) -> f64 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        let mut t = x;
        for _ in 0..6 {
            let dx = ((self.ax * t + self.bx) * t + self.cx) * t - x;
            if dx.abs() < 1e-4 {
                break;
            }
            let d = (3.0 * self.ax * t + 2.0 * self.bx) * t + self.cx;
            if d.abs() < 1e-6 {
                break;
            }
            t -= dx / d;
        }
        ((self.ay * t + self.by) * t + self.cy) * t
    }
}

pub const GLIDE: Bezier = Bezier::new(0.32, 0.72, 0.0, 1.0);

pub fn lerp_rect(a: Rect, b: Rect, p: f64) -> Rect {
    let r = |x: f64| (x + 0.5).floor();
    Rect::new(
        r(a.x + (b.x - a.x) * p),
        r(a.y + (b.y - a.y) * p),
        r(a.width + (b.width - a.width) * p).max(1.0),
        r(a.height + (b.height - a.height) * p).max(1.0),
    )
}

/// LIFE-20: Windows' "Show animations" switch, read before every move.
pub fn reduced() -> bool {
    let mut on: i32 = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes a BOOL.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            &mut on as *mut i32 as *mut core::ffi::c_void,
            0,
        )
    };
    ok != 0 && on == 0
}

fn generations() -> &'static Mutex<HashMap<isize, u64>> {
    static G: OnceLock<Mutex<HashMap<isize, u64>>> = OnceLock::new();
    G.get_or_init(|| Mutex::new(HashMap::new()))
}

fn bump(hwnd: isize) -> u64 {
    let mut g = generations().lock();
    let e = g.entry(hwnd).or_insert(0);
    *e += 1;
    *e
}

fn running() -> &'static Mutex<HashMap<isize, bool>> {
    static R: OnceLock<Mutex<HashMap<isize, bool>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Stops whatever animation the window has; it stays where it got to.
pub fn stop(hwnd: isize) {
    bump(hwnd);
    running().lock().remove(&hwnd);
}

pub fn is_animating(hwnd: isize) -> bool {
    running().lock().get(&hwnd).copied().unwrap_or(false)
}

/// B-093: the window is gone, so are its entries; a glide still running
/// for it sees its generation gone and ends.
pub fn forget(hwnd: isize) {
    generations().lock().remove(&hwnd);
    running().lock().remove(&hwnd);
}

/// Whether anything is kept for `hwnd` (tests).
pub fn tracks(hwnd: isize) -> bool {
    generations().lock().contains_key(&hwnd) || running().lock().contains_key(&hwnd)
}

/// The window's glass rect in DIPs.
pub fn glass(hwnd: isize) -> Rect {
    let m = flags::meta(hwnd).margin;
    stage::deflate(screen::px_to_dip(flags::rect_px(hwnd)), m)
}

/// Puts the window's glass on `r` (DIPs) at once, through the display the
/// window is on while `r` is still on it (B-025).
pub fn set_glass(hwnd: isize, r: Rect) {
    let on = screen::matching_px(flags::rect_px(hwnd));
    set_glass_on(hwnd, r, through(on, screen::matching(r), r));
}

/// Puts the window's glass on `r`, which is in `display`'s DIPs.
pub fn set_glass_on(hwnd: isize, r: Rect, display: screen::Display) {
    let m = flags::meta(hwnd).margin;
    flags::set_rect_px(hwnd, display.dip_to_px(stage::inflate(r, m)));
}

/// Which display a glass rect in DIPs is written through. The DIP ranges of
/// displays with different scales overlap (each origin is its physical
/// origin over its own scale), so `r` alone cannot say; the display the
/// window was read from can.
fn through(on: screen::Display, matched: screen::Display, r: Rect) -> screen::Display {
    if r.overlap(&on.bounds) > 0.0 {
        on
    } else {
        matched
    }
}

pub type Done = Box<dyn FnOnce() + Send + 'static>;

/// Glides the glass from `from` (or where it is) to `to` over `ms`.
/// `on_done` runs on completion; a later call takes over and drops it.
pub fn animate(hwnd: isize, from: Option<Rect>, to: Rect, ms: u64, on_done: Option<Done>) {
    let gen = bump(hwnd);
    if reduced() || ms == 0 {
        running().lock().remove(&hwnd);
        set_glass(hwnd, to);
        if let Some(f) = on_done {
            f();
        }
        return;
    }
    let from = from.unwrap_or_else(|| glass(hwnd));
    running().lock().insert(hwnd, true);
    set_glass(hwnd, lerp_rect(from, to, 0.0));
    std::thread::spawn(move || {
        let started = Instant::now();
        let total = ms as f64;
        loop {
            std::thread::sleep(FRAME);
            if generations().lock().get(&hwnd).copied() != Some(gen) {
                return;
            }
            let linear = (started.elapsed().as_secs_f64() * 1000.0 / total).min(1.0);
            if linear >= 1.0 {
                running().lock().remove(&hwnd);
                set_glass(hwnd, to);
                if let Some(f) = on_done {
                    f();
                }
                return;
            }
            set_glass(hwnd, lerp_rect(from, to, GLIDE.at(linear)));
        }
    });
}

/// lib/motion.js shrunkFrom: slightly smaller, held to the edge it grows
/// away from.
pub fn shrunk_from(r: Rect, anchor_v: &str, anchor_h: &str) -> Rect {
    let rd = |x: f64| (x + 0.5).floor();
    let w = rd(r.width * 0.92);
    let h = rd(r.height * 0.86);
    let x = match anchor_h {
        "right" => r.x + (r.width - w),
        "left" => r.x,
        _ => r.x + rd((r.width - w) / 2.0),
    };
    let y = match anchor_v {
        "bottom" => r.y + (r.height - h),
        "center" => r.y + rd((r.height - h) / 2.0),
        _ => r.y,
    };
    Rect::new(x, y, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(id: isize, scale: f64, px: Rect, dip: Rect) -> screen::Display {
        screen::Display {
            id,
            primary: id == 1,
            scale,
            bounds_px: px,
            work_px: px,
            bounds: dip,
            work_area: dip,
        }
    }

    #[test]
    fn set_glass_round_trips_in_overlap_band() {
        let left_px = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let left = display(1, 1.0, left_px, left_px);
        // At 150% to the right: its DIP origin is 1920 / 1.5 = 1280, inside
        // the first display's DIP range.
        let right_px = Rect::new(1920.0, 0.0, 3840.0, 2160.0);
        let right_dip = Rect::new(1280.0, 0.0, 2560.0, 1440.0);
        let right = display(2, 1.5, right_px, right_dip);
        // A window on the right display, in the band both DIP ranges cover.
        let px = Rect::new(2400.0, 150.0, 600.0, 300.0);
        let r = right.px_to_dip(px);
        assert_eq!(r, Rect::new(1600.0, 100.0, 400.0, 200.0));
        assert!(r.overlap(&left.bounds) > 0.0);
        assert_eq!(through(right, left, r).dip_to_px(r), px);
        // A rect that has left the window's display goes by where it sits.
        let far = Rect::new(100.0, 100.0, 400.0, 200.0);
        assert_eq!(through(right, left, far).id, 1);
    }

    #[test]
    fn glide_curve_matches_the_browser() {
        assert_eq!(GLIDE.at(0.0), 0.0);
        assert_eq!(GLIDE.at(1.0), 1.0);
        // Values from the JS port in lib/motion.js.
        assert!(
            (GLIDE.at(0.5) - 0.954_831_9).abs() < 1e-6,
            "{}",
            GLIDE.at(0.5)
        );
        assert!((GLIDE.at(0.1) - 0.269_796_0).abs() < 1e-6);
        assert!(GLIDE.at(0.1) > 0.2 && GLIDE.at(0.1) < 0.4);
        let mut last = 0.0;
        for i in 1..=100 {
            let v = GLIDE.at(f64::from(i) / 100.0);
            assert!(v >= last - 1e-6);
            last = v;
        }
        let linear = Bezier::new(0.0, 0.0, 1.0, 1.0);
        assert!((linear.at(0.3) - 0.3).abs() < 1e-3);
    }

    #[test]
    fn lerp_and_shrink() {
        let a = Rect::new(0.0, 0.0, 100.0, 50.0);
        let b = Rect::new(100.0, 100.0, 300.0, 150.0);
        assert_eq!(lerp_rect(a, b, 0.5), Rect::new(50.0, 50.0, 200.0, 100.0));
        let s = shrunk_from(Rect::new(0.0, 0.0, 100.0, 100.0), "bottom", "right");
        assert_eq!(s, Rect::new(8.0, 14.0, 92.0, 86.0));
    }
}
