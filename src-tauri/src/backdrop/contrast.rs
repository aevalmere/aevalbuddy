//! The contrast gate's measurement (lib/contrast.js, PARITY GLASS-08): text
//! measured against the pixels it is actually drawn on. The page half (the
//! probe that lists visible text runs, and the stylesheet that hides every
//! glyph) is the same JavaScript, generated from lib/contrast.js; the
//! photographs come from agent (e)'s CapturePreview. This file does the
//! arithmetic: WCAG 2.2 luminance and ratio, the alpha blend of the text
//! colour over each background pixel, the 5th-percentile worst background,
//! 4.5:1 for body text and 3:1 for large text.

use serde::{Deserialize, Serialize};

/// Runs in the page; resolves to `[{x, y, w, h, color, size, weight, text, sel}]`.
pub const PROBE: &str = include_str!("contrast-probe.js");
/// Inserted while the background photograph is taken.
pub const HIDE_TEXT: &str = include_str!("contrast-hide.css");

#[derive(Clone, Debug, Deserialize)]
pub struct Run {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub color: String,
    pub size: f64,
    pub weight: f64,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub sel: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Fail {
    pub sel: String,
    pub text: String,
    pub ratio: f64,
    pub need: f64,
    pub size: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Measure {
    pub runs: usize,
    /// `f64::INFINITY` when nothing was measured.
    pub min: f64,
    pub fails: Vec<Fail>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

/// `rgb(..)` / `rgba(..)` in any of the separators Chromium prints.
pub fn parse_color(s: &str) -> Option<Rgba> {
    let start = s.find("rgb")?;
    let open = s[start..].find('(')? + start;
    let close = s[open..].find(')')? + open;
    let parts: Vec<f64> = s[open + 1..close]
        .split(|c: char| c == ' ' || c == ',' || c == '/')
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<f64>().unwrap_or(f64::NAN))
        .collect();
    if parts.len() < 3 {
        return None;
    }
    Some(Rgba {
        r: parts[0],
        g: parts[1],
        b: parts[2],
        a: if parts.len() > 3 { parts[3] } else { 1.0 },
    })
}

fn lin(c: f64) -> f64 {
    let v = c / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn lum(r: f64, g: f64, b: f64) -> f64 {
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

pub fn ratio(a: f64, b: f64) -> f64 {
    let hi = a.max(b);
    let lo = a.min(b);
    (hi + 0.05) / (lo + 0.05)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Measures `runs` against a BGRA background photograph of `width` x
/// `height` pixels; `k` is device pixels per CSS pixel.
pub fn measure(runs: &[Run], bgra: &[u8], width: usize, height: usize, k: f64) -> Measure {
    if runs.is_empty() {
        return Measure {
            runs: 0,
            min: f64::INFINITY,
            fails: Vec::new(),
        };
    }
    let mut fails = Vec::new();
    let mut min = f64::INFINITY;
    for t in runs {
        let Some(fg) = parse_color(&t.color) else {
            continue;
        };
        if fg.a < 0.05 {
            continue;
        }
        let x0 = (t.x * k).floor().max(0.0) as usize;
        let y0 = (t.y * k).floor().max(0.0) as usize;
        let x1 = ((t.x + t.w) * k).ceil().min(width as f64).max(0.0) as usize;
        let y1 = ((t.y + t.h) * k).ceil().min(height as f64).max(0.0) as usize;
        if x1 <= x0 || y1 <= y0 {
            continue;
        }
        let area = ((x1 - x0) * (y1 - y0)) as f64;
        let step = ((area / 900.0).sqrt().round() as usize).max(1);
        let mut ratios = Vec::new();
        let mut y = y0;
        while y < y1 {
            let mut x = x0;
            while x < x1 {
                let i = (y * width + x) * 4;
                if i + 2 < bgra.len() {
                    let (br, bg, bb) = (bgra[i + 2] as f64, bgra[i + 1] as f64, bgra[i] as f64);
                    let r = fg.r * fg.a + br * (1.0 - fg.a);
                    let g = fg.g * fg.a + bg * (1.0 - fg.a);
                    let b = fg.b * fg.a + bb * (1.0 - fg.a);
                    ratios.push(ratio(lum(r, g, b), lum(br, bg, bb)));
                }
                x += step;
            }
            y += step;
        }
        if ratios.is_empty() {
            continue;
        }
        ratios.sort_by(|a, b| a.total_cmp(b));
        let worst = ratios[(ratios.len() as f64 * 0.05).floor() as usize];
        let large = t.size >= 24.0 || (t.size >= 18.66 && t.weight >= 700.0);
        let need = if large { 3.0 } else { 4.5 };
        if worst < min {
            min = worst;
        }
        if worst < need {
            fails.push(Fail {
                sel: t.sel.clone(),
                text: t.text.clone(),
                ratio: round2(worst),
                need,
                size: t.size,
            });
        }
    }
    Measure {
        runs: runs.len(),
        min: if min.is_finite() { round2(min) } else { min },
        fails,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(color: &str, size: f64) -> Run {
        Run {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            color: color.into(),
            size,
            weight: 400.0,
            text: "t".into(),
            sel: "p".into(),
        }
    }

    fn solid(w: usize, h: usize, v: u8) -> Vec<u8> {
        [v, v, v, 255].repeat(w * h)
    }

    #[test]
    fn colours_parse() {
        assert_eq!(parse_color("rgb(1, 2, 3)"), Some(Rgba { r: 1.0, g: 2.0, b: 3.0, a: 1.0 }));
        assert_eq!(parse_color("rgba(1, 2, 3, 0.5)").unwrap().a, 0.5);
        assert_eq!(parse_color("rgb(1 2 3 / 0.25)").unwrap().a, 0.25);
        assert_eq!(parse_color("transparent"), None);
    }

    #[test]
    fn wcag_numbers() {
        assert!((ratio(lum(0.0, 0.0, 0.0), lum(255.0, 255.0, 255.0)) - 21.0).abs() < 1e-9);
        assert!((ratio(lum(119.0, 119.0, 119.0), lum(255.0, 255.0, 255.0)) - 4.48).abs() < 0.01);
    }

    #[test]
    fn black_on_white_passes_grey_fails() {
        let bg = solid(20, 20, 255);
        let m = measure(&[run("rgb(0, 0, 0)", 13.0)], &bg, 20, 20, 1.0);
        assert_eq!(m.fails.len(), 0);
        assert_eq!(m.min, 21.0);
        let m = measure(&[run("rgb(170, 170, 170)", 13.0)], &bg, 20, 20, 1.0);
        assert_eq!(m.fails.len(), 1);
        assert_eq!(m.fails[0].need, 4.5);
    }

    #[test]
    fn large_text_needs_three() {
        let bg = solid(20, 20, 255);
        // 3.5:1 grey passes at 24 px, fails at 13 px.
        let m = measure(&[run("rgb(130, 130, 130)", 24.0)], &bg, 20, 20, 1.0);
        assert!(m.fails.is_empty(), "{m:?}");
        let m = measure(&[run("rgb(130, 130, 130)", 13.0)], &bg, 20, 20, 1.0);
        assert_eq!(m.fails.len(), 1);
    }

    #[test]
    fn alpha_text_is_blended_and_invisible_text_skipped() {
        let bg = solid(20, 20, 255);
        let m = measure(&[run("rgba(0, 0, 0, 0.3)", 13.0)], &bg, 20, 20, 1.0);
        assert_eq!(m.fails.len(), 1);
        let m = measure(&[run("rgba(0, 0, 0, 0.01)", 13.0)], &bg, 20, 20, 1.0);
        assert_eq!(m.min, f64::INFINITY);
    }

    #[test]
    fn probe_is_the_one_from_lib_contrast() {
        let js = include_str!("../../../lib/contrast.js");
        assert!(PROBE.contains("const clipTo = (el, r) =>"));
        assert!(js.contains("const clipTo = (el, r) =>"));
        assert!(PROBE.contains(r"/^inset\(\s*50%/"));
        assert!(HIDE_TEXT.starts_with("*, *::before, *::after { color: transparent !important;"));
    }
}
