//! The bar lane's labels (`tools/smoke-labels/bar.txt`), with Electron's
//! semantics from lib/smoke-v2.js, lib/smoke-v21.js and lib/smoke-glass.js
//! on `main`, plus the labels the bar items add: `barMultiline` (B-056),
//! `tokenReply` (B-150) and `earlyKeys` (B-154).

use std::time::Duration;

use serde_json::{json, Value};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowLongPtrW, GWL_EXSTYLE, GWL_STYLE,
};

use crate::selftest::{Cx, Input};
use crate::windows::{self, flags, OVERLAY};
use crate::{paths, state, store};

const WAIT: Duration = Duration::from_secs(5);
const BOOT: Duration = Duration::from_secs(20);

// setIgnoreMouseEvents owns WS_EX_TRANSPARENT and WS_EX_LAYERED, and
// setFocusable owns WS_EX_NOACTIVATE, which is checked against the mode on
// its own; WS_VISIBLE follows show and hide. None of them is churn.
const NOACTIVATE: u32 = 0x0800_0000;
const EX_MASK: u32 = !(0x20 | 0x8_0000 | NOACTIVATE);
const STYLE_MASK: u32 = !0x1000_0000;

/// One window's styles at one stage: (stage, window, style, extended style).
type Sample = (&'static str, String, u32, u32);

const INTENT_LABELS: [&str; 4] = [
    "intentTable",
    "intentPrefixes",
    "intentFast",
    "intentDoNeedsMargin",
];

// Three lines a person pastes: each one a question, so Enter asks.
const LINES: &str = "why does the build fail?\nwhat does LNK1104 mean?\nhow do I fix it?";

const READY_JS: &str = "!!(window.AevalIntent && document.getElementById('query-input'))";
const VALUE_JS: &str = "document.getElementById('query-input').value";
const MODE_JS: &str = "document.body.dataset.mode || ''";

const TOKEN_JS: &str = r#"
(() => {
  const t = document.getElementById('intent');
  return t ? t.dataset.intent + (t.dataset.locked === 'true' ? '!' : '') : 'none';
})()
"#;

const TOKEN_STATE_JS: &str = r#"
(() => {
  const t = document.getElementById('intent');
  const label = t.getAttribute('aria-label') || '';
  return [t.dataset.intent, label, t.getAttribute('aria-disabled') || ''].join('|');
})()
"#;

// The field emptied without telling the bar, as Electron's barKeys did.
const EMPTY_JS: &str = r#"
(() => {
  document.getElementById('query-input').value = '';
  return true;
})()
"#;

const TYPE_JS: &str = r#"
(() => {
  const i = document.getElementById('query-input');
  i.focus();
  i.value = $TEXT;
  i.dispatchEvent(new Event('input', { bubbles: true }));
  return i.value;
})()
"#;

const PRESS_JS: &str = r#"
(() => {
  const i = document.getElementById('query-input');
  i.focus();
  const e = new KeyboardEvent('keydown', {
    key: $KEY,
    ctrlKey: $CTRL,
    bubbles: true,
    cancelable: true,
  });
  i.dispatchEvent(e);
  return e.defaultPrevented;
})()
"#;

const ROWS_JS: &str = r#"
(() => {
  const lines = document.getElementById('query-input').value.split('\n').length;
  return lines + ':' + (document.getElementById('bar').dataset.rows || '');
})()
"#;

const EXAMPLES_JS: &str = r#"
(() => {
  const e = document.getElementById('examples');
  return (e.hidden ? 'hidden:' : 'shown:') + e.querySelectorAll('button').length;
})()
"#;

// lib/smoke-v21.js basics(): the guess against what a person types, the
// prefixes, the cost per keystroke and the margin rule, run in the page.
const INTENT_JS: &str = r##"
(() => {
  const I = window.AevalIntent;
  const SCREEN = 'Invoice 2291 Total due $4,120.00 Please settle by Friday Submit Cancel refund policy 0x1F3A';
  const TABLE = [
    ['why is the build red', 'ask'],
    ['how do I save this as pdf', 'ask'],
    ['what does this error mean?', 'ask'],
    ['explain this function', 'ask'],
    ['summarize this page', 'ask'],
    ['is this email a scam?', 'ask'],
    ['write me a haiku', 'ask'],
    ['translate this paragraph to French', 'ask'],
    ['find the bug in this function', 'ask'],
    ['what is the total of this column', 'ask'],
    ['which button saves the file?', 'ask'],
    ['save', 'ask'],
    ['/etc/hosts', 'ask'],
    ['can you open notepad?', 'ask'],
    ['', 'ask'],
    ['hello', 'ask'],
    ['refund policy', 'find'],
    ['?invoice', 'ask'],
    ['?rename these files', 'ask'],
    ['invoice', 'find'],
    ['2291', 'find'],
    ['total due', 'find'],
    ['0x1F3A', 'find'],
    ['Please settle by Friday', 'find'],
    ['where is the submit button', 'find'],
    ['/invoice', 'find'],
    ['//total', 'find'],
    ['/ deadline', 'find'],
    ['find invoice', 'find'],
    ['where is the cancel button', 'find'],
    ['rename these screenshots by date', 'do'],
    ['open Notepad and write a haiku', 'do'],
    ['send the report to Sam', 'do'],
    ['delete the old ones', 'do'],
    ['do it', 'do'],
    ['can you open notepad', 'do'],
    ['>save', 'do'],
    ['>what time is it', 'do'],
    ['close all the tabs except this one', 'do'],
    ['sort this sheet by date', 'do'],
    ['open a new tab and search for the weather', 'do'],
    ['move these files into a folder called old', 'do'],
  ];
  const PARSE = [
    ['/invoice', 'find', 'invoice', false],
    ['//total', 'find', 'total', true],
    ['?why', 'ask', 'why', false],
    ['> open notepad', 'do', 'open notepad', false],
    ['/etc/hosts', null, '/etc/hosts', false],
  ];
  const ctx = { screen: SCREEN, screenFolded: I.fold(SCREEN) };
  const wrong = [];
  for (const [text, want] of TABLE) {
    const got = I.classify(text, ctx).intent;
    if (got !== want) wrong.push(`"${text}" ${got}!=${want}`);
  }
  const badParse = [];
  for (const [raw, lock, body, literal] of PARSE) {
    const p = I.parse(raw);
    if (p.lock !== lock || p.body !== body || p.literal !== literal) {
      badParse.push(`${raw}:${JSON.stringify(p)}`);
    }
  }
  // Under 5 ms a keystroke against a full window of screen text.
  const big = I.fold(SCREEN.repeat(150));
  I.score('warm up', { screenFolded: big });
  let worst = 0;
  for (const [text] of TABLE) {
    const t0 = performance.now();
    I.score(text, { screenFolded: big });
    worst = Math.max(worst, performance.now() - t0);
  }
  const weak = I.classify('fix it', {});
  return { n: TABLE.length, wrong, badParse, worst, weak: { i: weak.intent, m: weak.margin } };
})()
"##;

pub fn run(cx: &mut Cx) {
    probe(cx);
}

fn probe(cx: &Cx) {
    let ready = cx.wait_for(BOOT, || page(cx, READY_JS) == json!(true));
    if !ready {
        cx.log("barPage", "the overlay page never loaded");
    }
    intent(cx);
    key_swallow(cx);
    style(cx);
    bar_keys(cx);
    token_reply(cx);
    early_keys(cx);
    lines_and_suggestions(cx);
    page(cx, &TYPE_JS.replace("$TEXT", "''"));
    collapse(cx);
}

// ----------------------------------------------------------------- helpers

fn page(cx: &Cx, js: &str) -> Value {
    cx.eval(OVERLAY, js).unwrap_or(Value::Null)
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn strings(v: &Value) -> Vec<String> {
    let all = v.as_array().cloned().unwrap_or_default();
    all.iter().map(text).collect()
}

fn mode(cx: &Cx) -> String {
    text(&page(cx, MODE_JS))
}

/// The page shows `want`, then time for the morph and the window's resize.
fn settled(cx: &Cx, want: &str) -> bool {
    let ok = cx.wait_for(WAIT, || mode(cx) == want);
    cx.sleep(500);
    ok
}

fn open(cx: &Cx) -> bool {
    cx.on_main(|app| windows::open_bar(app, None, None));
    settled(cx, "panel")
}

fn collapse(cx: &Cx) -> bool {
    cx.on_main(|app| windows::collapse_panel(app));
    settled(cx, "pill")
}

/// Opens the bar with the real hotkey, so its window takes the foreground
/// the way it does for a person; opens it directly if that fails.
fn open_by_hotkey(cx: &Cx, chord: &str) -> bool {
    if cx.input(Input::Key(chord)).is_ok() && settled(cx, "panel") {
        return true;
    }
    open(cx)
}

/// A synthetic keydown on the field, the way Electron's barKeys pressed.
fn press(cx: &Cx, key: &str, ctrl: bool) -> Value {
    let js = PRESS_JS
        .replace("$KEY", &json!(key).to_string())
        .replace("$CTRL", &ctrl.to_string());
    page(cx, &js)
}

fn type_text(cx: &Cx, value: &str) {
    page(cx, &TYPE_JS.replace("$TEXT", &json!(value).to_string()));
}

fn held(cx: &Cx, code: Code) -> bool {
    let sc = Shortcut::new(None, code);
    let on = cx.on_main(move |app| app.global_shortcut().is_registered(sc));
    on.unwrap_or(true)
}

fn styles(hwnd: isize) -> (u32, u32) {
    let h = hwnd as HWND;
    // SAFETY: reading a style of a live window.
    let style = unsafe { GetWindowLongPtrW(h, GWL_STYLE) };
    // SAFETY: as above.
    let ex = unsafe { GetWindowLongPtrW(h, GWL_EXSTYLE) };
    (style as u32, ex as u32)
}

fn foreground_is_overlay() -> bool {
    // SAFETY: a query with no arguments.
    let fg = unsafe { GetForegroundWindow() } as isize;
    flags::hwnd_for(OVERLAY) == Some(fg)
}

fn last_question() -> String {
    let history = store::get().get_input_history();
    history.last().cloned().unwrap_or_default()
}

/// `hasSentFirstRequest` as the settings file on disk has it: what the next
/// start reads.
fn saved_flag() -> bool {
    let file = paths::user_data_dir().join("aevalbuddy.json");
    let body = std::fs::read_to_string(file).unwrap_or_default();
    let body = body.trim_start_matches('\u{feff}');
    let v: Value = serde_json::from_str(body).unwrap_or_default();
    v["hasSentFirstRequest"] == true
}

// ------------------------------------------------------------------ labels

/// intentTable, intentPrefixes, intentWorstMs, intentFast and
/// intentDoNeedsMargin (lib/smoke-v21.js).
fn intent(cx: &Cx) {
    let r = match cx.eval(OVERLAY, INTENT_JS) {
        Ok(v) => v,
        Err(e) => {
            for label in INTENT_LABELS {
                cx.must(label, false, &e);
            }
            return;
        }
    };
    let n = r["n"].as_u64().unwrap_or(0);
    let wrong = strings(&r["wrong"]);
    let right = n.saturating_sub(wrong.len() as u64);
    cx.log("intentTable", format!("{right}/{n}"));
    let table = n >= 40 && wrong.is_empty();
    cx.must("intentTable", table, wrong.join(" | "));
    let bad = strings(&r["badParse"]);
    cx.must("intentPrefixes", bad.is_empty(), bad.join(" | "));
    let worst = r["worst"].as_f64().unwrap_or(f64::MAX);
    cx.log("intentWorstMs", format!("{worst:.2}"));
    cx.must("intentFast", worst < 5.0, format!("{worst:.2}ms"));
    // A guessed task is never a run: the margin rule leaves weak ones as Ask.
    let weak = &r["weak"];
    let margin = weak["m"].as_f64().unwrap_or(0.0);
    let ok = weak["i"] != "do" || margin >= 0.15;
    cx.must("intentDoNeedsMargin", ok, weak);
}

/// keySwallow: no global Enter ever, and no global Escape while only the
/// bar is open or nothing is (lib/smoke-v21.js).
fn key_swallow(cx: &Cx) {
    open(cx);
    cx.sleep(300);
    let enter = held(cx, Code::Enter);
    let esc_bar = held(cx, Code::Escape);
    collapse(cx);
    cx.sleep(200);
    let esc_idle = held(cx, Code::Escape);
    let seen = format!("enter={enter} escBar={esc_bar} escIdle={esc_idle}");
    let ok = !enter && !esc_bar && !esc_idle;
    cx.must("keySwallow", ok, seen);
}

/// styleSamples and styleStable: the windows' styles never change across
/// pill, bar and back, except the bits named above, and WS_EX_NOACTIVATE is
/// set on the pill and clear on the bar, exactly (lib/smoke-glass.js).
fn style(cx: &Cx) {
    let mut samples = Vec::new();
    collapse(cx);
    sample(cx, "pill", &mut samples);
    open(cx);
    sample(cx, "bar", &mut samples);
    collapse(cx);
    sample(cx, "pill-again", &mut samples);
    open(cx);
    sample(cx, "bar-again", &mut samples);
    let mut raw = Vec::new();
    for (stage, win, style, ex) in &samples {
        raw.push(format!("{stage}/{win}:{style:#x}/{ex:#x}"));
    }
    cx.log("styleSamples", raw.join(" "));
    let mut drift = Vec::new();
    for s in &samples {
        let (stage, win, style, ex) = s;
        let base = samples.iter().find(|b| b.1 == *win).unwrap_or(s);
        let style_moved = (style & STYLE_MASK) != (base.2 & STYLE_MASK);
        let ex_moved = (ex & EX_MASK) != (base.3 & EX_MASK);
        if style_moved || ex_moved {
            let was = format!("{}/{:#x}/{:#x}", base.0, base.2, base.3);
            let now = format!("{stage}/{style:#x}/{ex:#x}");
            drift.push(format!("{win} {was}->{now}"));
        }
        let pill = stage.starts_with("pill");
        let has = (ex & NOACTIVATE) != 0;
        if win == OVERLAY && has != pill {
            let line = format!("overlay {stage} WS_EX_NOACTIVATE={has} (want {pill})");
            drift.push(line);
        }
    }
    let ok = samples.len() >= 4 && drift.is_empty();
    let why = if drift.is_empty() {
        format!("samples={}", samples.len())
    } else {
        drift.join("; ")
    };
    cx.must("styleStable", ok, why);
}

/// One sample per visible window: the overlay and any output.
fn sample(cx: &Cx, stage: &'static str, out: &mut Vec<Sample>) {
    let st = state::shared(&cx.app);
    let outputs: Vec<String> = st.lock().outputs.keys().cloned().collect();
    let mut labels = vec![OVERLAY.to_string()];
    labels.extend(outputs);
    for win in labels {
        let visible = flags::hwnd_for(&win).filter(|h| flags::is_visible(*h));
        if let Some(h) = visible {
            let (style, ex) = styles(h);
            out.push((stage, win, style, ex));
        }
    }
}

/// barKeys: Tab is never taken by the field; Ctrl+1..3 lock the token and
/// keep the text; Backspace on an empty locked field unlocks it
/// (lib/smoke-v2.js).
fn bar_keys(cx: &Cx) {
    open(cx);
    page(cx, EMPTY_JS);
    let tab = press(cx, "Tab", false);
    type_text(cx, "invoice");
    press(cx, "2", true);
    let jumped = text(&page(cx, TOKEN_JS));
    let kept = text(&page(cx, VALUE_JS));
    press(cx, "3", true);
    let do_lock = text(&page(cx, TOKEN_JS));
    press(cx, "1", true);
    let back = text(&page(cx, TOKEN_JS));
    page(cx, EMPTY_JS);
    press(cx, "Backspace", false);
    let unlocked = text(&page(cx, TOKEN_JS));
    let seen = format!("tab={tab}:{jumped}:{kept}:{do_lock}:{back}:{unlocked}");
    cx.log("barKeys", &seen);
    let locks = jumped == "find!" && do_lock == "do!" && back == "ask!";
    let ok = tab == json!(false) && kept == "invoice" && locks && !unlocked.ends_with('!');
    cx.must("barKeys", ok, seen);
}

/// Rust's own overlay:state with the agent waiting on the person.
fn send_waiting(cx: &Cx, waiting: &str) {
    let agent = json!({ "running": true, "step": 2, "state": waiting });
    cx.on_main(move |app| {
        let mut st = windows::overlay_state(app, None);
        st["agent"] = agent;
        windows::emit(app, OVERLAY, "overlay:state", st);
    });
}

/// tokenReply (B-150): while a task asks, the token says Reply and Enter
/// belongs to the task; with a plan waiting and the field empty it says
/// Start; once the task stops waiting it is the guess again.
fn token_reply(cx: &Cx) {
    open(cx);
    type_text(cx, "");
    send_waiting(cx, "asking");
    let reply = "do|Reply. Enter sends this to the running task.|true";
    let asked = cx.wait_for(WAIT, || text(&page(cx, TOKEN_STATE_JS)) == reply);
    let got_reply = text(&page(cx, TOKEN_STATE_JS));
    send_waiting(cx, "plan");
    let start = "do|Start. Enter sends this to the running task.|true";
    let planned = cx.wait_for(WAIT, || text(&page(cx, TOKEN_STATE_JS)) == start);
    let got_start = text(&page(cx, TOKEN_STATE_JS));
    cx.on_main(windows::send_overlay_state);
    let rest = |t: &str| t.starts_with("ask|") && t.ends_with('|');
    let back = cx.wait_for(WAIT, || rest(&text(&page(cx, TOKEN_STATE_JS))));
    let got_back = text(&page(cx, TOKEN_STATE_JS));
    let why = format!("asking={got_reply} plan={got_start} after={got_back}");
    cx.must("tokenReply", asked && planned && back, why);
}

/// earlyKeys (B-154): keys typed 5 ms after the hotkey land in the field.
fn early_keys(cx: &Cx) {
    let hotkey = store::get().shortcut("captureQuery");
    // Opened by the hotkey once, so the bar's window has the foreground as
    // it does for a person, then folded with the field empty.
    open_by_hotkey(cx, &hotkey);
    type_text(cx, "");
    collapse(cx);
    let sent = cx.input(Input::Key(&hotkey));
    cx.sleep(5);
    let typed = cx.input(Input::Text("early"));
    let kept = cx.wait_for(WAIT, || text(&page(cx, VALUE_JS)) == "early");
    let value = text(&page(cx, VALUE_JS));
    let fg = foreground_is_overlay();
    let keys = format!("{hotkey}={sent:?} text={typed:?}");
    let why = format!("{keys} field={value:?} foreground={fg}");
    cx.must("earlyKeys", kept, why);
}

/// barMultiline (B-056): three pasted lines stay three rows, and Enter sends
/// them with both line breaks. suggestionsOnce (NEW-10): that first request
/// sets the flag in the settings file a restart reads, and the reopened bar
/// shows no suggestions.
fn lines_and_suggestions(cx: &Cx) {
    let s = store::get();
    let sent_before = s.get_has_sent_first_request();
    let has_key = s.has_api_key();
    open(cx);
    type_text(cx, "");
    let shown_before = text(&page(cx, EXAMPLES_JS));
    let clip = cx.app.clipboard();
    let kept = clip.read_text().ok();
    let pasted = clip.write_text(LINES).is_ok();
    let paste = cx.input(Input::Key("Ctrl+V"));
    let three = cx.wait_for(WAIT, || text(&page(cx, ROWS_JS)) == "3:3");
    let rows = text(&page(cx, ROWS_JS));
    let enter = cx.input(Input::Key("Enter"));
    let sent = cx.wait_for(WAIT, || last_question() == LINES);
    let question = last_question();
    let breaks = question.matches('\n').count();
    let ok = pasted && three && sent && breaks == 2;
    let keys = format!("paste={paste:?} rows={rows} enter={enter:?}");
    let why = format!("{keys} sent={question:?} key={has_key}");
    cx.must("barMultiline", ok, why);
    if let Some(t) = kept {
        let _ = clip.write_text(t);
    }

    let flag = s.get_has_sent_first_request();
    let on_disk = cx.wait_for(WAIT, saved_flag);
    let st = cx.on_main(|app| windows::overlay_state(app, None));
    let in_state = st.is_some_and(|v| v["hasSentFirstRequest"] == true);
    type_text(cx, "");
    collapse(cx);
    open(cx);
    let after = text(&page(cx, EXAMPLES_JS));
    let hidden = after.starts_with("hidden");
    let ok = flag && on_disk && in_state && hidden;
    let before = format!("before={sent_before}/{shown_before}");
    let why = format!("{before} after={after} disk={on_disk} state={in_state}");
    cx.must("suggestionsOnce", ok, why);
}
