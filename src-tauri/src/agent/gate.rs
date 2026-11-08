//! The executor's safety gate (PARITY AGENT-05..09). Port of `gate`,
//! `chords`, `chordRisk`, `quote` and `chordLabel` in lib/agent.js.
//!
//! The gate is allowlist-shaped and fails closed. A step runs without a
//! card only when the executor can see what it touches and that thing is on
//! the short list of harmless ones; anything it cannot read, or does not
//! know, asks. Names are chosen by whoever built the page, so a name alone
//! never clears a step: the app, the dialog's own text and the page address
//! count too.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::{json, Value};

use super::seams::DesktopOps;
use super::tools::{is_pointer_tool, is_read_tool};

/// JavaScript `/\b(...)\b/i` with ASCII word boundaries.
fn words(alternatives: &str) -> Regex {
    let pattern = format!(r"(?i)(?-u:\b)(?:{alternatives})(?-u:\b)");
    Regex::new(&pattern).expect("gate pattern")
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("gate pattern")
}

#[rustfmt::skip]
const VERB_LIST: &str = "send|submit|post|publish|reply|delete|remove|erase|discard|empty|purchase|buy|order|checkout|pay|transfer|subscribe|sign|agree|accept|confirm|install|uninstall|format|reset|share|invite|merge|deploy|overwrite|replace all";

#[rustfmt::skip]
const NAME_EXTRA: &str = "save|upload|execute|apply|finish|allow|approve|authori[sz]e|grant|unlock|run|move|place order|pay now";

struct Patterns {
    verbs: Regex,
    name_verbs: Regex,
    dialog_sentence: Regex,
    compose: Regex,
    url_ok: Regex,
    app_name: Regex,
    terminals: Regex,
    browsers: Regex,
    mail_chat: Regex,
    plain_editors: Regex,
    address_bar: Regex,
    risky_page: Regex,
    camel: Regex,
    joins: Regex,
    space: Regex,
    function_key: Regex,
    run_title: Regex,
    role_button: Regex,
    role_document: Regex,
    has_path: Regex,
}

#[rustfmt::skip]
fn pats() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| {
        let name_list = format!("{VERB_LIST}|{NAME_EXTRA}");
        Patterns {
            verbs: words(VERB_LIST),
            name_verbs: words(&name_list),
            dialog_sentence: re(&format!(r"(?i)[^.?!]*(?-u:\b)(?:{name_list})(?-u:\b)[^.?!]*")),
            compose: words("message|compose|comment|reply|chat|post|email|mail|tweet|note to"),
            url_ok: re(r"(?i)^(https?://|mailto:)"),
            app_name: re(r"^[\p{L}\p{N} ._-]{1,40}$"),
            terminals: re(r"^(cmd|powershell|pwsh|windowsterminal|wt|conhost|openconsole|mintty|bash|wsl|alacritty|wezterm-gui)$"),
            browsers: re(r"^(chrome|msedge|firefox|brave|opera|vivaldi|arc|iexplore)$"),
            mail_chat: re(r"^(outlook|olk|hxoutlook|thunderbird|teams|ms-teams|slack|discord|whatsapp|telegram|signal|zoom|skype|mailspring|element)$"),
            plain_editors: re(r"^(notepad|wordpad)$"),
            address_bar: re(r"(?i)address|enter address|search or (type|enter)|url|location bar|omnibox"),
            risky_page: re(r"(?i)checkout|payment|(?-u:\b)pay(?-u:\b)|billing|(?-u:\b)cart(?-u:\b)|(?-u:\b)order(?-u:\b)|purchase|transfer|wire|compose|new message|inbox|mail\.google|outlook\.(live|office)|sign ?in|log ?in|password"),
            camel: re(r"([a-z])([A-Z])"),
            joins: re(r"[_-]+"),
            space: re(r"[\s\x{FEFF}]+"),
            function_key: re(r"^f[0-9]{1,2}$"),
            run_title: re(r"(?i)^run$"),
            role_button: re(r"button|menuitem|link"),
            role_document: re(r"document|edit"),
            has_path: re(r"[:\\/]"),
        }
    })
}

pub fn url_ok(t: &str) -> bool {
    pats().url_ok.is_match(t)
}

fn js_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

/// `String(v || '')`.
pub fn js_string(v: &Value) -> String {
    match v {
        Value::Null | Value::Bool(false) => String::new(),
        Value::String(s) => s.clone(),
        Value::Number(n) if n.as_f64() == Some(0.0) => String::new(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 9e15 => (f as i64).to_string(),
            _ => n.to_string(),
        },
        Value::Bool(true) => "true".into(),
        Value::Array(a) => a.iter().map(js_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// `"text"`, white space collapsed, cut to `n` UTF-16 units with an ellipsis.
pub fn quote(s: &str, n: usize) -> String {
    let collapsed = pats().space.replace_all(s, " ");
    let t = collapsed.trim_matches(js_space);
    let units: Vec<u16> = t.encode_utf16().collect();
    if units.len() > n {
        let head = String::from_utf16_lossy(&units[..n.saturating_sub(1)]);
        format!("\"{head}\u{2026}\"")
    } else {
        format!("\"{t}\"")
    }
}

fn upper_first(k: &str) -> Option<String> {
    let mut chars = k.chars();
    let first = chars.next()?;
    if k.encode_utf16().count() == 1 {
        return Some(k.to_uppercase());
    }
    Some(format!("{}{}", first.to_uppercase(), chars.as_str()))
}

/// "ctrl+shift+s tab" as "Ctrl+Shift+S Tab". `None` where agent.js threw
/// (an empty part, as in "ctrl++").
pub fn chord_label(keys: &str) -> Option<String> {
    let mut out = Vec::new();
    for chord in keys.split(js_space).filter(|c| !c.is_empty()) {
        let parts: Option<Vec<String>> = chord.split('+').map(upper_first).collect();
        out.push(parts?.join("+"));
    }
    Some(out.join(" "))
}

fn canon_key(k: &str) -> String {
    let lower = k.trim_matches(js_space).to_lowercase();
    let alias = match lower.as_str() {
        "return" => "enter",
        "esc" => "escape",
        "del" => "delete",
        "ins" => "insert",
        "pgup" | "page_up" | "prior" => "pageup",
        "pgdn" | "page_down" | "next" => "pagedown",
        "back" => "backspace",
        "control" => "ctrl",
        "option" => "alt",
        "cmd" | "meta" | "super" | "windows" => "win",
        "arrowleft" => "left",
        "arrowright" => "right",
        "arrowup" => "up",
        "arrowdown" => "down",
        "apps" => "menu",
        _ => return lower,
    };
    alias.to_string()
}

/// One chord: its modifiers (ctrl, alt, shift, win order), its key, and
/// its canonical text (win, ctrl, alt, shift order).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chord {
    pub mods: Vec<String>,
    pub key: String,
    pub text: String,
}

const MODS: [&str; 4] = ["ctrl", "alt", "shift", "win"];
const TEXT_ORDER: [&str; 4] = ["win", "ctrl", "alt", "shift"];

fn rank(order: &[&str], m: &str) -> usize {
    order.iter().position(|o| *o == m).unwrap_or(usize::MAX)
}

fn plus_or_key(x: &str) -> String {
    if x.is_empty() {
        "+".to_string()
    } else {
        canon_key(x)
    }
}

/// "ctrl+Return tab" as two chords, ctrl+enter and tab.
pub fn chords(keys: &str) -> Vec<Chord> {
    let mut out = Vec::new();
    let trimmed = keys.trim_matches(js_space);
    for c in trimmed.split(js_space).filter(|c| !c.is_empty()) {
        let raw: Vec<&str> = c.split('+').collect();
        let last = raw.len() - 1;
        let parts: Vec<String> = raw
            .iter()
            .enumerate()
            .filter(|(i, x)| !x.is_empty() || *i == last)
            .map(|(_, x)| plus_or_key(x))
            .collect();
        let key = parts.last().cloned().unwrap_or_default();
        let mut mods: Vec<String> = Vec::new();
        for m in &parts[..parts.len() - 1] {
            if MODS.contains(&m.as_str()) && !mods.contains(m) {
                mods.push(m.clone());
            }
        }
        mods.sort_by_key(|m| rank(&MODS, m));
        let mut ordered = mods.clone();
        ordered.sort_by_key(|m| rank(&TEXT_ORDER, m));
        ordered.push(key.clone());
        let text = ordered.join("+");
        out.push(Chord { mods, key, text });
    }
    out
}

fn hard_chord(text: &str) -> Option<&'static str> {
    Some(match text {
        "win+r" => "opens the Run box, which runs a command",
        "win+x" => "opens the power-user menu",
        "ctrl+shift+escape" => "opens Task Manager",
        "ctrl+alt+delete" => "opens the security screen",
        "alt+f4" => "closes the window",
        "ctrl+w" => "closes a tab or window",
        "ctrl+q" => "quits the app",
        "ctrl+shift+delete" => "clears browsing data",
        "shift+delete" => "deletes permanently",
        _ => return None,
    })
}

/// Keys that only move, select, copy or undo.
const FREE_KEYS: [&str; 15] = [
    "left",
    "right",
    "up",
    "down",
    "home",
    "end",
    "pageup",
    "pagedown",
    "tab",
    "space",
    "backspace",
    "escape",
    "capslock",
    "shift",
    "menu",
];

#[rustfmt::skip]
const FREE_CTRL: [&str; 14] = [
    "c", "x", "v", "a", "z", "y", "f", "home", "end", "left", "right", "up", "down", "backspace",
];

/// Why one chord needs a yes, or "" when it is on the free list. `None`
/// where agent.js threw, which the gate turns into a question.
pub fn chord_risk(ch: &Chord) -> Option<String> {
    let label = |t: &str| chord_label(t);
    if let Some(hard) = hard_chord(&ch.text) {
        return Some(format!("{} {hard}", label(&ch.text)?));
    }
    let has = |m: &str| ch.mods.iter().any(|x| x == m);
    let key = ch.key.as_str();
    if key == "enter" {
        return Some("Enter can send, submit or run".into());
    }
    if key == "delete" {
        return Some("Delete removes things".into());
    }
    if pats().function_key.is_match(key) {
        return Some(format!("{} is a command key", label(key)?));
    }
    if has("win") {
        return Some(format!("{} is a Windows shortcut", label(&ch.text)?));
    }
    if has("alt") {
        return Some(format!("{} is an app shortcut", label(&ch.text)?));
    }
    if has("ctrl") {
        if FREE_CTRL.contains(&key) {
            return Some(String::new());
        }
        return Some(format!("{} is an app shortcut", label(&ch.text)?));
    }
    if FREE_KEYS.contains(&key) || key.encode_utf16().count() == 1 {
        return Some(String::new());
    }
    Some(format!("{} is not a plain typing key", label(&ch.text)?))
}

/// What the gate decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Read,
    Input,
    Consequential,
    Refused,
}

impl Class {
    pub fn as_str(self) -> &'static str {
        match self {
            Class::Read => "read",
            Class::Input => "input",
            Class::Consequential => "consequential",
            Class::Refused => "refused",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub cls: Class,
    pub why: String,
}

impl Verdict {
    fn new(cls: Class, why: impl Into<String>) -> Verdict {
        Verdict {
            cls,
            why: why.into(),
        }
    }
}

/// Where a drag ends.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DragEnd {
    pub node: Option<Value>,
    pub app: String,
}

/// What a call will touch (`inspect()` in agent.js).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Touches {
    pub node: Option<Value>,
    pub window_handle: Value,
    pub app: String,
    pub error: bool,
    pub end: Option<DragEnd>,
}

/// What the gate reads besides the call.
pub struct GateEnv<'a> {
    pub desktop: &'a dyn DesktopOps,
    /// `permission: 'every'` asks for every action.
    pub every: bool,
    pub target: Value,
    pub plan_gate: bool,
    pub plan_approved: bool,
}

fn states_of(n: &Value) -> Vec<&str> {
    let list = n["states"].as_array();
    let list = list.map(Vec::as_slice).unwrap_or_default();
    list.iter().filter_map(Value::as_str).collect()
}

fn has_state(n: &Value, state: &str) -> bool {
    states_of(n).contains(&state)
}

fn name_and_id(n: &Value) -> String {
    let (name, id) = (js_string(&n["name"]), js_string(&n["automationId"]));
    format!("{name} {id}")
}

fn proc_of(w: Option<&Value>) -> String {
    let p = w.map(|w| js_string(&w["process"])).unwrap_or_default();
    let p = p.to_lowercase();
    p.strip_suffix(".exe").map(str::to_string).unwrap_or(p)
}

const ONLY_LINKS: &str = "only http, https and mailto links or an app name can be opened, not";
const RUNS_TYPED: &str = "is a command line, and Enter runs what is typed";
const UNSAVED: &str = "unsaved work there could be lost";

/// The safety class for one call: read, input, consequential, or refused.
pub async fn gate(name: &str, args: &Value, info: &Touches, env: &GateEnv<'_>) -> Verdict {
    match gate_inner(name, args, info, env).await {
        Some(v) => v,
        // A check that broke is a check that did not pass.
        None => {
            let why = "the safety check could not finish, so it asks";
            Verdict::new(Class::Consequential, why)
        }
    }
}

struct Why(Vec<String>);

impl Why {
    fn add(&mut self, w: impl Into<String>) {
        let w = w.into();
        if !w.is_empty() && !self.0.contains(&w) {
            self.0.push(w);
        }
    }
}

struct Lazy<'a> {
    desktop: &'a dyn DesktopOps,
    handle: Value,
    tree: Option<Option<Value>>,
}

impl Lazy<'_> {
    /// The tree, once, for the focused field, dialog text and password
    /// fields; `None` when it could not be read.
    async fn tree(&mut self) -> Option<&Value> {
        if self.tree.is_none() {
            let args = json!({ "windowHandle": self.handle, "filter": "all" });
            self.tree = Some(self.desktop.op("readTree", args).await.ok());
        }
        self.tree.as_ref().and_then(Option::as_ref)
    }
}

fn nodes(t: &Value) -> &[Value] {
    t["nodes"].as_array().map(Vec::as_slice).unwrap_or_default()
}

async fn gate_inner(
    name: &str,
    a: &Value,
    info: &Touches,
    env: &GateEnv<'_>,
) -> Option<Verdict> {
    let p = pats();
    if name == "clipboard_read" {
        let why = "it reads your clipboard, which can hold a password";
        return Some(Verdict::new(Class::Consequential, why));
    }
    if is_read_tool(name) || name == "focus_window" || name == "scroll_to" {
        return Some(Verdict::new(Class::Read, ""));
    }
    if env.plan_gate && !env.plan_approved {
        let why = "this task waits for you to approve its plan; call update_plan first";
        return Some(Verdict::new(Class::Refused, why));
    }
    if name == "open" {
        let target = js_string(&a["target"]);
        let t = target.trim_matches(js_space);
        let link = p.url_ok.is_match(t);
        if link || (!p.has_path.is_match(t) && p.app_name.is_match(t)) {
            let why = if link { "it opens a link" } else { "it starts an app" };
            return Some(Verdict::new(Class::Consequential, why));
        }
        let q = quote(t, 30);
        let why = format!("{ONLY_LINKS} {q}");
        return Some(Verdict::new(Class::Refused, why));
    }
    let mut why = Why(Vec::new());

    // Where it lands: the window, its app, its class.
    let handle = match &info.window_handle {
        Value::Null => env.target.clone(),
        h => h.clone(),
    };
    let windows = if handle.is_null() {
        Vec::new()
    } else {
        let list = json!({ "includeMinimized": true });
        let got = env.desktop.op("listWindows", list).await.ok();
        got.and_then(|v| v.as_array().cloned()).unwrap_or_default()
    };
    let known = !handle.is_null();
    let win = windows.iter().find(|x| known && x["handle"] == handle);
    let proc = proc_of(win);
    let app_name = if !info.app.is_empty() {
        info.app.clone()
    } else if !proc.is_empty() {
        proc.clone()
    } else {
        "the app".to_string()
    };
    let title = win.map(|w| js_string(&w["title"])).unwrap_or_default();
    let is_dialog = win.is_some_and(|w| w["className"] == "#32770");
    let is_run = is_dialog && p.run_title.is_match(title.trim_matches(js_space));
    let mut lazy = Lazy {
        desktop: env.desktop,
        handle: handle.clone(),
        tree: None,
    };

    let node = info.node.as_ref();
    let node_name = node.map(|n| js_string(&n["name"])).unwrap_or_default();
    // sendButton, btn_delete: ids are split into words before matching.
    let extra = match node {
        Some(n) => {
            let (id, help) = (js_string(&n["automationId"]), js_string(&n["helpText"]));
            let raw = format!("{id} {help}");
            let spaced = p.camel.replace_all(&raw, "$1 $2");
            p.joins.replace_all(&spaced, " ").into_owned()
        }
        None => String::new(),
    };
    if !node_name.is_empty() && p.name_verbs.is_match(&node_name) {
        why.add(format!("the control is named {}", quote(&node_name, 40)));
    } else if p.name_verbs.is_match(&extra) {
        why.add("the control is labelled as one that sends or deletes");
    }
    let reason = js_string(&a["reason"]);
    if let Some(m) = p.verbs.find(&reason) {
        let verb = m.as_str().to_lowercase();
        why.add(format!("the step says it will {verb}"));
    }
    if a["consequential"] == true {
        why.add("the agent says this could send or delete something");
    }

    // A dialog asking to delete or send: its title or its own text.
    if is_dialog && ["left_click", "double_click", "key"].contains(&name) {
        if p.name_verbs.is_match(&title) {
            why.add(format!("the dialog is titled {}", quote(&title, 40)));
        } else {
            match lazy.tree().await {
                Some(t) => {
                    let texts: Vec<String> = nodes(t)
                        .iter()
                        .filter(|n| n["role"] == "text" || n["role"] == "static")
                        .map(|n| js_string(&n["name"]))
                        .collect();
                    let body = texts.join(" ");
                    if p.name_verbs.is_match(&body) {
                        let hit = p.dialog_sentence.find(&body);
                        let said = hit.map_or(&body[..], |m| m.as_str());
                        why.add(format!("the dialog says {}", quote(said, 60)));
                    }
                }
                None => why.add("it could not read the dialog"),
            }
        }
    }

    // The page or app it is in.
    if p.browsers.is_match(&proc) && p.risky_page.is_match(&title) {
        why.add(format!("the page is {}", quote(&title, 50)));
    }
    let role = node.map(|n| js_string(&n["role"])).unwrap_or_default();
    let clicky = node.is_none() || p.role_button.is_match(&role);
    if p.mail_chat.is_match(&proc) && name.contains("click") && clicky {
        why.add(format!("a button in {app_name} can send"));
    }
    let typing = ["type", "key", "form_input"].contains(&name);
    if p.terminals.is_match(&proc) && typing {
        why.add(format!("{app_name} {RUNS_TYPED}"));
    }
    if is_run {
        why.add("the Run box runs what is typed in it");
    }

    if name == "left_click_drag" {
        let end = info.end.clone().unwrap_or_default();
        let into = if end.app.is_empty() {
            String::new()
        } else {
            format!(" into {}", end.app)
        };
        let end_name = match &end.node {
            Some(n) => js_string(&n["name"]),
            None => String::new(),
        };
        let named = if end_name.is_empty() {
            String::new()
        } else {
            format!(" ({})", quote(&end_name, 40))
        };
        why.add(format!("dragging moves things{into}{named}"));
    }

    let lands = !["scroll", "hover", "left_click_drag"].contains(&name);
    if is_pointer_tool(name) && lands {
        let blank = |k: &str| match node {
            Some(n) => js_string(&n[k]).trim().is_empty(),
            None => true,
        };
        if info.error || node.is_none() {
            why.add("it could not read what this lands on");
        } else if blank("name") && blank("automationId") {
            why.add("the target has no name to check");
        }
    }

    if name == "key" {
        let keys = match js_string(&a["text"]) {
            k if k.is_empty() => js_string(&a["keys"]),
            k => k,
        };
        let list = chords(&keys);
        if list.is_empty() {
            why.add("no keys were named");
        }
        for ch in &list {
            why.add(chord_risk(ch)?);
        }
        let rep = repeat_of(&a["repeat"]);
        if !why.0.is_empty() && rep > 1 {
            why.add(format!("repeated {rep} times"));
        }
        if list.iter().any(|ch| ch.key == "enter") {
            let browser = p.browsers.is_match(&proc);
            let focused = lazy.tree().await.and_then(|t| {
                let hit = nodes(t).iter().find(|n| has_state(n, "focused"));
                hit.cloned()
            });
            if let Some(f) = focused {
                let label = name_and_id(&f);
                if p.address_bar.is_match(&label) && browser {
                    why.add("Enter in the address bar goes to a new page");
                } else if p.compose.is_match(&label) {
                    why.add("Enter sends what is in a message field");
                }
            }
        }
    }

    if name == "type" {
        let text = js_string(&a["text"]);
        if text.contains(['\r', '\n']) {
            why.add("the text has a line break, which presses Enter");
        } else if text.contains('\t') {
            why.add("the text has a tab, which moves to another field");
        }
        let tree = lazy.tree().await.cloned();
        let mut target = node.cloned();
        if target.is_none() {
            if let Some(t) = &tree {
                target = nodes(t).iter().find(|n| has_state(n, "focused")).cloned();
            }
        }
        let any_password = match &tree {
            Some(t) => nodes(t).iter().any(|n| has_state(n, "password")),
            None => true,
        };
        match &target {
            Some(n) if has_state(n, "password") => why.add("it types into a password field"),
            None if any_password => why.add(
                "it could not tell which field has focus, and this window has a password field",
            ),
            _ => {}
        }
        if let Some(n) = &target {
            if p.browsers.is_match(&proc) && p.address_bar.is_match(&name_and_id(n)) {
                why.add("it types into the address bar");
            }
        }
        // Notepad's own document is the one place a newline is only a newline.
        let only_break = why.0.len() == 1 && why.0[0].contains("line break");
        let doc = target.as_ref().is_some_and(|n| {
            let role = js_string(&n["role"]);
            p.role_document.is_match(&role)
        });
        if only_break && p.plain_editors.is_match(&proc) && doc {
            why.0.clear();
        }
    }

    let password = node.is_some_and(|n| has_state(n, "password"));
    if name == "form_input" && password {
        why.add("it fills a password field");
    }
    if name == "close_window" {
        why.add(format!("it closes {app_name}; {UNSAVED}"));
    }

    if env.every {
        why.add("you asked to approve every action");
    }
    if why.0.is_empty() {
        return Some(Verdict::new(Class::Input, ""));
    }
    Some(Verdict::new(Class::Consequential, why.0.join("; ")))
}

/// `Math.round(Number(a.repeat) || 1)`.
pub fn repeat_of(v: &Value) -> i64 {
    let n = crate::llm::catalogue::js_number(v);
    let n = if n.is_nan() || n == 0.0 { 1.0 } else { n };
    (n + 0.5).floor() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::seams::testing::FakeDesktop;

    fn block<T>(f: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(f)
    }

    #[test]
    fn chords_are_canonical() {
        let c = chords("Ctrl+Return  shift+ctrl+S tab ctrl++");
        let texts: Vec<&str> = c.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(texts, ["ctrl+enter", "ctrl+shift+s", "tab", "ctrl++"]);
        assert_eq!(c[1].mods, ["ctrl", "shift"]);
        assert_eq!(chords("win+ctrl+d")[0].text, "win+ctrl+d");
        assert_eq!(chord_label("ctrl+shift+s tab").unwrap(), "Ctrl+Shift+S Tab");
        assert_eq!(chord_label("ctrl++"), None);
    }

    #[test]
    fn chord_risks() {
        let risk = |k: &str| chord_risk(&chords(k)[0]);
        let run = Some("Win+R opens the Run box, which runs a command".to_string());
        assert_eq!(risk("win+r"), run);
        assert_eq!(risk("enter").unwrap(), "Enter can send, submit or run");
        assert_eq!(risk("f5").unwrap(), "F5 is a command key");
        assert_eq!(risk("ctrl+c").unwrap(), "");
        assert_eq!(risk("ctrl+s").unwrap(), "Ctrl+S is an app shortcut");
        assert_eq!(risk("a").unwrap(), "");
        assert_eq!(risk("pageup").unwrap(), "");
        assert_eq!(risk("insert").unwrap(), "Insert is not a plain typing key");
        assert_eq!(risk("ctrl++"), None);
    }

    #[test]
    fn quotes_are_cut() {
        assert_eq!(quote("  a\n b ", 40), "\"a b\"");
        assert_eq!(quote("abcdef", 4), "\"abc\u{2026}\"");
    }

    fn env(desktop: &FakeDesktop) -> GateEnv<'_> {
        GateEnv {
            desktop,
            every: false,
            target: json!(7),
            plan_gate: false,
            plan_approved: false,
        }
    }

    fn named(name: &str, role: &str) -> Touches {
        Touches {
            node: Some(json!({ "name": name, "role": role })),
            window_handle: json!(7),
            app: "Notepad".into(),
            ..Touches::default()
        }
    }

    fn notepad() -> FakeDesktop {
        let d = FakeDesktop::default();
        let win = json!([{ "handle": 7, "process": "notepad.exe", "title": "notes.txt" }]);
        d.reply("listWindows", win);
        let focused = json!({ "role": "document", "name": "Text editor", "states": ["focused"] });
        d.reply("readTree", json!({ "nodes": [focused] }));
        d
    }

    #[test]
    fn harmless_steps_pass_and_risky_ones_ask() {
        let d = notepad();
        let e = env(&d);
        let none = json!({});
        let ok = block(gate("left_click", &none, &named("Format", "menuitem"), &e));
        assert_eq!(ok.cls, Class::Consequential);
        assert_eq!(ok.why, "the control is named \"Format\"");
        let ok = block(gate("left_click", &none, &named("Edit", "menuitem"), &e));
        assert_eq!(ok, Verdict::new(Class::Input, ""));
        let read = block(gate("read_page", &json!({}), &Touches::default(), &e));
        assert_eq!(read.cls, Class::Read);
        let blind = block(gate("left_click", &json!({}), &Touches::default(), &e));
        assert_eq!(blind.why, "it could not read what this lands on");
        let typed = json!({ "text": "line one\nline two" });
        let v = block(gate("type", &typed, &Touches::default(), &e));
        // A newline in Notepad's own document is only a newline.
        assert_eq!(v.cls, Class::Input);
        let send = json!({ "text": "x", "reason": "Send the reply" });
        let v = block(gate("type", &send, &Touches::default(), &e));
        assert_eq!(v.why, "the step says it will send");
    }

    #[test]
    fn open_and_plans_and_keys() {
        let d = notepad();
        let mut e = env(&d);
        let link = json!({ "target": "https://example.com" });
        let v = block(gate("open", &link, &Touches::default(), &e));
        assert_eq!(v, Verdict::new(Class::Consequential, "it opens a link"));
        let path = json!({ "target": "C:\\Windows\\system32\\cmd.exe" });
        let v = block(gate("open", &path, &Touches::default(), &e));
        assert_eq!(v.cls, Class::Refused);
        let keys = json!({ "text": "ctrl+s", "repeat": 3 });
        let v = block(gate("key", &keys, &Touches::default(), &e));
        assert_eq!(v.why, "Ctrl+S is an app shortcut; repeated 3 times");
        let broken = json!({ "text": "ctrl++" });
        let v = block(gate("key", &broken, &Touches::default(), &e));
        assert_eq!(v.why, "the safety check could not finish, so it asks");
        e.plan_gate = true;
        let v = block(gate("left_click", &json!({}), &Touches::default(), &e));
        assert_eq!(v.cls, Class::Refused);
        e.plan_gate = false;
        e.every = true;
        let none = json!({});
        let v = block(gate("left_click", &none, &named("Edit", "menuitem"), &e));
        assert_eq!(v.why, "you asked to approve every action");
    }
}
