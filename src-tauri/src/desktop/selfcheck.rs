//! The desktop engine's acceptance checks: a port of lib/desktop/selfcheck.js
//! with the same labels, pass conditions and Notepad choreography (PARITY
//! DESK-09). Shared by `AEVAL_SMOKE` (agent e) and the CI desktop check.
//!
//! The Notepad half opens a real window and types into it, so it only runs
//! when asked. It touches nothing but the Notepad window it launched: told
//! apart by diffing the window list and by a unique file name in its title;
//! every keystroke goes through the engine's foreground guard; the temp file
//! is deleted at the end and the window closes without saving.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::client::Desktop;
use super::protocol::DesktopError;

pub const FIXTURE: &str = "Invoice total due 42.00";
pub const TYPED: &str = "AevalBuddy smoke 4271";
pub const SET: &str = "Replaced by setValue 9137";

/// `[SMOKE] k=v` lines and the failed invariants, as main.js's log/must.
type Lines = (Vec<(String, String)>, Vec<String>);

#[derive(Clone, Default)]
pub struct Report {
    pub prefix: &'static str,
    inner: Arc<Mutex<Lines>>,
}

impl Report {
    pub fn new(prefix: &'static str) -> Report {
        Report {
            prefix,
            inner: Arc::default(),
        }
    }

    pub fn log(&self, label: &str, value: impl std::fmt::Display) {
        let v = value.to_string();
        println!("[{}] {label}={v}", self.prefix);
        if let Ok(mut g) = self.inner.lock() {
            g.0.push((label.to_string(), v));
        }
    }

    pub fn must(&self, label: &str, ok: bool, detail: &str) {
        let v = if ok {
            "ok".to_string()
        } else if detail.is_empty() {
            "FAIL".to_string()
        } else {
            format!("FAIL:{detail}")
        };
        self.log(label, v);
        if !ok {
            if let Ok(mut g) = self.inner.lock() {
                g.1.push(label.to_string());
            }
        }
    }

    pub fn failures(&self) -> Vec<String> {
        self.inner.lock().map(|g| g.1.clone()).unwrap_or_default()
    }

    pub fn value(&self, label: &str) -> Option<String> {
        self.inner
            .lock()
            .ok()?
            .0
            .iter()
            .rev()
            .find(|(k, _)| k == label)
            .map(|(_, v)| v.clone())
    }
}

pub struct Options {
    pub notepad: bool,
    /// A point (physical) on one of our visible windows.
    pub own_point: Option<(i64, i64)>,
    /// Our window handles as `'w…'` strings.
    pub own_handles: Vec<String>,
}

async fn sleep(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn rect(v: &Value) -> Option<(f64, f64, f64, f64)> {
    Some((
        v.get("x")?.as_f64()?,
        v.get("y")?.as_f64()?,
        v.get("width")?.as_f64()?,
        v.get("height")?.as_f64()?,
    ))
}

fn inside(r: &Value, outer: &Value) -> bool {
    let slack = 2.0;
    match (rect(r), rect(outer)) {
        (Some(r), Some(o)) => {
            r.0 >= o.0 - slack
                && r.1 >= o.1 - slack
                && r.0 + r.2 <= o.0 + o.2 + slack
                && r.1 + r.3 <= o.1 + o.3 + slack
        }
        _ => false,
    }
}

fn err_text(e: &DesktopError) -> String {
    format!("{}:{}", e.code, e.message)
}

fn list(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

/// The text area: the largest document or edit, preferring one that exposes
/// text or a writable value over a small field such as a find box.
pub fn pick_editor(nodes: &[Value]) -> Option<Value> {
    let area = |n: &Value| rect(&n["rect"]).map(|r| r.2 * r.3).unwrap_or(0.0);
    let score = |n: &Value| {
        let pats: Vec<&str> = n["patterns"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let textish = pats.contains(&"text") || pats.contains(&"value");
        area(n) * if textish { 2.0 } else { 1.0 } * if s(&n["role"]) == "document" { 1.5 } else { 1.0 }
    };
    let mut c: Vec<&Value> = nodes
        .iter()
        .filter(|n| matches!(s(&n["role"]), "document" | "edit"))
        .collect();
    // Stable sort, highest score first, as Array.prototype.sort does.
    c.sort_by(|a, b| score(b).total_cmp(&score(a)));
    c.first().map(|n| (*n).clone())
}

async fn wait_for<F, Fut>(timeout_ms: u64, step_ms: u64, mut f: F) -> Option<Value>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<Value>>,
{
    let end = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if let Some(v) = f().await {
            return Some(v);
        }
        if Instant::now() > end {
            return None;
        }
        sleep(step_ms).await;
    }
}

pub async fn run(desktop: &Arc<Desktop>, rep: &Report, o: &Options) {
    // ---------------------------------------------------------------- B2
    let t = Instant::now();
    let mut started = true;
    if let Err(e) = async {
        desktop.start().await?;
        desktop.ping().await
    }
    .await
    {
        started = false;
        rep.log("desktopError", err_text(&e));
    }
    let ready_ms = t.elapsed().as_millis();
    rep.must(
        "desktopReady",
        started && ready_ms < 5000,
        &format!("ready={started} ms={ready_ms}"),
    );
    rep.log("desktopReadyMs", ready_ms);
    if !started {
        return;
    }
    rep.log("desktopDpi", desktop.status()["dpi"].to_string().trim_matches('"'));

    let windows = list(&desktop.list_windows(false).await.unwrap_or(Value::Null));
    rep.must(
        "desktopWindows",
        !windows.is_empty(),
        &format!("listWindows returned {}", windows.len()),
    );
    rep.log("desktopWindowCount", windows.len());
    let leaked: Vec<&str> = windows
        .iter()
        .filter(|w| o.own_handles.iter().any(|h| h == s(&w["handle"])))
        .map(|w| s(&w["title"]))
        .collect();
    rep.must("desktopOwnExcluded", leaked.is_empty(), &leaked.join(", "));

    // ---------------------------------------------------------------- B5
    if let Some((x, y)) = o.own_point {
        let code = match desktop.click(&json!({ "x": x, "y": y })).await {
            Ok(_) => "clicked".to_string(),
            Err(e) => e.code,
        };
        rep.must("ownWindowBlocked", code == "BLOCKED_OWN_WINDOW", &code);
    }

    if !o.notepad {
        rep.log("desktopNotepad", "skipped (set AEVAL_DESKTOP_NOTEPAD=1, or run on CI)");
        return;
    }
    notepad(desktop, rep).await;
}

async fn notepad(desktop: &Arc<Desktop>, rep: &Report) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let tag = format!("aeval-desktop-check-{}-{stamp:x}", std::process::id());
    let file = std::env::temp_dir().join(format!("{tag}.txt"));
    if std::fs::write(&file, format!("{FIXTURE}\r\n")).is_err() {
        rep.must("desktopNotepadIncluded", false, "could not write the temp file");
        return;
    }
    let before_list = list(&desktop.list_windows(true).await.unwrap_or(Value::Null));
    let before: HashSet<String> = before_list.iter().map(|w| s(&w["handle"]).to_string()).collect();
    let had_notepad = before_list.iter().any(|w| s(&w["process"]).eq_ignore_ascii_case("notepad"));

    let _ = std::process::Command::new("notepad.exe").arg(&file).spawn();
    let win = wait_for(15000, 250, || {
        let before = before.clone();
        let tag = tag.clone();
        let d = desktop.clone();
        async move {
            let l = list(&d.list_windows(false).await.ok()?);
            l.into_iter()
                .find(|w| !before.contains(s(&w["handle"])) && s(&w["title"]).contains(&tag))
        }
    })
    .await;
    rep.must(
        "desktopNotepadIncluded",
        win.is_some(),
        "no new window titled with the temp file appeared",
    );
    if let Some(win) = &win {
        steps(desktop, rep, win).await;
        let closed = close_without_saving(desktop, win, &tag, had_notepad, rep).await;
        rep.must("notepadClosed", closed, "the check tab or window is still open");
    }
    if std::fs::remove_file(&file).is_err() {
        rep.log("tempFileLeft", file.display());
    }
}

async fn steps(desktop: &Arc<Desktop>, rep: &Report, win: &Value) {
    let h = win["handle"].clone();
    rep.log(
        "notepadWindow",
        format!("{}:{}:{}", s(&win["handle"]), s(&win["process"]), s(&win["className"])),
    );
    // Each step stands alone, so one that cannot run does not hide the rest.
    macro_rules! step {
        ($name:expr, $body:expr) => {
            if let Err(e) = $body.await {
                rep.must($name, false, &err_text(&e));
            }
        };
    }

    step!("focusWindow", async {
        let f = desktop.focus_window(&h).await?;
        rep.log("focusMethod", s(&f["method"]));
        sleep(400).await;
        Ok::<_, DesktopError>(())
    });

    // -------------------------------------------------------------- B3
    let mut editor: Option<Value> = None;
    step!("uiaTree", async {
        let t = Instant::now();
        let tree = desktop
            .read_tree(&json!({ "windowHandle": h, "filter": "interactive" }))
            .await?;
        let tree_ms = t.elapsed().as_millis();
        let nodes = list(&tree["nodes"]);
        let refs_ok = nodes.iter().all(|n| {
            let r = s(&n["ref"]);
            r.strip_prefix("ref_").map(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit())).unwrap_or(false)
        });
        editor = pick_editor(&nodes);
        let chars = s(&tree["text"]).encode_utf16().count();
        rep.must(
            "uiaTree",
            nodes.len() >= 5 && refs_ok && editor.is_some() && chars < 50000,
            &format!("nodes={} refs={refs_ok} editor={} chars={chars}", nodes.len(), editor.is_some()),
        );
        rep.log("uiaTreeNodes", nodes.len());
        rep.log("uiaTreeMs", tree_ms);
        let mut roles: Vec<String> = Vec::new();
        for n in &nodes {
            let r = s(&n["role"]).to_string();
            if !roles.contains(&r) {
                roles.push(r);
            }
        }
        rep.log("uiaTreeRoles", roles.join(","));
        Ok::<_, DesktopError>(())
    });
    if editor.is_none() {
        return;
    }
    step!("editorPick", async {
        let all = desktop
            .read_tree(&json!({ "windowHandle": h, "filter": "all", "depth": 30 }))
            .await?;
        let nodes = list(&all["nodes"]);
        if let Some(best) = pick_editor(&nodes) {
            editor = Some(best);
        }
        let cands: Vec<String> = nodes
            .iter()
            .filter(|n| matches!(s(&n["role"]), "document" | "edit"))
            .map(|n| {
                let j = |k: &str| {
                    n[k].as_array()
                        .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("+"))
                        .unwrap_or_default()
                };
                let size = rect(&n["rect"]).map(|r| format!("{}x{}", r.2, r.3)).unwrap_or("-".into());
                format!("{}:{}:{}:{}:{}:{size}", s(&n["ref"]), s(&n["role"]), s(&n["name"]), j("patterns"), j("states"))
            })
            .collect();
        rep.log("editorCandidates", cands.join(" | "));
        if let Some(e) = &editor {
            rep.log("editor", format!("{}:{}:{}", s(&e["ref"]), s(&e["role"]), s(&e["name"])));
        }
        Ok::<_, DesktopError>(())
    });
    let editor = editor.unwrap_or(Value::Null);
    let eref = s(&editor["ref"]).to_string();

    // -------------------------------------------------------------- B6
    let mut zoomed = false;
    step!("notepadZoom", async {
        desktop.key("ctrl+plus", 8, Some(&h)).await?;
        zoomed = true;
        sleep(500).await;
        Ok::<_, DesktopError>(())
    });
    step!("ocrWords", async {
        let t = Instant::now();
        let o = desktop.ocr(&Value::Null, Some(&h), "auto").await?;
        rep.log("ocrMs", t.elapsed().as_millis());
        let spans = list(&o["spans"]);
        let words: Vec<String> = spans.iter().map(|sp| s(&sp["text"]).to_lowercase()).collect();
        let hits: Vec<&str> = ["invoice", "total", "due"]
            .into_iter()
            .filter(|w| words.iter().any(|x| x == w))
            .collect();
        let rects_in = spans
            .iter()
            .filter(|sp| hits.contains(&s(&sp["text"]).to_lowercase().as_str()))
            .all(|sp| inside(&sp["rect"], &win["rect"]));
        let detail = format!(
            "hits={} words={} inWindow={rects_in} lang={}",
            hits.join("|"),
            spans.len(),
            s(&o["lang"])
        );
        rep.must("ocrWords", hits.len() >= 2 && rects_in, &detail);
        rep.log("ocrDetail", &detail);
        Ok::<_, DesktopError>(())
    });
    if zoomed {
        step!("notepadZoomReset", async { desktop.key("ctrl+0", 1, Some(&h)).await.map(|_| ()) });
    }

    // -------------------------------------------------------------- B4
    step!("uiaTypeRead", async {
        desktop.key("ctrl+end", 1, Some(&h)).await?;
        let typed = desktop.type_text(&format!(" {TYPED}"), Some(&eref), None).await?;
        rep.log("typeFocus", typed["focus"].to_string().trim_matches('"'));
        // A WinUI editor can take a moment to expose new text to UIA.
        let mut g = Value::Null;
        for _ in 0..8 {
            sleep(250).await;
            g = desktop.get_text(Some(&h), &Value::Null, 50000, false).await?;
            if s(&g["text"]).contains(TYPED) {
                break;
            }
        }
        let head: String = s(&g["text"]).chars().take(300).collect();
        rep.log("typeReadText", serde_json::to_string(&head).unwrap_or_default());
        rep.must(
            "uiaTypeRead",
            s(&g["text"]).contains(TYPED) && s(&g["source"]) == "uia",
            &format!("source={} spans={}", s(&g["source"]), list(&g["spans"]).len()),
        );
        Ok::<_, DesktopError>(())
    });

    step!("uiaSetValue", async {
        let t = Instant::now();
        let sv = desktop.set_value(&eref, json!(SET)).await?;
        rep.log("uiaSetValueMs", t.elapsed().as_millis());
        rep.log("uiaSetValueVia", s(&sv["via"]));
        sleep(200).await;
        let g = desktop.get_text(Some(&h), &Value::Null, 50000, false).await?;
        rep.must("uiaSetValue", s(&g["text"]).contains(SET), &format!("via={}", s(&sv["via"])));
        rep.log("getTextSpans", list(&g["spans"]).len());
        Ok::<_, DesktopError>(())
    });

    step!("findEditor", async {
        let found = desktop.find("text editor", Some(&h), 3, 45).await?;
        let nodes = list(&found["nodes"]);
        let top = nodes.first();
        let roles: Vec<&str> = nodes.iter().map(|n| s(&n["role"])).collect();
        rep.must(
            "findEditor",
            top.map(|t| matches!(s(&t["role"]), "document" | "edit")).unwrap_or(false),
            &serde_json::to_string(&roles).unwrap_or_default(),
        );
        rep.log(
            "findTop",
            top.map(|t| format!("{}:{}:{}", s(&t["role"]), s(&t["name"]), t["score"]))
                .unwrap_or_else(|| "none".into()),
        );
        Ok::<_, DesktopError>(())
    });

    step!("elementAt", async {
        let r = rect(&editor["rect"]).unwrap_or_default();
        let at = desktop
            .element_at((r.0 + (r.2 / 2.0).round()) as i64, (r.1 + (r.3 / 2.0).round()) as i64)
            .await?;
        rep.log("elementAt", format!("{}:{}", s(&at["node"]["role"]), s(&at["node"]["ref"])));
        Ok::<_, DesktopError>(())
    });
    step!("restoreCursor", async { desktop.restore_cursor().await.map(|_| ()) });
}

/// Closing uses patterns only (Invoke on buttons, WM_CLOSE on the window),
/// so it works even when the check never got the keyboard focus. The tabbed
/// Windows 11 Notepad closes the check's tab through its own Close Tab
/// button, and the window only when Notepad was not running before.
pub async fn close_without_saving(desktop: &Arc<Desktop>, win: &Value, tag: &str, had_notepad: bool, rep: &Report) -> bool {
    let h = win["handle"].clone();
    let mut tabbed = false;
    let first = async {
        let tree = desktop.read_tree(&json!({ "windowHandle": h, "filter": "all" })).await?;
        let nodes = list(&tree["nodes"]);
        let tabs: Vec<&Value> = nodes.iter().filter(|n| s(&n["role"]) == "tabitem").collect();
        tabbed = !tabs.is_empty();
        if tabbed {
            let mine = tabs.iter().find(|n| s(&n["name"]).contains(tag));
            let close = mine.and_then(|m| {
                nodes.iter().find(|n| {
                    n["parentRef"] == m["ref"]
                        && s(&n["role"]) == "button"
                        && s(&n["name"]).to_lowercase().contains("close")
                })
            });
            match close {
                Some(c) => {
                    desktop.invoke(s(&c["ref"]), "invoke").await?;
                }
                None => return Err(DesktopError::new("NOT_FOUND", "no Close Tab button on the check tab")),
            }
        } else {
            desktop.close_window(&h).await?;
        }
        Ok::<_, DesktopError>(())
    }
    .await;
    if let Err(e) = first {
        rep.log("closeError", err_text(&e));
        return false;
    }
    let pid = win["pid"].clone();
    let done = wait_for(10000, 400, || {
        let d = desktop.clone();
        let tag = tag.to_string();
        let pid = pid.clone();
        let rep = rep.clone();
        async move {
            let l = list(&d.list_windows(false).await.ok()?);
            if !l.iter().any(|w| s(&w["title"]).contains(&tag)) {
                return Some(json!(true));
            }
            for w in l.iter().filter(|x| x["pid"] == pid) {
                // The dialog may vanish between the read and the click.
                if let Ok(hit) = d.find("don't save", Some(&w["handle"]), 1, 70).await {
                    if let Some(n) = hit["nodes"].as_array().and_then(|a| a.first()) {
                        if d.click(&json!({ "ref": n["ref"] })).await.is_ok() {
                            rep.log("dontSave", format!("{}:{}", s(&n["role"]), s(&n["name"])));
                        }
                    }
                }
            }
            None
        }
    })
    .await
    .is_some();
    if done && tabbed && !had_notepad {
        let all = list(&desktop.list_windows(true).await.unwrap_or(Value::Null));
        if all.iter().any(|w| w["handle"] == h) {
            let _ = desktop.close_window(&h).await;
        }
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_pick_prefers_big_text_documents() {
        let nodes = vec![
            json!({"role":"edit","rect":{"x":0,"y":0,"width":100,"height":20},"patterns":["value"]}),
            json!({"role":"document","rect":{"x":0,"y":0,"width":800,"height":600},"patterns":["text"]}),
            json!({"role":"button","rect":{"x":0,"y":0,"width":9000,"height":9000}}),
        ];
        assert_eq!(pick_editor(&nodes).unwrap()["role"], "document");
        assert!(pick_editor(&[]).is_none());
    }

    #[test]
    fn inside_with_slack() {
        let outer = json!({"x":0,"y":0,"width":100,"height":100});
        assert!(inside(&json!({"x":-2,"y":0,"width":102,"height":10}), &outer));
        assert!(!inside(&json!({"x":-3,"y":0,"width":10,"height":10}), &outer));
    }

    #[test]
    fn report_collects_failures() {
        let r = Report::new("TEST");
        r.must("a", true, "");
        r.must("b", false, "why");
        assert_eq!(r.failures(), vec!["b".to_string()]);
        assert_eq!(r.value("b").as_deref(), Some("FAIL:why"));
    }
}
