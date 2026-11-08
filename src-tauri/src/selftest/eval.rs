//! `Cx::eval`: runs a script in one window's page and hands its value back,
//! as Electron's `webContents.executeJavaScript` did for main.js's smoke.
//!
//! Tauri's own `eval` returns nothing, so the script is wrapped: it awaits
//! the expression and emits the JSON of its value (or the error) as a Tauri
//! event tagged with a fresh id, and the caller waits for that id.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Once, OnceLock};
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::Value;
use tauri::{AppHandle, Listener, Manager};

const EVENT: &str = "aeval-selftest";

type Answer = Result<Value, String>;
type Waiters = Mutex<HashMap<u64, mpsc::Sender<Answer>>>;

fn waiters() -> &'static Waiters {
    static W: OnceLock<Waiters> = OnceLock::new();
    W.get_or_init(Waiters::default)
}

fn listen(app: &AppHandle) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = app.listen_any(EVENT, |event| {
            if let Some((id, answer)) = decode(event.payload()) {
                if let Some(tx) = waiters().lock().remove(&id) {
                    let _ = tx.send(answer);
                }
            }
        });
    });
}

/// The page's message: `{id, ok: true, v: "<json>"}` or `{id, ok: false,
/// err}`.
fn decode(payload: &str) -> Option<(u64, Answer)> {
    let msg: Value = serde_json::from_str(payload).ok()?;
    let id = msg["id"].as_u64()?;
    let answer = if msg["ok"] == Value::Bool(true) {
        let v = msg["v"].as_str().unwrap_or("null");
        serde_json::from_str(v).map_err(|e| e.to_string())
    } else {
        let err = msg["err"].as_str().unwrap_or("the script threw");
        Err(err.to_string())
    };
    Some((id, answer))
}

/// `js` on its own lines, so a trailing comment cannot swallow the wrapper.
fn wrap(id: u64, js: &str) -> String {
    format!(
        "(() => {{\n\
         const send = (m) => window.__TAURI__.event.emit('{EVENT}', Object.assign({{ id: {id} }}, m));\n\
         Promise.resolve()\n\
         .then(() => (\n{js}\n))\n\
         .then((v) => ({{ ok: true, v: JSON.stringify(v === undefined ? null : v) }}),\n\
         (e) => ({{ ok: false, err: String((e && e.stack) || e) }}))\n\
         .catch((e) => ({{ ok: false, err: String(e) }}))\n\
         .then(send);\n\
         }})();"
    )
}

pub fn eval(app: &AppHandle, label: &str, js: &str, timeout: Duration) -> Answer {
    let win = app
        .get_webview_window(label)
        .ok_or_else(|| format!("no window {label}"))?;
    listen(app);
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel();
    waiters().lock().insert(id, tx);
    if let Err(e) = win.eval(wrap(id, js)) {
        waiters().lock().remove(&id);
        return Err(format!("eval in {label}: {e}"));
    }
    match rx.recv_timeout(timeout) {
        Ok(answer) => answer,
        Err(_) => {
            waiters().lock().remove(&id);
            Err(format!("no answer from {label} in {timeout:?}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn answers_decode() {
        let ok = r#"{"id":7,"ok":true,"v":"[\"object\",42]"}"#;
        assert_eq!(decode(ok), Some((7, Ok(json!(["object", 42])))));
        let undefined = r#"{"id":8,"ok":true}"#;
        assert_eq!(decode(undefined), Some((8, Ok(Value::Null))));
        let threw = r#"{"id":9,"ok":false,"err":"ReferenceError: x"}"#;
        assert_eq!(decode(threw), Some((9, Err("ReferenceError: x".into()))));
        assert_eq!(decode("not json"), None);
        assert_eq!(decode(r#"{"ok":true}"#), None);
    }

    #[test]
    fn the_wrapper_keeps_the_expression_apart() {
        let js = wrap(3, "1 + 1 // a comment");
        assert!(js.contains("{ id: 3 }"));
        assert!(js.contains("\n1 + 1 // a comment\n))"));
        assert!(js.contains("'aeval-selftest'"));
    }
}
