//! `Cx::eval`: runs a script in one window's page and hands its value back,
//! as Electron's `webContents.executeJavaScript` did for main.js's smoke.
//!
//! It goes through `ICoreWebView2::ExecuteScript`, so it needs no capability
//! and no event a page could also send. ExecuteScript returns the JSON of a
//! value but does not wait for a Promise, so the expression runs in two
//! steps: [`start`] begins it and keeps its settled value in the page under
//! a fresh id, and [`poll`] collects that value once it is there.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::webview::PlatformWebview;
use tauri::{AppHandle, Manager};
use webview2_com::ExecuteScriptCompletedHandler;
use windows::core::HSTRING;

pub type Answer = Result<Value, String>;

/// Runs `script` in the page of window `label` and returns ExecuteScript's
/// result: the JSON text of the script's value (`null` when it threw).
fn execute(app: &AppHandle, label: &str, script: String, timeout: Duration) -> Answer {
    let win = app
        .get_webview_window(label)
        .ok_or_else(|| format!("no window {label}"))?;
    let (tx, rx) = mpsc::channel::<Result<String, String>>();
    let failed = tx.clone();
    let queued = win.with_webview(move |pw| {
        if let Err(e) = run_script(&pw, &script, tx) {
            let _ = failed.send(Err(format!("ExecuteScript: {e}")));
        }
    });
    queued.map_err(|e| format!("with_webview on {label}: {e}"))?;
    let json = rx
        .recv_timeout(timeout)
        .map_err(|_| format!("no answer from {label} in {timeout:?}"))??;
    serde_json::from_str(&json).map_err(|e| format!("ExecuteScript gave {json}: {e}"))
}

/// On the main thread, inside `with_webview`, as every WebView2 call must be.
fn run_script(
    pw: &PlatformWebview,
    script: &str,
    tx: mpsc::Sender<Result<String, String>>,
) -> windows::core::Result<()> {
    // SAFETY: the controller is alive for the duration of `with_webview`.
    let core = unsafe { pw.controller().CoreWebView2()? };
    let handler = ExecuteScriptCompletedHandler::create(Box::new(move |done, json| {
        let _ = tx.send(done.map(|()| json).map_err(|e| e.to_string()));
        Ok(())
    }));
    // SAFETY: a live handler and a string that outlives the call.
    unsafe { core.ExecuteScript(&HSTRING::from(script), &handler)? };
    Ok(())
}

/// Begins `js`; the page keeps `{ok, v: "<json>"}` or `{ok: false, err}`
/// under `id`. `js` sits on its own lines, so a trailing comment cannot
/// swallow the wrapper. Evaluates to true once started.
fn start(id: u64, js: &str) -> String {
    format!(
        "(() => {{\n\
         const s = (window.__aevalSelftest = window.__aevalSelftest || {{}});\n\
         s[{id}] = null;\n\
         Promise.resolve()\n\
         .then(() => (\n{js}\n))\n\
         .then((v) => {{ const j = JSON.stringify(v === undefined ? null : v);\n\
         return {{ ok: true, v: j === undefined ? 'null' : j }}; }},\n\
         (e) => ({{ ok: false, err: String((e && e.stack) || e) }}))\n\
         .catch((e) => ({{ ok: false, err: String(e) }}))\n\
         .then((r) => {{ s[{id}] = r; }});\n\
         return true;\n\
         }})()"
    )
}

/// The settled value kept under `id` (and forgotten), or null while pending.
fn poll(id: u64) -> String {
    format!(
        "(() => {{ const s = window.__aevalSelftest || {{}}; const r = s[{id}];\n\
         if (r) delete s[{id}]; return r || null; }})()"
    )
}

/// What [`poll`] returned: None while the expression is still pending.
fn decode(polled: &Value) -> Option<Answer> {
    if polled.is_null() {
        return None;
    }
    if polled["ok"] == Value::Bool(true) {
        let v = polled["v"].as_str().unwrap_or("null");
        return Some(serde_json::from_str(v).map_err(|e| e.to_string()));
    }
    let err = polled["err"].as_str().unwrap_or("the script threw");
    Some(Err(err.to_string()))
}

pub fn eval(app: &AppHandle, label: &str, js: &str, timeout: Duration) -> Answer {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let end = Instant::now() + timeout;
    let started = execute(app, label, start(id, js), timeout)?;
    if started != Value::Bool(true) {
        return Err(format!(
            "the expression did not parse, or threw at once: {started}"
        ));
    }
    loop {
        let left = end.saturating_duration_since(Instant::now());
        let polled = execute(app, label, poll(id), left.max(Duration::from_secs(1)))?;
        if let Some(answer) = decode(&polled) {
            return answer;
        }
        if Instant::now() >= end {
            return Err(format!("no answer from {label} in {timeout:?}"));
        }
        thread::sleep(Duration::from_millis(40));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn answers_decode() {
        let ok = json!({ "ok": true, "v": "[\"object\",42]" });
        assert_eq!(decode(&ok), Some(Ok(json!(["object", 42]))));
        let undefined = json!({ "ok": true, "v": "null" });
        assert_eq!(decode(&undefined), Some(Ok(Value::Null)));
        let threw = json!({ "ok": false, "err": "ReferenceError: x" });
        assert_eq!(decode(&threw), Some(Err("ReferenceError: x".into())));
        assert_eq!(decode(&Value::Null), None);
    }

    #[test]
    fn the_wrapper_keeps_the_expression_apart() {
        let js = start(3, "1 + 1 // a comment");
        assert!(js.contains("s[3] = null;"));
        assert!(js.contains("\n1 + 1 // a comment\n))"));
        assert!(js.ends_with("})()"));
        assert!(poll(3).contains("s[3]"));
    }
}
