//! The sheet lane's smoke labels, listed in `tools/smoke-labels/sheet.txt`.
//! `selftest/mod.rs` documents the probe API, with a worked example.
//!
//! Everything here runs in a real output window: the renderer is the same
//! one ui-shots photographs, but the wheel is a real wheel and the page is
//! inside WebView2 at the runner's real scale.

use std::time::Duration;

use serde_json::{json, Value};

use crate::selftest::{Cx, Input};
use crate::windows::{self, flags, outputs};

const LABELS: &[&str] = &[
    "noOutputInput",
    "markdown",
    "markdownStructure",
    "readingMeasure",
    "transcriptScroll",
    "transcriptWheel",
    "chatCeiling",
    "scrollThumb",
    "moreIndicator",
    "globMorphMs",
];

const READY: &str = "typeof window.renderMarkdownInto === 'function' && !!window.AevalGlob";

const NO_INPUT: &str = "document.querySelectorAll('input, textarea').length";

const MARKDOWN: &str = r#"(() => {
  const d = document.createElement('div');
  window.renderMarkdownInto(d, '# Title\n\n- **one**\n- two');
  const h = d.innerHTML;
  return h.includes('<li') && h.includes('<strong>') && h.includes('md-h');
})()"#;

const STRUCTURE: &str = r#"(() => {
  const d = document.createElement('div');
  window.renderMarkdownInto(d, [
    '# One', '## Two', '',
    '1. first', '2. second', '',
    '3. third', '   - nested', '4. fourth', '',
    '| A | B |', '|:---|---:|', '| x | y |', '',
    'See [the book](https://doc.rust-lang.org/book/).', '',
    '<img src=x onerror="window.__pwned=1">',
  ].join('\n'));
  const ol = d.querySelector(':scope > ol');
  const items = ol ? [...ol.children].filter((li) => li.tagName === 'LI') : [];
  const link = d.querySelector('a');
  const host = link && link.nextElementSibling;
  return {
    heads: !!d.querySelector('h2.md-h1') && !!d.querySelector('h3.md-h2'),
    items: items.length,
    nested: !!(items[2] && items[2].querySelector('ul')),
    table: [...d.querySelectorAll('.md-table th')].map((x) => x.dataset.align).join('|'),
    host: host ? host.textContent : '',
    inert: !d.querySelector('img, script, [onerror]') && !window.__pwned,
  };
})()"#;

const MEASURE: &str = r#"(async () => {
  const s = document.getElementById('opanel');
  const was = s.style.cssText;
  s.style.right = 'auto';
  s.style.width = '1560px';
  await new Promise((r) => setTimeout(r, 60));
  const ps = [...document.querySelectorAll('#chat .md-p')];
  const wide = Math.max(0, ...ps.map((p) => p.getBoundingClientRect().width));
  s.style.cssText = was;
  return wide;
})()"#;

const VIEW: &str = r#"(() => {
  const hidden = (id) => document.getElementById(id).hidden;
  return [document.body.dataset.view || '', hidden('opanel'), hidden('chat')].join(':');
})()"#;

const SCROLL: &str = r#"(() => {
  const c = document.getElementById('chat');
  return [c.scrollHeight, c.clientHeight, Math.round(c.scrollTop), getComputedStyle(c).overflowY].join(':');
})()"#;

const CHAT_CENTRE: &str = r#"(() => {
  const c = document.getElementById('chat');
  c.scrollTop = 0;
  const r = c.getBoundingClientRect();
  return { x: r.left + r.width / 2, y: r.top + r.height / 2, dpr: window.devicePixelRatio };
})()"#;

const SCROLL_TOP: &str = "document.getElementById('chat').scrollTop";

const CEILING: &str = r#"(() => {
  const c = document.getElementById('chat');
  const s = document.getElementById('opanel');
  const b = c.getBoundingClientRect().bottom;
  const p = s.getBoundingClientRect().bottom;
  return { max: parseFloat(getComputedStyle(c).maxHeight) || 0, chat: b, sheet: p, inner: window.innerHeight };
})()"#;

const THUMB: &str =
    "getComputedStyle(document.documentElement).getPropertyValue('--scroll-thumb').trim()";

const MORE: &str = r#"(async () => {
  const c = document.getElementById('chat');
  const pause = () => new Promise((r) => setTimeout(r, 150));
  c.scrollTop = 0;
  await pause();
  const top = document.body.dataset.more;
  c.scrollTop = c.scrollHeight;
  await pause();
  return `${top}:${document.body.dataset.more}`;
})()"#;

const GLOB: &str = r#"(async () => {
  const s = document.getElementById('opanel');
  const t0 = performance.now();
  await window.AevalGlob.open(s);
  return {
    ms: Math.round(performance.now() - t0),
    reduced: document.documentElement.dataset.motion === 'reduced' || matchMedia('(prefers-reduced-motion: reduce)').matches,
  };
})()"#;

/// Eight questions with answers long enough to overflow any window.
fn long_chat() -> Value {
    let mut messages = Vec::new();
    for i in 1..=8 {
        let answer = format!("Paragraph {i} of the answer. ").repeat(24);
        messages.push(json!({ "role": "user", "text": format!("Question {i}") }));
        messages.push(json!({ "role": "assistant", "text": answer }));
    }
    json!({ "messages": messages })
}

fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn fail_all(cx: &Cx, why: &str) {
    for label in LABELS {
        cx.must(label, false, why);
    }
}

pub fn run(cx: &mut Cx) {
    let Some(label) = cx.on_main(outputs::target).flatten() else {
        fail_all(cx, "no output window opened");
        return;
    };
    let ready = cx.wait_for(Duration::from_secs(20), || {
        cx.eval(&label, READY) == Ok(json!(true))
    });
    if ready {
        check(cx, &label);
    } else {
        fail_all(cx, "the output page never loaded");
    }
    let l = label.clone();
    cx.on_main(move |app| outputs::close(app, &l));
}

fn check(cx: &Cx, label: &str) {
    let none = cx.eval(label, NO_INPUT);
    cx.must("noOutputInput", none == Ok(json!(0)), format!("{none:?}"));

    let md = cx.eval(label, MARKDOWN);
    cx.must("markdown", md == Ok(json!(true)), format!("{md:?}"));

    let shape = cx.eval(label, STRUCTURE);
    let want = json!({
        "heads": true,
        "items": 4,
        "nested": true,
        "table": "left|right",
        "host": "(doc.rust-lang.org)",
        "inert": true,
    });
    cx.must(
        "markdownStructure",
        shape.as_ref() == Ok(&want),
        format!("{shape:?}"),
    );

    // Which view the window is in before and after a conversation lands in
    // it, and whether a Find is still open: one that outlived the bar
    // probes would hide the transcript behind its results.
    let before = cx.eval(label, VIEW);
    let l = label.to_string();
    cx.on_main(move |app| windows::emit(app, &l, "chat:sync", long_chat()));
    // The window grows to fit, then stops at its ceiling.
    cx.sleep(1200);
    let after = cx.eval(label, VIEW);
    let open = cx.on_main(|app| crate::state::shared(app).lock().find_open);
    cx.log(
        "sheetView",
        format!("{label} before={before:?} after={after:?} findOpen={open:?}"),
    );

    transcript(cx, label);
    wheel(cx, label);

    let ceiling = cx.eval(label, CEILING);
    let fits = ceiling.as_ref().is_ok_and(|v| {
        num(v, "max") > 0.0
            && num(v, "chat") <= num(v, "sheet") + 1.0
            && num(v, "sheet") <= num(v, "inner") + 1.0
    });
    cx.must("chatCeiling", fits, format!("{ceiling:?}"));

    let thumb = cx.eval(label, THUMB);
    let has_thumb = thumb
        .as_ref()
        .is_ok_and(|v| v.as_str().is_some_and(|s| !s.is_empty()));
    cx.must("scrollThumb", has_thumb, format!("{thumb:?}"));

    let more = cx.eval(label, MORE);
    cx.must(
        "moreIndicator",
        more == Ok(json!("1:0")),
        format!("{more:?}"),
    );

    let wide = cx.eval(label, MEASURE);
    let kept = wide
        .as_ref()
        .is_ok_and(|v| v.as_f64().is_some_and(|w| w > 0.0 && w <= 641.0));
    cx.must("readingMeasure", kept, format!("{wide:?}"));

    let morph = cx.eval_for(label, GLOB, Duration::from_secs(8));
    let timed = morph.as_ref().is_ok_and(|v| {
        let ms = num(v, "ms");
        v.get("reduced") == Some(&json!(true)) || (150.0..900.0).contains(&ms)
    });
    cx.must("globMorphMs", timed, format!("{morph:?}"));
}

fn transcript(cx: &Cx, label: &str) {
    let got = cx.eval(label, SCROLL);
    let text = got
        .as_ref()
        .ok()
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let parts: Vec<&str> = text.split(':').collect();
    let at = |i: usize| {
        parts
            .get(i)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    let scrolls = at(0) > at(1) + 40.0
        && at(2) > 0.0
        && matches!(parts.get(3).copied(), Some("auto" | "scroll"));
    cx.must("transcriptScroll", scrolls, format!("{got:?}"));
}

/// A real wheel over the middle of the transcript moves it.
fn wheel(cx: &Cx, label: &str) {
    let centre = cx.eval(label, CHAT_CENTRE);
    let (Ok(c), Some(hwnd)) = (&centre, flags::hwnd_for(label)) else {
        cx.must(
            "transcriptWheel",
            false,
            format!("no point to wheel over: {centre:?}"),
        );
        return;
    };
    let rect = flags::rect_px(hwnd);
    let dpr = num(c, "dpr").max(1.0);
    let at = (
        (rect.x + num(c, "x") * dpr).round() as i32,
        (rect.y + num(c, "y") * dpr).round() as i32,
    );
    // The pointer arrives first, so the window stops passing clicks through.
    let moved = cx.input(Input::Move(at.0, at.1));
    cx.sleep(300);
    let sent = cx.input(Input::Wheel { at, notches: -3 });
    let scrolled = cx.wait_for(Duration::from_secs(3), || {
        cx.eval(label, SCROLL_TOP)
            .is_ok_and(|v| v.as_f64().is_some_and(|t| t > 0.0))
    });
    cx.must(
        "transcriptWheel",
        moved.is_ok() && sent.is_ok() && scrolled,
        format!("at {at:?} moved {moved:?} sent {sent:?}"),
    );
}
