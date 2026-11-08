//! The reader's ELEMENTS table and the name matcher behind `[[point Save]]`
//! (PARITY POINT-02, POINT-03). Port of `parseElements`, `toGrid`,
//! `cleanLabel`, `normLabel` and `matchElement` in lib/openrouter.js.

use regex::Regex;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub label: String,
    pub x: f64,
    pub y: f64,
}

/// The size of the image a model was given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub width: f64,
    pub height: f64,
}

struct Patterns {
    bullet: Regex,
    coord: Regex,
    header: Regex,
    field: Regex,
    tail_sep: Regex,
    head_mark: Regex,
    tail_mark: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        bullet: Regex::new(r"^\s*(?:[-*\x{2022}]|[0-9]{1,2}[.)])\s+").unwrap(),
        coord: Regex::new(concat!(
            r"(?i)(?:^|[\s|(:,])\s*\(?\s*(?:x\s*[=:]\s*)?([0-9]{1,5}(?:\.[0-9]+)?%?)",
            r"\s*(?:,|\s)\s*(?:y\s*[=:]\s*)?([0-9]{1,5}(?:\.[0-9]+)?%?)\s*\)?\s*$"
        ))
        .unwrap(),
        header: Regex::new(r"(?i)^\s*ELEMENTS\s*:").unwrap(),
        field: Regex::new(r"^[A-Z][A-Z ]{2,}\s*:").unwrap(),
        tail_sep: Regex::new(r"[|(:,\s-]+$").unwrap(),
        head_mark: Regex::new(r#"^[*_`"'\s]+"#).unwrap(),
        tail_mark: Regex::new(r#"[*_`"'\s]+$"#).unwrap(),
    })
}

/// JS `parseFloat` for the token shapes the coordinate regex admits.
fn parse_num(raw: &str) -> f64 {
    raw.trim_end_matches('%').parse::<f64>().unwrap_or(f64::NAN)
}

/// Two raw coordinate tokens onto the 0-1000 grid. Percentages and fractions
/// of one are recovered; values over 1000 are read as view pixels when the
/// view is known and they fit inside it, and dropped otherwise.
pub fn to_grid(raw_x: &str, raw_y: &str, view: Option<View>) -> Option<(f64, f64)> {
    let mut x = parse_num(raw_x);
    let mut y = parse_num(raw_y);
    if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
        return None;
    }
    if raw_x.ends_with('%') && raw_y.ends_with('%') {
        x *= 10.0;
        y *= 10.0;
    } else if x <= 1.0 && y <= 1.0 && (raw_x.contains('.') || raw_y.contains('.')) {
        x *= 1000.0;
        y *= 1000.0;
    } else if x > 1000.0 || y > 1000.0 {
        let v = view.filter(|v| v.width > 0.0 && v.height > 0.0)?;
        if x > v.width || y > v.height {
            return None;
        }
        x = x / v.width * 1000.0;
        y = y / v.height * 1000.0;
    }
    if x > 1000.0 || y > 1000.0 {
        return None;
    }
    Some((js_round(x), js_round(y)))
}

fn js_round(n: f64) -> f64 {
    (n + 0.5).floor()
}

/// Whatever is left in front of the coordinates: the separator that split
/// them off, and any markdown around the name.
pub fn clean_label(text: &str) -> String {
    let p = patterns();
    let t = p.tail_sep.replace(text, "");
    let t = p.head_mark.replace(&t, "");
    let t = p.tail_mark.replace(&t, "");
    t.trim().to_string()
}

/// The reader's ELEMENTS table as label and grid point, at most 8 rows. Any
/// other FIELD: line ends the block.
pub fn parse_elements(report: Option<&str>, view: Option<View>) -> Vec<Element> {
    let p = patterns();
    let mut out = Vec::new();
    let mut inside = false;
    for line in report.unwrap_or("").split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if p.header.is_match(line) {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if p.field.is_match(line) {
            break;
        }
        let body = p.bullet.replace(line, "");
        let Some(m) = p.coord.captures(&body) else {
            continue;
        };
        let Some((x, y)) = to_grid(&m[1], &m[2], view) else {
            continue;
        };
        let start = m.get(0).map_or(0, |g| g.start());
        let label = clean_label(&body[..start]);
        if label.is_empty() {
            continue;
        }
        out.push(Element { label, x, y });
        if out.len() >= 8 {
            break;
        }
    }
    out
}

/// Lowercase, everything but ASCII letters, digits and spaces to a space,
/// runs of whitespace collapsed.
pub fn norm_label(text: &str) -> String {
    let lower = text.to_lowercase();
    let mapped: String = lower
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                ' '
            }
        })
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

const MATCH_MIN: f64 = 0.5;
const MATCH_MARGIN: f64 = 0.08;

/// The element a named directive meant. Every row is scored and the best
/// wins only if it clears 0.5 and leads the runner-up by 0.08; an exact name
/// wins outright.
pub fn match_element<'a>(label: &str, elements: &'a [Element]) -> Option<&'a Element> {
    let want = norm_label(label);
    if want.is_empty() || elements.is_empty() {
        return None;
    }
    let want_words: Vec<&str> = want.split(' ').filter(|w| !w.is_empty()).collect();
    let mut best: Option<&Element> = None;
    let mut best_score = 0.0;
    let mut runner_up = 0.0;
    for el in elements {
        let key = norm_label(&el.label);
        if key.is_empty() {
            continue;
        }
        if key == want {
            return Some(el);
        }
        let mut score: f64 = 0.0;
        if key.contains(&want) || want.contains(&key) {
            let (a, b) = (key.encode_utf16().count(), want.encode_utf16().count());
            let shorter = a.min(b) as f64;
            let longer = a.max(b) as f64;
            score = 0.6 + 0.35 * (shorter / longer);
        }
        let mut words: Vec<&str> = key.split(' ').filter(|w| !w.is_empty()).collect();
        words.sort_unstable();
        words.dedup();
        let hits = want_words.iter().filter(|w| words.contains(*w)).count();
        if hits > 0 {
            let overlap = (2 * hits) as f64 / (want_words.len() + words.len()) as f64;
            score = score.max(overlap.min(0.95));
        }
        if score > best_score {
            runner_up = best_score;
            best_score = score;
            best = Some(el);
        } else if score > runner_up {
            runner_up = score;
        }
    }
    if best_score < MATCH_MIN || best_score - runner_up < MATCH_MARGIN {
        return None;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn el(label: &str) -> Element {
        Element {
            label: label.into(),
            x: 1.0,
            y: 2.0,
        }
    }

    #[test]
    fn rows_in_every_shape() {
        let report = "SCREEN: x\nELEMENTS:\n- Save | 120,340\n2) **Open** (x=500, y=20)\n\
* Close: 50% 50%\n- Tiny | 0.5, 0.25\n- Wide | 1600,450\n- Junk without numbers\nTEXT: done\n\
- After | 1,1";
        let rows = parse_elements(Some(report), None);
        let got: Vec<(&str, f64, f64)> =
            rows.iter().map(|e| (e.label.as_str(), e.x, e.y)).collect();
        assert_eq!(
            got,
            [
                ("Save", 120.0, 340.0),
                ("Open", 500.0, 20.0),
                ("Close", 500.0, 500.0),
                ("Tiny", 500.0, 250.0),
            ]
        );
        let view = Some(View {
            width: 1600.0,
            height: 900.0,
        });
        let rows = parse_elements(Some(report), view);
        assert_eq!(rows[4].label, "Wide");
        assert_eq!((rows[4].x, rows[4].y), (1000.0, 500.0));
    }

    #[test]
    fn at_most_eight_rows() {
        let mut r = String::from("ELEMENTS:\n");
        for i in 0..12 {
            r.push_str(&format!("- B{i} | {i}0,10\n"));
        }
        assert_eq!(parse_elements(Some(&r), None).len(), 8);
    }

    #[test]
    fn names_resolve_only_when_clear() {
        let rows = [el("Terminal tab"), el("Problems tab"), el("Save")];
        assert_eq!(match_element("Save", &rows).unwrap().label, "Save");
        assert!(match_element("tab", &rows).is_none());
        let hit = match_element("the Terminal tab", &rows).unwrap();
        assert_eq!(hit.label, "Terminal tab");
        assert!(match_element("Print", &rows).is_none());
        let rows = [el("Save"), el("Save As Template")];
        assert_eq!(match_element("save", &rows).unwrap().label, "Save");
    }

    #[test]
    fn labels_are_normalised() {
        assert_eq!(norm_label("  **Save**  As!"), "save as");
        assert_eq!(clean_label("**Open** ("), "Open");
    }
}
