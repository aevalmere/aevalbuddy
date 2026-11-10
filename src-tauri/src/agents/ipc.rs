//! The `agents:*` channels. The card page says when it is ready, how tall
//! the card is, when the pointer is on it, and what was clicked; Settings
//! reads and changes the Agents page. `None` for any other channel, which
//! falls through to the next router.

use serde_json::{json, Value};
use tauri::AppHandle;

pub async fn route(app: &AppHandle, channel: &str, p: &Value) -> Option<Result<Value, String>> {
    let out = match channel {
        // The page loaded (or reloaded): it gets the card that is up.
        "agents:ready" => {
            super::send_card(app, &super::prefs());
            json!({ "ok": true })
        }
        "agents:size" => super::page_size(app, p),
        // The card has folded away: the window may hide.
        "agents:hidden" => super::page_hidden(app),
        // The countdown waits while the pointer is on the card.
        "agents:hover" => {
            let on = p.get("on").and_then(Value::as_bool).unwrap_or(false);
            super::state().lock().hover = on;
            json!({ "ok": true })
        }
        "agents:open" => super::open(p),
        "agents:dismiss" => {
            super::hide_card();
            json!({ "ok": true })
        }
        "agents:act" => super::act(p),
        "agents:get" => super::snapshot(),
        "agents:set" => {
            let patch = p.clone();
            // Hooks are files in other tools' folders: off the IPC thread.
            tauri::async_runtime::spawn_blocking(move || super::set(&patch))
                .await
                .map_err(|e| e.to_string())
                .unwrap_or_else(|why| json!({ "error": why }))
        }
        "agents:test" => super::test_card(),
        _ => return None,
    };
    Some(Ok(out))
}
