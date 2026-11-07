//! The renderer's only door into Rust: one Tauri command, `aeval_invoke`,
//! routed by channel name. The pages keep calling
//! `window.aeval.invoke(channel, payload)` and `window.aeval.on(channel, cb)`
//! exactly as they did through preload.js; `bridge/aeval-bridge.js` provides
//! that object on top of Tauri. ARCHITECTURE.md section 4 is the contract.
//!
//! Phase 1 routes every channel to `not_ported` except the few a page calls
//! while it loads. Each porting agent replaces only the rows it owns.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use serde_json::Value;
use tauri::{State, WebviewWindow};

/// Channels a page may invoke. Must equal `INVOKE_CHANNELS` in preload.js
/// (checked by `invoke_allowlist_matches_preload`).
pub const INVOKE_CHANNELS: &[&str] = &[
    "overlay:trigger",
    "overlay:collapse",
    "overlay:resize",
    "overlay:set-ignore",
    "overlay:open-settings",
    "output:resize",
    "open-external",
    "query:submit",
    "query:cancel",
    "query:retry",
    "chat:new",
    "region:pick",
    "region:clear",
    "region:done",
    "region:cancel",
    "overlay:open-history",
    "history:list",
    "history:open",
    "history:remove",
    "history:clear",
    "history:close",
    "settings:get",
    "settings:save-key",
    "settings:clear-key",
    "settings:save-model",
    "settings:save-skill",
    "settings:save-brain",
    "settings:set-brain-enabled",
    "settings:set-show-paid",
    "settings:set-save-history",
    "settings:clear-history",
    "settings:set-launch-at-login",
    "settings:test-key",
    "update:check",
    "update:restart",
    "settings:close",
    "models:list",
    "clipboard:write",
    "hud:enter",
    "hud:exit",
    "hud:set-dock",
    "shortcuts:set",
    "shortcuts:reset",
    "shortcuts:capture-mode",
    "buddy:resolve-result",
    "buddy:dismiss",
    "buddy:advance",
    "buddy:set-ignore",
    "app:quit",
    "bar:mode",
    "find:open",
    "find:query",
    "find:step",
    "find:scope",
    "find:to-ask",
    "find:close",
    "agent:start",
    "agent:stop",
    "agent:confirm",
    "agent:continue",
    "agent:consent",
    "agent:do-it",
    "ui:context-menu",
    "settings:save-find-model",
    "settings:save-agent-model",
    "settings:set-find",
    "settings:set-agent",
    "tour:replay",
    "buddy:card",
    "buddy:skip",
    "find:related",
    "bar:history",
    "bar:history-push",
    "ui:focus-answer",
    "ui:focus-bar",
    "app:help",
    "settings:clip-key",
    "settings:use-clip-key",
    "hud:nudge",
    "buddy:focus",
    "glass:hello",
    "glass:passthrough",
    "settings:set-surface",
];

/// Channels a page may listen on. Must equal `LISTEN_CHANNELS` in preload.js.
pub const LISTEN_CHANNELS: &[&str] = &[
    "overlay:state",
    "hud:snapped",
    "hud:docked",
    "query:start",
    "query:stage",
    "query:delta",
    "query:done",
    "query:error",
    "chat:user",
    "chat:reset",
    "chat:sync",
    "history:changed",
    "buddy:resolve",
    "buddy:watch",
    "buddy:show",
    "buddy:hide",
    "buddy:done",
    "buddy:move",
    "settings:data",
    "update:state",
    "find:hits",
    "find:state",
    "agent:begin",
    "agent:step",
    "agent:state",
    "agent:confirm-request",
    "agent:done",
    "agent:consent-request",
    "lens:draw",
    "view:set",
    "bar:prefill",
    "settings:section",
    "bar:open",
    "bar:context",
    "glass:frame",
    "glass:tier",
    "lens:loupe",
];

const BRIDGE_JS: &str = include_str!("../bridge/aeval-bridge.js");

/// The initialization script injected into every webview: the bridge plus
/// the two allowlists, so the JS and Rust lists come from one place.
pub fn bridge_script() -> String {
    let invoke = js_list(INVOKE_CHANNELS);
    let listen = js_list(LISTEN_CHANNELS);
    format!("(() => {{\n{BRIDGE_JS}\naevalBridge({invoke}, {listen});\n}})();\n")
}

fn js_list(list: &[&str]) -> String {
    serde_json::to_string(list).unwrap_or_default()
}

/// Which windows have called through the bridge, and with which channels.
/// The scaffold smoke reads it to prove every page loaded with a working
/// `window.aeval`.
#[derive(Default)]
pub struct Seen(Mutex<BTreeMap<String, BTreeSet<String>>>);

impl Seen {
    pub fn snapshot(&self) -> BTreeMap<String, BTreeSet<String>> {
        self.0.lock().map(|m| m.clone()).unwrap_or_default()
    }

    fn record(&self, label: &str, channel: &str) {
        if let Ok(mut map) = self.0.lock() {
            map.entry(label.to_string())
                .or_default()
                .insert(channel.to_string());
        }
    }
}

#[tauri::command]
pub fn aeval_invoke(
    window: WebviewWindow,
    seen: State<'_, Seen>,
    channel: String,
    payload: Option<Value>,
) -> Result<Value, String> {
    if !INVOKE_CHANNELS.contains(&channel.as_str()) {
        return Err(format!("Blocked IPC channel: {channel}"));
    }
    seen.record(window.label(), &channel);
    let payload = payload.unwrap_or(Value::Null);
    // Rows owned by agent (c): region:*, buddy:*, glass:hello.
    if let Some(r) = crate::desktop::channels::route(&window, &channel, &payload) {
        return r;
    }
    route(&channel, payload)
}

fn route(channel: &str, payload: Value) -> Result<Value, String> {
    match channel {
        // Called by every glass page while it loads, and by pointer hover
        // logic; harmless no-ops until agents (a) and (c) port them.
        "glass:hello" | "glass:passthrough" | "overlay:set-ignore" => Ok(Value::Null),
        _ => not_ported(channel, &payload),
    }
}

fn not_ported(channel: &str, payload: &Value) -> Result<Value, String> {
    let shape = match payload {
        Value::Object(m) => m.keys().cloned().collect::<Vec<_>>().join(","),
        Value::Null => String::new(),
        _ => "value".to_string(),
    };
    Err(format!(
        "{channel} is not ported to Rust yet (payload keys: [{shape}])"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRELOAD: &str = include_str!("../../preload.js");

    fn preload_set(name: &str) -> Vec<String> {
        let start = PRELOAD
            .find(&format!("const {name} = new Set(["))
            .expect("set declared in preload.js");
        let body = &PRELOAD[start..];
        let end = body.find("]);").expect("set closed in preload.js");
        body[..end]
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with('\''))
            .map(|l| l.trim_end_matches(',').trim_matches('\'').to_string())
            .collect()
    }

    fn ours(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn invoke_allowlist_matches_preload() {
        assert_eq!(ours(INVOKE_CHANNELS), preload_set("INVOKE_CHANNELS"));
    }

    #[test]
    fn listen_allowlist_matches_preload() {
        assert_eq!(ours(LISTEN_CHANNELS), preload_set("LISTEN_CHANNELS"));
    }

    #[test]
    fn unknown_channel_is_refused_with_the_electron_wording() {
        assert!(!INVOKE_CHANNELS.contains(&"fs:read"));
        let err = not_ported("query:submit", &serde_json::json!({ "question": "x" }));
        assert_eq!(
            err.unwrap_err(),
            "query:submit is not ported to Rust yet (payload keys: [question])"
        );
    }

    #[test]
    fn bridge_script_carries_both_allowlists() {
        let s = bridge_script();
        assert!(s.contains("aevalBridge([\"overlay:trigger\""));
        assert!(s.contains("\"lens:loupe\"]);"));
    }
}
