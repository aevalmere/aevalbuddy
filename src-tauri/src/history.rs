//! Conversations that outlive the process (PARITY HIST-01..06, FILE-03).
//!
//! Text only, never a screenshot or a reading of one. `history.json` in the
//! user-data folder is `{ version: 1, conversations: [...] }`, rewritten whole
//! and durably (`store::durable`: unique temp, fsync, write-through replace).
//!
//! Writes are throttled from the first change, as Electron's history.js
//! did: a change schedules a write 400 ms later, changes inside that window
//! ride along, and nothing postpones it. One long-lived writer thread does
//! the timed writes; quitting writes at once, under the same lock (D-12).
//!
//! A file that does not parse is renamed aside, never overwritten; a file
//! that exists but cannot be read keeps this session from writing (B-003).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{json, Value};

use crate::store::durable;

pub const MAX_ENTRIES: usize = 200;
/// JavaScript's `JSON.stringify(list).length`: UTF-16 units, not bytes.
const MAX_UNITS: usize = 4 * 1024 * 1024;
/// UTF-16 units, as JavaScript's `line.length`.
const TITLE_MAX: usize = 90;
/// From the first change to the write it schedules.
const THROTTLE: Duration = Duration::from_millis(400);

struct Inner {
    cache: Option<Vec<Value>>,
    enabled: bool,
    /// False when the file exists but could not be read: it is never
    /// overwritten this session.
    writable: bool,
    /// The cache has changes the file does not.
    dirty: bool,
}

pub struct History {
    file: PathBuf,
    inner: Mutex<Inner>,
    /// Held across snapshot, write and replace, so the timed write and
    /// `flush_now` never overlap and the later one carries the newer list.
    write: Mutex<()>,
    /// Wakes the writer thread, which starts with the first change.
    wake: Mutex<Option<mpsc::Sender<()>>>,
    read: fn(&Path) -> io::Result<Vec<u8>>,
    #[cfg(test)]
    writing: std::sync::atomic::AtomicUsize,
    #[cfg(test)]
    overlaps: std::sync::atomic::AtomicUsize,
    #[cfg(test)]
    writes: std::sync::atomic::AtomicUsize,
}

/// The file's shape, serialised in Electron's key order.
#[derive(Serialize)]
struct Body<'a> {
    version: u32,
    conversations: &'a [Value],
}

static GLOBAL: OnceLock<History> = OnceLock::new();

pub fn init(user_data: &Path) -> &'static History {
    GLOBAL.get_or_init(|| History::new(user_data))
}

pub fn get() -> &'static History {
    GLOBAL.get().expect("history::init runs at boot")
}

fn read_file(path: &Path) -> io::Result<Vec<u8>> {
    fs::read(path)
}

/// Whitespace collapsed; past 90 UTF-16 units, the first 89 and an
/// ellipsis, as history.js counted, except that a surrogate pair is never
/// split (B-083).
pub fn first_line(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.encode_utf16().count() <= TITLE_MAX {
        return line;
    }
    let mut out = String::new();
    let mut units = 0;
    for c in line.chars() {
        units += c.len_utf16();
        if units > TITLE_MAX - 1 {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

fn text_of(m: &Value) -> String {
    text_ref(m).to_string()
}

fn at_of(e: &Value) -> f64 {
    e.get("at").and_then(Value::as_f64).unwrap_or(0.0)
}

fn id_of(e: &Value) -> &str {
    e.get("id").and_then(Value::as_str).unwrap_or_default()
}

/// The length JavaScript's JSON.stringify gives `v`, in UTF-16 units (serde
/// escapes the same characters).
fn js_len(v: &Value) -> usize {
    serde_json::to_string(v).map_or(0, |s| s.encode_utf16().count())
}

impl History {
    pub fn new(user_data: &Path) -> History {
        Self::with_reader(user_data, read_file)
    }

    fn with_reader(user_data: &Path, read: fn(&Path) -> io::Result<Vec<u8>>) -> History {
        History {
            file: user_data.join("history.json"),
            inner: Mutex::new(Inner {
                cache: None,
                enabled: true,
                writable: true,
                dirty: false,
            }),
            write: Mutex::new(()),
            wake: Mutex::new(None),
            read,
            #[cfg(test)]
            writing: Default::default(),
            #[cfg(test)]
            overlaps: Default::default(),
            #[cfg(test)]
            writes: Default::default(),
        }
    }

    fn load<'a>(&self, inner: &'a mut Inner) -> &'a mut Vec<Value> {
        if inner.cache.is_none() {
            let (list, writable) = self.read_list();
            inner.cache = Some(list);
            inner.writable = writable;
        }
        inner.cache.get_or_insert_with(Vec::new)
    }

    /// HIST-05: never a failed start. A missing file is an empty list. A
    /// file that does not parse is renamed to `history.json.corrupt-<ms>`
    /// first. A file that cannot be read is left alone and not written.
    fn read_list(&self) -> (Vec<Value>, bool) {
        let bytes = match (self.read)(&self.file) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (Vec::new(), true),
            Err(e) => {
                let kind = e.kind();
                let why = format!("read failed: {kind:?}; history is not saved this session");
                crate::crash::write("history", &why);
                return (Vec::new(), false);
            }
        };
        let text = String::from_utf8_lossy(&bytes);
        let body = text.trim_start_matches('\u{feff}');
        if let Ok(mut v) = serde_json::from_str::<Value>(body) {
            if let Some(Value::Array(list)) = v.get_mut("conversations").map(Value::take) {
                return (list, true);
            }
        }
        let mut aside = self.file.as_os_str().to_owned();
        aside.push(format!(".corrupt-{}", crate::state::now_ms()));
        let aside = PathBuf::from(aside);
        match fs::rename(&self.file, &aside) {
            Ok(()) => {
                let why = format!("history.json did not parse; kept as {}", aside.display());
                crate::crash::write("history", &why);
                (Vec::new(), true)
            }
            Err(e) => {
                let why = format!("history.json did not parse or move ({e}); not saved");
                crate::crash::write("history", &why);
                (Vec::new(), false)
            }
        }
    }

    pub fn set_enabled(&self, on: bool) {
        self.inner.lock().enabled = on;
    }

    /// Starts the writer on the first change, then wakes it.
    fn schedule_write(&'static self) {
        let mut wake = self.wake.lock();
        if wake.is_none() {
            let (tx, rx) = mpsc::channel();
            let spawned = std::thread::Builder::new()
                .name("history-writer".into())
                .spawn(move || self.writer(rx));
            if let Err(e) = spawned {
                crate::crash::write("history", &format!("writer not started: {e}"));
                return;
            }
            *wake = Some(tx);
        }
        if let Some(tx) = wake.as_ref() {
            let _ = tx.send(());
        }
    }

    /// The one writer: a change wakes it, it waits out the window (changes
    /// that arrive meanwhile ride along), then writes.
    fn writer(&'static self, rx: mpsc::Receiver<()>) {
        while rx.recv().is_ok() {
            std::thread::sleep(THROTTLE);
            while rx.try_recv().is_ok() {}
            crate::crash::guard("history.write", || self.flush());
        }
    }

    /// Writes the cache if it has changes. A failure is not fatal: the
    /// conversation is still on screen, and the next write tries again.
    pub fn flush(&self) {
        let _one = self.write.lock();
        let body = {
            let mut inner = self.inner.lock();
            if !inner.dirty || !inner.writable {
                return;
            }
            let Some(list) = inner.cache.as_ref() else {
                return;
            };
            let body = serde_json::to_vec(&Body {
                version: 1,
                conversations: list,
            });
            inner.dirty = false;
            body
        };
        #[cfg(test)]
        self.enter_write();
        let result = body
            .map_err(io::Error::from)
            .and_then(|b| durable::write(&self.file, &b));
        #[cfg(test)]
        self.leave_write();
        if let Err(e) = result {
            self.inner.lock().dirty = true;
            eprintln!("[aevalbuddy] history not saved: {e}");
        }
    }

    /// On quit, where the timer would never fire (LIFE-11).
    pub fn flush_now(&self) {
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
        inner.dirty = true;
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
                    .any(|m| text_ref(m).to_lowercase().contains(&q))
            })
            .map(|e| {
                // What history.js reads: id, at, title, asked, reply.
                let msgs = messages(e);
                let reply = msgs
                    .iter()
                    .find(|m| m["role"] == "assistant")
                    .map_or("", text_ref);
                json!({
                    "id": e.get("id").cloned().unwrap_or(Value::Null),
                    "at": e.get("at").cloned().unwrap_or(Value::Null),
                    "title": e.get("title").cloned().unwrap_or(Value::Null),
                    "asked": msgs.iter().filter(|m| m["role"] == "user").count(),
                    "reply": first_line(reply),
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
            let found = list.iter().position(|e| id_of(e) == id);
            if let Some(i) = found {
                list.remove(i);
                inner.dirty = true;
            }
            found.is_some()
        };
        if removed {
            self.schedule_write();
        }
        removed
    }

    pub fn clear(&'static self) -> bool {
        {
            let mut inner = self.inner.lock();
            self.load(&mut inner).clear();
            inner.dirty = true;
        }
        self.schedule_write();
        true
    }

    pub fn count(&self) -> usize {
        let mut inner = self.inner.lock();
        self.load(&mut inner).len()
    }

    #[cfg(test)]
    fn enter_write(&self) {
        use std::sync::atomic::Ordering;
        if self.writing.fetch_add(1, Ordering::SeqCst) > 0 {
            self.overlaps.fetch_add(1, Ordering::SeqCst);
        }
        self.writes.fetch_add(1, Ordering::SeqCst);
    }

    #[cfg(test)]
    fn leave_write(&self) {
        use std::sync::atomic::Ordering;
        self.writing.fetch_sub(1, Ordering::SeqCst);
    }
}

fn messages(e: &Value) -> &[Value] {
    e.get("messages")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

fn text_ref(m: &Value) -> &str {
    m.get("text").and_then(Value::as_str).unwrap_or_default()
}

/// Newest first, at most 200, then the oldest dropped while the list's JSON
/// is over 4 Mi UTF-16 units. Each entry is measured once: the array's
/// length is the entries' plus the brackets and commas, so the trim is
/// linear (B-083).
fn trim(list: &mut Vec<Value>) {
    list.sort_by(|a, b| at_of(b).total_cmp(&at_of(a)));
    list.truncate(MAX_ENTRIES);
    let sizes: Vec<usize> = list.iter().map(js_len).collect();
    let mut total = 2 + sizes.iter().sum::<usize>() + sizes.len().saturating_sub(1);
    while list.len() > 1 && total > MAX_UNITS {
        list.pop();
        total -= sizes[list.len()] + 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aeval-hist-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn temp(name: &str) -> &'static History {
        Box::leak(Box::new(History::new(&dir(name))))
    }

    fn names(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }

    fn read_json(path: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
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
        // Only what history.js reads; the transcript stays behind.
        assert!(rows[1].get("count").is_none());
        assert!(rows[1].get("messages").is_none());
        let kept = h.entry("c1").unwrap()["messages"].as_array().unwrap().len();
        assert_eq!(kept, 3, "system and empty messages are not saved");
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

    #[test]
    fn corrupt_file_is_kept_aside_not_overwritten() {
        let h = temp("corrupt");
        let dir = h.file.parent().unwrap();
        let torn = r#"{"conversations": [{"id""#;
        fs::write(&h.file, torn).unwrap();
        assert_eq!(h.count(), 0);
        let aside: Vec<String> = names(dir)
            .into_iter()
            .filter(|n| n.starts_with("history.json.corrupt-"))
            .collect();
        assert_eq!(aside.len(), 1, "{aside:?}");
        assert_eq!(fs::read_to_string(dir.join(&aside[0])).unwrap(), torn);
        h.save_inner("c1", "m", &[msg("user", "new")], 5);
        h.flush_now();
        assert_eq!(read_json(&h.file)["conversations"][0]["title"], "new");
        assert_eq!(fs::read_to_string(dir.join(&aside[0])).unwrap(), torn);
    }

    fn denied(_: &Path) -> io::Result<Vec<u8>> {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }

    #[test]
    fn unreadable_file_is_never_written() {
        let dir = dir("denied");
        let file = dir.join("history.json");
        fs::write(&file, "precious").unwrap();
        let h = History::with_reader(&dir, denied);
        assert_eq!(h.count(), 0);
        assert!(h.save_inner("c1", "m", &[msg("user", "q")], 1));
        h.flush_now();
        assert_eq!(fs::read_to_string(&file).unwrap(), "precious");
        assert_eq!(names(&dir), vec!["history.json".to_string()]);
    }

    #[test]
    fn steady_saves_still_write_within_400ms() {
        let h = temp("steady");
        let start = std::time::Instant::now();
        let mut first = None;
        let mut i = 0;
        // A change every 50 ms would postpone a debounce forever; the
        // throttle writes 400 ms after the first one regardless.
        while start.elapsed() < Duration::from_millis(1500) {
            h.save(&format!("c{i}"), "m", &[msg("user", "q")]);
            i += 1;
            std::thread::sleep(Duration::from_millis(50));
            if first.is_none() && h.file.exists() {
                first = Some(start.elapsed());
            }
        }
        let at = first.expect("written while saves kept coming");
        assert!(at < Duration::from_millis(900), "first write after {at:?}");
        assert!(at >= THROTTLE, "written before the window closed: {at:?}");
    }

    #[test]
    fn flush_now_and_timer_never_overlap() {
        use std::sync::atomic::Ordering;
        let h = temp("overlap");
        std::thread::scope(|scope| {
            scope.spawn(|| {
                for i in 0..60 {
                    h.save(&format!("c{i}"), "m", &[msg("user", "q")]);
                    std::thread::sleep(Duration::from_millis(10));
                }
            });
            for _ in 0..3 {
                scope.spawn(|| {
                    for _ in 0..60 {
                        h.flush_now();
                        std::thread::sleep(Duration::from_millis(7));
                    }
                });
            }
        });
        std::thread::sleep(THROTTLE + Duration::from_millis(200));
        h.flush_now();
        assert_eq!(h.overlaps.load(Ordering::SeqCst), 0);
        assert!(h.writes.load(Ordering::SeqCst) > 1);
        let v = read_json(&h.file);
        assert_eq!(v["conversations"].as_array().map(Vec::len), Some(60));
        let dir = h.file.parent().unwrap();
        assert_eq!(names(dir), vec!["history.json".to_string()]);
    }

    fn entry(id: &str, at: u64, text: &str) -> Value {
        json!({ "id": id, "at": at, "title": "t", "model": "", "messages": [msg("user", text)] })
    }

    #[test]
    fn cjk_budget_matches_electron() {
        // 3 MB of UTF-8 each, 1 Mi UTF-16 units each: JavaScript keeps both.
        let text = "漢".repeat(1024 * 1024);
        let mut list = vec![entry("a", 2, &text), entry("b", 1, &text)];
        trim(&mut list);
        assert_eq!(list.len(), 2);
        let v = Value::Array(list);
        let bytes = serde_json::to_string(&v).unwrap().len();
        assert!(js_len(&v) < MAX_UNITS, "counted in UTF-16 units");
        assert!(bytes > MAX_UNITS, "over the budget in UTF-8 bytes");
    }

    #[test]
    fn trim_result_serializes_under_4mb() {
        let text = "x".repeat(300 * 1024);
        let mut list: Vec<Value> = (0..30u64)
            .map(|i| entry(&format!("c{i}"), i, &text))
            .collect();
        trim(&mut list);
        let kept = list.len();
        assert!(kept > 1 && kept < 30, "{kept}");
        assert_eq!(list[0]["id"], "c29", "newest first");
        let units = js_len(&Value::Array(list.clone()));
        assert!(units <= MAX_UNITS, "{units}");
        // One more and it would not have fitted.
        list.push(entry("older", 0, &text));
        assert!(js_len(&Value::Array(list)) > MAX_UNITS);
    }

    #[test]
    fn emoji_title_cut_on_unit_boundary() {
        let title = first_line(&"😀".repeat(100));
        assert!(title.ends_with('…'));
        // 89 units would split the 45th emoji: 44 of them, then the ellipsis.
        assert_eq!(title.chars().filter(|c| *c == '😀').count(), 44);
        assert_eq!(title.encode_utf16().count(), 89);
        let mixed = first_line(&format!("{}😀tail", "a".repeat(88)));
        assert_eq!(mixed, format!("{}…", "a".repeat(88)));
        assert_eq!(first_line(&"😀".repeat(45)), "😀".repeat(45));
    }
}
