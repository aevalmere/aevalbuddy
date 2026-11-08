//! Conversations that outlive the process (PARITY HIST-01..06, FILE-03).
//!
//! Text only, never a screenshot or a reading of one. `history.json` in the
//! user-data folder is `{ version: 1, conversations: [...] }`, rewritten whole
//! through a `.tmp` file and a rename, 400 ms after the last change, and at
//! once on quit.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{json, Value};

pub const MAX_ENTRIES: usize = 200;
const MAX_BYTES: usize = 4 * 1024 * 1024;
const TITLE_MAX: usize = 90;
const DEBOUNCE: Duration = Duration::from_millis(400);

#[derive(Default)]
struct Inner {
    cache: Option<Vec<Value>>,
    enabled: bool,
    /// Bumped by each schedule; a writer thread only flushes if it is still
    /// the newest.
    pending: u64,
}

pub struct History {
    file: PathBuf,
    inner: Mutex<Inner>,
}

static GLOBAL: OnceLock<History> = OnceLock::new();

pub fn init(user_data: &Path) -> &'static History {
    GLOBAL.get_or_init(|| History::new(user_data))
}

pub fn get() -> &'static History {
    GLOBAL.get().expect("history::init runs at boot")
}

/// Whitespace collapsed, 90 characters with an ellipsis.
pub fn first_line(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars: Vec<char> = line.chars().collect();
    if chars.len() > TITLE_MAX {
        let mut s: String = chars[..TITLE_MAX - 1].iter().collect();
        s.push('…');
        s
    } else {
        line
    }
}

fn text_of(m: &Value) -> String {
    m.get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn at_of(e: &Value) -> f64 {
    e.get("at").and_then(Value::as_f64).unwrap_or(0.0)
}

fn id_of(e: &Value) -> &str {
    e.get("id").and_then(Value::as_str).unwrap_or_default()
}

/// The JSON length JavaScript's JSON.stringify would give, near enough for a
/// 4 MB budget (serde escapes the same characters).
fn json_len(list: &[Value]) -> usize {
    serde_json::to_string(list).map(|s| s.len()).unwrap_or(0)
}

impl History {
    pub fn new(user_data: &Path) -> History {
        History {
            file: user_data.join("history.json"),
            inner: Mutex::new(Inner {
                cache: None,
                enabled: true,
                pending: 0,
            }),
        }
    }

    fn load<'a>(&self, inner: &'a mut Inner) -> &'a mut Vec<Value> {
        if inner.cache.is_none() {
            // Missing on first run, unreadable if hand-edited: an empty list
            // either way, never a failed start (HIST-05).
            let list = fs::read_to_string(&self.file)
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|v| v.get("conversations").and_then(Value::as_array).cloned())
                .unwrap_or_default();
            inner.cache = Some(list);
        }
        inner.cache.get_or_insert_with(Vec::new)
    }

    pub fn set_enabled(&self, on: bool) {
        self.inner.lock().enabled = on;
    }

    fn schedule_write(&'static self) {
        let ticket = {
            let mut inner = self.inner.lock();
            inner.pending += 1;
            inner.pending
        };
        std::thread::spawn(move || {
            std::thread::sleep(DEBOUNCE);
            if self.inner.lock().pending == ticket {
                self.flush();
            }
        });
    }

    /// Writes the cache now. Write failures are ignored: the conversation
    /// is still on screen.
    pub fn flush(&self) {
        let Some(list) = self.inner.lock().cache.clone() else {
            return;
        };
        let body = json!({ "version": 1, "conversations": list });
        let tmp = PathBuf::from(format!("{}.tmp", self.file.display()));
        if let Some(dir) = self.file.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if fs::write(&tmp, body.to_string()).is_ok() {
            let _ = fs::rename(&tmp, &self.file);
        }
    }

    /// On quit, where the debounce would never fire (LIFE-11).
    pub fn flush_now(&self) {
        self.inner.lock().pending += 1;
        self.flush();
    }

    /// Replaces the stored version of this conversation (HIST-02).
    pub fn save(&'static self, id: &str, model: &str, messages: &[Value]) {
        if !self.save_inner(id, model, messages, crate::state::now_ms()) {
            return;
        }
        self.schedule_write();
    }

    fn save_inner(&self, id: &str, model: &str, messages: &[Value], at: u64) -> bool {
        let mut inner = self.inner.lock();
        if !inner.enabled {
            return false;
        }
        let kept: Vec<Value> = messages
            .iter()
            .filter(|m| {
                let role = m.get("role").and_then(Value::as_str);
                matches!(role, Some("user" | "assistant")) && !text_of(m).is_empty()
            })
            .map(|m| {
                let role = m.get("role").cloned().unwrap_or(Value::Null);
                if m.get("kind").and_then(Value::as_str) == Some("trail") {
                    json!({ "role": role, "kind": "trail", "text": text_of(m) })
                } else {
                    json!({ "role": role, "text": text_of(m) })
                }
            })
            .collect();
        if kept.is_empty() {
            return false;
        }
        let first = kept
            .iter()
            .find(|m| m["role"] == "user")
            .unwrap_or(&kept[0]);
        let entry = json!({
            "id": id,
            "at": at,
            "title": first_line(&text_of(first)),
            "model": model,
            "messages": kept,
        });
        let list = self.load(&mut inner);
        match list.iter().position(|e| id_of(e) == id) {
            Some(i) => list[i] = entry,
            None => list.push(entry),
        }
        trim(list);
        true
    }

    /// Rows for the History window, newest first (HIST-03).
    pub fn list(&self, query: &str) -> Vec<Value> {
        let q = query.trim().to_lowercase();
        let mut inner = self.inner.lock();
        let list = self.load(&mut inner);
        let mut rows: Vec<Value> = list
            .iter()
            .filter(|e| {
                if q.is_empty() {
                    return true;
                }
                let title = e.get("title").and_then(Value::as_str).unwrap_or_default();
                if title.to_lowercase().contains(&q) {
                    return true;
                }
                messages(e)
                    .iter()
                    .any(|m| text_of(m).to_lowercase().contains(&q))
            })
            .map(|e| {
                let msgs = messages(e);
                let reply = msgs
                    .iter()
                    .find(|m| m["role"] == "assistant")
                    .map(text_of)
                    .unwrap_or_default();
                json!({
                    "id": e.get("id").cloned().unwrap_or(Value::Null),
                    "at": e.get("at").cloned().unwrap_or(Value::Null),
                    "title": e.get("title").cloned().unwrap_or(Value::Null),
                    "model": e.get("model").cloned().unwrap_or(Value::Null),
                    "count": msgs.len(),
                    "asked": msgs.iter().filter(|m| m["role"] == "user").count(),
                    "reply": first_line(&reply),
                })
            })
            .collect();
        rows.sort_by(|a, b| at_of(b).total_cmp(&at_of(a)));
        rows
    }

    pub fn entry(&self, id: &str) -> Option<Value> {
        let mut inner = self.inner.lock();
        self.load(&mut inner)
            .iter()
            .find(|e| id_of(e) == id)
            .cloned()
    }

    pub fn remove(&'static self, id: &str) -> bool {
        let removed = {
            let mut inner = self.inner.lock();
            let list = self.load(&mut inner);
            match list.iter().position(|e| id_of(e) == id) {
                Some(i) => {
                    list.remove(i);
                    true
                }
                None => false,
            }
        };
        if removed {
            self.schedule_write();
        }
        removed
    }

    pub fn clear(&'static self) -> bool {
        self.inner.lock().cache = Some(Vec::new());
        self.schedule_write();
        true
    }

    pub fn count(&self) -> usize {
        let mut inner = self.inner.lock();
        self.load(&mut inner).len()
    }
}

fn messages(e: &Value) -> Vec<Value> {
    e.get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Newest first, at most 200, then oldest dropped while over 4 MB.
fn trim(list: &mut Vec<Value>) {
    list.sort_by(|a, b| at_of(b).total_cmp(&at_of(a)));
    list.truncate(MAX_ENTRIES);
    while list.len() > 1 && json_len(list) > MAX_BYTES {
        list.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> &'static History {
        let dir = std::env::temp_dir().join(format!("aeval-hist-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Box::leak(Box::new(History::new(&dir)))
    }

    fn msg(role: &str, text: &str) -> Value {
        json!({ "role": role, "text": text })
    }

    #[test]
    fn titles() {
        assert_eq!(first_line("  a\n\tb  "), "a b");
        let long = "x".repeat(200);
        let t = first_line(&long);
        assert_eq!(t.chars().count(), 90);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn upsert_filter_and_search() {
        let h = temp("upsert");
        let chat = vec![
            msg("user", "How do I fix the build?"),
            msg("assistant", "Run cargo clean."),
            json!({ "role": "system", "text": "dropped" }),
            msg("assistant", ""),
        ];
        assert!(h.save_inner("c1", "m", &chat, 10));
        let mut more = chat.clone();
        more.push(json!({ "role": "assistant", "kind": "trail", "text": "1. Opened" }));
        assert!(h.save_inner("c1", "m", &more, 20));
        assert!(h.save_inner("c2", "m", &[msg("user", "Other")], 30));
        let rows = h.list("");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], "c2");
        assert_eq!(rows[1]["count"], 3);
        assert_eq!(rows[1]["asked"], 1);
        assert_eq!(rows[1]["reply"], "Run cargo clean.");
        assert_eq!(h.list("CARGO").len(), 1);
        assert_eq!(h.list("fix the").len(), 1);
        let e = h.entry("c1").unwrap();
        assert_eq!(e["messages"][2]["kind"], "trail");
    }

    #[test]
    fn disabled_writes_nothing_and_keeps_rows() {
        let h = temp("disabled");
        h.save_inner("c1", "", &[msg("user", "a")], 1);
        h.set_enabled(false);
        assert!(!h.save_inner("c2", "", &[msg("user", "b")], 2));
        assert_eq!(h.count(), 1);
    }

    #[test]
    fn caps_at_two_hundred_newest_first() {
        let h = temp("cap");
        for i in 0..210u64 {
            h.save_inner(&format!("c{i}"), "", &[msg("user", "q")], i);
        }
        assert_eq!(h.count(), MAX_ENTRIES);
        assert!(h.entry("c0").is_none());
        assert!(h.entry("c209").is_some());
    }

    #[test]
    fn flush_writes_the_electron_shape_and_reads_back() {
        let h = temp("flush");
        h.save_inner("c1", "mod", &[msg("user", "hi")], 5);
        h.flush_now();
        let text = fs::read_to_string(&h.file).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(v["conversations"][0]["title"], "hi");
        let again = History::new(h.file.parent().unwrap());
        assert_eq!(again.count(), 1);
    }

    #[test]
    fn unreadable_file_is_an_empty_list() {
        let h = temp("bad");
        fs::write(&h.file, "{{{").unwrap();
        assert_eq!(h.count(), 0);
        assert!(!h.remove("x"));
    }
}
