//! Which coding agents are working, which need you, and what to say when
//! one changes: the platform-free half of agent watching. Port of
//! Aevalsistant's `Core.cs` (SessionBook, Status, Snippet, Transcript).
//!
//! Every time here is milliseconds since the Unix epoch, handed in by the
//! caller, so the rules can be tested with any clock.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::specs;

/// A session whose agent has said nothing for this long is no longer
/// counted as working.
pub const STALE_WORKING_MS: u64 = 45 * 60_000;
/// A session with no agent process to watch is forgotten after this long.
pub const FORGET_UNTRACKED_MS: u64 = 12 * 60 * 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    Done,
    NeedsYou,
    Info,
}

impl CardKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CardKind::Done => "done",
            CardKind::NeedsYou => "needs-you",
            CardKind::Info => "info",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    Working,
    Done,
    NeedsYou,
}

impl RowState {
    pub fn as_str(self) -> &'static str {
        match self {
            RowState::Working => "working",
            RowState::Done => "done",
            RowState::NeedsYou => "needs-you",
        }
    }
}

/// A subagent that started and has not stopped. A background subagent keeps
/// running after its parent's turn ends.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Subagent {
    /// "Explore", "general-purpose"; empty when the agent does not say.
    pub kind: String,
    pub started: u64,
    /// Its last sign of life: its start, or a write to its transcript.
    pub seen: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Session {
    pub id: String,
    pub cwd: String,
    pub transcript: String,
    pub hwnd: i64,
    pub pid: u32,
    pub pid_start: u64,
    /// The agent process that started this session's agent, when one did
    /// (`claude -p` run from a Claude Code session's shell).
    pub outer_pid: u32,
    pub outer_start: u64,
    pub working: bool,
    /// A permission prompt is open.
    pub blocked: bool,
    pub notified_since_prompt: bool,
    pub last_event: u64,
    /// The newest of hook events and transcript writes.
    pub last_activity: u64,
    /// The transcript's modified time at the last sweep.
    pub transcript_seen: u64,
    /// Hook process start times, to order deliveries that arrive late.
    pub prompt_at: u64,
    pub stop_at: u64,
    /// The app hosting it: "VS Code", "Terminal", "Claude".
    pub host: String,
    /// claude, codex, gemini, copilot, cursor, windsurf, opencode, chat.
    pub agent: String,
    /// A fixed name for sessions without a folder (chat apps).
    pub label: String,
    /// The agent's last reply, for agents that send it before Stop.
    pub pending_text: String,
    pub working_since: u64,
    /// Insertion order is start order.
    pub subagents: Vec<(String, Subagent)>,
    stopped_early: HashSet<String>,
}

impl Session {
    pub fn busy(&self) -> bool {
        self.working || !self.subagents.is_empty()
    }

    /// Where it runs, as shown after the title.
    pub fn place(&self) -> String {
        specs::place(&self.agent, &self.host)
    }

    /// The last folder of the working directory.
    pub fn project(&self) -> String {
        if !self.label.is_empty() {
            return self.label.clone();
        }
        project_of(&self.cwd).unwrap_or_else(|| specs::get(&self.agent).name.to_string())
    }

    /// A subagent's own transcript:
    /// `<session transcript minus .jsonl>/subagents/agent-<id>.jsonl`.
    pub fn subagent_transcript(&self, agent_id: &str) -> Option<String> {
        let t = &self.transcript;
        if agent_id.is_empty() || t.len() < 6 || !t.to_lowercase().ends_with(".jsonl") {
            return None;
        }
        let base = &t[..t.len() - 6];
        Some(format!("{base}\\subagents\\agent-{agent_id}.jsonl"))
    }

    fn subagent_started(&mut self, id: &str, kind: &str, now: u64) {
        if self.stopped_early.remove(id) {
            return;
        }
        let sub = Subagent {
            kind: kind.to_string(),
            started: now,
            seen: now,
        };
        match self.subagents.iter_mut().find(|(k, _)| k == id) {
            Some(slot) => slot.1 = sub,
            None => self.subagents.push((id.to_string(), sub)),
        }
    }

    /// True when this was the last one running.
    fn subagent_stopped(&mut self, id: &str) -> bool {
        let before = self.subagents.len();
        self.subagents.retain(|(k, _)| k != id);
        if self.subagents.len() < before {
            return self.subagents.is_empty();
        }
        // An async stop that beat its start.
        self.stopped_early.insert(id.to_string());
        false
    }
}

/// The last folder of a path, split by hand so Windows paths read the same
/// wherever this runs.
pub fn project_of(cwd: &str) -> Option<String> {
    let trimmed = cwd.trim_end_matches(['\\', '/']);
    if trimmed.is_empty() {
        return None;
    }
    let name = trimmed.rsplit(['\\', '/']).next().unwrap_or(trimmed);
    Some(name.to_string())
}

/// One hook call, in Claude Code's event names (`specs::normalize` maps the
/// other agents onto them).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HookEvent {
    pub name: String,
    pub session_id: String,
    pub cwd: String,
    pub transcript: String,
    pub message: String,
    pub notification_type: String,
    pub last_assistant: String,
    pub hwnd: i64,
    pub pid: u32,
    pub pid_start: u64,
    /// When the hook process started; async hooks can arrive out of order.
    pub at: u64,
    pub host: String,
    pub agent_id: String,
    /// A subagent's kind, from SubagentStart.
    pub agent_type: String,
    pub agent: String,
    pub label: String,
    pub cwd_guessed: bool,
    pub outer_pid: u32,
    pub outer_start: u64,
}

impl HookEvent {
    /// The envelope the `--hook` client sends:
    /// `{"t":..,"agent":"codex","event":"","pwd":"..","host":"..","hwnd":..,
    /// "pid":..,"pidStart":..,"outer":..,"outerStart":..,"raw":"<stdin>"}`.
    pub fn from_envelope(text: &str) -> Option<HookEvent> {
        let env: Value = serde_json::from_str(text).ok()?;
        let raw: Value = serde_json::from_str(env.get("raw")?.as_str()?).ok()?;
        if !raw.is_object() {
            return None;
        }
        let s = |k: &str| env.get(k).and_then(Value::as_str).unwrap_or_default();
        let n = |k: &str| env.get(k).and_then(Value::as_u64).unwrap_or(0);
        let agent = if s("agent").is_empty() {
            "claude"
        } else {
            s("agent")
        };
        let mut e = HookEvent {
            agent: agent.to_string(),
            name: s("event").to_string(),
            hwnd: env.get("hwnd").and_then(Value::as_i64).unwrap_or(0),
            pid: n("pid") as u32,
            pid_start: n("pidStart"),
            at: n("t"),
            host: s("host").to_string(),
            outer_pid: n("outer") as u32,
            outer_start: n("outerStart"),
            ..HookEvent::default()
        };
        specs::normalize(agent, &raw, &mut e, s("pwd"));
        if e.session_id.is_empty() || e.name.is_empty() {
            return None;
        }
        Some(e)
    }
}

/// A card to show: which session changed, and what to say.
#[derive(Clone, Debug, PartialEq)]
pub struct CardRequest {
    /// None for the app's own messages.
    pub session_id: Option<String>,
    pub hwnd: i64,
    /// Shown after the title, so you know which window it is.
    pub host: String,
    pub kind: CardKind,
    pub title: String,
    pub detail: String,
    /// Fill `detail` from the transcript just before showing.
    pub wants_snippet: bool,
    /// From work running in the background rather than from your own prompt:
    /// a session another agent started, a turn the agent took on its own, or
    /// the last background subagent finishing. Settings decide whether these
    /// show a card.
    pub background: bool,
}

fn card(
    s: &Session,
    kind: CardKind,
    title: String,
    detail: String,
    background: bool,
) -> CardRequest {
    CardRequest {
        session_id: Some(s.id.clone()),
        hwnd: s.hwnd,
        host: s.place(),
        kind,
        title,
        detail,
        wants_snippet: false,
        background,
    }
}

/// One line of the session list: a session, or one of its subagents.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeRow {
    pub session_id: String,
    pub subagent_id: Option<String>,
    pub depth: u8,
}

/// What the transcript's tail says, for the sweep.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TranscriptProbe {
    pub modified: u64,
    pub ends_in_interrupt: bool,
}

#[derive(Default)]
pub struct SessionBook {
    map: HashMap<String, Session>,
}

impl SessionBook {
    pub fn get(&self, id: &str) -> Option<&Session> {
        self.map.get(id)
    }

    pub fn busy_count(&self) -> usize {
        self.map.values().filter(|s| s.busy()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Whether a coding agent's hooks already report a busy session in this
    /// window (Claude Code in the Claude app's Code tab), so the chat watcher
    /// leaves it alone and it does not show twice.
    pub fn hook_busy_in(&self, hwnd: i64) -> bool {
        hwnd != 0
            && self
                .map
                .values()
                .any(|s| s.busy() && s.agent != "chat" && s.hwnd == hwnd)
    }

    /// Drops every session of `agent`. True when there were any.
    pub fn forget_agent(&mut self, agent: &str) -> bool {
        let before = self.map.len();
        self.map.retain(|_, s| s.agent != agent);
        self.map.len() != before
    }

    /// The session whose agent process started this one's, if it is known.
    pub fn parent_of(&self, s: &Session) -> Option<&Session> {
        if s.outer_pid == 0 {
            return None;
        }
        self.map.values().find(|p| {
            p.id != s.id
                && p.pid == s.outer_pid
                && (p.pid_start == 0 || s.outer_start == 0 || p.pid_start == s.outer_start)
        })
    }

    fn root_of<'a>(&'a self, s: &'a Session) -> &'a Session {
        let mut cur = s;
        for _ in 0..8 {
            match self.parent_of(cur) {
                Some(p) => cur = p,
                None => break,
            }
        }
        cur
    }

    /// Sessions in display order: needs you first, then done, then working.
    /// With `nest`, each is followed by what it started (its subagents, then
    /// sessions its agent started), one level in. `head` is left out of the
    /// list, because the card shows it above, but its own children lead.
    pub fn tree(&self, head: Option<&str>, nest: bool) -> Vec<TreeRow> {
        let rank = |s: &Session| match state_of(s) {
            RowState::NeedsYou => 0,
            RowState::Done => 1,
            RowState::Working => 2,
        };
        let mut ordered: Vec<&Session> = self.map.values().collect();
        ordered.sort_by(|a, b| {
            rank(a)
                .cmp(&rank(b))
                .then(b.last_event.cmp(&a.last_event))
                .then(a.id.cmp(&b.id))
        });
        let mut rows = Vec::new();
        let children = |parent: &Session, rows: &mut Vec<TreeRow>| {
            for (id, _) in &parent.subagents {
                rows.push(TreeRow {
                    session_id: parent.id.clone(),
                    subagent_id: Some(id.clone()),
                    depth: 1,
                });
            }
            for c in &ordered {
                let is_head = head == Some(c.id.as_str());
                if c.id != parent.id && !is_head && self.root_of(c).id == parent.id {
                    rows.push(TreeRow {
                        session_id: c.id.clone(),
                        subagent_id: None,
                        depth: 1,
                    });
                }
            }
        };
        if nest {
            if let Some(h) = head.and_then(|h| self.get(h)) {
                children(h, &mut rows);
            }
        }
        for s in &ordered {
            let is_head = head == Some(s.id.as_str());
            if is_head || (nest && self.root_of(s).id != s.id) {
                continue;
            }
            rows.push(TreeRow {
                session_id: s.id.clone(),
                subagent_id: None,
                depth: 0,
            });
            if nest {
                children(s, &mut rows);
            }
        }
        rows
    }

    /// Takes one hook event in. Returns the card it calls for, if any.
    pub fn apply(&mut self, e: &HookEvent, now: u64) -> Option<CardRequest> {
        if e.name == "SessionEnd" {
            self.map.remove(&e.session_id);
            return None;
        }
        {
            let s = self
                .map
                .entry(e.session_id.clone())
                .or_insert_with(|| Session {
                    id: e.session_id.clone(),
                    agent: "claude".into(),
                    ..Session::default()
                });
            if !e.cwd.is_empty() && (!e.cwd_guessed || s.cwd.is_empty()) {
                s.cwd = e.cwd.clone();
            }
            if !e.transcript.is_empty() {
                s.transcript = e.transcript.clone();
            }
            if e.hwnd != 0 {
                s.hwnd = e.hwnd;
            }
            if !e.host.is_empty() {
                s.host = e.host.clone();
            }
            if !e.agent.is_empty() {
                s.agent = e.agent.clone();
            }
            if !e.label.is_empty() {
                s.label = e.label.clone();
            }
            if e.pid != 0 {
                s.pid = e.pid;
                s.pid_start = e.pid_start;
            }
            if e.outer_pid != 0 {
                s.outer_pid = e.outer_pid;
                s.outer_start = e.outer_start;
            }
            s.last_event = now;
            s.last_activity = now;
        }
        let child = self
            .get(&e.session_id)
            .is_some_and(|s| self.parent_of(s).is_some());
        let s = self.map.get_mut(&e.session_id)?;

        let is_prompt = e.name == "UserPromptSubmit";
        let is_stop = matches!(e.name.as_str(), "Stop" | "StopFailure" | "Interrupt");
        if e.at != 0 {
            // A late prompt from before the latest stop (or the reverse) is
            // stale.
            if (is_prompt && e.at < s.stop_at) || (is_stop && e.at < s.prompt_at) {
                return None;
            }
            if is_prompt {
                s.prompt_at = e.at;
            }
            if is_stop {
                s.stop_at = e.at;
            }
        }
        let sub_id = if e.agent_id.is_empty() {
            "unnamed"
        } else {
            e.agent_id.as_str()
        };

        match e.name.as_str() {
            "AgentText" => {
                s.pending_text = e.last_assistant.clone();
                None
            }
            "Interrupt" => {
                // You stopped it yourself, so there is nothing to tell you.
                s.working = false;
                s.blocked = false;
                s.notified_since_prompt = true;
                None
            }
            "SubagentStart" => {
                s.subagent_started(sub_id, &e.agent_type, now);
                None
            }
            "SubagentStop" => {
                // Subagents that finish mid-turn are part of that turn. The
                // last background one finishing after the turn ended is the
                // end of everything you started.
                if !s.subagent_stopped(sub_id) || s.working {
                    return None;
                }
                let title = format!("{} finished", s.project());
                let detail = "Its background agents are done.".to_string();
                Some(card(s, CardKind::Done, title, detail, true))
            }
            "UserPromptSubmit" => {
                if !s.working {
                    s.working_since = now;
                }
                s.working = true;
                s.blocked = false;
                s.notified_since_prompt = false;
                s.pending_text.clear();
                None
            }
            "Stop" => {
                // A second Stop with no prompt in between is a turn the agent
                // took on its own, usually because a background subagent
                // reported back.
                let on_its_own = s.notified_since_prompt;
                s.working = false;
                s.blocked = false;
                s.notified_since_prompt = true;
                let reply = if e.last_assistant.is_empty() {
                    std::mem::take(&mut s.pending_text)
                } else {
                    s.pending_text.clear();
                    e.last_assistant.clone()
                };
                let claude = s.agent == "claude";
                let verb = if s.subagents.is_empty() {
                    "finished"
                } else {
                    "is waiting"
                };
                let title = format!("{} {verb}", s.project());
                let detail = if !reply.is_empty() || claude {
                    clean_snippet(&reply)
                } else {
                    "Ready for your next prompt.".to_string()
                };
                let mut c = card(s, CardKind::Done, title, detail, child || on_its_own);
                // Only Claude Code's transcript format is known well enough
                // to read the reply from.
                c.wants_snippet = reply.is_empty() && claude;
                Some(c)
            }
            "StopFailure" => {
                s.working = false;
                s.blocked = false;
                s.notified_since_prompt = true;
                let title = format!("{} stopped early", s.project());
                let detail = "The turn ended on an error. The terminal has the details.";
                Some(card(
                    s,
                    CardKind::NeedsYou,
                    title,
                    detail.to_string(),
                    child,
                ))
            }
            "Notification" => {
                let kind = if e.notification_type.is_empty() {
                    guess_type(&e.message)
                } else {
                    e.notification_type.as_str()
                };
                match kind {
                    "idle_prompt" => {
                        s.working = false;
                        s.blocked = false;
                        if s.notified_since_prompt {
                            return None;
                        }
                        s.notified_since_prompt = true;
                        let title = format!("{} is waiting", s.project());
                        let detail = or_default(&e.message, "Ready for your next prompt.");
                        Some(card(s, CardKind::Done, title, detail, child))
                    }
                    "permission_prompt"
                    | "elicitation_dialog"
                    | "elicitation_url_dialog"
                    | "agent_needs_input" => {
                        // Still mid-turn: the PC stays awake, and the human
                        // is asked for.
                        s.blocked = true;
                        let title = format!("{} needs you", s.project());
                        let detail = or_default(&e.message, "Claude is asking for permission.");
                        Some(card(s, CardKind::NeedsYou, title, detail, child))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Called every few seconds. `alive` says whether a session's agent
    /// process still runs, `probe` reads its transcript's tail, and
    /// `subagent_write` gives the time a subagent's transcript was last
    /// written. True when anything changed.
    pub fn sweep(
        &mut self,
        now: u64,
        alive: impl Fn(&Session) -> bool,
        probe: impl Fn(&Session) -> Option<TranscriptProbe>,
        subagent_write: impl Fn(&Session, &str) -> Option<u64>,
    ) -> bool {
        let mut changed = false;
        let ids: Vec<String> = self.map.keys().cloned().collect();
        for id in ids {
            let Some(s) = self.map.get_mut(&id) else {
                continue;
            };
            let gone = if s.pid != 0 {
                !alive(s)
            } else {
                now.saturating_sub(s.last_event) > FORGET_UNTRACKED_MS
            };
            if gone {
                self.map.remove(&id);
                changed = true;
                continue;
            }
            // A subagent counts as alive while its own transcript keeps
            // being written.
            let writes: Vec<Option<u64>> = s
                .subagents
                .iter()
                .map(|(sub_id, _)| subagent_write(s, sub_id))
                .collect();
            for ((_, sub), wrote) in s.subagents.iter_mut().zip(writes) {
                if let Some(w) = wrote.filter(|w| *w > sub.seen) {
                    sub.seen = w;
                }
            }
            let before = s.subagents.len();
            s.subagents
                .retain(|(_, sub)| now.saturating_sub(sub.seen) <= STALE_WORKING_MS);
            changed |= s.subagents.len() != before;
            if !s.working {
                continue;
            }
            if let Some(p) = probe(s).filter(|p| p.modified > s.transcript_seen) {
                s.transcript_seen = p.modified;
                s.last_activity = s.last_activity.max(p.modified);
                if p.ends_in_interrupt {
                    // Esc in Claude Code skips the Stop hook; the transcript
                    // records it instead.
                    s.working = false;
                    s.blocked = false;
                    changed = true;
                    continue;
                }
            }
            if now.saturating_sub(s.last_activity) > STALE_WORKING_MS {
                s.working = false;
                s.blocked = false;
                changed = true;
            }
        }
        changed
    }
}

fn or_default(text: &str, default: &str) -> String {
    if text.is_empty() {
        default.to_string()
    } else {
        text.to_string()
    }
}

fn guess_type(message: &str) -> &'static str {
    let m = message.to_lowercase();
    if m.contains("waiting for your input") {
        "idle_prompt"
    } else if m.contains("permission") {
        "permission_prompt"
    } else {
        ""
    }
}

// ------------------------------------------------------------------ status

pub fn state_of(s: &Session) -> RowState {
    if s.blocked {
        RowState::NeedsYou
    } else if s.busy() {
        RowState::Working
    } else {
        RowState::Done
    }
}

/// "12m", "1h 5m"; empty under a minute or past a week.
fn age(ms: u64) -> String {
    let minutes = ms / 60_000;
    if !(1..=7 * 24 * 60).contains(&minutes) {
        String::new()
    } else if minutes < 60 {
        format!("{minutes}m")
    } else {
        format!("{}h {}m", minutes / 60, minutes % 60)
    }
}

fn join(word: &str, age: &str, suffix: &str) -> String {
    if age.is_empty() {
        word.to_string()
    } else if suffix.is_empty() {
        format!("{word} {age}")
    } else {
        format!("{word} {age} {suffix}")
    }
}

/// The short state shown for a session: "needs you", "working 12m",
/// "done 3m ago", "2 subagents running".
pub fn status_text(s: &Session, now: u64) -> String {
    let n = s.subagents.len();
    let subs = if n == 1 {
        "1 subagent".to_string()
    } else {
        format!("{n} subagents")
    };
    if s.blocked {
        return "needs you".to_string();
    }
    if s.working {
        let base = join("working", &age(now.saturating_sub(s.working_since)), "");
        return if n > 0 {
            format!("{base}, {subs}")
        } else {
            base
        };
    }
    if n > 0 {
        return format!("{subs} running");
    }
    join("done", &age(now.saturating_sub(s.last_event)), "ago")
}

/// A subagent's line: "running 4m".
pub fn subagent_text(a: &Subagent, now: u64) -> String {
    join("running", &age(now.saturating_sub(a.started)), "")
}

// ----------------------------------------------------------------- snippet

/// A reply as one plain line: code fences and Markdown marks removed, white
/// space collapsed, 600 characters at most.
pub fn clean_snippet(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    // Code fences, closed or not.
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        out.push_str(&rest[..start]);
        out.push(' ');
        let after = &rest[start + 3..];
        match after.find("```") {
            Some(end) => rest = &after[end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    // Line-leading marks: headings, bullets and quotes.
    let lines: Vec<&str> = out
        .lines()
        .map(|line| {
            let t = line.trim_start();
            let hashes = t.trim_start_matches('#');
            if hashes.len() < t.len() {
                return hashes.trim_start();
            }
            match t.strip_prefix(['-', '*', '>']) {
                Some(body) if body.starts_with(char::is_whitespace) => body.trim_start(),
                _ => line,
            }
        })
        .collect();
    let joined = lines
        .join(" ")
        .replace("**", "")
        .replace("__", "")
        .replace('`', "");
    let flat = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(600).collect()
}

// -------------------------------------------------------------- transcript

fn text_of(message: Option<&Value>) -> String {
    let Some(content) = message.and_then(|m| m.get("content")) else {
        return String::new();
    };
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    let Some(blocks) = content.as_array() else {
        return String::new();
    };
    let parts: Vec<&str> = blocks
        .iter()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect();
    parts.join("\n").trim().to_string()
}

fn entry(line: &str) -> Option<Value> {
    serde_json::from_str::<Value>(line)
        .ok()
        .filter(Value::is_object)
}

/// The last thing the assistant said, from a Claude Code transcript's lines.
pub fn last_assistant_text(lines: &[String]) -> String {
    for line in lines.iter().rev() {
        let Some(o) = entry(line) else { continue };
        if o["type"] != "assistant" {
            continue;
        }
        let text = text_of(o.get("message"));
        if !text.is_empty() {
            return text;
        }
    }
    String::new()
}

/// Whether the transcript ends with the user pressing Esc.
pub fn ends_in_interrupt(lines: &[String]) -> bool {
    for line in lines.iter().rev() {
        let Some(o) = entry(line) else { continue };
        match o["type"].as_str() {
            Some("assistant") => return false,
            Some("user") => {
                let text = text_of(o.get("message"));
                return text
                    .trim_start()
                    .starts_with("[Request interrupted by user");
            }
            _ => {}
        }
    }
    false
}

/// The whole lines in the last 256 KB of `bytes`, which were read starting
/// at `start` in the file: the first piece is a partial line unless the
/// read began at byte zero.
pub fn tail_lines(bytes: &[u8], start: u64) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    let skip = usize::from(start > 0);
    text.split('\n')
        .skip(skip)
        .filter(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(name: &str, id: &str) -> HookEvent {
        HookEvent {
            name: name.into(),
            session_id: id.into(),
            agent: "claude".into(),
            cwd: format!("C:\\code\\{id}"),
            ..HookEvent::default()
        }
    }

    const MIN: u64 = 60_000;

    #[test]
    fn a_turn_works_then_finishes_with_a_card() {
        let mut book = SessionBook::default();
        assert!(book
            .apply(&ev("UserPromptSubmit", "rover"), 1_000)
            .is_none());
        assert_eq!(book.busy_count(), 1);
        let s = book.get("rover").unwrap();
        assert_eq!(state_of(s), RowState::Working);
        assert_eq!(status_text(s, 1_000 + 12 * MIN), "working 12m");
        let mut stop = ev("Stop", "rover");
        stop.last_assistant = "## Done\n- **Fixed** the `parser`.".into();
        let c = book.apply(&stop, 2_000).unwrap();
        assert_eq!(c.title, "rover finished");
        assert_eq!(c.detail, "Done Fixed the parser.");
        assert_eq!(c.kind, CardKind::Done);
        assert!(!c.background && !c.wants_snippet);
        assert_eq!(book.busy_count(), 0);
        let s = book.get("rover").unwrap();
        assert_eq!(status_text(s, 2_000 + 3 * MIN), "done 3m ago");
        assert_eq!(status_text(s, 2_500), "done");
    }

    #[test]
    fn a_permission_prompt_needs_you_and_stays_busy() {
        let mut book = SessionBook::default();
        book.apply(&ev("UserPromptSubmit", "site"), 1);
        let mut n = ev("Notification", "site");
        n.message = "Claude needs your permission to use Bash".into();
        let c = book.apply(&n, 2).unwrap();
        assert_eq!(c.kind, CardKind::NeedsYou);
        assert_eq!(c.title, "site needs you");
        let s = book.get("site").unwrap();
        assert!(s.busy() && s.blocked);
        assert_eq!(status_text(s, 3), "needs you");
        assert_eq!(state_of(s), RowState::NeedsYou);
    }

    #[test]
    fn late_async_hooks_are_dropped() {
        let mut book = SessionBook::default();
        let mut prompt = ev("UserPromptSubmit", "a");
        prompt.at = 200;
        book.apply(&prompt, 1);
        // A Stop whose hook started before that prompt belongs to the turn
        // before it.
        let mut stale = ev("Stop", "a");
        stale.at = 100;
        assert!(book.apply(&stale, 2).is_none());
        assert!(book.get("a").unwrap().working);
        let mut stop = ev("Stop", "a");
        stop.at = 300;
        assert!(book.apply(&stop, 3).is_some());
        let mut late = ev("UserPromptSubmit", "a");
        late.at = 250;
        assert!(book.apply(&late, 4).is_none());
        assert!(!book.get("a").unwrap().working);
    }

    #[test]
    fn background_subagents_keep_the_session_busy() {
        let mut book = SessionBook::default();
        book.apply(&ev("UserPromptSubmit", "a"), 1);
        let mut start = ev("SubagentStart", "a");
        start.agent_id = "x1".into();
        start.agent_type = "Explore".into();
        book.apply(&start, 2);
        let c = book.apply(&ev("Stop", "a"), 3).unwrap();
        assert_eq!(c.title, "a is waiting");
        let s = book.get("a").unwrap();
        assert!(s.busy() && !s.working);
        assert_eq!(status_text(s, 4), "1 subagent running");
        assert_eq!(subagent_text(&s.subagents[0].1, 2 + 4 * MIN), "running 4m");
        let mut stop = ev("SubagentStop", "a");
        stop.agent_id = "x1".into();
        let c = book.apply(&stop, 5).unwrap();
        assert!(c.background);
        assert_eq!(c.detail, "Its background agents are done.");
        assert_eq!(book.busy_count(), 0);
        // A stop that beats its start cancels it.
        let mut early = ev("SubagentStop", "a");
        early.agent_id = "x2".into();
        assert!(book.apply(&early, 6).is_none());
        let mut start2 = ev("SubagentStart", "a");
        start2.agent_id = "x2".into();
        book.apply(&start2, 7);
        assert!(book.get("a").unwrap().subagents.is_empty());
    }

    #[test]
    fn a_turn_the_agent_took_on_its_own_is_background() {
        let mut book = SessionBook::default();
        book.apply(&ev("UserPromptSubmit", "a"), 1);
        assert!(!book.apply(&ev("Stop", "a"), 2).unwrap().background);
        assert!(book.apply(&ev("Stop", "a"), 3).unwrap().background);
        // You stopped it yourself: nothing to say.
        book.apply(&ev("UserPromptSubmit", "a"), 4);
        assert!(book.apply(&ev("Interrupt", "a"), 5).is_none());
        assert!(!book.get("a").unwrap().busy());
    }

    #[test]
    fn child_sessions_nest_under_their_parent() {
        let mut book = SessionBook::default();
        let mut parent = ev("UserPromptSubmit", "parent");
        parent.pid = 10;
        parent.pid_start = 77;
        book.apply(&parent, 1);
        let mut child = ev("UserPromptSubmit", "child");
        child.pid = 20;
        child.outer_pid = 10;
        child.outer_start = 77;
        book.apply(&child, 2);
        let mut other = ev("UserPromptSubmit", "other");
        other.pid = 30;
        book.apply(&other, 3);
        let c = book.apply(&ev("Stop", "child"), 4).unwrap();
        assert!(c.background, "a child session's finish is background work");
        let rows = book.tree(None, true);
        let shape: Vec<(&str, u8)> = rows
            .iter()
            .map(|r| (r.session_id.as_str(), r.depth))
            .collect();
        assert_eq!(shape, [("other", 0), ("parent", 0), ("child", 1)]);
        // Flat when nesting is off, and the head is left out.
        let flat = book.tree(Some("other"), false);
        let ids: Vec<&str> = flat.iter().map(|r| r.session_id.as_str()).collect();
        assert_eq!(ids, ["child", "parent"]);
        // The head's own children lead the list.
        let headed = book.tree(Some("parent"), true);
        assert_eq!(headed[0].session_id, "child");
        assert_eq!(headed[0].depth, 1);
    }

    #[test]
    fn the_sweep_drops_dead_sessions_and_idles_stale_ones() {
        let mut book = SessionBook::default();
        let mut a = ev("UserPromptSubmit", "a");
        a.pid = 1;
        book.apply(&a, 0);
        book.apply(&ev("UserPromptSubmit", "b"), 0);
        let no_probe = |_: &Session| None;
        let no_sub = |_: &Session, _: &str| None;
        // Nothing yet.
        assert!(!book.sweep(MIN, |_| true, no_probe, no_sub));
        // Esc: the transcript says so.
        let esc = |s: &Session| {
            (s.id == "b").then_some(TranscriptProbe {
                modified: 2 * MIN,
                ends_in_interrupt: true,
            })
        };
        assert!(book.sweep(2 * MIN, |_| true, esc, no_sub));
        assert!(!book.get("b").unwrap().working);
        // 45 quiet minutes.
        assert!(book.sweep(46 * MIN, |_| true, no_probe, no_sub));
        assert!(!book.get("a").unwrap().working);
        // The agent exited.
        assert!(book.sweep(47 * MIN, |_| false, no_probe, no_sub));
        assert!(book.get("a").is_none());
        // No process to watch: forgotten after twelve hours.
        assert!(book.get("b").is_some());
        assert!(book.sweep(13 * 60 * MIN, |_| true, no_probe, no_sub));
        assert!(book.is_empty());
    }

    #[test]
    fn session_end_forgets_and_names_fall_back() {
        let mut book = SessionBook::default();
        let mut e = ev("UserPromptSubmit", "codex:9");
        e.agent = "codex".into();
        e.cwd = String::new();
        e.host = "Terminal".into();
        book.apply(&e, 1);
        let s = book.get("codex:9").unwrap();
        assert_eq!(s.project(), "Codex");
        assert_eq!(s.place(), "Codex \u{b7} Terminal");
        book.apply(&ev("SessionEnd", "codex:9"), 2);
        assert!(book.is_empty());
        assert_eq!(
            project_of("C:\\Users\\me\\code\\rover\\").as_deref(),
            Some("rover")
        );
        assert_eq!(project_of("/home/me/site").as_deref(), Some("site"));
        assert_eq!(project_of(""), None);
    }

    #[test]
    fn snippets_are_one_plain_line() {
        let text =
            "# Title\n\nSome **bold** and `code`.\n```rust\nfn x() {}\n```\n> quoted\n* item";
        assert_eq!(clean_snippet(text), "Title Some bold and code. quoted item");
        assert_eq!(clean_snippet("  \n "), "");
        assert_eq!(clean_snippet("a ```unclosed"), "a");
        assert_eq!(clean_snippet(&"x".repeat(700)).chars().count(), 600);
        assert_eq!(clean_snippet("2 * 3 - 1"), "2 * 3 - 1");
    }

    #[test]
    fn transcripts_give_the_last_reply_and_the_interrupt() {
        let lines: Vec<String> = [
            r#"{"type":"user","message":{"content":"hi"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"},{"type":"tool_use"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use"}]}}"#,
            "not json",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(last_assistant_text(&lines), "Hello");
        assert!(!ends_in_interrupt(&lines));
        let mut esc = lines.clone();
        esc.push(r#"{"type":"user","message":{"content":[{"type":"text","text":"[Request interrupted by user]"}]}}"#.into());
        assert!(ends_in_interrupt(&esc));
        assert_eq!(tail_lines(b"partial\n{\"a\":1}\n\n{\"b\":2}", 10).len(), 2);
        assert_eq!(tail_lines(b"{\"a\":1}\n{\"b\":2}\n", 0).len(), 2);
    }

    #[test]
    fn envelopes_carry_the_window_and_the_payload() {
        let raw = r#"{"hook_event_name":"Stop","session_id":"s1","cwd":"C:\\code\\rover","last_assistant_message":"ok"}"#;
        let env = serde_json::json!({
            "t": 5, "agent": "", "event": "", "pwd": "C:\\x", "host": "VS Code",
            "hwnd": 4242, "pid": 7, "pidStart": 9, "outer": 0, "outerStart": 0, "raw": raw,
        });
        let e = HookEvent::from_envelope(&env.to_string()).unwrap();
        assert_eq!((e.name.as_str(), e.session_id.as_str()), ("Stop", "s1"));
        assert_eq!((e.hwnd, e.pid, e.pid_start, e.at), (4242, 7, 9, 5));
        assert_eq!(e.host, "VS Code");
        assert_eq!(e.cwd, "C:\\code\\rover");
        assert!(HookEvent::from_envelope("{}").is_none());
        assert!(HookEvent::from_envelope(r#"{"raw":"[]"}"#).is_none());
        assert!(HookEvent::from_envelope(r#"{"raw":"{}"}"#).is_none());
    }
}
