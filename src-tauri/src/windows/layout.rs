//! Placement maths from main.js, pure so it can be tested (PARITY POS-02,
//! POS-03, POS-06, POS-07, NEW-01). Everything is in DIPs and speaks the
//! glass rectangle, never the window with its stage margin.

use serde_json::{json, Value};

use crate::state::Rect;

pub const PILL: (f64, f64) = (44.0, 44.0);
pub const PANEL_WIDTH: f64 = 420.0;
pub const INPUT_MIN_HEIGHT: f64 = 62.0;
pub const INPUT_MAX_HEIGHT: f64 = 280.0;
pub const DEFAULT_INPUT_HEIGHT: f64 = 76.0;
pub const OUTPUT_MIN_HEIGHT: f64 = 90.0;
pub const OUTPUT_MAX_FRACTION: f64 = 0.66;
pub const OUTPUT_MAX_FLOOR: f64 = 380.0;
pub const DEFAULT_OUTPUT_HEIGHT: f64 = 120.0;
pub const HUD_INPUT_HEIGHT: f64 = 140.0;
pub const DOCK_GAP: f64 = 8.0;
pub const SNAP_MARGIN: f64 = 16.0;
pub const SNAP_DISTANCE: f64 = 110.0;
pub const DOCK_DISTANCE: f64 = 150.0;
pub const DROP_SETTLE_MS: u64 = 160;
pub const CHAT_HISTORY_LIMIT: usize = 12;
pub const T_EXPAND: u64 = 260;
pub const T_COLLAPSE: u64 = 170;
pub const T_OUT_IN: u64 = 300;
pub const T_OUT_OUT: u64 = 150;
pub const T_OUT_GROW: u64 = 200;
/// Offset between output windows that would otherwise land on each other.
pub const CASCADE: f64 = 28.0;

/// An anchor config: which edge each axis hugs, and the offset from it.
#[derive(Clone, Debug, PartialEq)]
pub struct Anchor {
    pub h: String,
    pub v: String,
    pub off_x: f64,
    pub off_y: f64,
}

impl Anchor {
    /// From a sanitised store value (`store::sanitize_pos`).
    pub fn from_value(v: &Value) -> Anchor {
        Anchor {
            h: v["h"].as_str().unwrap_or("right").to_string(),
            v: v["v"].as_str().unwrap_or("top").to_string(),
            off_x: v["offX"].as_f64().unwrap_or(16.0),
            off_y: v["offY"].as_f64().unwrap_or(16.0),
        }
    }

    pub fn to_value(&self) -> Value {
        json!({ "h": self.h, "v": self.v, "offX": self.off_x, "offY": self.off_y })
    }
}

pub fn clamp(n: f64, lo: f64, hi: f64) -> f64 {
    n.min(hi).max(lo)
}

/// JavaScript Math.round.
fn round(n: f64) -> f64 {
    (n + 0.5).floor()
}

/// main.js boundsForSize: hug an edge with offset, or centre; clamp into the
/// work area.
pub fn bounds_for_size(wa: Rect, size: (f64, f64), p: &Anchor) -> Rect {
    let (w, h) = size;
    let x = match p.h.as_str() {
        "left" => wa.x + p.off_x,
        "right" => wa.x + wa.width - p.off_x - w,
        _ => wa.x + round((wa.width - w) / 2.0),
    };
    let y = match p.v.as_str() {
        "top" => wa.y + p.off_y,
        "bottom" => wa.y + wa.height - p.off_y - h,
        _ => wa.y + round((wa.height - h) / 2.0),
    };
    Rect::new(
        clamp(x, wa.x, wa.x + wa.width - w),
        clamp(y, wa.y, wa.y + wa.height - h),
        w,
        h,
    )
}

pub fn output_max_height(wa: Rect) -> f64 {
    OUTPUT_MAX_FLOOR.max(round(wa.height * OUTPUT_MAX_FRACTION))
}

/// The output height (POS-05): a dragged height wins, else the transcript.
pub fn output_height(wa: Rect, content: f64, user: f64) -> f64 {
    if user > 0.0 {
        clamp(user, OUTPUT_MIN_HEIGHT, wa.height - 40.0)
    } else {
        clamp(round(content), OUTPUT_MIN_HEIGHT, output_max_height(wa))
    }
}

/// main.js dockedBounds: below a top-anchored bar, above a bottom one.
pub fn docked_bounds(wa: Rect, bar: Rect, size: (f64, f64), bar_v: &str) -> Rect {
    let (w, h) = size;
    let y = if bar_v == "bottom" {
        bar.y - DOCK_GAP - h
    } else {
        bar.y + bar.height + DOCK_GAP
    };
    Rect::new(
        clamp(bar.x, wa.x, wa.x + wa.width - w),
        clamp(y, wa.y, wa.y + wa.height - h),
        w,
        h,
    )
}

/// NEW-01: where a new output window opens. The slot against the bar on its
/// free side first; if an open window covers most of that slot, the next
/// slot across (toward the middle of the screen), then a cascade.
pub fn place_output(
    wa: Rect,
    bar: Rect,
    bar_anchor: &Anchor,
    size: (f64, f64),
    open: &[Rect],
) -> Rect {
    let first = docked_bounds(wa, bar, size, &bar_anchor.v);
    let free = |r: &Rect| open.iter().all(|o| o.overlap(r) < r.width * r.height * 0.5);
    if free(&first) {
        return first;
    }
    let dir = if bar_anchor.h == "left" { 1.0 } else { -1.0 };
    for i in 1..4 {
        let x = first.x + dir * f64::from(i) * (size.0 + DOCK_GAP);
        if x < wa.x || x + size.0 > wa.x + wa.width {
            break;
        }
        let r = Rect::new(x, first.y, size.0, size.1);
        if free(&r) {
            return r;
        }
    }
    let mut r = first;
    for _ in 0..open.len().min(12) {
        r.x = clamp(r.x - dir * CASCADE, wa.x, wa.x + wa.width - size.0);
        r.y = clamp(r.y + CASCADE, wa.y, wa.y + wa.height - size.1);
        if free(&r) {
            break;
        }
    }
    r
}

/// main.js posFromRect: each axis hugs the nearer edge, keeping the offset.
pub fn pos_from_rect(wa: Rect, r: Rect) -> Anchor {
    let h = if r.x + r.width / 2.0 < wa.x + wa.width / 2.0 {
        "left"
    } else {
        "right"
    };
    let v = if r.y + r.height / 2.0 < wa.y + wa.height / 2.0 {
        "top"
    } else {
        "bottom"
    };
    Anchor {
        h: h.into(),
        v: v.into(),
        off_x: if h == "left" {
            r.x - wa.x
        } else {
            wa.x + wa.width - r.x - r.width
        },
        off_y: if v == "top" {
            r.y - wa.y
        } else {
            wa.y + wa.height - r.y - r.height
        },
    }
}

fn center_distance(a: Rect, b: Rect) -> f64 {
    let (ax, ay) = a.center();
    let (bx, by) = b.center();
    (ax - bx).hypot(ay - by)
}

/// POS-07: the nearest of the nine anchors within 110 px of centre distance.
pub fn find_snap(wa: Rect, r: Rect, size: (f64, f64)) -> Option<(Anchor, Rect)> {
    let mut best: Option<(Anchor, Rect, f64)> = None;
    for h in ["left", "center", "right"] {
        for v in ["top", "center", "bottom"] {
            let pos = Anchor {
                h: h.into(),
                v: v.into(),
                off_x: SNAP_MARGIN,
                off_y: SNAP_MARGIN,
            };
            let target = bounds_for_size(wa, size, &pos);
            let d = center_distance(target, r);
            let better = best.as_ref().map_or(true, |b| d < b.2);
            if d <= SNAP_DISTANCE && better {
                best = Some((pos, target, d));
            }
        }
    }
    best.map(|(p, r, _)| (p, r))
}

/// POS-18: arrows step the bar across the 3x3 grid of the work area.
pub fn nudge(wa: Rect, b: Rect, dx: f64, dy: f64) -> Rect {
    let col = round((b.x - wa.x) / (wa.width - b.width).max(1.0) * 2.0) + sign(dx);
    let row = round((b.y - wa.y) / (wa.height - b.height).max(1.0) * 2.0) + sign(dy);
    let col = clamp(col, 0.0, 2.0);
    let row = clamp(row, 0.0, 2.0);
    Rect::new(
        round(wa.x + (wa.width - b.width) * col / 2.0),
        round(wa.y + (wa.height - b.height) * row / 2.0),
        b.width,
        b.height,
    )
}

/// JavaScript Math.sign (0 stays 0).
fn sign(n: f64) -> f64 {
    if n > 0.0 {
        1.0
    } else if n < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// The pill's place for an anchor.
pub fn pill_bounds(wa: Rect, anchor: &Anchor) -> Rect {
    bounds_for_size(wa, PILL, anchor)
}

/// The open bar's place (inputBounds).
pub fn input_bounds(wa: Rect, anchor: &Anchor, width: f64, height: f64) -> Rect {
    let h = clamp(round(height), INPUT_MIN_HEIGHT, INPUT_MAX_HEIGHT);
    bounds_for_size(wa, (width, h), anchor)
}

/// POS-23: the side of the display an output dodges to.
pub fn dodge_x(wa: Rect, b: Rect) -> f64 {
    let on_left = b.x + b.width / 2.0 < wa.x + wa.width / 2.0;
    if on_left {
        wa.x + wa.width - b.width - SNAP_MARGIN
    } else {
        wa.x + SNAP_MARGIN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wa() -> Rect {
        Rect::new(0.0, 0.0, 1920.0, 1040.0)
    }

    fn a(h: &str, v: &str, x: f64, y: f64) -> Anchor {
        Anchor {
            h: h.into(),
            v: v.into(),
            off_x: x,
            off_y: y,
        }
    }

    #[test]
    fn bounds_hug_and_clamp() {
        let r = bounds_for_size(wa(), (420.0, 76.0), &a("right", "top", 16.0, 16.0));
        assert_eq!(r, Rect::new(1484.0, 16.0, 420.0, 76.0));
        let c = bounds_for_size(wa(), (44.0, 44.0), &a("center", "bottom", 0.0, 16.0));
        assert_eq!(c, Rect::new(938.0, 980.0, 44.0, 44.0));
        let off = bounds_for_size(wa(), (420.0, 76.0), &a("left", "top", 8000.0, 8000.0));
        assert_eq!(off, Rect::new(1500.0, 964.0, 420.0, 76.0));
    }

    #[test]
    fn output_heights() {
        assert_eq!(output_max_height(wa()), 686.0);
        assert_eq!(output_max_height(Rect::new(0.0, 0.0, 800.0, 500.0)), 380.0);
        assert_eq!(output_height(wa(), 40.0, 0.0), 90.0);
        assert_eq!(output_height(wa(), 2000.0, 0.0), 686.0);
        assert_eq!(output_height(wa(), 2000.0, 5000.0), 1000.0);
    }

    #[test]
    fn docked_and_free_placement() {
        let bar = Rect::new(1484.0, 16.0, 420.0, 76.0);
        let top = a("right", "top", 16.0, 16.0);
        let first = place_output(wa(), bar, &top, (420.0, 200.0), &[]);
        assert_eq!(first, Rect::new(1484.0, 100.0, 420.0, 200.0));
        let second = place_output(wa(), bar, &top, (420.0, 200.0), &[first]);
        assert_eq!(second, Rect::new(1056.0, 100.0, 420.0, 200.0));
        let third = place_output(wa(), bar, &top, (420.0, 200.0), &[first, second]);
        assert_eq!(third.x, 628.0);
        let low = Rect::new(100.0, 948.0, 420.0, 76.0);
        let up = place_output(
            wa(),
            low,
            &a("left", "bottom", 100.0, 16.0),
            (420.0, 200.0),
            &[],
        );
        assert_eq!(up, Rect::new(100.0, 740.0, 420.0, 200.0));
    }

    #[test]
    fn pos_from_rect_and_snap() {
        let p = pos_from_rect(wa(), Rect::new(1500.0, 900.0, 400.0, 100.0));
        assert_eq!(p, a("right", "bottom", 20.0, 40.0));
        let (pos, r) =
            find_snap(wa(), Rect::new(30.0, 40.0, 420.0, 140.0), (420.0, 140.0)).unwrap();
        assert_eq!((pos.h.as_str(), pos.v.as_str()), ("left", "top"));
        assert_eq!(r, Rect::new(16.0, 16.0, 420.0, 140.0));
        assert!(find_snap(wa(), Rect::new(600.0, 300.0, 420.0, 140.0), (420.0, 140.0)).is_none());
    }

    #[test]
    fn nudges_step_the_grid() {
        let b = Rect::new(16.0, 16.0, 420.0, 140.0);
        let r = nudge(wa(), b, 1.0, 0.0);
        assert_eq!(r.x, 750.0);
        let r = nudge(wa(), r, 1.0, 1.0);
        assert_eq!((r.x, r.y), (1500.0, 450.0));
        let r = nudge(wa(), r, 1.0, 0.0);
        assert_eq!(r.x, 1500.0);
    }

    #[test]
    fn dodge() {
        assert_eq!(dodge_x(wa(), Rect::new(10.0, 0.0, 400.0, 10.0)), 1504.0);
        assert_eq!(dodge_x(wa(), Rect::new(1500.0, 0.0, 400.0, 10.0)), 16.0);
    }
}
