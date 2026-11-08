//! Right-click menus inside the windows (main.js contextMenu, SPEC-v2 4.5):
//! an answer (Copy, Find this on screen, Do it), a Find hit (Go to, Copy
//! text, Ask about this) and a trail row (Copy step, Retry from here). The
//! bridge asks for the edit menu on a text field and Copy / Select all on a
//! selection (main.js attachContextMenu, WIN-14).

use std::collections::HashMap;
use std::sync::OnceLock;

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Manager, Wry};

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

type Item = Box<dyn IsMenuItem<Wry>>;

/// The actions of the menu shown last, by item id. Locked only to swap a
/// map in or take one out, never across a main-thread round trip.
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
            let first: String = from
                .split('\n')
                .next()
                .unwrap_or("")
                .chars()
                .take(60)
                .collect();
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

/// main.js attachContextMenu, by Electron role (`-` is a separator): a
/// text field gets the edit commands, a selection Copy and Select all.
pub fn edit_template(kind: &str) -> &'static [&'static str] {
    match kind {
        "edit" => &["cut", "copy", "paste", "-", "selectAll"],
        "selection" => &["copy", "selectAll"],
        _ => &[],
    }
}

fn predefined(app: &AppHandle, role: &str) -> tauri::Result<PredefinedMenuItem<Wry>> {
    match role {
        "cut" => PredefinedMenuItem::cut(app, None),
        "copy" => PredefinedMenuItem::copy(app, None),
        "paste" => PredefinedMenuItem::paste(app, None),
        "selectAll" => PredefinedMenuItem::select_all(app, None),
        _ => PredefinedMenuItem::separator(app),
    }
}

/// Makes one menu's items with `make`, then swaps their actions in as the
/// pending ones. Making an item is a round trip to the main thread, where a
/// click may be waiting on `pending`, so the lock is never held meanwhile.
fn stage<T>(
    items: Vec<(&'static str, Action)>,
    mut make: impl FnMut(&str, &'static str) -> Result<T, String>,
) -> Vec<T> {
    let mut built = Vec::new();
    let mut actions = HashMap::new();
    for (i, (text, action)) in items.into_iter().enumerate() {
        let id = format!("{PREFIX}{i}");
        match make(&id, text) {
            Ok(item) => {
                actions.insert(id, action);
                built.push(item);
            }
            Err(e) => crate::crash::write("context-menu", &e),
        }
    }
    *pending().lock() = actions;
    built
}

fn run(app: &AppHandle, action: Action) {
    let app = app.clone();
    let work = async move {
        match action {
            Action::Copy(t) => {
                use tauri_plugin_clipboard_manager::ClipboardExt;
                let _ = app.clipboard().write_text(t);
            }
            Action::FindThis(t) => {
                let a = app.clone();
                let _ = app.run_on_main_thread(move || {
                    crate::crash::guard("context-menu.find", || {
                        crate::windows::open_bar(&a, Some("find"), Some(t));
                    });
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
    };
    tauri::async_runtime::spawn(crate::crash::guard_async("context-menu.run", work));
}

/// `ui:context-menu`: pops the menu at the pointer in the calling window.
pub fn popup(app: &AppHandle, label: &str, p: &Value) -> Value {
    let done = json!({ "ok": true });
    let roles = edit_template(p["kind"].as_str().unwrap_or(""));
    let items = items_for(p);
    if roles.is_empty() && items.is_empty() {
        return done;
    }
    let Some(win) = app.get_webview_window(label) else {
        return done;
    };
    let mut built: Vec<Item> = Vec::new();
    if roles.is_empty() {
        built = stage(items, |id, text| {
            let made = MenuItem::with_id(app, id, text, true, None::<&str>);
            let item = made.map_err(|e| e.to_string())?;
            Ok(Box::new(item) as Item)
        });
    } else {
        for role in roles {
            match predefined(app, role) {
                Ok(m) => built.push(Box::new(m)),
                Err(e) => crate::crash::write("context-menu", &e.to_string()),
            }
        }
    }
    let refs: Vec<&dyn IsMenuItem<Wry>> = built.iter().map(|m| &**m).collect();
    match Menu::with_items(app, &refs) {
        Ok(menu) => {
            let _ = app.run_on_main_thread(move || {
                crate::crash::guard("context-menu.popup", || {
                    let _ = win.popup_menu(&menu);
                });
            });
        }
        Err(e) => crate::crash::write("context-menu", &e.to_string()),
    }
    done
}

/// Routes clicks on these menus (ids `ctx:<n>`); other menus' ids pass by.
pub fn init(app: &AppHandle) {
    app.on_menu_event(|app, event| {
        crate::crash::guard("context-menu.click", || {
            let id = event.id().as_ref();
            if !id.starts_with(PREFIX) {
                return;
            }
            let action = std::mem::take(&mut *pending().lock()).remove(id);
            if let Some(a) = action {
                run(app, a);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menus_match_main_js() {
        let answer =
            items_for(&json!({ "kind": "answer", "text": "line one\nline two", "question": "q" }));
        let labels: Vec<&str> = answer.iter().map(|(l, _)| *l).collect();
        assert_eq!(labels, ["Copy", "Find this on screen", "Do it"]);
        match &answer[1].1 {
            Action::FindThis(t) => assert_eq!(t, "line one"),
            other => panic!("{other:?}"),
        }
        let row = items_for(&json!({ "kind": "row", "text": "Typed", "ended": true, "task": "t" }));
        assert_eq!(row.len(), 2);
        let open =
            items_for(&json!({ "kind": "row", "text": "Typed", "ended": false, "task": "t" }));
        assert_eq!(open.len(), 1);
        assert!(items_for(&json!({ "kind": "other" })).is_empty());
    }

    #[test]
    fn edit_and_selection_templates_match_main_js() {
        assert_eq!(
            edit_template("edit"),
            ["cut", "copy", "paste", "-", "selectAll"]
        );
        assert_eq!(edit_template("selection"), ["copy", "selectAll"]);
        assert!(edit_template("answer").is_empty());
        assert!(edit_template("").is_empty());
    }

    #[test]
    fn pending_lock_not_held_while_building() {
        let items = items_for(&json!({ "kind": "hit", "index": 2, "text": "t" }));
        let built = stage(items, |id, text| {
            let free = pending().try_lock().is_some();
            assert!(free, "pending held while making {id}");
            Ok::<_, String>(format!("{id} {text}"))
        });
        let want = ["ctx:0 Go to", "ctx:1 Copy text", "ctx:2 Ask about this"];
        assert_eq!(built, want);
        let action = std::mem::take(&mut *pending().lock()).remove("ctx:1");
        assert!(matches!(action, Some(Action::Copy(t)) if t == "t"));
        assert!(pending().lock().is_empty());
    }
}
