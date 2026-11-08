//! The executor's safety gate (PARITY AGENT-05..09). Port of `gate`,
//! `chords`, `chordRisk`, `quote` and `chordLabel` in lib/agent.js, made
//! stricter by B-008 and B-049 (decision D-27).
//!
//! Reads, `focus_window` and `scroll_to` run without a card. Every other
//! step runs without one only when no check below finds a reason to ask:
//!
//! - a click (left, double, triple) must land on something the executor can
//!   read and name, whose role is on the short harmless list (menus, tabs,
//!   tree items, scroll bars, text fields, documents, cells, headers) or
//!   whose name is (Zoom in, Bold, Settings and a few more); anything else
//!   it lands on asks. A right click only opens a menu, so a readable, named
//!   target is enough. A double click in File Explorer or on the desktop
//!   always asks: it opens or runs the file.
//! - a key chord must only move, select, copy or undo. Space and Enter press
//!   the control with keyboard focus: Space is free only in a text field or
//!   on a control that would pass as a click target, Enter always asks.
//! - typed text asks when it presses Enter or Tab, lands in a password field
//!   or the address bar, or holds a space while the focus is on a control
//!   that would not pass as a click target.
//! - names, ids and help text that send, delete, buy, pay, post or remove
//!   (in English and the shipped UI languages), the step's own reason, the
//!   model's consequential flag, a dialog whose title or text says so (a
//!   Win32 dialog, or a `dialog` node or nested window in the tree), a risky
//!   page, a mail or chat app, a terminal, the Run box, a drag, closing a
//!   window, and the person's "approve every action" all ask.
//!
//! Anything the gate cannot read asks: an unreadable or unnamed target,
//! unknown keyboard focus under a Space or a typed space, a check that
//! failed. Names are chosen by whoever built the page, so a name alone never
//! clears a step another check flags. `open` takes only links and app
//! names, and a plan the person must approve refuses every action until it
//! is approved.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::{json, Value};

use super::seams::DesktopOps;
use super::tools::{is_pointer_tool, is_read_tool};

/// Any alternative as a whole word, case-blind, with Unicode word
/// boundaries (so "Löschen" matches and "payé" does not match "pay"). The
/// `cjk` alternatives match anywhere: those scripts put no spaces between
/// words.
fn words(alternatives: &str, cjk: &str) -> Regex {
    let pattern = if cjk.is_empty() {
        format!(r"(?i)\b(?:{alternatives})\b")
    } else {
        format!(r"(?i)\b(?:{alternatives})\b|(?:{cjk})")
    };
    Regex::new(&pattern).expect("gate pattern")
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("gate pattern")
}

#[rustfmt::skip]
const VERB_LIST: &str = "send|submit|post|publish|reply|delete|remove|erase|discard|empty|purchase|buy|order|checkout|pay|transfer|subscribe|sign|agree|accept|confirm|install|uninstall|format|reset|share|invite|merge|deploy|overwrite|replace all|shut ?down|restart|trash|clear all|clear history|unsubscribe|cancel subscription|deactivate|close account|end task";

#[rustfmt::skip]
const NAME_EXTRA: &str = "save|upload|execute|apply|finish|allow|approve|authori[sz]e|grant|unlock|run|move|place order|pay now";

/// Send, delete, remove, buy, pay and post in the shipped UI languages that
/// use spaces: German, French, Spanish, Italian, Portuguese.
#[rustfmt::skip]
const LOCAL_VERBS: &str = "senden|absenden|versenden|abschicken|löschen|entfernen|kaufen|bezahlen|zahlen|posten|veröffentlichen|envoyer|supprimer|retirer|effacer|acheter|payer|publier|enviar|eliminar|borrar|quitar|comprar|pagar|publicar|invia|inviare|elimina|eliminare|cancella|rimuovi|acquista|compra|paga|pubblica|excluir|apagar|remover|postar";

/// The same in Japanese and Chinese (simplified and traditional).
#[rustfmt::skip]
const CJK_VERBS: &str = "送信|削除|購入|支払|投稿|消去|发送|發送|删除|刪除|购买|購買|支付|付款|发布|發佈|發布|发表|發表|移除";

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
        let name_list = format!("{VERB_LIST}|{NAME_EXTRA}|{LOCAL_VERBS}");
        let verb_list = format!("{VERB_LIST}|{LOCAL_VERBS}");
        Patterns {
            verbs: words(&verb_list, CJK_VERBS),
            name_verbs: words(&name_list, CJK_VERBS),
            dialog_sentence: re(&format!(r"(?i)[^.?!。？！]*(?:\b(?:{name_list})\b|{CJK_VERBS})[^.?!。？！]*")),
            compose: words("message|compose|comment|reply|chat|post|email|mail|tweet|note to", ""),
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

/// Keys that only move, select, copy or undo. Space is not one: it presses
/// the control with keyboard focus (B-008).
const FREE_KEYS: [&str; 14] = [
    "left",
    "right",
    "up",
    "down",
    "home",
    "end",
    "pageup",
    "pagedown",
    "tab",
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
    if key == "space" {
        return Some(format!("{} presses the focused control", label(&ch.text)?));
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
    // For logs and the smoke harness.
    #[allow(dead_code)]
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
    /// The control with keyboard focus, for typing or a Space or Enter
    /// without a ref (B-008); `None` when it could not be told.
    pub focused: Option<Value>,
    /// The window's tree (filter `all`) when `inspect` already read it.
    pub tree: Option<Value>,
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

    fn loaded(&self) -> bool {
        matches!(self.tree, Some(Some(_)))
    }
}

fn nodes(t: &Value) -> &[Value] {
    t["nodes"].as_array().map(Vec::as_slice).unwrap_or_default()
}

/// A `key` call's keys: `text`, or `keys` as older models send them.
pub fn key_text(a: &Value) -> String {
    match js_string(&a["text"]) {
        k if k.is_empty() => js_string(&a["keys"]),
        k => k,
    }
}

fn has_press(list: &[Chord]) -> bool {
    let press = |ch: &Chord| ch.key == "space" || ch.key == "enter";
    list.iter().any(press)
}

/// Whether a call lands on whatever has keyboard focus (B-008): typing, or
/// keys with a Space or an Enter in them.
pub fn presses_focus(name: &str, a: &Value) -> bool {
    match name {
        "type" => true,
        "key" => has_press(&chords(&key_text(a))),
        _ => false,
    }
}

/// The node with keyboard focus in a tree from `readTree`.
pub fn focused_in(t: &Value) -> Option<Value> {
    nodes(t).iter().find(|n| has_state(n, "focused")).cloned()
}

/// sendButton, btn_delete: an element's id and help text, split into words.
fn id_words(n: &Value) -> String {
    let p = pats();
    let raw = format!("{} {}", js_string(&n["automationId"]), js_string(&n["helpText"]));
    let spaced = p.camel.replace_all(&raw, "$1 $2");
    p.joins.replace_all(&spaced, " ").into_owned()
}

/// Roles where a Space is only a space: text fields and documents.
fn editable(n: &Value) -> bool {
    let role = js_string(&n["role"]);
    ["edit", "document", "combobox", "spinner"].contains(&role.as_str())
}

/// What a click on these does is select, focus, open a menu, scroll or sort
/// (B-049). Menu items still ask when their name says they send or delete.
#[rustfmt::skip]
const HARMLESS_ROLES: [&str; 15] = [
    "menubar", "menu", "menuitem", "tab", "tabitem", "treeitem", "scrollbar", "thumb", "edit",
    "document", "combobox", "spinner", "dataitem", "headeritem", "titlebar",
];

/// Controls that only change the view, by their whole name.
#[rustfmt::skip]
const HARMLESS_NAMES: [&str; 22] = [
    "back", "bold", "collapse", "expand", "find", "forward", "italic", "maximize", "menu",
    "minimize", "more actions", "more options", "options", "restore", "restore down", "search",
    "see more", "settings", "show more", "underline", "zoom in", "zoom out",
];

fn harmless(n: &Value) -> bool {
    let role = js_string(&n["role"]);
    if HARMLESS_ROLES.contains(&role.as_str()) {
        return true;
    }
    let name = js_string(&n["name"]);
    let name = pats().space.replace_all(name.trim(), " ").to_lowercase();
    HARMLESS_NAMES.contains(&name.as_str())
}

/// `button "Continue"`, for a card.
fn what(n: &Value) -> String {
    let role = js_string(&n["role"]);
    let kind = match role.as_str() {
        "hyperlink" => "link",
        "listitem" => "list item",
        "radiobutton" => "radio button",
        "splitbutton" => "button",
        "" => "control",
        other => other,
    };
    let name = js_string(&n["name"]);
    let label = if name.trim().is_empty() {
        js_string(&n["automationId"])
    } else {
        name
    };
    format!("{kind} {}", quote(&label, 40))
}

const FOCUS_UNKNOWN: &str = "it could not tell which control has keyboard focus";
const FOCUS_MOVES: &str = "an earlier key can move the focus first";
const TYPED_SPACE: &str = "a typed space presses the focused control";

/// The checks a click gets, run on the control a Space or a typed space
/// would press (B-008). Empty when it would pass as a click target.
fn focus_reasons(f: &Value, proc: &str, app: &str) -> Vec<String> {
    let p = pats();
    let mut out = Vec::new();
    let name = js_string(&f["name"]);
    if !name.is_empty() && p.name_verbs.is_match(&name) {
        out.push(format!("the focused control is named {}", quote(&name, 40)));
    } else if p.name_verbs.is_match(&id_words(f)) {
        out.push("the focused control is labelled as one that sends or deletes".to_string());
    }
    let unnamed = name.trim().is_empty() && js_string(&f["automationId"]).trim().is_empty();
    if unnamed {
        out.push("the focused control has no name to check".to_string());
    }
    let role = js_string(&f["role"]);
    if p.mail_chat.is_match(proc) && p.role_button.is_match(&role) {
        out.push(format!("a button in {app} can send"));
    }
    if out.is_empty() && !harmless(f) {
        out.push(format!("it is not known what the focused {} does", what(f)));
    }
    out
}

/// Whether the keyboard focus stays where it was after `ch`. In a text
/// field, characters and caret keys keep it; anywhere else a letter can
/// jump a list's selection, so only Space and Shift keep it.
fn keeps_focus(ch: &Chord, in_text: bool) -> bool {
    if ch.mods.iter().any(|m| m != "shift") {
        return false;
    }
    let k = ch.key.as_str();
    if ["space", "shift", "capslock"].contains(&k) {
        return true;
    }
    let caret = [
        "left",
        "right",
        "up",
        "down",
        "home",
        "end",
        "pageup",
        "pagedown",
        "backspace",
        "delete",
    ];
    in_text && (k.encode_utf16().count() == 1 || caret.contains(&k))
}

/// The dialog drawn around `target` in the tree, as its title and its own
/// text: the nearest ancestor whose role is `dialog`, or a window nested in
/// the window (UI Automation puts an app's modal window under its owner).
fn dialog_around(t: &Value, target: &Value) -> Option<(String, String)> {
    let all = nodes(t);
    let by_ref: HashMap<&str, &Value> = all
        .iter()
        .filter_map(|n| Some((n["ref"].as_str()?, n)))
        .collect();
    let parent = |n: &Value| {
        let r = n["parentRef"].as_str()?;
        by_ref.get(r).copied()
    };
    let start = target["ref"].as_str().and_then(|r| by_ref.get(r).copied());
    let mut up = parent(start.unwrap_or(target));
    let mut dialog = None;
    for _ in 0..64 {
        let Some(n) = up else { break };
        let nested = n["role"] == "window" && n["depth"].as_i64().unwrap_or(0) > 0;
        if n["role"] == "dialog" || nested {
            dialog = Some(n);
            break;
        }
        up = parent(n);
    }
    let id = dialog?["ref"].as_str()?;
    let inside = |n: &Value| {
        let mut up = parent(n);
        for _ in 0..64 {
            match up {
                Some(p) if p["ref"].as_str() == Some(id) => return true,
                Some(p) => up = parent(p),
                None => return false,
            }
        }
        false
    };
    let texts: Vec<String> = all
        .iter()
        .filter(|n| (n["role"] == "text" || n["role"] == "static") && inside(n))
        .map(|n| js_string(&n["name"]))
        .collect();
    Some((js_string(&dialog?["name"]), texts.join(" ")))
}

async fn gate_inner(name: &str, a: &Value, info: &Touches, env: &GateEnv<'_>) -> Option<Verdict> {
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
            let why = if link {
                "it opens a link"
            } else {
                "it starts an app"
            };
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
        tree: info.tree.clone().map(Some),
    };

    let node = info.node.as_ref();
    let node_name = node.map(|n| js_string(&n["name"])).unwrap_or_default();
    // sendButton, btn_delete: ids are split into words before matching.
    let extra = node.map(id_words).unwrap_or_default();
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
        } else if name == "double_click" && proc == "explorer" {
            why.add("a double click in File Explorer or on the desktop opens or runs the file");
        } else if let Some(n) = node.filter(|n| why.0.is_empty() && !harmless(n)) {
            // The allowlist (B-049): a click nothing else flagged still asks
            // unless what it lands on is known to be harmless. A right click
            // only opens a menu.
            if name != "right_click" {
                why.add(format!("it is not known what {} does", what(n)));
            }
        }
    }

    // What a Space, an Enter or typed text lands on: the ref's element, else
    // the control with keyboard focus (B-008).
    let list = if name == "key" {
        chords(&key_text(a))
    } else {
        Vec::new()
    };
    let mut keyboard = None;
    if name == "type" || has_press(&list) {
        keyboard = node.cloned().or_else(|| info.focused.clone());
        if keyboard.is_none() {
            keyboard = lazy.tree().await.and_then(focused_in);
        }
    }

    if name == "key" {
        if list.is_empty() {
            why.add("no keys were named");
        }
        let rep = repeat_of(&a["repeat"]);
        // A repeat runs the keys again, so a Tab at the end moves the focus
        // before the next pass's Space.
        let passes = if rep > 1 { 2 } else { 1 };
        let in_text = keyboard.as_ref().is_some_and(editable);
        let mut moved = false;
        let mut space = None;
        for ch in list.iter().cycle().take(list.len() * passes) {
            let risk = chord_risk(ch)?;
            let plain_space = ch.key == "space" && ch.mods.iter().all(|m| m == "shift");
            if !plain_space {
                why.add(risk);
            } else if moved {
                why.add(risk);
                why.add(FOCUS_MOVES);
            } else if space.is_none() {
                space = Some(risk);
            }
            moved = moved || !keeps_focus(ch, in_text);
        }
        // Space is free in a text field; anywhere else it gets the checks a
        // click on the focused control would.
        if let Some(risk) = space {
            match &keyboard {
                Some(f) if editable(f) => {}
                Some(f) => {
                    let found = focus_reasons(f, &proc, &app_name);
                    if !found.is_empty() {
                        why.add(risk);
                    }
                    for r in found {
                        why.add(r);
                    }
                }
                None => {
                    why.add(risk);
                    why.add(FOCUS_UNKNOWN);
                }
            }
        }
        if !why.0.is_empty() && rep > 1 {
            why.add(format!("repeated {rep} times"));
        }
        if list.iter().any(|ch| ch.key == "enter") {
            if let Some(f) = &keyboard {
                let browser = p.browsers.is_match(&proc);
                let label = name_and_id(f);
                if p.address_bar.is_match(&label) && browser {
                    why.add("Enter in the address bar goes to a new page");
                } else if p.compose.is_match(&label) {
                    why.add("Enter sends what is in a message field");
                }
                if !editable(f) {
                    for r in focus_reasons(f, &proc, &app_name) {
                        why.add(r);
                    }
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
        let any_password = match lazy.tree().await {
            Some(t) => nodes(t).iter().any(|n| has_state(n, "password")),
            None => true,
        };
        let target = keyboard.as_ref();
        match target {
            Some(n) if has_state(n, "password") => why.add("it types into a password field"),
            None if any_password => why.add(
                "it could not tell which field has focus, and this window has a password field",
            ),
            _ => {}
        }
        if let Some(n) = target {
            if p.browsers.is_match(&proc) && p.address_bar.is_match(&name_and_id(n)) {
                why.add("it types into the address bar");
            }
        }
        // A typed space presses a focused button the way Space does.
        if text.contains(' ') {
            match target {
                Some(n) if editable(n) => {}
                Some(n) => {
                    let found = focus_reasons(n, &proc, &app_name);
                    if !found.is_empty() {
                        why.add(TYPED_SPACE);
                    }
                    for r in found {
                        why.add(r);
                    }
                }
                None => {
                    why.add(TYPED_SPACE);
                    why.add(FOCUS_UNKNOWN);
                }
            }
        }
        // Notepad's own document is the one place a newline is only a newline.
        let only_break = why.0.len() == 1 && why.0[0].contains("line break");
        let doc = target.is_some_and(|n| {
            let role = js_string(&n["role"]);
            p.role_document.is_match(&role)
        });
        if only_break && p.plain_editors.is_match(&proc) && doc {
            why.0.clear();
        }
    }

    // A dialog drawn inside the window (a web page's, an app's own) asking
    // to delete or send (B-049). Its text is read when the step asks anyway
    // or the tree is already here, so the card says what the dialog is.
    let pressed = match name {
        "key" | "type" => keyboard.as_ref(),
        "left_click" | "double_click" | "triple_click" => node,
        _ => None,
    };
    let look = !is_dialog && (!why.0.is_empty() || lazy.loaded());
    if let Some(target) = pressed.filter(|_| look) {
        let found = lazy.tree().await.and_then(|t| dialog_around(t, target));
        if let Some((title, body)) = found {
            if p.name_verbs.is_match(&title) {
                why.add(format!("the dialog is titled {}", quote(&title, 40)));
            } else if p.name_verbs.is_match(&body) {
                let hit = p.dialog_sentence.find(&body);
                let said = hit.map_or(&body[..], |m| m.as_str());
                why.add(format!("the dialog says {}", quote(said, 60)));
            }
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
        let press = "Space presses the focused control";
        assert_eq!(risk("space").unwrap(), press);
        assert_eq!(risk("shift+space").unwrap(), "Shift+Space presses the focused control");
        assert_eq!(risk("tab").unwrap(), "");
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

    /// Window 7 of `process`, whose tree is `nodes`.
    fn desk(process: &str, title: &str, class: &str, nodes: Value) -> FakeDesktop {
        let d = FakeDesktop::default();
        let win = json!([{ "handle": 7, "process": process, "title": title, "className": class }]);
        d.reply("listWindows", win);
        d.reply("readTree", json!({ "nodes": nodes }));
        d
    }

    /// A step in window 7 with no ref.
    fn plain() -> Touches {
        Touches {
            window_handle: json!(7),
            ..Touches::default()
        }
    }

    fn at(node: Value) -> Touches {
        Touches {
            node: Some(node),
            ..plain()
        }
    }

    fn asks(v: &Verdict, part: &str) {
        assert_eq!(v.cls, Class::Consequential, "{}", v.why);
        assert!(v.why.contains(part), "{} lacks {part}", v.why);
    }

    fn notes(nodes: Value) -> FakeDesktop {
        desk("notes.exe", "Draft", "Notes", nodes)
    }

    /// A control that has keyboard focus.
    fn focus(role: &str, name: &str) -> Value {
        json!({ "role": role, "name": name, "states": ["focused"] })
    }

    /// Tree node `ref_<r>` under `ref_<parent>` (0: the window's root).
    fn tn(r: u32, parent: u32, role: &str, name: &str) -> Value {
        let mut n = json!({ "ref": format!("ref_{r}"), "role": role, "name": name, "depth": 0 });
        if parent > 0 {
            n["parentRef"] = json!(format!("ref_{parent}"));
            n["depth"] = json!(1);
        }
        n
    }

    /// Attack: the model never clicks Send. It focuses it (Tab, or a click
    /// on the field before it) and presses Space, which Electron let
    /// through as a free key.
    #[test]
    fn space_on_focused_send_button_asks() {
        let send = focus("button", "Send");
        let d = notes(json!([send]));
        let space = json!({ "text": "space" });
        let v = block(gate("key", &space, &plain(), &env(&d)));
        asks(&v, "the focused control is named \"Send\"");
        assert!(v.why.starts_with("Space presses the focused control"));
        let shift = json!({ "text": "shift+space" });
        asks(&block(gate("key", &shift, &plain(), &env(&d))), "Send");
        let held = json!({ "text": "space", "repeat": 5 });
        asks(&block(gate("key", &held, &plain(), &env(&d))), "repeated 5 times");
        // The focus inspect found with the helper's own op counts the same.
        let empty = notes(json!([]));
        let found = Touches {
            focused: Some(send.clone()),
            ..plain()
        };
        asks(&block(gate("key", &space, &found, &env(&empty))), "Send");
        // An unnamed button cannot be checked, and nor can unknown focus.
        let bare = focus("button", "");
        let d = notes(json!([bare]));
        let v = block(gate("key", &space, &plain(), &env(&d)));
        asks(&v, "the focused control has no name to check");
        asks(&block(gate("key", &space, &plain(), &env(&empty))), FOCUS_UNKNOWN);
        // Enter always asked; now it also says what it presses.
        let enter = json!({ "text": "enter" });
        let d = notes(json!([send]));
        asks(&block(gate("key", &enter, &plain(), &env(&d))), "named \"Send\"");
    }

    /// Attack: text whose spaces land on a focused button presses it once
    /// per space.
    #[test]
    fn typed_space_on_button_asks() {
        let delete = focus("button", "Delete all");
        let d = notes(json!([delete]));
        let typed = json!({ "text": "looks like a note" });
        let v = block(gate("type", &typed, &plain(), &env(&d)));
        asks(&v, TYPED_SPACE);
        asks(&v, "the focused control is named \"Delete all\"");
        // A plain name on a button is still a button nobody checked.
        let ok = focus("button", "OK");
        let d = notes(json!([ok]));
        let v = block(gate("type", &typed, &plain(), &env(&d)));
        asks(&v, "it is not known what the focused button \"OK\" does");
        // No space, nothing pressed.
        let one = json!({ "text": "x" });
        assert_eq!(block(gate("type", &one, &plain(), &env(&d))).cls, Class::Input);
        // Spaces with no idea what has focus ask.
        let empty = notes(json!([]));
        asks(&block(gate("type", &typed, &plain(), &env(&empty))), FOCUS_UNKNOWN);
    }

    #[test]
    fn space_in_text_field_is_free() {
        let field = focus("edit", "Search");
        let d = notes(json!([field]));
        let free = Verdict::new(Class::Input, "");
        let space = json!({ "text": "space" });
        assert_eq!(block(gate("key", &space, &plain(), &env(&d))), free);
        let typed = json!({ "text": "two words" });
        assert_eq!(block(gate("type", &typed, &plain(), &env(&d))), free);
        let edits = json!({ "text": "a space b backspace left space" });
        assert_eq!(block(gate("key", &edits, &plain(), &env(&d))), free);
        // Attack: Tab first moves the focus off the field, so the Space
        // presses whatever comes next.
        let moved = json!({ "text": "tab space" });
        asks(&block(gate("key", &moved, &plain(), &env(&d))), FOCUS_MOVES);
        let again = json!({ "text": "space tab", "repeat": 2 });
        asks(&block(gate("key", &again, &plain(), &env(&d))), FOCUS_MOVES);
        // Space on a focused tab item passes as a click on it would, and
        // Tab alone stays free.
        let tab = focus("tabitem", "View");
        let d = notes(json!([tab]));
        assert_eq!(block(gate("key", &space, &plain(), &env(&d))), free);
        let tab_key = json!({ "text": "tab" });
        assert_eq!(block(gate("key", &tab_key, &plain(), &env(&d))), free);
    }

    /// Attack: a page names its button something bland. Electron's verb
    /// list found no reason to ask, so the click went through.
    #[test]
    fn unknown_named_button_asks() {
        let d = notepad();
        let e = env(&d);
        let none = json!({});
        let v = block(gate("left_click", &none, &named("Continue", "button"), &e));
        let want = "it is not known what button \"Continue\" does";
        assert_eq!(v, Verdict::new(Class::Consequential, want));
        let link = named("Learn more", "hyperlink");
        asks(&block(gate("left_click", &none, &link, &e)), "link \"Learn more\"");
        let check = named("Keep me posted", "checkbox");
        asks(&block(gate("triple_click", &none, &check, &e)), "checkbox");
        let id_only = at(json!({ "role": "button", "name": "", "automationId": "nextStep" }));
        asks(&block(gate("left_click", &none, &id_only, &e)), "button \"nextStep\"");
        #[rustfmt::skip]
        let harmless = [
            ("View", "tabitem"), ("Documents", "treeitem"), ("Vertical", "scrollbar"),
            ("Text editor", "document"), ("Search", "edit"), ("Help", "menuitem"),
            ("Zoom in", "button"), ("A1", "dataitem"), ("More options", "button"),
        ];
        for (name, role) in harmless {
            let v = block(gate("left_click", &none, &named(name, role), &e));
            assert_eq!(v, Verdict::new(Class::Input, ""), "{name} {role}");
        }
        // A right click only opens a menu.
        let v = block(gate("right_click", &none, &named("Continue", "button"), &e));
        assert_eq!(v.cls, Class::Input);
    }

    /// Attack: a double click on a downloaded file in Explorer runs it.
    #[test]
    fn explorer_double_click_asks() {
        let d = desk("explorer.exe", "Downloads", "CabinetWClass", json!([]));
        let e = env(&d);
        let none = json!({});
        let file = named("setup.exe", "listitem");
        asks(&block(gate("double_click", &none, &file, &e)), "opens or runs the file");
        let folder = named("Downloads", "treeitem");
        asks(&block(gate("double_click", &none, &folder, &e)), "opens or runs");
        // One click on a tree item only selects it.
        assert_eq!(block(gate("left_click", &none, &folder, &e)).cls, Class::Input);
        // The desktop belongs to Explorer too.
        let d = desk("explorer.exe", "", "Progman", json!([]));
        let icon = named("Invoice.pdf", "listitem");
        asks(&block(gate("double_click", &none, &icon, &env(&d))), "desktop");
    }

    /// Attack: a page draws its own "delete everything?" dialog. Electron
    /// read a dialog's text only in a Win32 #32770 window.
    #[test]
    fn web_dialog_text_is_read() {
        let said = "This will permanently delete your account. It cannot be undone.";
        let tree = vec![
            tn(1, 0, "document", "Account"),
            tn(2, 1, "dialog", "Are you sure?"),
            tn(3, 2, "text", said),
            tn(4, 2, "button", "Yes"),
            tn(5, 1, "text", "Send feedback"),
        ];
        let title = "Account settings - Google Chrome";
        let d = desk("chrome.exe", title, "Chrome_WidgetWin_1", json!(tree));
        let yes = at(tn(4, 2, "button", "Yes"));
        let v = block(gate("left_click", &json!({}), &yes, &env(&d)));
        asks(&v, "the dialog says \"This will permanently delete your account\"");
        assert!(!v.why.contains("feedback"));
        // A window nested in the tree is an app's modal dialog, and its
        // title counts; Enter on its focused OK reads it.
        let mut ok = tn(3, 2, "button", "OK");
        ok["states"] = json!(["focused"]);
        let tree = vec![
            tn(1, 0, "window", "Report - Editor"),
            tn(2, 1, "window", "Delete 3 files?"),
            ok,
        ];
        let d = desk("editor.exe", "Report - Editor", "EditorMain", json!(tree));
        let enter = json!({ "text": "enter" });
        let v = block(gate("key", &enter, &plain(), &env(&d)));
        asks(&v, "the dialog is titled \"Delete 3 files?\"");
    }

    /// Attack: the same Send button in a German, French or Japanese UI
    /// slipped past an English-only verb list.
    #[test]
    fn localized_send_asks() {
        let d = notepad();
        let e = env(&d);
        let none = json!({});
        #[rustfmt::skip]
        let local = [
            "Senden", "Löschen", "Envoyer", "Supprimer", "Enviar", "Elimina", "Excluir",
            "Comprar", "Bezahlen", "送信", "削除", "删除", "發送", "支付",
        ];
        for name in local {
            let v = block(gate("left_click", &none, &named(name, "menuitem"), &e));
            assert_eq!(v.why, format!("the control is named \"{name}\""));
        }
        #[rustfmt::skip]
        let english = [
            "Shut down", "Restart", "Move to trash", "Clear all", "Clear history", "Unsubscribe",
            "Cancel subscription", "Deactivate", "Close account", "End task",
        ];
        for name in english {
            let v = block(gate("left_click", &none, &named(name, "menuitem"), &e));
            asks(&v, name);
        }
        // The step's own reason, in the person's language.
        let step = json!({ "text": "down", "reason": "Datei löschen" });
        let v = block(gate("key", &step, &plain(), &e));
        assert_eq!(v.why, "the step says it will löschen");
        // Whole words with Unicode boundaries: "Payé" is not "pay".
        let v = block(gate("left_click", &none, &named("Payé", "menuitem"), &e));
        assert_eq!(v.cls, Class::Input);
    }
}
