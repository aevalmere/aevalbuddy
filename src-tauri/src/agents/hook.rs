//! `AevalBuddy.exe --hook [agent] [event]`: what a coding agent runs on each
//! lifecycle event, with its hook payload on stdin. It adds which window and
//! which agent process the call came from, hands everything to the running
//! app over a named pipe, and exits 0. It runs before anything else in the
//! app starts, so a hook costs a process start and nothing more. Port of
//! Aevalsistant's `HookClient`.
//!
//! A hook must never fail or hold up the agent that ran it: whatever goes
//! wrong here, the agent carries on.

use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Value};

use super::sessions::{project_of, HookEvent};
use super::{native, specs};

/// The pipe the running app listens on, one per Windows user.
pub fn pipe_name() -> String {
    let user: String = std::env::var("USERNAME")
        .unwrap_or_default()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .collect();
    format!(r"\\.\pipe\aevalbuddy.agents.{user}")
}

/// The arguments after the exe, when this is a hook call: the agent
/// (Claude Code passes none) and the event (only Copilot CLI passes it).
pub fn hook_args(args: &[String]) -> Option<(String, String)> {
    if args.first().map(String::as_str) != Some("--hook") {
        return None;
    }
    let agent = args.get(1).cloned().unwrap_or_else(|| "claude".to_string());
    let event = args.get(2).cloned().unwrap_or_default();
    Some((agent, event))
}

/// Runs the hook client when the command line asks for it. True when it
/// did, and the process should exit.
pub fn run_if_asked() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((agent, event)) = hook_args(&args) else {
        return false;
    };
    let _ = std::panic::catch_unwind(|| forward(&agent, &event));
    reply(&agent);
    true
}

fn forward(agent: &str, event: &str) {
    // App not running: return at once instead of waiting on a pipe nobody
    // serves.
    let Some(mut pipe) = open_pipe() else {
        return;
    };
    let raw = read_stdin(Duration::from_secs(2));
    if raw.is_empty() {
        return;
    }
    let pwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let project = project_in(agent, event, &raw, &pwd);
    let found = native::from_hook_process(&project, agent);
    let started = native::start_time(native::own_pid());
    let text = envelope(agent, event, &pwd, &found, started, &raw);
    let _ = pipe.write_all(text.as_bytes());
    let _ = pipe.flush();
}

/// ERROR_PIPE_BUSY: the app is taking another hook's call this instant.
const PIPE_BUSY: i32 = 231;

/// The app's pipe, or None when the app is not running. A busy pipe is
/// tried again for a moment: two agents can finish at once.
fn open_pipe() -> Option<std::fs::File> {
    for _ in 0..10 {
        match std::fs::OpenOptions::new().write(true).open(pipe_name()) {
            Ok(pipe) => return Some(pipe),
            Err(e) if e.raw_os_error() == Some(PIPE_BUSY) => {
                std::thread::sleep(Duration::from_millis(30));
            }
            Err(_) => return None,
        }
    }
    None
}

/// The project folder's name, to pick the right window among several.
fn project_in(agent: &str, event: &str, raw: &str, pwd: &str) -> String {
    let Ok(payload) = serde_json::from_str::<Value>(raw) else {
        return String::new();
    };
    let mut probe = HookEvent {
        name: event.to_string(),
        ..HookEvent::default()
    };
    specs::normalize(agent, &payload, &mut probe, pwd);
    project_of(&probe.cwd).unwrap_or_default()
}

fn envelope(
    agent: &str,
    event: &str,
    pwd: &str,
    found: &native::Found,
    started: u64,
    raw: &str,
) -> String {
    json!({
        "t": started,
        "agent": agent,
        "event": event,
        "pwd": pwd,
        "host": found.host,
        "hwnd": found.window,
        "pid": found.agent_pid,
        "pidStart": found.agent_start,
        "outer": found.outer_pid,
        "outerStart": found.outer_start,
        "raw": raw,
    })
    .to_string()
}

/// Cursor reads a JSON answer from its hooks: beforeSubmitPrompt takes
/// `"continue": true` to let the prompt through, and the others ignore it.
/// Other agents read nothing.
fn reply(agent: &str) {
    if agent == "cursor" {
        let mut out = std::io::stdout();
        let _ = out.write_all(br#"{"continue":true}"#);
        let _ = out.flush();
    }
}

/// Everything on stdin, or what arrived within `limit` (4 MB at most).
fn read_stdin(limit: Duration) -> String {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = std::io::stdin().take(4 << 20).read_to_end(&mut buf);
        let _ = tx.send(buf);
    });
    let bytes = rx.recv_timeout(limit).unwrap_or_default();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn only_a_hook_call_is_a_hook_call() {
        assert_eq!(hook_args(&args(&[])), None);
        assert_eq!(hook_args(&args(&["--hidden"])), None);
        let claude = hook_args(&args(&["--hook"])).unwrap();
        assert_eq!(claude, ("claude".to_string(), String::new()));
        let copilot = hook_args(&args(&["--hook", "copilot", "agentStop"])).unwrap();
        assert_eq!(copilot, ("copilot".to_string(), "agentStop".to_string()));
    }

    #[test]
    fn the_envelope_reads_back_as_the_event() {
        let raw =
            r#"{"hook_event_name":"UserPromptSubmit","session_id":"s","cwd":"C:\\code\\rover"}"#;
        assert_eq!(project_in("claude", "", raw, "C:\\x"), "rover");
        assert_eq!(project_in("claude", "", "not json", "C:\\x"), "");
        let found = native::Found {
            window: 99,
            agent_pid: 4,
            agent_start: 5,
            outer_pid: 6,
            outer_start: 7,
            host: "Terminal".into(),
        };
        let text = envelope("claude", "", "C:\\x", &found, 11, raw);
        let e = HookEvent::from_envelope(&text).unwrap();
        assert_eq!(e.name, "UserPromptSubmit");
        assert_eq!((e.hwnd, e.pid, e.pid_start), (99, 4, 5));
        assert_eq!((e.outer_pid, e.outer_start, e.at), (6, 7, 11));
        assert_eq!(e.host, "Terminal");
        assert!(pipe_name().starts_with(r"\\.\pipe\aevalbuddy.agents."));
    }
}
