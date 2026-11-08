//! The transparent margin around the bar and the output windows, so the
//! glass can cast its shadow (lib/stage.js, PARITY WIN-11). The rest of the
//! window manager speaks the glass rectangle; only the OS window is bigger.

use crate::state::Rect;

pub const MARGIN: f64 = 20.0;

pub fn inflate(r: Rect, m: f64) -> Rect {
    Rect::new(r.x - m, r.y - m, r.width + 2.0 * m, r.height + 2.0 * m)
}

pub fn deflate(r: Rect, m: f64) -> Rect {
    Rect::new(
        r.x + m,
        r.y + m,
        (r.width - 2.0 * m).max(1.0),
        (r.height - 2.0 * m).max(1.0),
    )
}

/// A glass size limit as a window limit; 0 (no limit) stays 0.
pub fn limit(v: f64, m: f64) -> f64 {
    if v > 0.0 {
        v + 2.0 * m
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn margin_round_trip() {
        let g = Rect::new(100.0, 50.0, 420.0, 76.0);
        assert_eq!(inflate(g, MARGIN), Rect::new(80.0, 30.0, 460.0, 116.0));
        assert_eq!(deflate(inflate(g, MARGIN), MARGIN), g);
        assert_eq!(deflate(Rect::new(0.0, 0.0, 10.0, 10.0), MARGIN).width, 1.0);
        assert_eq!(limit(0.0, MARGIN), 0.0);
        assert_eq!(limit(320.0, MARGIN), 360.0);
    }
}
