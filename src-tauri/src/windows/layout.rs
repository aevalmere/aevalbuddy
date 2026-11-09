//! Placement maths from main.js, pure so it can be tested (PARITY POS-02,
//! POS-03, POS-06, POS-07, NEW-01). Everything is in DIPs and speaks the
//! glass rectangle, never the window with its stage margin.

use serde_json::{json, Value};

use crate::state::Rect;

pub const PILL: (f64, f64) = (44.0, 44.0);
/// The bar's default width, the store's default (B-106; main.js 420).
pub const PANEL_WIDTH: f64 = 600.0;
pub const INPUT_MIN_HEIGHT: f64 = 62.0;
/// The open bar's tallest glass at 100% text: room for a seven-line question
/// with the token's menu or the @ model menu open under it, so a menu is
/// never cut by the window (B-polish). Scaled by the text size.
pub const INPUT_MAX_HEIGHT: f64 = 400.0;
pub const DEFAULT_INPUT_HEIGHT: f64 = 76.0;
/// Output window minimum (ARCHITECTURE.md 10.3: 320x160).
pub const OUTPUT_MIN_HEIGHT: f64 = 160.0;
pub const OUTPUT_MAX_FRACTION: f64 = 0.66;
pub const OUTPUT_MAX_FLOOR: f64 = 380.0;
pub const DEFAULT_OUTPUT_HEIGHT: f64 = 120.0;
pub const HUD_INPUT_HEIGHT: f64 = 140.0;
/// Glass to glass, bar to a new output window. Inside both 20 px stage
/// margins, so the page's glob can draw across it (ARCHITECTURE.md 10.3).
pub const OUTPUT_GAP: f64 = 12.0;
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
pub const CASCADE: f64 = 24.0;

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

/// B-027: the glass of a window whose sheet follows its transcript. The
/// sheet's anchored edge stays where `spot` has it (the bottom for a sheet
/// growing up, the top otherwise) and the window takes `room` away from
/// it, less whatever would leave the work area.
pub fn output_glass(wa: Rect, spot: Rect, room: f64, up: bool) -> Rect {
    let bottom = spot.y + spot.height;
    let free = if up {
        bottom - wa.y
    } else {
        wa.y + wa.height - spot.y
    };
    let h = room.min(free).max(OUTPUT_MIN_HEIGHT);
    let y = if up { bottom - h } else { spot.y };
    Rect::new(spot.x, y, spot.width, h)
}

/// The first slot: on the bar's free side (below a top-anchored bar, above
/// a bottom one), 12 px away, left edges aligned.
pub fn docked_bounds(wa: Rect, bar: Rect, size: (f64, f64), bar_v: &str) -> Rect {
    let (w, h) = size;
    let y = if bar_v == "bottom" {
        bar.y - OUTPUT_GAP - h
    } else {
        bar.y + bar.height + OUTPUT_GAP
    };
    Rect::new(
        clamp(bar.x, wa.x, wa.x + wa.width - w),
        clamp(y, wa.y, wa.y + wa.height - h),
        w,
        h,
    )
}

/// NEW-01 with B-099: where a new output window opens. The slot against
/// the bar on its free side first; while an open window already sits there,
/// 24 px further along the bar's edge toward the middle of the screen,
/// wrapping within the work area. Every window keeps the 12 px gap to the
/// bar, so the page's glob can reach it.
pub fn place_output(
    wa: Rect,
    bar: Rect,
    bar_anchor: &Anchor,
    size: (f64, f64),
    open: &[Rect],
) -> Rect {
    let first = docked_bounds(wa, bar, size, &bar_anchor.v);
    // Open windows are taller than a new sheet (B-027): compare the edge
    // that faces the bar, which both of them share.
    let edge = |r: &Rect| {
        if bar_anchor.v == "bottom" {
            r.y + r.height
        } else {
            r.y
        }
    };
    let close = |a: f64, b: f64| (a - b).abs() < CASCADE / 2.0;
    let taken = |r: &Rect| {
        open.iter()
            .any(|o| close(o.x, r.x) && close(edge(o), edge(r)))
    };
    let step = if bar_anchor.h == "right" {
        -CASCADE
    } else {
        CASCADE
    };
    let lo = wa.x;
    let span = (wa.width - size.0).max(0.0);
    let mut r = first;
    for i in 1..=open.len().min(64) {
        if !taken(&r) {
            break;
        }
        let x = first.x + step * i as f64;
        r.x = if span > 0.0 {
            lo + (x - lo).rem_euclid(span)
        } else {
            lo
        };
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

/// `window:nudge`: moved by (dx, dy) and resized by (dw, dh), kept inside
/// the work area and the size limits (a 0 maximum is no limit).
pub fn nudge_rect(
    wa: Rect,
    r: Rect,
    d: (f64, f64, f64, f64),
    min: (f64, f64),
    max: (f64, f64),
) -> Rect {
    let size = |v: f64, lo: f64, hi: f64, room: f64| {
        let hi = if hi > 0.0 { hi.min(room) } else { room };
        clamp(v, lo.min(hi), hi)
    };
    let w = size(r.width + d.2, min.0, max.0, wa.width);
    let h = size(r.height + d.3, min.1, max.1, wa.height);
    Rect::new(
        clamp(r.x + d.0, wa.x, wa.x + wa.width - w),
        clamp(r.y + d.1, wa.y, wa.y + wa.height - h),
        w,
        h,
    )
}

/// B-025: `r` inside `wa`, the work area of its own display: no bigger than
/// the area, and moved in only as far as it has to be.
pub fn clamp_to(wa: Rect, r: Rect) -> Rect {
    let w = r.width.min(wa.width);
    let h = r.height.min(wa.height);
    Rect::new(
        clamp(r.x, wa.x, wa.x + wa.width - w),
        clamp(r.y, wa.y, wa.y + wa.height - h),
        w,
        h,
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

/// The open bar's place (inputBounds). `text_scale` (1..3) scales the
/// height cap with the type, so a menu at 200% text fits as it does at 100%;
/// the work area caps it too.
pub fn input_bounds(wa: Rect, anchor: &Anchor, width: f64, height: f64, text_scale: f64) -> Rect {
    let s = if text_scale.is_finite() {
        text_scale.clamp(1.0, 3.0)
    } else {
        1.0
    };
    let max = (INPUT_MAX_HEIGHT * s).min(wa.height.max(INPUT_MIN_HEIGHT));
    let h = clamp(round(height), INPUT_MIN_HEIGHT, max);
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
        assert_eq!(output_height(wa(), 40.0, 0.0), 160.0);
        assert_eq!(output_height(wa(), 2000.0, 0.0), 686.0);
        assert_eq!(output_height(wa(), 2000.0, 5000.0), 1000.0);
    }

    #[test]
    fn docked_and_free_placement() {
        let bar = Rect::new(1484.0, 16.0, 420.0, 76.0);
        let top = a("right", "top", 16.0, 16.0);
        let first = place_output(wa(), bar, &top, (420.0, 200.0), &[]);
        // 12 px under the bar's glass.
        assert_eq!(first, Rect::new(1484.0, 104.0, 420.0, 200.0));
        // B-099: along the bar's edge, the gap kept.
        let second = place_output(wa(), bar, &top, (420.0, 200.0), &[first]);
        assert_eq!(second, Rect::new(1460.0, 104.0, 420.0, 200.0));
        let third = place_output(wa(), bar, &top, (420.0, 200.0), &[first, second]);
        assert_eq!(third, Rect::new(1436.0, 104.0, 420.0, 200.0));
        // A window moved away frees the first slot again.
        let moved = Rect::new(200.0, 500.0, 420.0, 200.0);
        assert_eq!(
            place_output(wa(), bar, &top, (420.0, 200.0), &[moved]),
            first
        );
        let low = Rect::new(100.0, 948.0, 420.0, 76.0);
        let up = place_output(
            wa(),
            low,
            &a("left", "bottom", 100.0, 16.0),
            (420.0, 200.0),
            &[],
        );
        assert_eq!(up, Rect::new(100.0, 736.0, 420.0, 200.0));
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
    fn keyboard_nudges_stay_inside() {
        let r = Rect::new(100.0, 100.0, 420.0, 200.0);
        let lim = ((320.0, 160.0), (0.0, 0.0));
        let n = |d| nudge_rect(wa(), r, d, lim.0, lim.1);
        let moved = n((16.0, -16.0, 0.0, 0.0));
        assert_eq!((moved.x, moved.y), (116.0, 84.0));
        assert_eq!((moved.width, moved.height), (420.0, 200.0));
        let grown = n((0.0, 0.0, 16.0, 16.0));
        assert_eq!((grown.width, grown.height), (436.0, 216.0));
        // Never under the minimum, never off the work area.
        assert_eq!(n((0.0, 0.0, -400.0, -400.0)).width, 320.0);
        assert_eq!(n((-500.0, 0.0, 0.0, 0.0)).x, 0.0);
        assert_eq!(n((0.0, 5000.0, 0.0, 0.0)).y, 840.0);
        // A maximum holds.
        let capped = nudge_rect(wa(), r, (0.0, 0.0, 900.0, 0.0), lim.0, (900.0, 0.0));
        assert_eq!(capped.width, 900.0);
    }

    #[test]
    fn output_glass_keeps_the_sheets_edge() {
        let wa = Rect::new(0.0, 0.0, 1920.0, 1040.0);
        let spot = Rect::new(100.0, 700.0, 420.0, 200.0);
        // Growing up: the bottom edge stays, the room goes above it.
        let up = output_glass(wa, spot, 686.0, true);
        assert_eq!(up, Rect::new(100.0, 214.0, 420.0, 686.0));
        // Growing down near the display's end: only what is left.
        let down = output_glass(wa, spot, 686.0, false);
        assert_eq!(down, Rect::new(100.0, 700.0, 420.0, 340.0));
        // Never below the minimum, whatever is left.
        let low = Rect::new(100.0, 1000.0, 420.0, 200.0);
        let kept = output_glass(wa, low, 686.0, false);
        assert_eq!(kept.height, OUTPUT_MIN_HEIGHT);
    }

    #[test]
    fn cascade_sees_taller_windows_above_a_bottom_bar() {
        let bar = Rect::new(16.0, 950.0, 420.0, 76.0);
        let bottom = a("left", "bottom", 16.0, 16.0);
        let size = (420.0, 200.0);
        let first = place_output(wa(), bar, &bottom, size, &[]);
        let tall = output_glass(wa(), first, 686.0, true);
        assert_ne!(tall.y, first.y);
        let next = place_output(wa(), bar, &bottom, size, &[tall]);
        assert_eq!(next.x, first.x + CASCADE);
        assert_eq!(next.y + next.height, first.y + first.height);
    }

    #[test]
    fn output_clamps_to_its_own_display() {
        let left = Rect::new(0.0, 0.0, 1920.0, 1040.0);
        let right = Rect::new(1920.0, 200.0, 1280.0, 680.0);
        // Past the right display's bottom: up into that display, not the other.
        let low = Rect::new(2600.0, 800.0, 420.0, 200.0);
        let raised = Rect::new(2600.0, 680.0, 420.0, 200.0);
        assert_eq!(clamp_to(right, low), raised);
        // Bigger than the area: capped to it.
        let big = Rect::new(2000.0, 300.0, 2000.0, 900.0);
        assert_eq!(clamp_to(right, big), right);
        // Already inside: untouched.
        let inside = Rect::new(100.0, 100.0, 420.0, 200.0);
        assert_eq!(clamp_to(left, inside), inside);
    }

    #[test]
    fn cascade_keeps_the_gap() {
        let bar = Rect::new(1484.0, 16.0, 420.0, 76.0);
        let top = a("right", "top", 16.0, 16.0);
        let size = (420.0, 200.0);
        let mut open: Vec<Rect> = Vec::new();
        for _ in 0..8 {
            let r = place_output(wa(), bar, &top, size, &open);
            assert_eq!(r.y, bar.y + bar.height + OUTPUT_GAP);
            assert!(!open.contains(&r));
            open.push(r);
        }
        assert_eq!(open[7].x, 1484.0 - 7.0 * CASCADE);
        // At the edge the cascade wraps to the far side, still inside.
        let edge = Rect::new(0.0, 16.0, 420.0, 76.0);
        let first = place_output(wa(), edge, &top, size, &[]);
        assert_eq!(first.x, 0.0);
        let wrapped = place_output(wa(), edge, &top, size, &[first]);
        assert_eq!(wrapped.x, 1476.0);
        assert!(wrapped.x + size.0 <= 1920.0);
        assert_eq!(wrapped.y, first.y);
    }

    /// B-097: every layout constant against main.js on `main`, with the
    /// documented departures: the 160 px output minimum (D-02: the 44 px
    /// title bar, one row and the stage margins do not fit in 90), the
    /// 12 px gap (glass to glass inside both stage margins, ARCHITECTURE
    /// 10.3; main.js DOCK_GAP 8 was window to window) and the 600 px panel
    /// width (B-106, the store default).
    #[test]
    fn constants_match_main_js() {
        assert_eq!(PILL, (44.0, 44.0));
        assert_eq!((INPUT_MIN_HEIGHT, INPUT_MAX_HEIGHT), (62.0, 400.0));
        // The cap follows the text size, and never passes the work area.
        let r = input_bounds(wa(), &a("left", "top", 0.0, 0.0), 600.0, 900.0, 2.0);
        assert_eq!(r.height, 800.0);
        let r = input_bounds(wa(), &a("left", "top", 0.0, 0.0), 600.0, 900.0, 1.0);
        assert_eq!(r.height, 400.0);
        let r = input_bounds(wa(), &a("left", "top", 0.0, 0.0), 600.0, 5000.0, 3.0);
        assert_eq!(r.height, 1040.0);
        assert_eq!(DEFAULT_INPUT_HEIGHT, 76.0);
        assert_eq!((OUTPUT_MAX_FRACTION, OUTPUT_MAX_FLOOR), (0.66, 380.0));
        assert_eq!(DEFAULT_OUTPUT_HEIGHT, 120.0);
        assert_eq!(HUD_INPUT_HEIGHT, 140.0);
        assert_eq!((SNAP_MARGIN, SNAP_DISTANCE), (16.0, 110.0));
        assert_eq!(DOCK_DISTANCE, 150.0);
        assert_eq!((DROP_SETTLE_MS, CHAT_HISTORY_LIMIT), (160, 12));
        assert_eq!((T_EXPAND, T_COLLAPSE), (260, 170));
        assert_eq!((T_OUT_IN, T_OUT_OUT, T_OUT_GROW), (300, 150, 200));
        // The departures.
        assert_eq!(OUTPUT_MIN_HEIGHT, 160.0);
        assert_eq!(OUTPUT_GAP, 12.0);
        assert_eq!(PANEL_WIDTH, 600.0);
    }

    #[test]
    fn dodge() {
        assert_eq!(dodge_x(wa(), Rect::new(10.0, 0.0, 400.0, 10.0)), 1504.0);
        assert_eq!(dodge_x(wa(), Rect::new(1500.0, 0.0, 400.0, 10.0)), 16.0);
    }
}
