//! One entry per coding agent: where its hooks live, what an entry looks
//! like, and how its payload maps onto the events `core` works in. Port of
//! Aevalsistant's `Agents.cs`.
//!
//! Every agent with a hook system gets the same treatment: one entry per
//! lifecycle event that runs `AevalBuddy.exe --hook <agent>`. The config
//! formats differ in three ways:
//!
//! - Grouped: `event -> [ { matcher?, hooks: [ {type, command, ..} ] } ]`
//!   (Claude Code, Codex, Gemini CLI)
//! - Flat: `event -> [ {command, ..} ]` (Cursor, Windsurf)
//! - Own: a whole file that belongs to this app (Copilot CLI, OpenCode)
//!
//! Everything in a file that is not ours stays as it was.

use serde_json::{json, Map, Value};

use super::core::HookEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Grouped,
    Flat,
    Own,
}

#[derive(Debug)]
pub struct AgentSpec {
    /// The argument after `--hook`.
    pub id: &'static str,
    /// Shown in cards and Settings.
    pub name: &'static str,
    /// The config folder under the user profile; the agent counts as
    /// installed when it exists.
    pub dir: &'static str,
    /// The file edited or owned, relative to the profile.
    pub file: &'static str,
    pub layout: Layout,
    pub events: &'static [&'static str],
    /// A one-time step the person has to take, shown in Settings.
    pub note: &'static str,
}

const COPILOT_EVENTS: [&str; 4] = [
    "userPromptSubmitted",
    "agentStop",
    "errorOccurred",
    "sessionEnd",
];

pub const ALL: [AgentSpec; 7] = [
    AgentSpec {
        id: "claude",
        name: "Claude Code",
        dir: ".claude",
        file: ".claude\\settings.json",
        layout: Layout::Grouped,
        events: &[
            "UserPromptSubmit",
            "Stop",
            "StopFailure",
            "Notification",
            "SubagentStart",
            "SubagentStop",
            "SessionEnd",
        ],
        note: "Restart open sessions once.",
    },
    AgentSpec {
        id: "codex",
        name: "Codex",
        dir: ".codex",
        file: ".codex\\hooks.json",
        layout: Layout::Grouped,
        events: &[
            "UserPromptSubmit",
            "Stop",
            "PermissionRequest",
            "Interrupt",
            "SubagentStart",
            "SubagentStop",
            "SessionEnd",
        ],
        note: "Run /hooks once in Codex to trust the new hooks.",
    },
    AgentSpec {
        id: "gemini",
        name: "Gemini CLI",
        dir: ".gemini",
        file: ".gemini\\settings.json",
        layout: Layout::Grouped,
        events: &["BeforeAgent", "AfterAgent", "Notification", "SessionEnd"],
        note: "Restart open sessions once.",
    },
    AgentSpec {
        id: "copilot",
        name: "Copilot CLI",
        dir: ".copilot",
        file: ".copilot\\hooks\\aevalbuddy.json",
        layout: Layout::Own,
        events: &COPILOT_EVENTS,
        note: "Restart open sessions once.",
    },
    AgentSpec {
        id: "cursor",
        name: "Cursor",
        dir: ".cursor",
        file: ".cursor\\hooks.json",
        layout: Layout::Flat,
        events: &["beforeSubmitPrompt", "afterAgentResponse", "stop", "sessionEnd"],
        note: "",
    },
    AgentSpec {
        id: "windsurf",
        name: "Windsurf",
        dir: ".codeium\\windsurf",
        file: ".codeium\\windsurf\\hooks.json",
        layout: Layout::Flat,
        events: &["pre_user_prompt", "post_cascade_response"],
        note: "",
    },
    AgentSpec {
        id: "opencode",
        name: "OpenCode",
        dir: ".config\\opencode",
        file: ".config\\opencode\\plugins\\aevalbuddy.js",
        layout: Layout::Own,
        events: &[],
        note: "Restart OpenCode once.",
    },
];

/// The spec for `id`; Claude Code's for an id this build does not know.
pub fn get(id: &str) -> &'static AgentSpec {
    ALL.iter().find(|a| a.id == id).unwrap_or(&ALL[0])
}

/// One hook entry for `event`.
fn entry(a: &AgentSpec, event: &str, cmd: &str) -> Value {
    match a.id {
        // Async so Claude Code never waits on us; SessionEnd stays
        // synchronous so the message leaves before the session exits.
        "claude" if event == "SessionEnd" => {
            json!({ "type": "command", "command": cmd, "timeout": 10 })
        }
        // Interrupt and SessionEnd allow at most 3 seconds.
        "codex" if event == "SessionEnd" || event == "Interrupt" => {
            json!({ "type": "command", "command": cmd, "timeout": 3 })
        }
        "claude" | "codex" => {
            json!({ "type": "command", "command": cmd, "timeout": 10, "async": true })
        }
        // Milliseconds.
        "gemini" => {
            json!({ "name": "aevalbuddy", "type": "command", "command": cmd, "timeout": 5000 })
        }
        // On Windows Windsurf runs the "powershell" field.
        "windsurf" => {
            json!({ "command": cmd, "powershell": powershell(cmd), "show_output": false })
        }
        _ => json!({ "command": cmd, "timeout": 10 }),
    }
}

/// The whole file for an `Own` agent.
fn own_content(a: &AgentSpec, cmd: &str) -> String {
    if a.id == "opencode" {
        return opencode_plugin(cmd);
    }
    // Copilot's camelCase payloads carry no event name, so the command line
    // does. On Windows Copilot runs the "powershell" field, falling back to
    // "command".
    let mut hooks = Map::new();
    for ev in COPILOT_EVENTS {
        let one = json!({
            "type": "command",
            "command": format!("{cmd} {ev}"),
            "powershell": format!("{} {ev}", powershell(cmd)),
            "timeoutSec": 10,
        });
        hooks.insert(ev.to_string(), json!([one]));
    }
    let root = json!({ "version": 1, "hooks": hooks });
    format!("{}\n", pretty(&root))
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

/// PowerShell runs a quoted path only behind its call operator.
fn powershell(command: &str) -> String {
    if command.starts_with('"') {
        format!("& {command}")
    } else {
        command.to_string()
    }
}

fn shell_safe(part: &str) -> bool {
    !part.is_empty()
        && part
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | '~' | ':'))
}

/// The exe path as a hook command can carry it. Hook commands run through
/// whichever shell each agent uses (bash, cmd or PowerShell), so the path
/// should work unquoted in all of them: forward slashes, and a folder whose
/// name has a space or a shell character swapped for its 8.3 short name
/// (`short_name` of the path up to that folder). With short names turned
/// off the path is quoted, which bash and cmd accept.
pub fn hook_path(exe: &str, short_name: impl Fn(&str) -> Option<String>) -> String {
    let parts: Vec<&str> = exe.split('\\').collect();
    let mut result: Vec<String> = parts.iter().map(|p| p.to_string()).collect();
    for i in 1..parts.len() {
        if shell_safe(parts[i]) {
            continue;
        }
        let upto = parts[..=i].join("\\");
        if let Some(short) = short_name(&upto) {
            if let Some(last) = short.rsplit('\\').next().filter(|l| !l.is_empty()) {
                result[i] = last.to_string();
            }
        }
    }
    let path = result.join("/");
    if result.iter().all(|p| shell_safe(p)) {
        path
    } else {
        format!("\"{path}\"")
    }
}

/// Claude Code takes the bare `--hook` form; every other agent names itself.
pub fn command_for(a: &AgentSpec, base: &str) -> String {
    if a.id == "claude" {
        base.to_string()
    } else {
        format!("{base} {}", a.id)
    }
}

/// Where a session runs, as shown after the title: "VS Code",
/// "Codex · Terminal", "Cursor".
pub fn place(agent: &str, host: &str) -> String {
    if agent.is_empty() || agent == "claude" || agent == "chat" {
        return host.to_string();
    }
    let name = get(agent).name;
    if host.is_empty() || host == name {
        name.to_string()
    } else {
        format!("{name} \u{b7} {host}")
    }
}

/// Whether a hook command is one this app wrote.
pub fn is_ours(command: &str) -> bool {
    command.to_lowercase().contains("aevalbuddy") && command.contains("--hook")
}

fn entry_is_ours(entry: &Value) -> bool {
    let has = |k: &str| entry.get(k).and_then(Value::as_str).is_some_and(is_ours);
    entry.is_object() && (has("command") || has("powershell"))
}

/// Takes our entries out of one event's list.
fn strip(list: &mut Vec<Value>, layout: Layout) {
    if layout == Layout::Flat {
        list.retain(|h| !entry_is_ours(h));
        return;
    }
    list.retain_mut(|group| {
        let Some(inner) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
            return true;
        };
        let before = inner.len();
        inner.retain(|h| !entry_is_ours(h));
        // A group that held only ours goes with them.
        !(inner.len() < before && inner.is_empty())
    });
}

/// The file text with our hooks added (`command` given) or removed (`None`),
/// and whether that changed anything. `Ok(None)` means the file should not
/// exist (an owned file on removal).
pub fn apply(
    a: &AgentSpec,
    text: Option<&str>,
    command: Option<&str>,
) -> Result<(Option<String>, bool), String> {
    if a.layout == Layout::Own {
        let want = command.map(|c| own_content(a, c));
        let changed = want.as_deref() != text;
        return Ok((want, changed));
    }
    let name = a.file.rsplit('\\').next().unwrap_or(a.file);
    let blank = text.is_none_or(|t| t.trim().is_empty());
    let mut root: Map<String, Value> = if blank {
        Map::new()
    } else {
        match serde_json::from_str::<Value>(text.unwrap_or_default()) {
            Ok(Value::Object(m)) => m,
            Ok(_) => return Err(format!("{name} is not a JSON object")),
            Err(e) => return Err(format!("{name} could not be read: {e}")),
        }
    };
    let before = (!blank).then(|| pretty(&Value::Object(root.clone())));

    if a.id == "cursor" && command.is_some() && !root.contains_key("version") {
        root.insert("version".into(), json!(1));
    }
    if root.get("hooks").is_some_and(|h| !h.is_object()) {
        return Err(format!("\"hooks\" in {name} is not an object"));
    }
    if command.is_some() && !root.contains_key("hooks") {
        root.insert("hooks".into(), json!({}));
    }
    if let Some(Value::Object(hooks)) = root.get_mut("hooks") {
        for ev in a.events {
            if let Some(Value::Array(list)) = hooks.get_mut(*ev) {
                strip(list, a.layout);
            }
            match command {
                Some(cmd) => {
                    let one = entry(a, ev, cmd);
                    let item = if a.layout == Layout::Grouped {
                        json!({ "hooks": [one] })
                    } else {
                        one
                    };
                    match hooks.get_mut(*ev) {
                        Some(Value::Array(list)) => list.push(item),
                        _ => {
                            hooks.insert((*ev).to_string(), json!([item]));
                        }
                    }
                }
                None => {
                    let empty = hooks
                        .get(*ev)
                        .and_then(Value::as_array)
                        .is_some_and(Vec::is_empty);
                    if empty {
                        hooks.shift_remove(*ev);
                    }
                }
            }
        }
    }
    let no_hooks = root
        .get("hooks")
        .and_then(Value::as_object)
        .is_some_and(Map::is_empty);
    if command.is_none() && no_hooks {
        root.shift_remove("hooks");
    }
    let after = pretty(&Value::Object(root));
    let changed = match &before {
        None => command.is_some(),
        Some(b) => *b != after,
    };
    Ok((Some(format!("{after}\n")), changed))
}

/// Whether every event of `a` already runs `command`.
pub fn is_installed(a: &AgentSpec, text: Option<&str>, command: &str) -> bool {
    if a.layout == Layout::Own {
        return text.is_some_and(|t| t == own_content(a, command));
    }
    let Some(root) = text.and_then(|t| serde_json::from_str::<Value>(t).ok()) else {
        return false;
    };
    let Some(hooks) = root.get("hooks").and_then(Value::as_object) else {
        return false;
    };
    let runs = |entry: &Value| entry.get("command").and_then(Value::as_str) == Some(command);
    a.events.iter().all(|ev| {
        let Some(list) = hooks.get(*ev).and_then(Value::as_array) else {
            return false;
        };
        list.iter().any(|item| match a.layout {
            Layout::Flat => runs(item),
            _ => item
                .get("hooks")
                .and_then(Value::as_array)
                .is_some_and(|inner| inner.iter().any(runs)),
        })
    })
}

// ---------------------------------------------------------------- adapters

/// Fills `e` from one agent's hook payload. Event names come out as Claude
/// Code's, which is what `SessionBook` works in; Interrupt and AgentText are
/// the two additions. `e.name` may already hold the event from the command
/// line (Copilot).
pub fn normalize(agent: &str, h: &Value, e: &mut HookEvent, pwd: &str) {
    let s = |k: &str| h.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    match agent {
        "codex" => {
            e.session_id = s("session_id");
            e.cwd = s("cwd");
            e.transcript = s("transcript_path");
            e.agent_id = s("agent_id");
            e.agent_type = s("agent_type");
            e.last_assistant = s("last_assistant_message");
            e.name = s("hook_event_name");
            if e.name == "PermissionRequest" {
                e.name = "Notification".into();
                e.notification_type = "permission_prompt".into();
                let tool = s("tool_name");
                let why = h["tool_input"]["description"].as_str().unwrap_or_default();
                e.message = if !why.is_empty() {
                    why.to_string()
                } else if !tool.is_empty() {
                    format!("Codex wants to use {tool}")
                } else {
                    "Codex is asking for permission.".to_string()
                };
            }
        }
        "gemini" => {
            e.session_id = s("session_id");
            e.cwd = s("cwd");
            e.transcript = s("transcript_path");
            match s("hook_event_name").as_str() {
                "BeforeAgent" => e.name = "UserPromptSubmit".into(),
                "AfterAgent" => {
                    e.name = "Stop".into();
                    e.last_assistant = s("prompt_response");
                }
                "SessionEnd" => e.name = "SessionEnd".into(),
                "Notification" => {
                    // Gemini also sends errors, warnings and info here. Only
                    // a tool permission request needs you; versions without
                    // the type sent only those.
                    let kind = s("notification_type");
                    if kind.is_empty() || kind == "ToolPermission" {
                        e.name = "Notification".into();
                        e.notification_type = "permission_prompt".into();
                        e.message = or(s("message"), "Gemini is asking for permission.");
                    }
                }
                _ => {}
            }
        }
        "copilot" => {
            e.session_id = or(s("sessionId"), &s("session_id"));
            e.cwd = s("cwd");
            e.transcript = s("transcriptPath");
            e.name = match e.name.as_str() {
                "userPromptSubmitted" => "UserPromptSubmit",
                "agentStop" => "Stop",
                "sessionEnd" => "SessionEnd",
                // A recoverable error does not end the turn.
                "errorOccurred" if h["recoverable"] == true => "",
                "errorOccurred" => "StopFailure",
                other => other,
            }
            .to_string();
        }
        "cursor" => {
            e.session_id = s("conversation_id");
            e.transcript = s("transcript_path");
            e.cwd = h["workspace_roots"]
                .as_array()
                .and_then(|roots| roots.iter().find_map(Value::as_str))
                .unwrap_or_default()
                .to_string();
            match s("hook_event_name").as_str() {
                "beforeSubmitPrompt" => e.name = "UserPromptSubmit".into(),
                "afterAgentResponse" => {
                    e.name = "AgentText".into();
                    e.last_assistant = s("text");
                }
                "sessionEnd" => e.name = "SessionEnd".into(),
                "stop" => {
                    e.name = match s("status").as_str() {
                        "aborted" => "Interrupt",
                        "error" => "StopFailure",
                        _ => "Stop",
                    }
                    .to_string();
                }
                _ => {}
            }
        }
        "windsurf" => {
            e.session_id = s("trajectory_id");
            match s("agent_action_name").as_str() {
                "pre_user_prompt" => e.name = "UserPromptSubmit".into(),
                "post_cascade_response" => {
                    e.name = "Stop".into();
                    let reply = h["tool_info"]["response"].as_str().unwrap_or_default();
                    e.last_assistant = reply.to_string();
                }
                _ => {}
            }
        }
        "opencode" => {
            // Sent by our own plugin: {"event": "..", "session": "..",
            // "cwd": ".."}.
            e.session_id = s("session");
            e.cwd = s("cwd");
            match s("event").as_str() {
                "busy" => e.name = "UserPromptSubmit".into(),
                "idle" => e.name = "Stop".into(),
                "error" => e.name = "StopFailure".into(),
                "permission" => {
                    e.name = "Notification".into();
                    e.notification_type = "permission_prompt".into();
                    e.message = or(s("message"), "OpenCode is asking for permission.");
                }
                _ => {}
            }
        }
        _ => {
            // Claude Code.
            e.name = s("hook_event_name");
            e.session_id = s("session_id");
            e.cwd = s("cwd");
            e.transcript = s("transcript_path");
            e.message = s("message");
            e.notification_type = s("notification_type");
            e.last_assistant = s("last_assistant_message");
            e.agent_id = s("agent_id");
            e.agent_type = s("agent_type");
        }
    }
    // No folder in the payload: the hook runs in the agent's working folder,
    // which is the next best guess. A guess never replaces a folder the
    // session already has.
    if e.cwd.is_empty() && !pwd.is_empty() {
        e.cwd = pwd.to_string();
        e.cwd_guessed = true;
    }
    // Session ids are only unique per agent.
    if !e.session_id.is_empty() && agent != "claude" {
        e.session_id = format!("{agent}:{}", e.session_id);
    }
}

fn or(text: String, default: &str) -> String {
    if text.is_empty() {
        default.to_string()
    } else {
        text
    }
}

/// OpenCode loads plugins from `~/.config/opencode/plugins`. This one
/// forwards session state changes to the same `--hook` entry point the other
/// agents use.
fn opencode_plugin(command: &str) -> String {
    let exe = match command.rfind(" --hook") {
        Some(cut) if cut > 0 => &command[..cut],
        _ => command,
    };
    let exe = serde_json::to_string(exe.trim_matches('"')).unwrap_or_default();
    OPENCODE_PLUGIN.replace("__EXE__", &exe)
}

const OPENCODE_PLUGIN: &str = r#"// AevalBuddy: tells the app when an OpenCode session starts and stops working.
// Written by AevalBuddy; removed by turning off "Connect coding agents" in its Settings.
const EXE = __EXE__;

export const AevalBuddy = async ({ directory }) => {
  const send = (event, session, message) => {
    try {
      const body = JSON.stringify({ event, session: session || "", cwd: directory || "", message: message || "" });
      Bun.spawn([EXE, "--hook", "opencode"], { stdin: new Blob([body]), stdout: "ignore", stderr: "ignore" });
    } catch (e) {
      // AevalBuddy is not installed or not reachable; OpenCode carries on.
    }
  };
  // session.status and the older session.idle both report a finish, so a finish is sent only
  // for a session that was busy, and only once.
  const busy = {};
  return {
    event: async ({ event }) => {
      const p = event.properties || {};
      const id = p.sessionID || (p.info && p.info.id) || "";
      const status = event.type === "session.status" && p.status ? p.status.type : "";
      if (status === "busy") { if (!busy[id]) { busy[id] = true; send("busy", id); } }
      else if (status === "idle" || event.type === "session.idle") { if (busy[id]) { busy[id] = false; send("idle", id); } }
      else if (event.type === "session.error") { busy[id] = false; send("error", id); }
      else if (event.type === "permission.asked") {
        const what = (p.permission || "") + (p.patterns && p.patterns.length ? ": " + p.patterns.join(", ") : "");
        send("permission", id, what ? "OpenCode wants to use " + what : "");
      }
    },
  };
};
"#;

#[cfg(test)]
mod tests {
    use super::*;

    const CMD: &str = "C:/Users/me/AppData/Local/AevalBuddy/AevalBuddy.exe --hook";

    fn applied(a: &AgentSpec, text: Option<&str>, cmd: Option<&str>) -> (Option<String>, bool) {
        apply(a, text, cmd).unwrap()
    }

    #[test]
    fn hooks_go_in_once_and_come_out_clean() {
        let claude = get("claude");
        let theirs = r#"{
  "model": "opus",
  "hooks": {
    "Stop": [
      { "hooks": [ { "type": "command", "command": "say done" } ] }
    ]
  }
}"#;
        let (text, changed) = applied(claude, Some(theirs), Some(CMD));
        let text = text.unwrap();
        assert!(changed);
        assert!(is_installed(claude, Some(&text), CMD));
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["model"], "opus");
        assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(v["hooks"]["Stop"][0]["hooks"][0]["command"], "say done");
        assert_eq!(v["hooks"]["Stop"][1]["hooks"][0]["async"], true);
        assert!(v["hooks"]["SessionEnd"][0]["hooks"][0].get("async").is_none());
        // A second sync changes nothing.
        let (again, changed) = applied(claude, Some(&text), Some(CMD));
        assert!(!changed);
        assert_eq!(again.unwrap(), text);
        // Removal leaves what was there before, and nothing of ours.
        let (clean, changed) = applied(claude, Some(&text), None);
        let clean = clean.unwrap();
        assert!(changed);
        assert!(!clean.to_lowercase().contains("aevalbuddy"));
        let v: Value = serde_json::from_str(&clean).unwrap();
        assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert!(v["hooks"].get("UserPromptSubmit").is_none());
        assert!(!is_installed(claude, Some(&clean), CMD));
    }

    #[test]
    fn another_apps_hooks_are_left_alone() {
        let claude = get("claude");
        let other = "C:/Users/me/AppData/Local/Aevalsistant/Aevalsistant.exe --hook";
        let theirs = json!({ "hooks": { "Stop": [ { "hooks": [
            { "type": "command", "command": other } ] } ] } });
        let (text, _) = applied(claude, Some(&theirs.to_string()), Some(CMD));
        let (clean, _) = applied(claude, text.as_deref(), None);
        let v: Value = serde_json::from_str(&clean.unwrap()).unwrap();
        assert_eq!(v["hooks"]["Stop"][0]["hooks"][0]["command"], other);
        assert!(!is_ours(other));
        assert!(is_ours(CMD));
    }

    #[test]
    fn empty_and_broken_files() {
        let claude = get("claude");
        let (text, changed) = applied(claude, None, Some(CMD));
        assert!(changed && is_installed(claude, text.as_deref(), CMD));
        // Removing from nothing writes nothing new.
        let (_, changed) = applied(claude, Some("  "), None);
        assert!(!changed);
        // With only ours in it, removal empties the file's hooks.
        let (clean, _) = applied(claude, text.as_deref(), None);
        assert_eq!(clean.unwrap(), "{}\n");
        assert!(apply(claude, Some("[1]"), Some(CMD)).is_err());
        assert!(apply(claude, Some("{ nope"), Some(CMD)).is_err());
        assert!(apply(claude, Some(r#"{"hooks": 3}"#), Some(CMD)).is_err());
    }

    #[test]
    fn flat_and_owned_layouts() {
        let cursor = get("cursor");
        let cmd = command_for(cursor, CMD);
        assert!(cmd.ends_with("--hook cursor"));
        let (text, _) = applied(cursor, Some("{}"), Some(&cmd));
        let text = text.unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(v["hooks"]["stop"][0]["command"], cmd.as_str());
        assert!(is_installed(cursor, Some(&text), &cmd));
        let (clean, _) = applied(cursor, Some(&text), None);
        assert!(!clean.unwrap().contains("--hook"));

        let copilot = get("copilot");
        let cmd = command_for(copilot, CMD);
        let (file, changed) = applied(copilot, None, Some(&cmd));
        let file = file.unwrap();
        assert!(changed && is_installed(copilot, Some(&file), &cmd));
        let v: Value = serde_json::from_str(&file).unwrap();
        let stop = &v["hooks"]["agentStop"][0];
        assert!(stop["command"].as_str().unwrap().ends_with("--hook copilot agentStop"));
        assert!(!applied(copilot, Some(&file), Some(&cmd)).1);
        assert_eq!(applied(copilot, Some(&file), None), (None, true));

        let opencode = get("opencode");
        let (plugin, _) = applied(opencode, None, Some(&command_for(opencode, CMD)));
        let plugin = plugin.unwrap();
        assert!(plugin.contains(r#"const EXE = "C:/Users/me/AppData/Local/AevalBuddy/AevalBuddy.exe";"#));
        assert!(!plugin.contains("__EXE__"));
    }

    #[test]
    fn hook_paths_survive_every_shell() {
        let none = |_: &str| None;
        let plain = hook_path("C:\\Users\\me\\AppData\\Local\\AevalBuddy\\AevalBuddy.exe", none);
        assert_eq!(plain, "C:/Users/me/AppData/Local/AevalBuddy/AevalBuddy.exe");
        let spaced = "C:\\Users\\Ada Lovelace\\AppData\\Local\\AevalBuddy\\AevalBuddy.exe";
        let short = |upto: &str| upto.ends_with("Lovelace").then(|| "C:\\Users\\ADALOV~1".to_string());
        let fixed = hook_path(spaced, short);
        assert_eq!(fixed, "C:/Users/ADALOV~1/AppData/Local/AevalBuddy/AevalBuddy.exe");
        // Short names off: quoted.
        let quoted = hook_path(spaced, none);
        assert!(quoted.starts_with('"') && quoted.ends_with('"'));
        assert_eq!(powershell(&quoted), format!("& {quoted}"));
    }

    fn norm(agent: &str, event: &str, raw: Value) -> HookEvent {
        let mut e = HookEvent {
            name: event.into(),
            ..HookEvent::default()
        };
        normalize(agent, &raw, &mut e, "C:\\pwd");
        e
    }

    #[test]
    fn every_agent_maps_onto_the_same_events() {
        let e = norm("codex", "", json!({ "hook_event_name": "PermissionRequest",
            "session_id": "1", "tool_name": "shell" }));
        assert_eq!((e.name.as_str(), e.session_id.as_str()), ("Notification", "codex:1"));
        assert_eq!(e.message, "Codex wants to use shell");
        assert!(e.cwd_guessed && e.cwd == "C:\\pwd");

        let e = norm("gemini", "", json!({ "hook_event_name": "AfterAgent",
            "session_id": "g", "cwd": "C:\\site", "prompt_response": "done" }));
        assert_eq!((e.name.as_str(), e.last_assistant.as_str()), ("Stop", "done"));
        assert!(!e.cwd_guessed);
        let e = norm("gemini", "", json!({ "hook_event_name": "Notification",
            "session_id": "g", "notification_type": "Info" }));
        assert_eq!(e.name, "");

        let e = norm("copilot", "errorOccurred", json!({ "sessionId": "c", "recoverable": true }));
        assert_eq!(e.name, "");
        let e = norm("copilot", "errorOccurred", json!({ "sessionId": "c" }));
        assert_eq!(e.name, "StopFailure");
        let e = norm("copilot", "agentStop", json!({ "sessionId": "c" }));
        assert_eq!((e.name.as_str(), e.session_id.as_str()), ("Stop", "copilot:c"));

        let e = norm("cursor", "", json!({ "hook_event_name": "stop", "status": "aborted",
            "conversation_id": "k", "workspace_roots": ["C:\\code\\app"] }));
        assert_eq!((e.name.as_str(), e.cwd.as_str()), ("Interrupt", "C:\\code\\app"));
        let e = norm("cursor", "", json!({ "hook_event_name": "afterAgentResponse",
            "conversation_id": "k", "text": "hi" }));
        assert_eq!((e.name.as_str(), e.last_assistant.as_str()), ("AgentText", "hi"));

        let e = norm("windsurf", "", json!({ "agent_action_name": "post_cascade_response",
            "trajectory_id": "w", "tool_info": { "response": "ok" } }));
        assert_eq!((e.name.as_str(), e.last_assistant.as_str()), ("Stop", "ok"));

        let e = norm("opencode", "", json!({ "event": "permission", "session": "o" }));
        assert_eq!(e.message, "OpenCode is asking for permission.");

        let e = norm("claude", "", json!({ "hook_event_name": "SubagentStart",
            "session_id": "s", "agent_id": "a1", "agent_type": "Explore" }));
        assert_eq!((e.session_id.as_str(), e.agent_type.as_str()), ("s", "Explore"));
    }

    #[test]
    fn places_name_the_agent_and_its_app() {
        assert_eq!(place("claude", "VS Code"), "VS Code");
        assert_eq!(place("codex", "Terminal"), "Codex \u{b7} Terminal");
        assert_eq!(place("cursor", "Cursor"), "Cursor");
        assert_eq!(place("gemini", ""), "Gemini CLI");
        assert_eq!(place("chat", "ChatGPT"), "ChatGPT");
    }
}
