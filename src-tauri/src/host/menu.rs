//! Right-click menus inside the windows (main.js contextMenu, SPEC-v2 4.5):
//! an answer (Copy, Find this on screen, Do it), a Find hit (Go to, Copy
//! text, Ask about this) and a trail row (Copy step, Retry from here).

use std::collections::HashMap;
use std::sync::OnceLock;

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::menu::{Menu, MenuItem};
use tauri::{AppHandle, Manager};

use super::agent::{start_run, StartReq};
use super::services;

const PREFIX: &str = "ctx:";

#[derive(Clone, Debug)]
enum Action {
    Copy(String),
    FindThis(String),
    DoIt { task: String, context: String },
    GoTo(i64),
    AskHit(i64),
}

fn pending() -> &'static Mutex<HashMap<String, Action>> {
    static P: OnceLock<Mutex<HashMap<String, Action>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(HashMap::new()))
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// The items for one right-click, as main.js built them.
fn items_for(p: &Value) -> Vec<(&'static str, Action)> {
    let mut items = Vec::new();
    let text = text_of(&p["text"]);
    let index = p["index"].as_f64().map(|f| f as i64).unwrap_or(-1);
    match p["kind"].as_str() {
        Some("answer") => {
            let sel = text_of(&p["selection"]);
            let from = if sel.is_empty() { &text } else { &sel };
            let first: String = from.split('\n').next().unwrap_or("").chars().take(60).collect();
            let question = text_of(&p["question"]);
            items.push(("Copy", Action::Copy(text.clone())));
            items.push(("Find this on screen", Action::FindThis(first)));
            items.push((
                "Do it",
                Action::DoIt {
                    task: if question.is_empty() {
                        "Do what this answer describes".into()
                    } else {
                        question
                    },
                    context: format!("Earlier answer:\n{text}"),
                },
            ));
        }
        Some("hit") => {
            items.push(("Go to", Action::GoTo(index)));
            items.push(("Copy text", Action::Copy(text)));
            items.push(("Ask about this", Action::AskHit(index)));
        }
        Some("row") => {
            items.push(("Copy step", Action::Copy(text.clone())));
            let task = text_of(&p["task"]);
            if crate::store::truthy(p.get("ended")) && !task.is_empty() {
                items.push((
                    "Retry from here",
                    Action::DoIt {
                        task,
                        context: format!(
                            "An earlier attempt got as far as: {text}. Continue from that step."
                        ),
                    },
                ));
            }
        }
        _ => {}
    }
    items
}

fn run(app: &AppHandle, action: Action) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match action {
            Action::Copy(t) => {
                use tauri_plugin_clipboard_manager::ClipboardExt;
                let _ = app.clipboard().write_text(t);
            }
            Action::FindThis(t) => {
                let a = app.clone();
                let _ = app.run_on_main_thread(move || {
                    crate::windows::open_bar(&a, Some("find"), Some(t))
                });
            }
            Action::DoIt { task, context } => {
                let req = StartReq {
                    task,
                    context: Some(context),
                    ..StartReq::default()
                };
                start_run(&app, req).await;
            }
            Action::GoTo(i) => {
                services().find.step(&json!({ "index": i })).await;
            }
            Action::AskHit(i) => {
                let index = (i >= 0).then_some(i);
                super::find::find_to_ask(&app, index).await;
            }
        }
    });
}

/// `ui:context-menu`: pops the menu at the pointer in the calling window.
pub fn popup(app: &AppHandle, label: &str, p: &Value) -> Value {
    let items = items_for(p);
    if items.is_empty() {
        return json!({ "ok": true });
    }
    let Some(win) = app.get_webview_window(label) else {
        return json!({ "ok": true });
    };
    let mut built = Vec::new();
    {
        let mut map = pending().lock();
        map.clear();
        for (i, (text, action)) in items.into_iter().enumerate() {
            let id = format!("{PREFIX}{i}");
            match MenuItem::with_id(app, &id, text, true, None::<&str>) {
                Ok(item) => {
                    map.insert(id, action);
                    built.push(item);
                }
                Err(e) => crate::crash::write("context-menu", &e.to_string()),
            }
        }
    }
    let refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
        built.iter().map(|m| m as &dyn tauri::menu::IsMenuItem<tauri::Wry>).collect();
    match Menu::with_items(app, &refs) {
        Ok(menu) => {
            let w = win.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = w.popup_menu(&menu);
            });
        }
        Err(e) => crate::crash::write("context-menu", &e.to_string()),
    }
    json!({ "ok": true })
}

/// Routes clicks on these menus (ids `ctx:<n>`); other menus' ids pass by.
pub fn init(app: &AppHandle) {
    app.on_menu_event(|app, event| {
        let id = event.id().as_ref();
        if !id.starts_with(PREFIX) {
            return;
        }
        let action = pending().lock().remove(id);
        if let Some(a) = action {
            pending().lock().clear();
            run(app, a);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menus_match_main_js() {
        let answer = items_for(&json!({ "kind": "answer", "text": "line one\nline two", "question": "q" }));
        let labels: Vec<&str> = answer.iter().map(|(l, _)| *l).collect();
        assert_eq!(labels, ["Copy", "Find this on screen", "Do it"]);
        match &answer[1].1 {
            Action::FindThis(t) => assert_eq!(t, "line one"),
            other => panic!("{other:?}"),
        }
        let row = items_for(&json!({ "kind": "row", "text": "Typed", "ended": true, "task": "t" }));
        assert_eq!(row.len(), 2);
        let open = items_for(&json!({ "kind": "row", "text": "Typed", "ended": false, "task": "t" }));
        assert_eq!(open.len(), 1);
        assert!(items_for(&json!({ "kind": "other" })).is_empty());
    }
}
