//! Skills: markdown procedures that ride along with every request, so a
//! small fast model is told how to think about the screen before it is told
//! what the user asked (PARITY PROMPT-02, PROMPT-03). Port of lib/skills.js.
//!
//! One file per skill in skills/, front matter for the label:
//!
//! ```text
//! ---
//! name: Screen assistant
//! summary: one line for the settings picker
//! ---
//! the procedure, sent verbatim
//! ```
//!
//! The four shipped skills are compiled in. [`Skills::load`] also reads a
//! folder of .md files (the packaged skills/ resource, or a user folder), so
//! adding a skill is still dropping in a file; a file there replaces the
//! built-in skill of the same id.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;
use serde_json::{json, Value};

use crate::llm::prompt::SkillText;

pub const DEFAULT_ID: &str = "screen-assist";

/// The most of a skill that rides along with a request: every question
/// pays for it in tokens, and a free model's context is small (B-125).
pub const MAX_BODY: usize = 8 * 1024;

/// Order in the settings picker; anything else follows alphabetically.
const ORDER: [&str; 4] = ["screen-assist", "debug", "explain", "rewrite"];

const BUILT_IN: [(&str, &str); 4] = [
    ("debug", include_str!("../../skills/debug.md")),
    ("explain", include_str!("../../skills/explain.md")),
    ("rewrite", include_str!("../../skills/rewrite.md")),
    (
        "screen-assist",
        include_str!("../../skills/screen-assist.md"),
    ),
];

/// Which skill fits which kind of request. The reader can name one of these
/// after looking at the screen, and the question is scored against the same
/// vocabulary, so automatic selection has one set of names end to end.
pub fn task_to_skill(task: &str) -> Option<&'static str> {
    match task {
        "debug" => Some("debug"),
        "explain" => Some("explain"),
        "rewrite" => Some("rewrite"),
        "assist" => Some(DEFAULT_ID),
        _ => None,
    }
}

struct Cue {
    skill: &'static str,
    strong: Regex,
    weak: Regex,
}

/// JavaScript's `/\b(...)\b/i`: the word boundary is ASCII, as in JS.
fn js_words(alternatives: &str) -> Regex {
    let pattern = format!(r"(?i)(?-u:\b)({alternatives})(?-u:\b)");
    Regex::new(&pattern).expect("cue pattern")
}

/// Scored against the question. Deliberately small and readable: a wrong
/// guess costs a slightly worse answer, so a transparent rule beats a clever
/// model call that adds a round trip.
#[rustfmt::skip]
fn cues() -> &'static [Cue] {
    static CUES: OnceLock<Vec<Cue>> = OnceLock::new();
    CUES.get_or_init(|| {
        vec![
            Cue {
                skill: "debug",
                strong: js_words(r"errors?|exceptions?|traceback|stack ?trace|failing|failed|crash(ed|ing)?|bugs?|broken|stuck"),
                weak: js_words(r"fix|debug|red|warnings?|tests?|build|compile|lint|wont|cannot"),
            },
            Cue {
                skill: "explain",
                strong: js_words(r"what (is|are|does|do)|explain|how does|what.s (this|that)|meaning of"),
                weak: js_words(r"understand|difference|means?|purpose"),
            },
            Cue {
                skill: "rewrite",
                strong: js_words(r"rewrite|reword|rephrase|proofread|shorten|draft|make (this|it) (shorter|clearer|better|nicer|polite|professional)"),
                weak: js_words(r"email|message|reply|wording|grammar|polish|tone"),
            },
        ]
    })
}

/// One skill file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub body: String,
    /// The file was longer than [`MAX_BODY`] and `body` is its start.
    pub truncated: bool,
}

impl Skill {
    pub fn text(&self) -> SkillText {
        SkillText {
            name: self.name.clone(),
            body: self.body.clone(),
        }
    }
}

fn front_matter_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    const PATTERN: &str = r"^---\r?\n((?s:.*?))\r?\n---\r?\n?((?s:.*))$";
    RE.get_or_init(|| Regex::new(PATTERN).expect("front"))
}

/// A front matter field: the first `key: value` line, case-insensitive.
fn field(head: &str, key: &str) -> String {
    // JS `^` and `$` with the m flag split at \n, \r, U+2028 and U+2029.
    let lines = head.split(['\n', '\r', '\u{2028}', '\u{2029}']);
    for line in lines {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        // `\s*:\s*(.+)$`: a value of spaces still matches, and reads as "".
        let named = k.trim_end().eq_ignore_ascii_case(key) && k.len() == k.trim_start().len();
        if named && !v.is_empty() {
            return v.trim().to_string();
        }
    }
    String::new()
}

/// Parses one skill file.
pub fn parse(id: &str, raw: &str) -> Skill {
    let text = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let (head, body) = match front_matter_re().captures(text) {
        Some(m) => {
            let part = |i: usize| m.get(i).map_or("", |g| g.as_str());
            (part(1), part(2))
        }
        None => ("", text),
    };
    let mut name = field(head, "name");
    if name.is_empty() {
        name = id.to_string();
    }
    let body = body.trim();
    let truncated = body.len() > MAX_BODY;
    let body = if truncated {
        let mut end = MAX_BODY;
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        body[..end].trim_end()
    } else {
        body
    };
    Skill {
        id: id.to_string(),
        name,
        summary: field(head, "summary"),
        body: body.to_string(),
        truncated,
    }
}

fn rank(id: &str) -> usize {
    ORDER.iter().position(|o| *o == id).unwrap_or(99)
}

/// `localeCompare` for skill names: case-insensitive first, lower case
/// before upper on a tie.
fn locale_cmp(a: &str, b: &str) -> Ordering {
    let (la, lb) = (a.to_lowercase(), b.to_lowercase());
    la.cmp(&lb).then_with(|| b.cmp(a))
}

/// `<exe folder>\skills`: where a person's own answer styles live.
pub fn folder_of(exe: &Path) -> Option<PathBuf> {
    exe.parent().map(|p| p.join("skills"))
}

/// Every loaded skill, in picker order.
#[derive(Clone, Debug, Default)]
pub struct Skills {
    all: Vec<Skill>,
}

impl Skills {
    /// The built-in skills, plus every .md file in `dir`.
    pub fn load(dir: Option<&Path>) -> Skills {
        let mut all: Vec<Skill> = BUILT_IN.iter().map(|(id, raw)| parse(id, raw)).collect();
        let mut files: Vec<_> = dir
            .and_then(|d| std::fs::read_dir(d).ok())
            .map(|d| d.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        files.retain(|p| {
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or_default();
            ext.eq_ignore_ascii_case("md")
        });
        files.sort();
        for f in files {
            let Some(id) = f.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(raw) = std::fs::read_to_string(&f) else {
                continue;
            };
            let skill = parse(id, &raw);
            all.retain(|s| s.id != skill.id);
            all.push(skill);
        }
        Skills::from_skills(all)
    }

    pub fn from_skills(mut all: Vec<Skill>) -> Skills {
        all.retain(|s| !s.body.is_empty());
        all.sort_by(|a, b| {
            let (ia, ib) = (rank(&a.id), rank(&b.id));
            if ia != 99 || ib != 99 {
                return ia.cmp(&ib);
            }
            locale_cmp(&a.name, &b.name)
        });
        Skills { all }
    }

    /// Every skill without its body: enough for the settings picker, which
    /// says when a file was cut to [`MAX_BODY`].
    pub fn list(&self) -> Value {
        let rows: Vec<Value> = self
            .all
            .iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "name": s.name,
                    "summary": s.summary,
                    "truncated": s.truncated,
                })
            })
            .collect();
        Value::Array(rows)
    }

    /// The skill to apply, falling back to the default and then to nothing.
    pub fn get(&self, id: &str) -> Option<&Skill> {
        let find = |want: &str| self.all.iter().find(|s| s.id == want);
        let chosen = find(id).or_else(|| find(DEFAULT_ID));
        chosen.or_else(|| self.all.first())
    }

    pub fn has(&self, id: &str) -> bool {
        self.all.iter().any(|s| s.id == id)
    }

    /// Picks the skill for one request: a TASK word from the reader, which
    /// has looked at the screen, outranks the wording of the question; then
    /// strong cues, then weak ones; else the general skill. Returns the id
    /// and why (`screen`, `asked` or `default`).
    pub fn route(&self, question: &str, task: Option<&str>) -> (String, &'static str) {
        let task = task.unwrap_or_default().trim().to_lowercase();
        if let Some(id) = task_to_skill(&task).filter(|id| self.has(id)) {
            return (id.to_string(), "screen");
        }
        for cue in cues() {
            if self.has(cue.skill) && cue.strong.is_match(question) {
                return (cue.skill.to_string(), "asked");
            }
        }
        for cue in cues() {
            if self.has(cue.skill) && cue.weak.is_match(question) {
                return (cue.skill.to_string(), "asked");
            }
        }
        (DEFAULT_ID.to_string(), "default")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_path_is_exe_relative() {
        let exe = Path::new(r"C:\Program Files\Aevaleron\Aevaleron.exe");
        let dir = folder_of(exe).unwrap();
        assert_eq!(dir, Path::new(r"C:\Program Files\Aevaleron").join("skills"));
        assert!(folder_of(Path::new("")).is_none());
    }

    #[test]
    fn built_ins_load_in_picker_order() {
        let s = Skills::load(None);
        let ids: Vec<&str> = s.all.iter().map(|k| k.id.as_str()).collect();
        assert_eq!(ids, ORDER);
        let debug = s.get("debug").unwrap();
        assert_eq!(debug.name, "Debug an error");
        assert!(debug.summary.starts_with("Errors, stack traces"));
        let lead = "Work through this before you reply.";
        assert!(debug.body.starts_with(lead));
        assert_eq!(s.get("no-such-skill").unwrap().id, DEFAULT_ID);
        assert_eq!(s.list()[0]["id"], json!("screen-assist"));
    }

    #[test]
    fn parse_handles_bom_crlf_and_missing_front_matter() {
        let raw = "\u{feff}---\r\nName : Hi\r\nsummary: s\r\n---\r\n body \r\n";
        let k = parse("x", raw);
        assert_eq!(k.name, "Hi");
        assert_eq!(k.summary, "s");
        assert_eq!(k.body, "body");
        let k = parse("plain", "just text");
        assert_eq!((k.name.as_str(), k.body.as_str()), ("plain", "just text"));
    }

    #[test]
    fn unknown_skills_sort_after_the_known_ones_by_name() {
        let mk = |id: &str, name: &str| Skill {
            id: id.into(),
            name: name.into(),
            summary: String::new(),
            body: "b".into(),
            truncated: false,
        };
        let s = Skills::from_skills(vec![mk("z", "beta"), mk("y", "Alpha"), mk("debug", "D")]);
        let ids: Vec<&str> = s.all.iter().map(|k| k.id.as_str()).collect();
        assert_eq!(ids, ["debug", "y", "z"]);
    }

    #[test]
    fn large_skill_is_truncated() {
        let long = format!("---\nname: Long\n---\n{}", "é".repeat(MAX_BODY));
        let k = parse("long", &long);
        assert!(k.truncated);
        assert!(k.body.len() <= MAX_BODY);
        assert!(k.body.chars().all(|c| c == 'é'));
        let s = Skills::from_skills(vec![k, parse("short", "fine")]);
        let rows = s.list();
        let list = rows.as_array().unwrap();
        let row = |id: &str| list.iter().find(|r| r["id"] == id).cloned().unwrap();
        assert_eq!(row("long")["truncated"], json!(true));
        assert_eq!(row("short")["truncated"], json!(false));
        assert!(!parse("x", "short body").truncated);
    }

    #[test]
    fn routing_matches_the_self_test() {
        let s = Skills::load(None);
        let qs = [
            "why did the build fail",
            "what is this dialog",
            "reword this email",
            "help",
        ];
        let routed: Vec<String> = qs.iter().map(|q| s.route(q, None).0).collect();
        assert_eq!(routed, ["debug", "explain", "rewrite", DEFAULT_ID]);
        assert_eq!(s.route("why did this fail", None).1, "default");
        assert_eq!(s.route("x", Some("rewrite")), ("rewrite".into(), "screen"));
        let assist = s.route("x", Some(" Assist "));
        assert_eq!(assist, (DEFAULT_ID.into(), "screen"));
        assert_eq!(s.route("what's this button", None).0, "explain");
        assert_eq!(s.route("make it shorter please", None).0, "rewrite");
        assert_eq!(s.route("can you polish my email", None).0, "rewrite");
        assert_eq!(s.route("fix the explain step", None).0, "explain");
        assert_eq!(s.route("hello", None), (DEFAULT_ID.into(), "default"));
        // JS word boundaries are ASCII: the é ends the word "error".
        assert_eq!(s.route("errorés", None).0, "debug");
        assert_eq!(s.route("terrors", None).0, DEFAULT_ID);
    }
}
