//! The polish lane's labels (`tools/smoke-labels/polish.txt`): a menu in
//! the bar is never cut by the bar's window. The bar's menus sit in the
//! page's flow and Rust sizes the window to them, so only the real window
//! can show whether the cap (`INPUT_MAX_HEIGHT`, scaled with the text) and
//! the resize during the opening glide leave room for the whole menu.
//!
//! - `barMenuInside`: the token's menu, opened the moment the bar starts to
//!   open (while it glides) and again under a four-line question, ends
//!   inside the window both times.

use std::time::Duration;

use serde_json::{json, Value};

use crate::selftest::Cx;
use crate::windows::{self, OVERLAY};

const WAIT: Duration = Duration::from_secs(5);

const MODE_JS: &str = "document.body.dataset.mode || ''";

const TYPE_JS: &str = r#"
(() => {
  const i = document.getElementById('query-input');
  i.focus();
  i.value = $TEXT;
  i.dispatchEvent(new Event('input', { bubbles: true }));
  return i.value;
})()
"#;

const OPEN_MENU_JS: &str = r#"
(() => {
  const m = document.getElementById('intent-menu');
  if (m.hidden) document.getElementById('intent').click();
  return !m.hidden;
})()
"#;

const CLOSE_MENU_JS: &str = r#"
(() => {
  const m = document.getElementById('intent-menu');
  if (!m.hidden) document.getElementById('intent').click();
  return m.hidden;
})()
"#;

// "ok", or where the menu is against the window.
const INSIDE_JS: &str = r#"
(() => {
  const m = document.getElementById('intent-menu');
  if (m.hidden) return 'menu hidden';
  const r = m.getBoundingClientRect();
  const w = document.documentElement.clientWidth;
  const h = document.documentElement.clientHeight;
  const ok = r.height > 0 && r.top >= -0.5 && r.left >= -0.5 && r.bottom <= h + 0.5 && r.right <= w + 0.5;
  return ok ? 'ok' : `menu ${Math.round(r.left)},${Math.round(r.top)} to ${Math.round(r.right)},${Math.round(r.bottom)} in ${w}x${h}`;
})()
"#;

pub fn run(cx: &mut Cx) {
    bar_menu_inside(cx);
}

fn page(cx: &Cx, js: &str) -> Value {
    cx.eval(OVERLAY, js).unwrap_or(Value::Null)
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn mode(cx: &Cx) -> String {
    text(&page(cx, MODE_JS))
}

fn inside_after_resize(cx: &Cx) -> String {
    // The glide (260 ms), the deferred resize and the menu's drop-in.
    let _ = cx.wait_for(Duration::from_millis(1500), || {
        text(&page(cx, INSIDE_JS)) == "ok"
    });
    text(&page(cx, INSIDE_JS))
}

fn bar_menu_inside(cx: &Cx) {
    cx.on_main(windows::collapse_panel);
    let _ = cx.wait_for(WAIT, || mode(cx) == "pill");
    cx.sleep(400);

    // Opened while the bar is still gliding open.
    cx.on_main(|app| windows::open_bar(app, None, None));
    let _ = cx.wait_for(WAIT, || mode(cx) == "panel");
    let opened = page(cx, OPEN_MENU_JS) == json!(true);
    let gliding = inside_after_resize(cx);
    page(cx, CLOSE_MENU_JS);

    // Under a question of four lines.
    let lines = json!("one\ntwo\nthree\nfour").to_string();
    page(cx, &TYPE_JS.replace("$TEXT", &lines));
    cx.sleep(300);
    page(cx, OPEN_MENU_JS);
    let tall = inside_after_resize(cx);
    page(cx, CLOSE_MENU_JS);
    page(cx, &TYPE_JS.replace("$TEXT", "''"));

    let ok = opened && gliding == "ok" && tall == "ok";
    cx.must(
        "barMenuInside",
        ok,
        format!("opened={opened} gliding={gliding} tall={tall}"),
    );
    cx.on_main(windows::collapse_panel);
    let _ = cx.wait_for(WAIT, || mode(cx) == "pill");
}
