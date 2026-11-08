//! What the trail and the confirm card say about a step (PARITY AGENT-10,
//! AGENT-14). Port of `describe`, `redact`, `friendly` and `seconds` in
//! lib/agent.js, verbatim.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use super::gate::{chord_label, js_string, quote, url_ok};

/// `seconds(v)`: a wait, 0 to 10, 1 when unreadable.
pub fn seconds(v: &Value) -> f64 {
    let n = match v {
        Value::Null => f64::NAN,
        other => crate::llm::catalogue::js_number(other),
    };
    if n.is_finite() {
        n.clamp(0.0, 10.0)
    } else {
        1.0
    }
}

fn js_num_text(n: f64) -> String {
    if n.fract() == 0.0 {
        (n as i64).to_string()
    } else {
        n.to_string()
    }
}

fn verb_of(name: &str) -> Option<&'static str> {
    Some(match name {
        "left_click" => "Click",
        "double_click" => "Double-click",
        "right_click" => "Right-click",
        "triple_click" => "Triple-click",
        "hover" => "Point at",
        "scroll" => "Scroll",
        "scroll_to" => "Scroll to",
        "form_input" => "Set",
        "left_click_drag" => "Drag",
        _ => return None,
    })
}

/// What `inspect()` learned about a step's target, for the wording.
#[derive(Clone, Debug, Default)]
pub struct Seen<'a> {
    pub node: Option<&'a Value>,
    pub app: &'a str,
    pub end_node: Option<&'a Value>,
    pub end_app: &'a str,
}

fn line_breaks() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[\r\n\t]+").expect("breaks"))
}

fn in_app(app: &str) -> String {
    if app.is_empty() {
        String::new()
    } else {
        format!(" in {app}")
    }
}

/// One step in words: "Click "Save" in Notepad".
pub fn describe(name: &str, a: &Value, info: &Seen<'_>) -> String {
    let node_name = info.node.map(|n| js_string(&n["name"])).unwrap_or_default();
    let label = match info.node {
        Some(_) if !node_name.is_empty() => quote(&node_name, 40),
        Some(n) => format!("the {}", js_string(&n["role"])),
        None => String::new(),
    };
    let app = in_app(info.app);
    let window = if info.app.is_empty() {
        "the window"
    } else {
        info.app
    };
    match name {
        "type" => {
            let t = js_string(&a["text"]);
            let enter = if t.contains(['\r', '\n']) {
                " and press Enter"
            } else {
                ""
            };
            let flat = line_breaks().replace_all(&t, " ");
            format!("Type {}{enter}{app}", quote(&flat, 40))
        }
        "key" => {
            let keys = match js_string(&a["text"]) {
                k if k.is_empty() => js_string(&a["keys"]),
                k => k,
            };
            let label = chord_label(&keys).unwrap_or(keys);
            format!("Press {label}{app}")
        }
        "form_input" => {
            let field = if label.is_empty() {
                "the field".to_string()
            } else {
                label
            };
            format!("Set {field} to {}{app}", quote(&js_string(&a["value"]), 40))
        }
        "scroll" => {
            let dir = match js_string(&a["scroll_direction"]) {
                d if d.is_empty() => "down".to_string(),
                d => d,
            };
            let within = if label.is_empty() {
                String::new()
            } else {
                format!(" in {label}")
            };
            format!("Scroll {dir}{within}{app}")
        }
        "read_page" => format!("Read {window}"),
        "get_page_text" => format!("Read the text of {window}"),
        "find" => format!("Look for {}", quote(&js_string(&a["query"]), 40)),
        "screenshot" => format!("Look at {window}"),
        "zoom" => "Look closer".into(),
        "list_windows" => "List the open windows".into(),
        "focus_window" => {
            if info.app.is_empty() {
                let which = match js_string(&a["title"]) {
                    t if t.is_empty() => js_string(&a["handle"]),
                    t => t,
                };
                format!("Switch to {}", quote(&which, 40))
            } else {
                format!("Switch to {}", info.app)
            }
        }
        "open" => {
            // The whole link: what matters in a URL is often past the first
            // 60 characters, where a query string can carry what was on
            // screen.
            let target = js_string(&a["target"]);
            let t = target.trim();
            if url_ok(t) {
                format!("Open {t}")
            } else {
                format!("Open {}", quote(&target, 60))
            }
        }
        "wait_for" => format!("Wait for {}", quote(&js_string(&a["text"]), 40)),
        "clipboard_read" => "Read the clipboard".into(),
        "clipboard_write" => {
            let text = quote(&js_string(&a["text"]), 40);
            format!("Copy {text} to the clipboard")
        }
        "close_window" => {
            let which = if info.app.is_empty() {
                "a window"
            } else {
                info.app
            };
            format!("Close {which}")
        }
        "wait" => format!("Wait {}s", js_num_text(seconds(&a["duration"]))),
        "update_plan" => "Plan".into(),
        "ask_user" => "Question".into(),
        "left_click_drag" => {
            let from = if label.is_empty() {
                String::new()
            } else {
                format!(" {label}")
            };
            let end_name = match info.end_node {
                Some(n) => js_string(&n["name"]),
                None => String::new(),
            };
            let to = if !end_name.is_empty() {
                format!(" to {}", quote(&end_name, 40))
            } else if !info.end_app.is_empty() {
                " to another place".to_string()
            } else {
                String::new()
            };
            let other = !info.end_app.is_empty() && info.end_app != info.app;
            let there = if other {
                format!(" in {}", info.end_app)
            } else {
                String::new()
            };
            format!("Drag{from}{app}{to}{there}")
        }
        _ => {
            let verb = verb_of(name).unwrap_or(name);
            let what = if label.is_empty() {
                "here".to_string()
            } else {
                label
            };
            format!("{verb} {what}{app}").trim().to_string()
        }
    }
}

fn password_line() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    let pattern = r"\([^)]*(?-u:\b)password(?-u:\b)[^)]*\)\s*$";
    RE.get_or_init(|| Regex::new(pattern).expect("password line"))
}

fn value_attr() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"value="(?:[^"\\]|\\.)*""#).expect("value"))
}

/// The second line of defence for a provider that leaks a password value
/// through a tree line; the engine already writes them as dots.
pub fn redact(text: &str) -> String {
    let lines = text.split('\n').map(|l| {
        if password_line().is_match(l) {
            let dots = "value=\"\u{2022}\u{2022}\u{2022}\"";
            value_attr().replace(l, dots).into_owned()
        } else {
            l.to_string()
        }
    });
    lines.collect::<Vec<_>>().join("\n")
}

/// A desktop error as the model reads it.
pub fn friendly(code: &str, message: &str) -> String {
    match code {
        "BLOCKED_OWN_WINDOW" => "Refused: that is AevalBuddy's own window.".into(),
        "ELEVATED_TARGET" => {
            "Refused: that window runs as administrator, and AevalBuddy does not.".into()
        }
        "STALE_REF" => message.to_string(),
        "" => message.to_string(),
        c => format!("{c}: {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn seen<'a>(node: Option<&'a Value>, app: &'a str) -> Seen<'a> {
        Seen {
            node,
            app,
            ..Seen::default()
        }
    }

    #[test]
    fn steps_read_as_sentences() {
        let save = json!({ "name": "Save", "role": "button" });
        let s = seen(Some(&save), "Notepad");
        let click = describe("left_click", &json!({}), &s);
        assert_eq!(click, "Click \"Save\" in Notepad");
        let typed = json!({ "text": "hi\nthere" });
        let want = "Type \"hi there\" and press Enter in Notepad";
        assert_eq!(describe("type", &typed, &s), want);
        let keys = json!({ "text": "ctrl+s" });
        assert_eq!(describe("key", &keys, &seen(None, "")), "Press Ctrl+S");
        let wait = json!({ "duration": 30 });
        assert_eq!(describe("wait", &wait, &s), "Wait 10s");
        let link = json!({ "target": " https://example.com/a?b=c " });
        let open = describe("open", &link, &s);
        assert_eq!(open, "Open https://example.com/a?b=c");
        let bare = json!({ "role": "pane" });
        let s = seen(Some(&bare), "");
        assert_eq!(describe("hover", &json!({}), &s), "Point at the pane");
    }

    #[test]
    fn passwords_are_dotted_and_errors_named() {
        let tree = "edit \"Password\" value=\"hunter2\" (password)\nedit \"User\" value=\"me\"";
        let want = "edit \"Password\" value=\"\u{2022}\u{2022}\u{2022}\" (password)\n\
                    edit \"User\" value=\"me\"";
        assert_eq!(redact(tree), want);
        let own = friendly("BLOCKED_OWN_WINDOW", "x");
        assert_eq!(own, "Refused: that is AevalBuddy's own window.");
        let named = friendly("NOT_FOUND", "no such ref");
        assert_eq!(named, "NOT_FOUND: no such ref");
        assert_eq!(seconds(&json!("x")), 1.0);
        assert_eq!(seconds(&json!(-2)), 0.0);
    }
}
