//! The windows lane's smoke labels, listed in `tools/smoke-labels/windows.txt`.
//! `selftest/mod.rs` documents the probe API, with a worked example.
//!
//! - `hoverKeepsAnswer`: the real pointer crosses an open answer window, its
//!   clear margin and its sheet, and the window stays shown, where it was,
//!   with its sheet in view. tao's click-through hid any window shown with
//!   `show_inactive` the first time the pointer reached the margin.
//! - `marginClickThrough`: the clear margin still lets clicks through and the
//!   sheet still takes them.
//! - `minimizeToPill`: Minimize folds the bar into the pill and hides the
//!   answer window; opening the bar again brings the same conversation back.

use std::time::Duration;

use serde_json::{json, Value};

use crate::selftest::{Cx, Input};
use crate::state::{shared, Rect};
use crate::store;
use crate::windows::{self, flags, outputs, screen, OVERLAY};

const WAIT: Duration = Duration::from_secs(5);
const BOOT: Duration = Duration::from_secs(20);

const LABELS: [&str; 3] = ["hoverKeepsAnswer", "marginClickThrough", "minimizeToPill"];

const OUTPUT_READY: &str = "!!(document.getElementById('opanel') && window.AevalGlass)";
const BAR_READY: &str = "!!document.getElementById('query-input')";
const MODE_JS: &str = "document.body.dataset.mode || ''";

/// The sheet as the page lays it out, in CSS px from the window's corner.
const SHEET_JS: &str = r#"(() => {
  const s = document.getElementById('opanel');
  const r = s.getBoundingClientRect();
  return {
    shown: !s.hidden && !document.hidden && !s.classList.contains('away') && r.height > 0,
    x: r.left + r.width / 2,
    top: r.top,
    bottom: r.bottom,
    dpr: devicePixelRatio,
    inner: innerHeight,
    msgs: document.querySelectorAll('#chat > .msg').length,
  };
})()"#;

fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn conversation() -> Vec<Value> {
    vec![
        json!({ "role": "user", "text": "Why does the build fail?" }),
        json!({ "role": "assistant", "text": "Because `vcruntime140.dll` is not on this machine." }),
    ]
}

pub fn run(cx: &mut Cx) {
    let Some(label) = cx.on_main(outputs::target).flatten() else {
        for l in LABELS {
            cx.must(l, false, "no output window opened");
        }
        return;
    };
    let ready = cx.wait_for(BOOT, || cx.eval(&label, OUTPUT_READY) == Ok(json!(true)))
        && cx.wait_for(BOOT, || cx.eval(OVERLAY, BAR_READY) == Ok(json!(true)));
    if !ready {
        for l in LABELS {
            cx.must(l, false, "a page never loaded");
        }
        return;
    }

    // What the later probes start from, put back at the end.
    let l = label.clone();
    let before = cx.on_main(move |app| {
        let o = shared(app).lock().outputs.get(&l).cloned();
        let shown = flags::hwnd_for(&l).is_some_and(flags::is_visible);
        o.map(|o| (o.chat.messages, o.active, shown))
    });
    let Some(Some((messages, active, shown))) = before else {
        for l in LABELS {
            cx.must(l, false, "the output has no state");
        }
        return;
    };

    let pos = store::get().get_overlay_pos();
    let l = label.clone();
    cx.on_main(move |app| {
        let payload = outputs::with(app, &l, |o| {
            o.chat.messages = conversation();
            outputs::chat_payload(o)
        });
        if let Some(p) = payload {
            windows::emit(app, &l, "chat:sync", p);
        }
    });

    // Andy lost the answer with the bar at the bottom left: the window
    // grows up from it, so the pointer comes down through its clear part.
    let mut kept = true;
    let mut through = true;
    let mut whys = Vec::new();
    for (h, v) in [
        ("left", "bottom"),
        ("right", "bottom"),
        ("left", "top"),
        ("center", "top"),
    ] {
        place(cx, &label, h, v);
        let (k, t, why) = hover(cx, &label);
        kept &= k;
        through &= t;
        whys.push(format!("[{h}-{v}] {why}"));
    }
    let why = whys.join(" ");
    cx.must("hoverKeepsAnswer", kept, &why);
    cx.must("marginClickThrough", through, why);

    minimize(cx, &label);
    store::get().set_overlay_pos(&pos);

    let l = label.clone();
    cx.on_main(move |app| {
        let payload = outputs::with(app, &l, |o| {
            o.chat.messages = messages;
            o.active = active;
            outputs::chat_payload(o)
        });
        if let Some(p) = payload {
            windows::emit(app, &l, "chat:sync", p);
        }
        if !shown {
            if let Some(w) = windows::window(app, &l) {
                flags::hide(&w);
            }
        }
        windows::collapse_panel(app);
    });
    cx.sleep(400);
}

/// The bar moved to that corner and the answer placed afresh beside it, as
/// a new answer would be.
fn place(cx: &Cx, label: &str, h: &str, v: &str) {
    cx.on_main(windows::collapse_panel);
    let _ = cx.wait_for(WAIT, || bar_mode(cx) == "pill");
    cx.sleep(300);
    store::get().set_overlay_pos(&json!({ "h": h, "v": v, "offX": 16, "offY": 16 }));
    let l = label.to_string();
    cx.on_main(move |app| {
        if let Some(w) = windows::window(app, &l) {
            flags::hide(&w);
        }
        outputs::with(app, &l, |o| {
            o.active = false;
            o.placed = false;
            o.rect = None;
            o.sheet = None;
            o.user_height = 0.0;
        });
        windows::open_bar(app, None, None);
    });
    let _ = cx.wait_for(WAIT, || bar_mode(cx) == "panel");
    cx.sleep(500);
    // The pointer starts off the windows, away from where they will be.
    let _ = cx.input(Input::Move(4, 4));
    let l = label.to_string();
    cx.on_main(move |app| outputs::show(app, &l));
    // The glob out and the page's first output:glass.
    cx.sleep(1200);
}

/// Inside the work area of the display it is on, to the pixel.
fn inside(r: Rect, wa: Rect) -> bool {
    r.x >= wa.x - 1.0
        && r.y >= wa.y - 1.0
        && r.x + r.width <= wa.x + wa.width + 1.0
        && r.y + r.height <= wa.y + wa.height + 1.0
}

fn same(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() < 1.0
        && (a.y - b.y).abs() < 1.0
        && (a.width - b.width).abs() < 1.0
        && (a.height - b.height).abs() < 1.0
}

/// One stop of the pointer: moved there, given time, then looked at.
fn stop(cx: &Cx, label: &str, hwnd: isize, at: (i32, i32)) -> (bool, Rect, bool, bool) {
    let moved = cx.input(Input::Move(at.0, at.1)).is_ok();
    cx.sleep(450);
    let page = cx.eval(label, SHEET_JS).unwrap_or(Value::Null);
    let shown = page.get("shown") == Some(&json!(true));
    (
        moved && flags::is_visible(hwnd),
        flags::rect_px(hwnd),
        shown,
        flags::is_click_through(label),
    )
}

/// The pointer over the sheet, the clear part and back: (stayed, clicks
/// passed where they should, the trail).
fn hover(cx: &Cx, label: &str) -> (bool, bool, String) {
    let page = cx.eval(label, SHEET_JS).unwrap_or(Value::Null);
    let Some(hwnd) = flags::hwnd_for(label) else {
        return (false, false, "no window handle".into());
    };
    let start = flags::rect_px(hwnd);
    // The glass, not the window: its clear stage margin may hang past the
    // work area's edge.
    let wa = screen::matching_px(start).work_area;
    let in_wa = inside(windows::motion::glass(hwnd), wa);
    let start_shown = flags::is_visible(hwnd) && page.get("shown") == Some(&json!(true));
    let dpr = num(&page, "dpr").max(1.0);
    let x = (start.x + num(&page, "x") * dpr).round() as i32;
    let on_sheet = (start.y + (num(&page, "top") + 24.0) * dpr).round() as i32;
    // The clear part of the window: it keeps room for the tallest sheet
    // (B-027), under the sheet when it grows down, over it when it grows up.
    let below = num(&page, "bottom") + 48.0;
    let above = num(&page, "top") - 48.0;
    let in_margin = if below < num(&page, "inner") - 4.0 {
        Some((start.y + below * dpr).round() as i32)
    } else if above > 4.0 {
        Some((start.y + above * dpr).round() as i32)
    } else {
        None
    };
    let outside = ((start.x - 60.0).max(0.0) as i32, on_sheet);

    let mut steps = vec![("sheet", (x, on_sheet))];
    if let Some(y) = in_margin {
        steps.extend([
            ("margin", (x, y)),
            ("sheet", (x, on_sheet)),
            ("margin", (x, y)),
        ]);
    }
    steps.push(("outside", outside));

    let mut ok = start_shown && in_wa;
    let mut through = in_margin.is_some();
    let mut trail = Vec::new();
    for (name, at) in steps {
        let (visible, rect, sheet, ignoring) = stop(cx, label, hwnd, at);
        ok &= visible && same(rect, start) && sheet && inside(windows::motion::glass(hwnd), wa);
        match name {
            "sheet" => through &= !ignoring,
            "margin" => through &= ignoring,
            _ => {}
        }
        trail.push(format!(
            "{name}:visible={visible},sheet={sheet},through={ignoring},rect={},{},{},{}",
            rect.x, rect.y, rect.width, rect.height
        ));
    }
    // Andy saw the answer only while he held Ctrl+Alt+Space: each repeat
    // showed it again. Once let go, the pointer over it must not hide it.
    let chord = store_chord();
    let pressed = !chord.is_empty() && cx.input(Input::Key(&chord)).is_ok();
    cx.sleep(600);
    if let Some(y) = in_margin {
        let (visible, rect, sheet, _) = stop(cx, label, hwnd, (x, y));
        ok &= visible && sheet;
        trail.push(format!(
            "after-hotkey({chord},pressed={pressed}):visible={visible},sheet={sheet},rect={},{},{},{}",
            rect.x, rect.y, rect.width, rect.height
        ));
    }
    let why = format!(
        "start_shown={start_shown} in_work_area={in_wa} wa={},{},{},{} {}",
        wa.x,
        wa.y,
        wa.width,
        wa.height,
        trail.join(" ")
    );
    cx.log("hoverTrail", &why);
    (ok, through, why)
}

/// The ask hotkey as the person set it (Ctrl+Alt+Space by default).
fn store_chord() -> String {
    store::get().shortcut("captureQuery")
}

fn bar_mode(cx: &Cx) -> String {
    cx.eval(OVERLAY, MODE_JS)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn minimize(cx: &Cx, label: &str) {
    let visible = || flags::hwnd_for(label).is_some_and(flags::is_visible);
    cx.on_main(|app| windows::open_bar(app, None, None));
    let opened = cx.wait_for(WAIT, || bar_mode(cx) == "panel") && cx.wait_for(WAIT, visible);
    cx.sleep(400);

    cx.on_main(windows::minimize);
    let folded = cx.wait_for(WAIT, || bar_mode(cx) == "pill") && cx.wait_for(WAIT, || !visible());
    cx.sleep(400);

    cx.on_main(|app| windows::open_bar(app, None, None));
    let back = cx.wait_for(WAIT, || bar_mode(cx) == "panel") && cx.wait_for(WAIT, visible);
    cx.sleep(600);
    let page = cx.eval(label, SHEET_JS).unwrap_or(Value::Null);
    let kept = num(&page, "msgs") >= 2.0 && page.get("shown") == Some(&json!(true));
    cx.must(
        "minimizeToPill",
        opened && folded && back && kept,
        format!("opened={opened} folded={folded} back={back} kept={kept} page={page}"),
    );
}
