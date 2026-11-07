//! Local literal and fuzzy scoring for `find` (PARITY DESK-09): a port of
//! index.js's norm / dice / scoreText / scoreNode with the same constants.
//! No model call; the agent layer reranks.

use std::collections::HashMap;

use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

/// Words in a query that name a role rather than the element.
pub fn role_words(word: &str) -> Option<&'static [&'static str]> {
    Some(match word {
        "button" | "btn" => &["button", "splitbutton"],
        "link" | "hyperlink" => &["hyperlink"],
        "field" => &["edit", "combobox", "document"],
        "input" => &["edit", "combobox"],
        "textbox" => &["edit"],
        "box" => &["edit", "combobox", "checkbox"],
        "search" => &["edit", "combobox"],
        "tab" => &["tabitem"],
        "checkbox" | "check" => &["checkbox"],
        "toggle" => &["checkbox", "button"],
        "radio" => &["radiobutton"],
        "menu" => &["menuitem", "menu", "menubar"],
        "item" => &["listitem", "treeitem", "menuitem", "dataitem"],
        "dropdown" | "combo" => &["combobox"],
        "select" => &["combobox", "list"],
        "slider" => &["slider"],
        "list" => &["list", "listitem"],
        "row" => &["dataitem", "listitem"],
        "heading" => &["header", "text"],
        "title" => &["titlebar", "text"],
        "image" => &["image"],
        "icon" => &["image", "button"],
        _ => return None,
    })
}

/// Lower case, accents stripped (NFKD minus U+0300..U+036F), every run of
/// non letters/digits one space, trimmed.
pub fn norm(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut gap = false;
    for c in lower.nfkd() {
        if ('\u{0300}'..='\u{036f}').contains(&c) {
            continue;
        }
        if c.is_alphabetic() || c.is_numeric() {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    out
}

fn bigrams(s: &str) -> HashMap<(char, char), usize> {
    let t: Vec<char> = format!(" {s} ").chars().collect();
    let mut out = HashMap::new();
    for w in t.windows(2) {
        *out.entry((w[0], w[1])).or_insert(0) += 1;
    }
    out
}

/// Sørensen-Dice over character bigrams with padding, 0..1.
pub fn dice(a: &str, b: &str) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let ga = bigrams(a);
    let gb = bigrams(b);
    let mut inter = 0usize;
    let mut total: usize = ga.values().sum();
    for (g, v) in &gb {
        total += v;
        if let Some(av) = ga.get(g) {
            inter += (*v).min(*av);
        }
    }
    if total == 0 {
        0.0
    } else {
        2.0 * inter as f64 / total as f64
    }
}

fn word_match(s: &str, q: &str) -> bool {
    // (^| )q( |$)
    let mut from = 0;
    while let Some(i) = s[from..].find(q) {
        let start = from + i;
        let end = start + q.len();
        let before = start == 0 || s[..start].ends_with(' ');
        let after = end == s.len() || s[end..].starts_with(' ');
        if before && after {
            return true;
        }
        from = start + s[start..].chars().next().map(char::len_utf8).unwrap_or(1);
    }
    false
}

/// 0..100: how well one field's text matches the (role-free) query.
pub fn score_text(q: &str, q_tokens: &[String], text: &str) -> f64 {
    let s = norm(text);
    if s.is_empty() || q.is_empty() {
        return 0.0;
    }
    if s == q {
        return 100.0;
    }
    if s.starts_with(q) {
        return 88.0;
    }
    if word_match(&s, q) {
        return 82.0;
    }
    if s.contains(q) {
        return 72.0;
    }
    let words: Vec<&str> = s.split(' ').collect();
    if q_tokens.len() > 1 && q_tokens.iter().all(|t| words.iter().any(|w| w.starts_with(t.as_str()))) {
        return 64.0;
    }
    let hit = q_tokens
        .iter()
        .filter(|t| {
            words
                .iter()
                .any(|w| w.starts_with(t.as_str()) || (t.chars().count() > 3 && dice(t, w) > 0.7))
        })
        .count();
    let partial = if q_tokens.is_empty() {
        0.0
    } else {
        hit as f64 / q_tokens.len() as f64 * 55.0
    };
    // Short labels against a typo: "sumbit" still finds "Submit".
    let fuzzy = if s.chars().count() <= 60 {
        dice(q, &s) * 60.0
    } else {
        0.0
    };
    partial.max(fuzzy)
}

fn field(n: &Value, k: &str) -> String {
    match n.get(k) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(v) => v.to_string(),
    }
}

/// 0..100 for one node against a query, role words weighting the role.
pub fn score_node(node: &Value, query: &str) -> i64 {
    let all: Vec<String> = norm(query).split(' ').filter(|t| !t.is_empty()).map(String::from).collect();
    let mut roles: Vec<&str> = Vec::new();
    let mut rest: Vec<String> = Vec::new();
    for t in &all {
        match role_words(t) {
            Some(r) => roles.extend_from_slice(r),
            None => rest.push(t.clone()),
        }
    }
    // A query made only of role words ("button") is about the role itself.
    let tokens = if rest.is_empty() { all.clone() } else { rest.clone() };
    let q = tokens.join(" ");
    let mut score = score_text(&q, &tokens, &field(node, "name"))
        .max(score_text(&q, &tokens, &field(node, "value")) * 0.85)
        .max(score_text(&q, &tokens, &field(node, "automationId")) * 0.7)
        .max(score_text(&q, &tokens, &field(node, "helpText")) * 0.6);
    let role = field(node, "role");
    let has_role = roles.contains(&role.as_str());
    if rest.is_empty() && !roles.is_empty() {
        score = if has_role { 60.0 } else { 0.0 };
    } else if !roles.is_empty() {
        score += if has_role { 15.0 } else { -12.0 };
    }
    if score <= 0.0 {
        return 0;
    }
    if node.get("interactive").and_then(Value::as_bool).unwrap_or(false) {
        score += 3.0;
    }
    let offscreen = node
        .get("states")
        .and_then(Value::as_array)
        .map(|a| a.iter().any(|s| s.as_str() == Some("offscreen")))
        .unwrap_or(false);
    if !offscreen {
        score += 2.0;
    }
    // JS Math.round rounds halves up.
    (score.min(100.0) + 0.5).floor() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn norm_matches_the_js() {
        assert_eq!(norm("  Café—Crème!! 42 "), "cafe creme 42");
        assert_eq!(norm("Save_As..."), "save as");
        assert_eq!(norm(""), "");
    }

    #[test]
    fn dice_values() {
        assert_eq!(dice("abc", "abc"), 1.0);
        assert_eq!(dice("", "abc"), 0.0);
        assert!(dice("sumbit", "submit") > 0.4);
    }

    #[test]
    fn exact_prefix_word_contains() {
        let t = vec!["send".to_string()];
        assert_eq!(score_text("send", &t, "Send"), 100.0);
        assert_eq!(score_text("send", &t, "Send now"), 88.0);
        assert_eq!(score_text("send", &t, "Please send it"), 82.0);
        assert_eq!(score_text("send", &t, "resend"), 72.0);
    }

    #[test]
    fn nodes() {
        let b = json!({"role":"button","name":"Submit","interactive":true,"states":[]});
        assert_eq!(score_node(&b, "submit button"), 100);
        assert_eq!(score_node(&b, "button"), 65);
        let e = json!({"role":"edit","name":"Submit","states":["offscreen"]});
        assert_eq!(score_node(&e, "submit button"), 88);
        assert_eq!(score_node(&e, "button"), 0);
        // Typo still clears the 45 floor.
        assert!(score_node(&b, "sumbit") >= 45);
        let doc = json!({"role":"document","name":"Text editor","interactive":true});
        assert!(score_node(&doc, "text editor") >= 100);
    }
}
