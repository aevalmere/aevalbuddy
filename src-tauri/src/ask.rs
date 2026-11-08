//! The Ask flow's pure parts (PARITY ASK-*): the constants main.js used,
//! the conversation id, the visual-question test, and the pointer
//! fraction. The submit flow itself (capture, reader, failover, streaming,
//! pointer, history) lands with the IPC wiring, behind `llm::client`.

// Nothing calls into this module until ipc.rs routes (b)'s channels.
#![allow(dead_code)]

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::Rng;
use regex::Regex;

/// What an empty question asks.
pub const DEFAULT_PROMPT: &str = "What's on my screen and what should I do?";
/// Earlier turns sent with each question.
pub const CHAT_HISTORY_LIMIT: usize = 12;
/// The skill setting that lets routing pick.
pub const AUTO_SKILL: &str = "auto";
/// Exact text shorter than this cannot stand in for the screen.
pub const TEXT_ONLY_CHARS: usize = 800;

/// The source of `VISUAL` in renderer/intent.js, kept identical (ASK-08).
#[rustfmt::skip]
pub const VISUAL_SOURCE: &str = r"\b(which (button|icon|one|tab|option)|where (is|are|do)|colou?r|icon|button|look(s)? like|see|picture|image|photo|chart|graph|layout|design|logo|font|screenshot|highlighted|red|green|blue)\b";

/// True when the question is about how the screen looks, so it needs the
/// picture rather than the text.
pub fn is_visual(question: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        let ascii = VISUAL_SOURCE.replace(r"\b", r"(?-u:\b)");
        Regex::new(&format!("(?i){ascii}")).expect("visual pattern")
    });
    re.is_match(question)
}

fn base36(mut n: u128) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".into();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// `'c' + Date.now().toString(36) + six random base-36 characters`.
pub fn new_chat_id() -> String {
    let since = SystemTime::now().duration_since(UNIX_EPOCH);
    let now = since.map_or(0, |d| d.as_millis());
    let mut rng = rand::thread_rng();
    let tail: String = (0..6).map(|_| base36(rng.gen_range(0..36))).collect();
    format!("c{}{tail}", base36(now))
}

/// A pointer directive's place as a fraction of the shot: the 0-1000 grid,
/// or pixels of the shot when a model gave those instead.
pub fn point_fraction(x: f64, y: f64, width: f64, height: f64) -> Option<(f64, f64)> {
    if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
        return None;
    }
    if x > 1000.0 || y > 1000.0 {
        if x > width || y > height {
            return None;
        }
        return Some((x / width, y / height));
    }
    Some((x / 1000.0, y / 1000.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_regex_matches_the_renderer() {
        let js = include_str!("../../renderer/intent.js");
        let want = format!("const VISUAL = /{VISUAL_SOURCE}/i;");
        assert!(js.contains(&want), "renderer/intent.js VISUAL changed");
        assert!(is_visual("Which button saves?"));
        assert!(is_visual("what colour is it"));
        assert!(!is_visual("summarise this email"));
        assert!(!is_visual("redo"));
    }

    #[test]
    fn chat_ids_look_like_electron_ones() {
        let id = new_chat_id();
        assert!(id.starts_with('c'));
        assert!(id.len() >= 13);
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric()));
        assert_eq!(base36(35), "z");
        assert_eq!(base36(36), "10");
    }

    #[test]
    fn fractions_take_grid_or_pixels() {
        let grid = point_fraction(500.0, 250.0, 1600.0, 900.0);
        assert_eq!(grid, Some((0.5, 0.25)));
        let px = point_fraction(1200.0, 450.0, 1600.0, 900.0);
        assert_eq!(px, Some((0.75, 0.5)));
        assert_eq!(point_fraction(2000.0, 10.0, 1600.0, 900.0), None);
        assert_eq!(point_fraction(-1.0, 10.0, 1600.0, 900.0), None);
    }
}
