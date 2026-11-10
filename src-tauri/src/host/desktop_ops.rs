//! `DesktopOps` over split (c)'s `Desktop` (ARCHITECTURE.md section 12): the
//! index.js export names and option objects split (b) speaks, mapped onto
//! (c)'s typed methods. `focusWindow` and `closeWindow` take `{handle}`;
//! `click`, `hover` and `scroll` take `{t, opts}`; everything else takes the
//! one options object index.js took.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Map, Value};
use tauri::AppHandle;

use crate::agent::seams::{DeskError, DesktopOps};
use crate::desktop::{Deps, Desktop, DesktopError};

pub struct DesktopAdapter {
    d: Arc<Desktop>,
}

impl DesktopAdapter {
    pub fn new(d: Arc<Desktop>) -> DesktopAdapter {
        DesktopAdapter { d }
    }

    pub fn inner(&self) -> &Arc<Desktop> {
        &self.d
    }
}

fn err(e: DesktopError) -> DeskError {
    DeskError::new(&e.code, e.message)
}

/// A JSON number as a whole number, whether it arrived as 3 or 3.0.
fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64().filter(|f| f.is_finite()),
        Value::String(s) => s.trim().parse::<f64>().ok().filter(|f| f.is_finite()),
        _ => None,
    }
}

fn uint(v: &Value, default: u64) -> u64 {
    num(v)
        .filter(|f| *f >= 0.0)
        .map(|f| f.round() as u64)
        .unwrap_or(default)
}

/// Rewrites integral floats (3.0) as integers, so (c)'s `as_u64` reads
/// them: split (b) computes counts and amounts in f64 as JS did.
fn whole(v: &Value) -> Value {
    match v {
        Value::Number(n) if n.as_u64().is_none() && n.as_i64().is_none() => match n.as_f64() {
            Some(f) if f.is_finite() && f.fract() == 0.0 && f.abs() < 9.0e15 => {
                if f >= 0.0 {
                    json!(f as u64)
                } else {
                    json!(f as i64)
                }
            }
            _ => v.clone(),
        },
        Value::Object(o) => {
            let m: Map<String, Value> = o.iter().map(|(k, x)| (k.clone(), whole(x))).collect();
            Value::Object(m)
        }
        Value::Array(a) => Value::Array(a.iter().map(whole).collect()),
        _ => v.clone(),
    }
}

/// `{...t, ...opts}` as index.js's click and scroll built it.
fn spread(args: &Value) -> Value {
    let mut out = Map::new();
    for part in [&args["t"], &args["opts"]] {
        if let Some(o) = part.as_object() {
            for (k, v) in o {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    Value::Object(out)
}

fn opt<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

fn str_of<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

impl DesktopAdapter {
    async fn dispatch(&self, method: &str, a: Value) -> Result<Value, DesktopError> {
        let d = &self.d;
        let null = Value::Null;
        match method {
            "listWindows" => {
                let all = a["includeMinimized"].as_bool().unwrap_or(false);
                d.list_windows(all).await
            }
            "foregroundWindow" => d.foreground_window().await,
            "focusWindow" => d.focus_window(&a["handle"]).await,
            "closeWindow" => d.close_window(&a["handle"]).await,
            "monitors" => d.monitors().await,
            "readTree" => d.read_tree(&a).await,
            "find" => {
                let query = str_of(&a, "query").unwrap_or("");
                let limit = uint(&a["limit"], 20) as usize;
                let min = num(&a["minScore"]).map(|f| f.round() as i64).unwrap_or(45);
                d.find(query, opt(&a, "windowHandle"), limit, min).await
            }
            "getText" => {
                let rect = opt(&a, "rect").unwrap_or(&null);
                let max = uint(&a["maxChars"], 50000);
                let ocr = a["ocrFallback"].as_bool().unwrap_or(true);
                d.get_text(opt(&a, "windowHandle"), rect, max, ocr).await
            }
            "ocr" => {
                let rect = opt(&a, "rect").unwrap_or(&null);
                let lang = str_of(&a, "lang").unwrap_or("auto");
                d.ocr(rect, opt(&a, "windowHandle"), lang).await
            }
            "screenshot" => {
                let rect = opt(&a, "rect").unwrap_or(&null);
                let max = uint(&a["maxEdge"], 1568);
                d.screenshot(opt(&a, "windowHandle"), rect, max).await
            }
            "zoom" => {
                let rect = opt(&a, "rect").unwrap_or(&null);
                d.zoom(rect, uint(&a["maxEdge"], 1568)).await
            }
            "elementAt" => {
                let x = num(&a["x"]).unwrap_or(0.0).round() as i64;
                let y = num(&a["y"]).unwrap_or(0.0).round() as i64;
                d.element_at(x, y).await
            }
            "windowStates" => d.window_states(whole(&a)).await,
            "resolve" => d.resolve(str_of(&a, "ref").unwrap_or("")).await,
            "focused" => d.focused(opt(&a, "windowHandle")).await,
            "click" => d.click(&whole(&spread(&a))).await,
            "hover" => d.hover(&whole(&spread(&a))).await,
            "scroll" => d.scroll(&whole(&spread(&a))).await,
            "drag" => {
                let steps = num(&a["steps"]).map(|f| f.max(0.0).round() as u64);
                d.drag(&whole(&a["from"]), &whole(&a["to"]), steps).await
            }
            "type" => {
                let Some(text) = str_of(&a, "text") else {
                    return Err(DesktopError::new("BAD_ARGS", "type needs text"));
                };
                d.type_text(text, str_of(&a, "ref"), opt(&a, "windowHandle"))
                    .await
            }
            "key" => {
                let keys = str_of(&a, "keys").unwrap_or("");
                let repeat = uint(&a["repeat"], 1).max(1);
                d.key(keys, repeat, opt(&a, "windowHandle")).await
            }
            "scrollTo" => d.scroll_to(str_of(&a, "ref").unwrap_or("")).await,
            "findText" => {
                let query = str_of(&a, "query").unwrap_or("");
                let max = uint(&a["max"], 400);
                d.find_text(opt(&a, "windowHandle"), query, max).await
            }
            "scrollToText" => {
                let r = str_of(&a, "ref").unwrap_or("");
                let query = str_of(&a, "query").unwrap_or("");
                d.scroll_to_text(r, query, uint(&a["n"], 0)).await
            }
            "setValue" => {
                let value = a.get("value").cloned().unwrap_or(Value::Null);
                d.set_value(str_of(&a, "ref").unwrap_or(""), value).await
            }
            "invoke" => {
                let action = str_of(&a, "action").unwrap_or("invoke");
                d.invoke(str_of(&a, "ref").unwrap_or(""), action).await
            }
            "restoreCursor" => d.restore_cursor().await,
            "cancel" => d.cancel().await,
            "start" => d.start().await.map(|_| json!({ "ok": true })),
            "ping" => d.ping().await,
            _ => Err(DesktopError::new(
                "BAD_ARGS",
                format!("unknown desktop method {method}"),
            )),
        }
    }
}

#[async_trait]
impl DesktopOps for DesktopAdapter {
    async fn op(&self, method: &str, args: Value) -> Result<Value, DeskError> {
        self.dispatch(method, args).await.map_err(err)
    }
}

/// What main.js passed to `desktop.init`: physical input goes to the agent
/// (takeover) and to Find (a wheel re-reads); a synthetic Escape lets go of
/// the global Stop until it has passed. The own-window list keeps (c)'s
/// default, which reads every window of this app.
pub fn init_desktop_deps(app: &AppHandle) {
    let before = app.clone();
    let after = app.clone();
    crate::desktop::client().init(Deps {
        want_input: Some(Arc::new(|| {
            super::try_services().is_some_and(|s| s.agent.is_running() || s.find.is_open())
        })),
        on_user_input: Some(Arc::new(|ev: &Value| {
            let Some(s) = super::try_services() else {
                return;
            };
            s.agent.on_user_input(ev);
            if ev["kind"] == "wheel" && s.find.is_open() {
                s.find.nudge();
            }
        })),
        before_synthetic_key: Some(Arc::new(move || crate::hotkeys::suspend_escape(&before))),
        after_synthetic_key: Some(Arc::new(move || crate::hotkeys::resume_escape(&after))),
        ..Deps::default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_spreads_options_over_the_target() {
        let a =
            json!({ "t": { "x": 10.0, "y": 20.0 }, "opts": { "count": 2.0, "button": "right" } });
        let s = whole(&spread(&a));
        assert_eq!(s["x"], json!(10));
        assert_eq!(s["count"].as_u64(), Some(2));
        assert_eq!(s["button"], "right");
    }

    #[test]
    fn whole_keeps_fractions_and_strings() {
        assert_eq!(whole(&json!(1.5)), json!(1.5));
        assert_eq!(whole(&json!("3")), json!("3"));
        assert_eq!(whole(&json!(-4.0)), json!(-4));
        assert_eq!(uint(&json!(3.0), 1), 3);
        assert_eq!(uint(&Value::Null, 7), 7);
    }
}
