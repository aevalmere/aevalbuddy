//! The IPC rows split (c) owns (ARCHITECTURE 4.4): region:*, buddy:* and
//! glass:hello. `ipc::aeval_invoke` asks here first; `None` means the channel
//! is someone else's. `aeval_invoke` is a sync command, so it runs on the
//! event loop thread: anything that builds a window or waits is moved off it.
//!
//! glass:passthrough stays with agent (a): it needs the output window's
//! click-through with forwarding (windows/flags.rs).

use serde_json::Value;
use tauri::{Manager, WebviewWindow};

use crate::{backdrop, buddy, region};

fn off_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(f);
}

pub fn route(
    window: &WebviewWindow,
    channel: &str,
    payload: &Value,
) -> Option<Result<Value, String>> {
    let ok = Some(Ok(Value::Null));
    match channel {
        "glass:hello" => {
            backdrop::hello(window.label());
            // The page's first state (ARCHITECTURE 10.3, 11.2).
            crate::windows::on_hello(window.app_handle(), window.label());
            Some(Ok(
                serde_json::json!({ "intensity": backdrop::intensity() }),
            ))
        }
        "region:pick" => {
            let app = window.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                region::pick(&app).await;
            });
            ok
        }
        "region:clear" => {
            region::clear_active();
            ok
        }
        "region:done" => {
            let app = window.app_handle().clone();
            region::finish(&app, payload);
            ok
        }
        "region:cancel" => {
            let app = window.app_handle().clone();
            region::cancel(&app);
            ok
        }
        "buddy:resolve-result" => {
            let p = payload.clone();
            off_thread(move || buddy::handle_resolve_result(&p));
            ok
        }
        "buddy:dismiss" => {
            off_thread(|| buddy::notify("dismiss"));
            ok
        }
        "buddy:advance" => {
            off_thread(buddy::advance);
            ok
        }
        "buddy:set-ignore" => {
            let ignore = payload["ignore"].as_bool().unwrap_or(true);
            buddy::set_click_through(ignore);
            ok
        }
        "buddy:skip" => {
            off_thread(buddy::skip);
            ok
        }
        "buddy:focus" => {
            buddy::focus_card();
            ok
        }
        "buddy:card" => {
            let id = payload["id"].as_str().unwrap_or("").to_string();
            off_thread(move || buddy::card(&id));
            ok
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_owned_channel_is_allowlisted() {
        for c in [
            "glass:hello",
            "region:pick",
            "region:clear",
            "region:done",
            "region:cancel",
            "buddy:resolve-result",
            "buddy:dismiss",
            "buddy:advance",
            "buddy:set-ignore",
            "buddy:skip",
            "buddy:focus",
            "buddy:card",
        ] {
            assert!(crate::ipc::INVOKE_CHANNELS.contains(&c), "{c}");
        }
    }
}
