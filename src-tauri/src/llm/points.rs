//! Strips `[[point ...]]` directives out of an answer stream before it is
//! shown, and collects them (PARITY POINT-01). Port of
//! `createPointExtractor` in lib/openrouter.js.

use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use std::sync::OnceLock;

/// A directive: a grid pair from a model that saw the screen, or a name
/// (`x`/`y` are `None`) from a brain that worked from a reading.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub label: String,
}

/// The longest text a directive may occupy before the buffer stops waiting
/// for its closing brackets.
pub const POINT_HOLD_MAX: usize = 220;

struct Patterns {
    pair: Regex,
    named: FancyRegex,
    any: Regex,
    trailing_gap: Regex,
    leading_gap: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        pair: Regex::new(concat!(
            r"(?i)^\[\[\s*point\s*[:=]?\s*\(?\s*(?:x\s*[=:]\s*)?([0-9]{1,5}(?:\.[0-9]+)?%?)",
            r"\s*(?:,|\s)\s*(?:y\s*[=:]\s*)?([0-9]{1,5}(?:\.[0-9]+)?%?)\s*\)?\s*",
            r"(?:[|,;\x{2013}\x{2014}-]\s*([^\]]{0,140}?))?\s*\]\]$"
        ))
        .unwrap(),
        // The one lookahead in the Electron pattern, kept with backtracking
        // semantics so the same strings match.
        named: FancyRegex::new(concat!(
            r"(?i)^\[\[\s*point\s*[:=]?\s*(?:at\s+)?(?!x\s*[=:])([^\]|]{1,120}?)\s*",
            r"(?:\|\s*[^\]]{0,140}?)?\s*\]\]$"
        ))
        .unwrap(),
        any: Regex::new(r"(?i)^\[\[\s*point\b").unwrap(),
        trailing_gap: Regex::new(r"[^\S\r\n]$").unwrap(),
        leading_gap: Regex::new(r"^[^\S\r\n]+").unwrap(),
    })
}

fn parse_float(raw: &str) -> f64 {
    raw.trim_end_matches('%').parse::<f64>().unwrap_or(f64::NAN)
}

/// Feed it deltas with `push`, finish with `flush`; each returns the
/// display-safe pieces in the order the Electron extractor called `onText`.
#[derive(Default)]
pub struct PointExtractor {
    buf: String,
    close_gap: bool,
    pub points: Vec<Point>,
    /// Everything shown so far.
    pub text: String,
}

impl PointExtractor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, chunk: &str) -> Vec<String> {
        self.buf.push_str(chunk);
        let mut out = Vec::new();
        self.drain(false, &mut out);
        out
    }

    pub fn flush(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        self.drain(true, &mut out);
        if !self.buf.is_empty() {
            let rest = std::mem::take(&mut self.buf);
            self.emit(&rest, &mut out);
        }
        out
    }

    fn emit(&mut self, text: &str, out: &mut Vec<String>) {
        if text.is_empty() {
            return;
        }
        let mut t = text.to_string();
        if self.close_gap {
            let p = patterns();
            if p.trailing_gap.is_match(&self.text) {
                t = p.leading_gap.replace(&t, "").into_owned();
            }
            if t.is_empty() {
                // Still nothing to show; keep waiting to close the gap.
                return;
            }
            self.close_gap = false;
        }
        self.text.push_str(&t);
        out.push(t);
    }

    fn drain(&mut self, last: bool, out: &mut Vec<String>) {
        loop {
            let Some(open) = self.buf.find("[[") else {
                // Keep a trailing single '['; the second may be in the next
                // delta.
                if !last && self.buf.ends_with('[') {
                    let head = self.buf[..self.buf.len() - 1].to_string();
                    self.emit(&head, out);
                    self.buf = "[".to_string();
                } else {
                    let all = std::mem::take(&mut self.buf);
                    self.emit(&all, out);
                }
                return;
            };
            let head = self.buf[..open].to_string();
            self.emit(&head, out);
            self.buf.drain(..open);
            let close = self.buf.find("]]");
            // A second '[[' before any ']]' means the first was ordinary text.
            let next_open = self.buf[2..].find("[[").map(|i| i + 2);
            if let Some(n) = next_open {
                let before_close = match close {
                    Some(c) => n < c,
                    None => true,
                };
                if before_close {
                    self.emit("[[", out);
                    self.buf.drain(..2);
                    continue;
                }
            }
            let Some(close) = close else {
                if !last && self.buf.encode_utf16().count() <= POINT_HOLD_MAX {
                    return;
                }
                // Not a directive after all; release the brackets.
                self.emit("[[", out);
                self.buf.drain(..2);
                continue;
            };
            let candidate = self.buf[..close + 2].to_string();
            self.take_directive(&candidate, out);
            self.buf.drain(..close + 2);
        }
    }

    fn take_directive(&mut self, candidate: &str, out: &mut Vec<String>) {
        let p = patterns();
        if let Some(m) = p.pair.captures(candidate) {
            let label = m.get(3).map_or("", |g| g.as_str()).trim().to_string();
            self.points.push(Point {
                x: Some(parse_float(&m[1])),
                y: Some(parse_float(&m[2])),
                label,
            });
            self.close_gap = true;
        } else if let Ok(Some(m)) = p.named.captures(candidate) {
            let label = m.get(1).map_or("", |g| g.as_str()).trim().to_string();
            self.points.push(Point {
                x: None,
                y: None,
                label,
            });
            self.close_gap = true;
        } else if p.any.is_match(candidate) {
            // Meant as a directive, too malformed to read: dropped, because
            // showing it loses the pointer anyway.
            self.close_gap = true;
        } else {
            self.emit(candidate, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(chunks: &[&str]) -> (String, Vec<Point>) {
        let mut x = PointExtractor::new();
        let mut shown = String::new();
        for c in chunks {
            shown.extend(x.push(c));
        }
        shown.extend(x.flush());
        assert_eq!(shown, x.text);
        (shown, x.points)
    }

    #[test]
    fn coordinate_directives_are_taken_out_across_deltas() {
        let (text, points) = run(&["Press [[poi", "nt x=500 y=", "250 | Save]] to save"]);
        assert_eq!(text, "Press to save");
        assert_eq!(points[0].x, Some(500.0));
        assert_eq!(points[0].y, Some(250.0));
        assert_eq!(points[0].label, "Save");
    }

    #[test]
    fn loose_shapes_and_names() {
        let (_, p) = run(&["[[point 120, 340 | Save]]"]);
        assert_eq!((p[0].x, p[0].y), (Some(120.0), Some(340.0)));
        let (_, p) = run(&["[[point: (x: 1, y: 2) - OK]]"]);
        assert_eq!(p[0].label, "OK");
        let (text, p) = run(&["Click\n[[point Save button]]\ndone"]);
        assert_eq!(p[0].x, None);
        assert_eq!(p[0].label, "Save button");
        assert_eq!(text, "Click\n\ndone");
        let (_, p) = run(&["[[point at the Run menu | why]]"]);
        assert_eq!(p[0].label, "the Run menu");
    }

    #[test]
    fn other_brackets_are_kept() {
        let (text, p) = run(&["a [[link]] b [x] c [", "[d]] e"]);
        assert_eq!(text, "a [[link]] b [x] c [[d]] e");
        assert!(p.is_empty());
        let (text, _) = run(&["[[ [[point 1 2]] ok"]);
        assert_eq!(text, "[[ ok");
    }

    #[test]
    fn malformed_directive_is_swallowed() {
        let (text, p) = run(&["x [[point]] y"]);
        assert_eq!(text, "x y");
        assert!(p.is_empty());
        // A blank name still reads as a named directive, as it did in JS.
        let (_, p) = run(&["[[point ]]"]);
        assert_eq!(p[0].label, "");
    }

    #[test]
    fn unclosed_directive_is_released_after_the_hold() {
        let long = format!("[[point {}", "a".repeat(230));
        let (text, p) = run(&[&long]);
        assert_eq!(text, long);
        assert!(p.is_empty());
    }
}
