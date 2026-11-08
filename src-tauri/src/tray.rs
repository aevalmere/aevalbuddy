//! The tray icon: the way back in when the overlay is hidden (PARITY
//! TRAY-01..08). The menu is built from data (`template`), so the smoke run
//! can check its order in every state without opening it.

use std::sync::OnceLock;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use parking_lot::Mutex;
use serde_json::{Map, Value};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

use crate::state::shared;

const TRAY_ID: &str = "main";

/// The buddy's arrow, 16 px and 32 px PNGs (from lib/tray.js).
const ICON_16: &str = concat!(
    "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAA40lEQVR42mNgQAInz98NYKAEbF5w4P/ujafmnzx/V4AsA5a1",
    "rv/f5tz2f/PCA+dPnr9rQJLmk+fvKoAMaLJqAOMVnZvenzx/N4EUAxwWVq+EGwDCc4sW/z95/i5xXgIZMCV+OooBINwfNvH/",
    "kSPXCHsJ5FxsBoAwKFx2bzyF30snz99twGUAsiEkGwASm5Y6u6DJqsGBkBfmwwyYnb/wfrdvD5gNok+ev9tPTCCuB2lssmpI",
    "QI/SrUsO3Sc2KuHRBUqRaNFJesJa1bcFHI3bVx0DGaBActIGRdvJ83f3k5Qi6Q4AIKPkKV/EBQ0AAAAASUVORK5CYII=",
);

const ICON_32: &str = concat!(
    "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAABsUlEQVR42u2Xv2rDMBDG/Qh+hD6CZ0+GzqWeujbP0LVQhKFZ",
    "mjYUCpmK+wcK9VAPKRRKIBmagKAgOmUSfgQ/gsoXJBBBDrUsK0sFt0TD/XLf3XdyEBgOZZxQxnPKeBjs4wBg+rAQlHFGGY/2",
    "AnB3OhHjk1vxtVzXlPGBb4A5ALKYiOHhUMym38KrJDqAgnibfPqTZBtAxeP5KyD6lwRJTAAI/L5crfuVhDK+aUATAAJ3si/6",
    "kQQATcn1vpCjCklSl8mjvwCoKG7eAYEYuwJI2gAg7s+eVV9AkgPvAKovFrOf7pJgxGwAtkzLXhK5iKwAVEjTspMEAPgXXQBU",
    "X1hVAgbzUaw6A1wdjRRA1NqG2wKgAUfH11UWk7kWifUeaAsguz935QOsCaDJntWScmbDJoCni6KE2WDUTOMn9U6dAMDjtQR1",
    "FpOBvKvQ3TtGL3dWAXSxTB7pJgW9m+RxIgOMQ+36LCah4b5qeitIF0yCPg+MCuU2Abxclu624g6AEKU2NaN6pHh5tuMdoENA",
    "FjkJxAfApgoYS4Boy6f2+XRPKeOlTIyonD7NWlbD/+fb/3F1fgFvqYMQkYvkrgAAAABJRU5ErkJggg==",
);

/// One menu row. `id` is what a click reports; a separator has id `-`.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: &'static str,
    pub label: String,
    /// Shown beside the label, never registered (TRAY-04).
    pub accelerator: Option<String>,
    pub enabled: bool,
}

impl Item {
    fn new(id: &'static str, label: impl Into<String>, accelerator: Option<String>) -> Self {
        Item {
            id,
            label: label.into(),
            accelerator: accelerator.filter(|a| !a.is_empty()),
            enabled: true,
        }
    }

    fn disabled(id: &'static str, label: impl Into<String>) -> Self {
        Item {
            enabled: false,
            ..Item::new(id, label, None)
        }
    }

    fn sep() -> Self {
        Item::disabled("-", "-")
    }

    pub fn is_separator(&self) -> bool {
        self.id == "-"
    }
}

/// The tray's own Check for updates answer (TRAY-05).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Manual {
    #[default]
    Idle,
    Checking,
    /// Up to date, shown until the given time (ms).
    Current {
        until: u64,
        version: String,
    },
    Failed,
}

fn manual() -> &'static Mutex<Manual> {
    static M: OnceLock<Mutex<Manual>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(Manual::Idle))
}

/// Everything the menu depends on.
pub struct Input<'a> {
    pub update: &'a Value,
    pub agent_running: bool,
    pub hidden: bool,
    pub shortcuts: &'a Map<String, Value>,
    pub manual: &'a Manual,
    pub now: u64,
}

fn check_item(u: &Value, m: &Manual, now: u64) -> Item {
    let status = u["status"].as_str().unwrap_or("");
    let version = u["version"].as_str().unwrap_or("");
    if *m == Manual::Checking {
        return Item::disabled("check", "Checking…");
    }
    if status == "downloading" && !version.is_empty() {
        let pct = (u["progress"].as_f64().unwrap_or(0.0) * 100.0).floor();
        return Item::disabled("check", format!("Downloading {version}… {pct}%"));
    }
    if let Manual::Current { until, version } = m {
        if now < *until {
            return Item::disabled("check", format!("Up to date ({version})"));
        }
    }
    if *m == Manual::Failed {
        return Item::new("check", "Update failed, try again", None);
    }
    Item::new("check", "Check for updates", None)
}

/// The menu as data, in SPEC-v2 section 4.4's order (TRAY-03).
pub fn template(i: &Input) -> Vec<Item> {
    let key = |a: &str| i.shortcuts.get(a).and_then(Value::as_str).map(String::from);
    let u = i.update;
    let mut out = Vec::new();
    if u["status"] == "ready" && u["restartPending"] == true {
        out.push(Item::disabled(
            "restarting",
            "Restarting after this answer…",
        ));
        out.push(Item::sep());
    } else if u["status"] == "ready" {
        let v = u["version"].as_str().unwrap_or("");
        out.push(Item::new(
            "restart",
            format!("Restart to update to {v}"),
            None,
        ));
        out.push(Item::sep());
    }
    if i.agent_running {
        out.push(Item::new("stop", "Stop agent", Some("Esc".into())));
        out.push(Item::sep());
    }
    out.push(Item::new(
        "capture",
        "Ask, find, or do…",
        key("captureQuery"),
    ));
    out.push(Item::new(
        "region",
        "Ask about an area…",
        key("captureRegion"),
    ));
    out.push(Item::new("find", "Find on screen…", key("findOnScreen")));
    out.push(Item::new("agent", "Do a task…", key("agentTask")));
    out.push(Item::sep());
    let toggle = if i.hidden {
        "Show the overlay"
    } else {
        "Hide the overlay"
    };
    out.push(Item::new("toggle", toggle, key("toggleVisibility")));
    out.push(Item::new("new", "New chat", None));
    out.push(Item::new("history", "History…", None));
    out.push(Item::sep());
    out.push(Item::new("settings", "Settings…", None));
    out.push(Item::new("help", "Help and feedback…", None));
    out.push(check_item(u, i.manual, i.now));
    out.push(Item::sep());
    out.push(Item::new("quit", "Quit AevalBuddy", None));
    out
}

/// Labels only, separators as `-`, for the order check.
pub fn labels(i: &Input) -> Vec<String> {
    template(i).into_iter().map(|it| it.label).collect()
}

/// TRAY-06.
pub fn tooltip(update: &Value, agent_running: bool, shortcuts: &Map<String, Value>) -> String {
    if update["status"] == "ready" {
        let v = update["version"].as_str().unwrap_or("");
        return format!("AevalBuddy · {v} is ready, restart to update");
    }
    if agent_running {
        return "AevalBuddy is using your computer · Esc to stop".into();
    }
    match shortcuts.get("captureQuery").and_then(Value::as_str) {
        Some(c) if !c.is_empty() => format!("AevalBuddy · {c}"),
        _ => "AevalBuddy".into(),
    }
}

fn icon() -> Option<Image<'static>> {
    let scale = crate::windows::screen::primary().scale;
    let b64 = if scale >= 1.5 { ICON_32 } else { ICON_16 };
    let bytes = B64.decode(b64).ok()?;
    Image::from_bytes(&bytes).ok().map(|i| i.to_owned())
}

fn current_input_parts(app: &AppHandle) -> (Value, bool, bool, Map<String, Value>) {
    let (running, hidden) = {
        let st = shared(app);
        let s = st.lock();
        (s.agent.running, s.hidden)
    };
    (
        crate::updater::get(),
        running,
        hidden,
        crate::store::get().get_shortcuts(),
    )
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let (update, agent_running, hidden, shortcuts) = current_input_parts(app);
    let m = manual().lock().clone();
    let input = Input {
        update: &update,
        agent_running,
        hidden,
        shortcuts: &shortcuts,
        manual: &m,
        now: crate::state::now_ms(),
    };
    let menu = Menu::new(app)?;
    for item in template(&input) {
        if item.is_separator() {
            menu.append(&PredefinedMenuItem::separator(app)?)?;
            continue;
        }
        // The chord is printed after a tab, which Windows right-aligns as
        // accelerator text; nothing is registered for it.
        let text = match &item.accelerator {
            Some(a) => format!("{}\t{a}", item.label),
            None => item.label.clone(),
        };
        let row = MenuItem::with_id(app, item.id, text, item.enabled, None::<&str>)?;
        menu.append(&row)?;
    }
    Ok(menu)
}

/// Creates the tray (boot, before hotkeys).
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let (update, running, _, shortcuts) = current_input_parts(app);
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip(tooltip(&update, running, &shortcuts))
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            let open = match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => true,
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => true,
                _ => false,
            };
            if open {
                let app = tray.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    crate::windows::trigger_or_trail(&app);
                });
            }
        });
    if let Some(img) = icon() {
        builder = builder.icon(img);
    }
    builder.build(app)?;
    Ok(())
}

/// Whether the tray came up (smoke `tray`).
pub fn is_ready(app: &AppHandle) -> bool {
    app.tray_by_id(TRAY_ID).is_some()
}

/// Rebuilds menu and tooltip; called whenever a label could change.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    if let Ok(menu) = build_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
    let (update, running, _, shortcuts) = current_input_parts(app);
    let _ = tray.set_tooltip(Some(tooltip(&update, running, &shortcuts)));
}

pub fn destroy(app: &AppHandle) {
    let _ = app.remove_tray_by_id(TRAY_ID);
}

fn run_check(app: &AppHandle) {
    *manual().lock() = Manual::Checking;
    refresh(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let u = crate::updater::check(&app).await;
        let next = match u["status"].as_str().unwrap_or("") {
            "current" => Manual::Current {
                until: crate::state::now_ms() + 10_000,
                version: u["current"].as_str().unwrap_or("").to_string(),
            },
            "downloading" | "ready" | "dev" => Manual::Idle,
            _ => Manual::Failed,
        };
        let held = matches!(next, Manual::Current { .. });
        *manual().lock() = next;
        refresh(&app);
        if held {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            let mut m = manual().lock();
            if matches!(*m, Manual::Current { .. }) {
                *m = Manual::Idle;
            }
            drop(m);
            refresh(&app);
        }
    });
}

fn on_menu(app: &AppHandle, id: &str) {
    let app = app.clone();
    let id = id.to_string();
    tauri::async_runtime::spawn(async move {
        use crate::windows as w;
        match id.as_str() {
            "restart" => {
                crate::updater::restart(&app);
            }
            "stop" => {
                shared(&app).call(&app, |h| h.stop_agent.as_ref());
            }
            "capture" => w::open_bar(&app, None, None),
            "region" => {
                shared(&app).call(&app, |h| h.start_region.as_ref());
            }
            "find" => w::open_bar(&app, Some("find"), None),
            "agent" => w::open_bar(&app, Some("agent"), None),
            "toggle" => w::toggle_visibility(&app),
            "new" => w::new_chat(&app),
            "history" => w::open_history(&app),
            "settings" => w::open_settings(&app, None),
            "help" => {
                w::open_help(&app);
            }
            "check" => run_check(&app),
            "quit" => app.exit(0),
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn shortcuts() -> Map<String, Value> {
        crate::store::default_shortcuts()
    }

    fn labels_for(update: Value, running: bool, hidden: bool, m: Manual) -> Vec<String> {
        let s = shortcuts();
        labels(&Input {
            update: &update,
            agent_running: running,
            hidden,
            shortcuts: &s,
            manual: &m,
            now: 1000,
        })
    }

    #[test]
    fn idle_order() {
        let l = labels_for(json!({"status": "idle"}), false, false, Manual::Idle);
        assert_eq!(
            l,
            vec![
                "Ask, find, or do…",
                "Ask about an area…",
                "Find on screen…",
                "Do a task…",
                "-",
                "Hide the overlay",
                "New chat",
                "History…",
                "-",
                "Settings…",
                "Help and feedback…",
                "Check for updates",
                "-",
                "Quit AevalBuddy",
            ]
        );
    }

    #[test]
    fn ready_running_hidden() {
        let u = json!({"status": "ready", "version": "1.2.0"});
        let l = labels_for(u, true, true, Manual::Idle);
        assert_eq!(
            &l[..4],
            ["Restart to update to 1.2.0", "-", "Stop agent", "-"]
        );
        assert!(l.contains(&"Show the overlay".to_string()));
        let pending = json!({"status": "ready", "version": "1.2.0", "restartPending": true});
        let l = labels_for(pending, false, false, Manual::Idle);
        assert_eq!(l[0], "Restarting after this answer…");
    }

    #[test]
    fn check_states() {
        let u = json!({"status": "downloading", "version": "1.2.0", "progress": 0.426});
        let s = shortcuts();
        let row = |u: &Value, m: Manual, now| check_item(u, &m, now).label;
        assert_eq!(row(&json!({}), Manual::Checking, 0), "Checking…");
        assert_eq!(row(&u, Manual::Idle, 0), "Downloading 1.2.0… 42%");
        let cur = Manual::Current {
            until: 500,
            version: "1.1.1".into(),
        };
        assert_eq!(row(&json!({}), cur.clone(), 100), "Up to date (1.1.1)");
        assert_eq!(row(&json!({}), cur, 900), "Check for updates");
        assert_eq!(
            row(&json!({}), Manual::Failed, 0),
            "Update failed, try again"
        );
        let items = template(&Input {
            update: &json!({}),
            agent_running: true,
            hidden: false,
            shortcuts: &s,
            manual: &Manual::Idle,
            now: 0,
        });
        assert_eq!(items[0].accelerator.as_deref(), Some("Esc"));
        assert_eq!(items[2].accelerator.as_deref(), Some("Ctrl+Alt+Space"));
        assert_eq!(items[3].accelerator, None);
    }

    #[test]
    fn tooltips() {
        let s = shortcuts();
        let ready = json!({"status": "ready", "version": "2.0.0"});
        assert_eq!(
            tooltip(&ready, true, &s),
            "AevalBuddy · 2.0.0 is ready, restart to update"
        );
        assert_eq!(
            tooltip(&json!({}), true, &s),
            "AevalBuddy is using your computer · Esc to stop"
        );
        assert_eq!(
            tooltip(&json!({}), false, &s),
            "AevalBuddy · Ctrl+Alt+Space"
        );
        assert_eq!(tooltip(&json!({}), false, &Map::new()), "AevalBuddy");
    }

    #[test]
    fn icons_decode() {
        for b64 in [ICON_16, ICON_32] {
            let bytes = B64.decode(b64).unwrap();
            assert_eq!(&bytes[1..4], b"PNG");
        }
    }
}
