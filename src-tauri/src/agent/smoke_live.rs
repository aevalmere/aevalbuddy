//! The agent's smoke checks that need a real desktop: a Notepad and a small
//! form of their own, driven through the real helper by an agent whose model
//! is the fake one. `selftest/probes/agent.rs` runs them on CI only. They are
//! lib/smoke-v2-desktop.js `agentRun` to `passwordRedacted`, plus the
//! helper's `cancelStopsTyping`, `helperTimeoutNoLateInput` and `clickExact`.

use std::collections::HashSet;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::runtime::Handle;
use tokio_util::sync::CancellationToken;

use crate::desktop::protocol::handle_of;
use crate::desktop::selfcheck::{self, close_without_saving};
use crate::host::agent::Host as AppHost;
use crate::host::desktop_ops::DesktopAdapter;
use crate::llm::catalogue::{default_for, Kind};
use crate::llm::client::{Llm, Reply, Transport};
use crate::llm::errors::ApiError;
use crate::llm::fake::Fake;
use crate::llm::settings::Active;
use crate::llm::wire::HttpRequest;

use super::run::{trail_text, Agent, AgentHost, AgentSettings, Point, RunReport, StartOpts};
use super::seams::{DesktopOps, Emitter};
use super::smoke::{line, until, Line};

const CATALOGUE: &str = include_str!("../../../tools/fixtures/llm/catalogue.json");
const SECRET: &str = "hunter2-secret-9154";
const CONFIRM: &str = "agent:confirm-request";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const NOTEPAD_LABELS: [&str; 6] = [
    "agentRun",
    "agentFocusKept",
    "agentJsonMode",
    "agentStopMs",
    "agentTakeover",
    "agentMaxSteps",
];
const FORM_LABELS: [&str; 5] = [
    "agentConfirmGate",
    "passwordRedacted",
    "clickExact",
    "cancelStopsTyping",
    "helperTimeoutNoLateInput",
];

/// Answers the model-list fetch, so a run gets native tools as the shipped
/// app does. Chat goes to the fake model.
struct CatalogueNet;

#[async_trait]
impl Transport for CatalogueNet {
    async fn send(&self, _: HttpRequest, _: &CancellationToken) -> Result<Reply, ApiError> {
        Ok(Reply::full(200, CATALOGUE))
    }
}

/// A host that records the run and draws nothing. It asks the real app only
/// whether one of its windows has taken the keyboard.
struct Probe {
    app: AppHost,
    events: Mutex<Vec<(String, Value)>>,
    report: Mutex<Option<RunReport>>,
    stopped_at: Mutex<Option<Instant>>,
}

impl Probe {
    fn reset(&self) {
        self.events.lock().clear();
        *self.report.lock() = None;
        *self.stopped_at.lock() = None;
    }

    fn last(&self, channel: &str) -> Option<Value> {
        let events = self.events.lock();
        let hit = events.iter().rev().find(|(c, _)| c == channel);
        hit.map(|(_, v)| v.clone())
    }

    fn saw_state(&self, state: &str) -> bool {
        let events = self.events.lock();
        events
            .iter()
            .any(|(c, p)| c == "agent:state" && p["state"] == state)
    }
}

impl Emitter for Probe {
    fn emit(&self, channel: &str, payload: Value) {
        if channel == "agent:state" && payload["state"] == "stopped" {
            self.stopped_at.lock().get_or_insert_with(Instant::now);
        }
        self.events.lock().push((channel.to_string(), payload));
    }
}

#[async_trait]
impl AgentHost for Probe {
    fn settings(&self) -> AgentSettings {
        AgentSettings::default()
    }
    fn active(&self) -> Option<Active> {
        None
    }
    fn model(&self) -> String {
        default_for(Kind::Agent).to_string()
    }
    fn online(&self) -> bool {
        true
    }
    async fn point_at(&self, _: Point, _: &str) {}
    fn frame(&self, _: bool) {}
    fn show_trail(&self, _: bool) {}
    async fn open_target(&self, _: &str) -> Result<(), String> {
        Err("nothing is opened in the smoke run".into())
    }
    async fn own_focused(&self) -> Option<String> {
        self.app.own_focused().await
    }
    fn on_finish(&self, run: &RunReport) {
        *self.report.lock() = Some(run.clone());
    }
}

/// The agent under test and what it talks to.
struct Rig {
    d: Arc<DesktopAdapter>,
    agent: Agent,
    host: Arc<Probe>,
    fake: Arc<Fake>,
}

impl Rig {
    async fn new(app: AppHandle, d: Arc<DesktopAdapter>) -> Rig {
        let fake = Arc::new(Fake::load());
        let mut llm = Llm::new(Arc::new(CatalogueNet), Some(fake.clone()));
        llm.set_offline(false);
        let llm = Arc::new(llm);
        llm.fetch_catalogue().await;
        let host = Arc::new(Probe {
            app: AppHost::new(app, llm.clone()),
            events: Mutex::default(),
            report: Mutex::default(),
            stopped_at: Mutex::default(),
        });
        let agent = Agent::new(d.clone(), llm, host.clone(), Handle::current());
        Rig {
            d,
            agent,
            host,
            fake,
        }
    }

    fn begin(&self, o: StartOpts) -> bool {
        self.host.reset();
        self.agent.start(o)["ok"] == true
    }

    /// The finished run's report; empty if it did not end within `ms`.
    async fn finished(&self, ms: u64) -> RunReport {
        until(ms, || self.host.report.lock().is_some()).await;
        self.host.report.lock().clone().unwrap_or_default()
    }

    async fn focus(&self, h: &Value) {
        let _ = self.d.op("focusWindow", json!({ "handle": h })).await;
        sleep(400).await;
    }

    async fn text(&self, h: &Value) -> String {
        let args = json!({ "windowHandle": h, "ocrFallback": false });
        match self.d.op("getText", args).await {
            Ok(g) => g["text"].as_str().unwrap_or_default().to_string(),
            Err(e) => format!("(getText failed: {} {})", e.code, e.message),
        }
    }

    async fn windows(&self) -> Vec<Value> {
        let args = json!({ "includeMinimized": true });
        let list = self.d.op("listWindows", args).await.unwrap_or(Value::Null);
        list.as_array().cloned().unwrap_or_default()
    }

    /// The first window `want` accepts, waiting up to `ms` for one.
    async fn window(&self, ms: u64, want: impl Fn(&Value) -> bool) -> Option<Value> {
        let end = Instant::now() + Duration::from_millis(ms);
        loop {
            if let Some(w) = self.windows().await.into_iter().find(&want) {
                return Some(w);
            }
            if Instant::now() >= end {
                return None;
            }
            sleep(250).await;
        }
    }

    async fn title(&self, h: &Value) -> String {
        let list = self.windows().await;
        let hit = list.iter().find(|w| w["handle"] == *h);
        hit.map_or_else(String::new, |w| str_of(&w["title"]).to_string())
    }

    async fn node(&self, h: &Value, role: &str, name: &str) -> Option<Value> {
        let args = json!({ "windowHandle": h, "filter": "all" });
        let tree = self.d.op("readTree", args).await.ok()?;
        let nodes = tree["nodes"].as_array()?;
        let wanted = |n: &&Value| n["role"] == role && n["name"] == name;
        nodes.iter().find(wanted).cloned()
    }
}

async fn sleep(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

fn str_of(v: &Value) -> &str {
    v.as_str().unwrap_or_default()
}

fn opts(task: &str) -> StartOpts {
    StartOpts {
        task: task.into(),
        ..StartOpts::default()
    }
}

fn stamp() -> u128 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH);
    since.map_or(0, |d| d.as_millis())
}

struct Note {
    win: Value,
    tag: String,
    file: PathBuf,
    had_notepad: bool,
}

async fn open_notepad(r: &Rig) -> Result<Note, String> {
    let tag = format!("aeval-agent-check-{}-{:x}", std::process::id(), stamp());
    let file = std::env::temp_dir().join(format!("{tag}.txt"));
    let wrote = std::fs::write(&file, "Agent smoke\r\n");
    wrote.map_err(|e| e.to_string())?;
    let before = r.windows().await;
    let is_notepad = |w: &Value| str_of(&w["process"]).eq_ignore_ascii_case("notepad");
    let had_notepad = before.iter().any(is_notepad);
    let seen: HashSet<String> = before.iter().map(|w| w["handle"].to_string()).collect();
    Command::new("notepad.exe")
        .arg(&file)
        .spawn()
        .map_err(|e| e.to_string())?;
    let new_one =
        |w: &Value| !seen.contains(&w["handle"].to_string()) && str_of(&w["title"]).contains(&tag);
    let win = r.window(15000, new_one).await;
    let win = win.ok_or_else(|| "no Notepad window titled with the temp file".to_string())?;
    Ok(Note {
        win,
        tag,
        file,
        had_notepad,
    })
}

/// `agentRun` and `agentFocusKept`: a plan, a window switch and a typed
/// line, in native-tool dialect.
async fn agent_run(r: &Rig, h: &Value) -> [Line; 2] {
    let typed = "Agent typed 5810";
    r.focus(h).await;
    r.fake.set_var("agentText", json!(format!(" {typed}")));
    let began = r.begin(opts("smoke notepad"));
    let run = r.finished(30000).await;
    let text = r.text(h).await;
    let shown = |x: &&Value| x["kind"] != "remove" && x["kind"] != "thinking";
    let rows: Vec<&Value> = run.rows.iter().filter(shown).collect();
    let kinds: Vec<&str> = rows.iter().map(|x| str_of(&x["kind"])).collect();
    let trail = trail_text(&run.rows);
    let kept = !trail.is_empty() && !trail.contains("data:image");
    let wrote = text.contains(typed);
    let ok = began && run.end_state == "done" && wrote && rows.len() == 4 && kept;
    let state = format!("state={} typed={wrote} trail={kept}", run.end_state);
    let why = format!("{state} rows={}", kinds.join("+"));
    let violations = run.focus_violations;
    let detail = run.focus_detail.unwrap_or_default();
    [
        line("agentRun", ok, why),
        line(
            "agentFocusKept",
            violations == 0,
            format!("{violations} steps found {detail} in front"),
        ),
    ]
}

async fn json_mode(r: &Rig, h: &Value) -> Line {
    let typed = "Json typed 6620";
    r.fake.set_var("agentText", json!(format!(" {typed}")));
    r.focus(h).await;
    let mut o = opts("smoke notepad json");
    o.json_mode = true;
    let began = r.begin(o);
    let run = r.finished(30000).await;
    let text = r.text(h).await;
    let sent = r.fake.requests();
    let last = sent.iter().rev().find(|x| x.purpose == "agent");
    let plain = last.is_some_and(|b| !b.body.contains("\"tools\""));
    let wrote = text.contains(typed);
    let ok = began && run.end_state == "done" && plain && wrote;
    let why = format!("state={} plain={plain} typed={wrote}", run.end_state);
    line("agentJsonMode", ok, why)
}

/// The form's text box length, which the form writes into its title after
/// `len=` on every change. getText reads only the lines on screen, so it
/// cannot count what was typed.
async fn typed_len(r: &Rig, h: &Value) -> usize {
    let title = r.title(h).await;
    let tail = title.rsplit(" len=").next().unwrap_or_default();
    tail.trim().parse().unwrap_or(0)
}

/// Clicks the middle of a node of the form; true when the click went out.
async fn click_node(r: &Rig, node: &Value) -> bool {
    let n = |k: &str| node["rect"][k].as_f64().unwrap_or(0.0);
    let x = (n("x") + n("width") / 2.0).floor() as i64;
    let y = (n("y") + n("height") / 2.0).floor() as i64;
    let opts = json!({ "button": "left", "count": 1, "modifiers": "" });
    let click = json!({ "t": { "x": x, "y": y }, "opts": opts });
    r.d.op("click", click).await.is_ok()
}

/// Puts the keyboard in the form's text box, away from the password.
async fn focus_box(r: &Rig, h: &Value) -> bool {
    r.focus(h).await;
    let Some(field) = r.node(h, "edit", "Typed").await else {
        return false;
    };
    let clicked = click_node(r, &field).await;
    let _ = r.d.op("restoreCursor", json!({})).await;
    sleep(200).await;
    clicked
}

/// B-050: Stop while the helper is typing leaves part of the text, and
/// nothing more arrives afterwards.
async fn cancel_stops_typing(r: &Rig, h: &Value) -> Line {
    if !focus_box(r, h).await {
        return line("cancelStopsTyping", false, "no Typed box in the form".into());
    }
    let before = typed_len(r, h).await;
    let d = r.d.clone();
    let args = json!({ "text": "x".repeat(2000), "windowHandle": h });
    let typing = tokio::spawn(async move { d.op("type", args).await });
    sleep(200).await;
    let cancelled = r.d.op("cancel", json!({})).await;
    let _ = typing.await;
    sleep(100).await;
    let first = typed_len(r, h).await;
    sleep(500).await;
    let later = typed_len(r, h).await;
    let typed = first.saturating_sub(before);
    let ok = cancelled.is_ok() && typed > 0 && typed < 2000 && first == later;
    let why = format!("typed={typed} first={first} later={later}");
    line("cancelStopsTyping", ok, why)
}

/// B-050: a request the helper gave up on stops sending input too.
async fn timeout_no_late_input(r: &Rig, h: &Value) -> Line {
    if !focus_box(r, h).await {
        return line("helperTimeoutNoLateInput", false, "no Typed box".into());
    }
    let before = typed_len(r, h).await;
    let window = handle_of(h).map_or(Value::Null, |n| json!(n));
    let args = json!({ "text": "y".repeat(2000), "windowHandle": window });
    let got = r.d.inner().call(Some("type"), args, 150).await;
    let code = got.err().map(|e| e.code).unwrap_or_default();
    // A chunk already sent can still be landing.
    sleep(100).await;
    let first = typed_len(r, h).await;
    sleep(800).await;
    let later = typed_len(r, h).await;
    let typed = first.saturating_sub(before);
    let ok = code == "TIMEOUT" && typed < 2000 && first == later;
    let why = format!("code={code} typed={typed} first={first} later={later}");
    line("helperTimeoutNoLateInput", ok, why)
}

async fn stop_ms(r: &Rig) -> Line {
    let began = r.begin(opts("smoke stop"));
    sleep(600).await;
    let before = r.agent.dispatched();
    let t0 = Instant::now();
    r.agent.stop(None);
    sleep(1200).await;
    let at = *r.host.stopped_at.lock();
    let ms = at.map(|a| a.saturating_duration_since(t0).as_millis());
    let after = r.agent.dispatched();
    let ok = began && ms.is_some_and(|m| m < 300) && after == before;
    let why = format!("ms={ms:?} dispatched after={}", after - before);
    line("agentStopMs", ok, why)
}

/// The helper's own user-input event, the path a real mouse takes, reaches
/// this agent through the event stream: the app's hook feeds its own.
async fn takeover(r: &Rig) -> Line {
    let desktop = r.d.inner().clone();
    let mut events = desktop.subscribe();
    let agent = r.agent.clone();
    let forward = tokio::spawn(async move {
        while let Ok(ev) = events.recv().await {
            if ev["event"] == "user-input" {
                agent.on_user_input(&ev);
            }
        }
    });
    // The app watches input only while its own agent runs.
    let _ = desktop.watch_user_input(true).await;
    let began = r.begin(opts("smoke takeover"));
    sleep(500).await;
    let sent = desktop.simulate_user_input("key").await;
    let paused = until(3000, || r.host.saw_state("paused")).await;
    let state = r.agent.current().map(|c| c.1).unwrap_or_default();
    r.agent.stop(None);
    r.finished(3000).await;
    forward.abort();
    let _ = desktop.release_watch().await;
    let ok = began && sent.is_ok() && paused;
    line("agentTakeover", ok, format!("state={state}"))
}

async fn max_steps(r: &Rig) -> Line {
    let mut o = opts("smoke limit");
    o.max_steps = Some(10);
    let began = r.begin(o);
    let run = r.finished(30000).await;
    let ok = began && run.end_state == "limit" && run.final_text.contains("10 steps");
    line(
        "agentMaxSteps",
        ok,
        format!("{}: {}", run.end_state, run.final_text),
    )
}

async fn notepad_lines(r: &Rig, out: &mut Vec<Line>) {
    let note = match open_notepad(r).await {
        Ok(n) => n,
        Err(why) => {
            out.extend(NOTEPAD_LABELS.map(|l| line(l, false, why.clone())));
            return;
        }
    };
    let h = note.win["handle"].clone();
    r.fake.set_var("notepad", h.clone());
    out.extend(agent_run(r, &h).await);
    out.push(json_mode(r, &h).await);
    out.push(stop_ms(r).await);
    out.push(takeover(r).await);
    out.push(max_steps(r).await);
    let rep = selfcheck::Report::new("SMOKE");
    let desktop = r.d.inner();
    let closed = close_without_saving(desktop, &note.win, &note.tag, note.had_notepad, &rep).await;
    rep.log("agentNotepadClosed", closed);
    if std::fs::remove_file(&note.file).is_err() {
        rep.log("agentTempFileLeft", note.file.display());
    }
}

/// A WinForms window with a password box, a Send button and a nine-pixel
/// button: the smallest real app with an IsPassword field, a control named
/// after a risky verb and a target a pixel error would miss. The test form
/// stands in for a webview because the app's own windows refuse input.
fn form_script(tag: &str) -> String {
    [
        "Add-Type -AssemblyName System.Windows.Forms".to_string(),
        "$f = New-Object Windows.Forms.Form".to_string(),
        format!("$f.Text = '{tag}'"),
        "$f.Width = 360; $f.Height = 240; $f.TopMost = $true".to_string(),
        "$t = New-Object Windows.Forms.TextBox".to_string(),
        "$t.Left = 20; $t.Top = 140; $t.Width = 300; $t.AccessibleName = 'Typed'".to_string(),
        format!("$t.Add_TextChanged({{ $f.Text = '{tag} len=' + $t.Text.Length }})"),
        "$f.Controls.Add($t)".to_string(),
        "$p = New-Object Windows.Forms.TextBox".to_string(),
        format!("$p.UseSystemPasswordChar = $true; $p.Text = '{SECRET}'"),
        "$p.Left = 20; $p.Top = 20; $p.Width = 200; $p.AccessibleName = 'Password'".to_string(),
        "$b = New-Object Windows.Forms.Button".to_string(),
        "$b.Text = 'Send'; $b.Left = 20; $b.Top = 60".to_string(),
        format!("$b.Add_Click({{ $f.Text = '{tag} clicked' }})"),
        "$d = New-Object Windows.Forms.Button".to_string(),
        "$d.Text = ''; $d.AccessibleName = 'Dot'; $d.Left = 250; $d.Top = 100".to_string(),
        "$d.Width = 9; $d.Height = 9".to_string(),
        format!("$d.Add_Click({{ $f.Text = '{tag} dot' }})"),
        "$f.Controls.Add($p); $f.Controls.Add($b); $f.Controls.Add($d)".to_string(),
        "[Windows.Forms.Application]::Run($f)".to_string(),
    ]
    .join("\r\n")
}

/// `$f` and friends need an STA thread, so this is a script file.
fn open_form(tag: &str) -> Result<(Child, PathBuf), String> {
    let script = std::env::temp_dir().join(format!("{tag}.ps1"));
    let wrote = std::fs::write(&script, form_script(tag));
    wrote.map_err(|e| e.to_string())?;
    let shell = crate::desktop::client::powershell();
    let args = ["-NoProfile", "-NonInteractive", "-STA"];
    let child = Command::new(shell)
        .args(args)
        .args(["-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok((child, script))
}

/// `agentConfirmGate` and `passwordRedacted`: Send asks first and, denied,
/// is not pressed; the password's value never reaches the model.
async fn confirm_gate(r: &Rig, h: &Value) -> [Line; 2] {
    r.focus(h).await;
    let Some(send) = r.node(h, "button", "Send").await else {
        let why = "no Send button in the form".to_string();
        return [
            line("agentConfirmGate", false, why.clone()),
            line("passwordRedacted", false, why),
        ];
    };
    r.fake.set_var("sendRef", send["ref"].clone());
    let before = r.fake.requests().len();
    let began = r.begin(opts("smoke confirm gate"));
    until(15000, || r.host.last(CONFIRM).is_some()).await;
    let req = r.host.last(CONFIRM).unwrap_or(Value::Null);
    r.agent.confirm(str_of(&req["id"]), false);
    r.finished(15000).await;
    let title = r.title(h).await;
    let asked = str_of(&req["text"]).contains("Send");
    let kept = !title.is_empty() && !title.contains("clicked");
    let why = format!("request={} title={title}", str_of(&req["text"]));
    let sent = r.fake.requests();
    let bodies: Vec<&str> = sent.iter().skip(before).map(|x| x.body.as_str()).collect();
    let bodies = bodies.join("\n");
    let hidden = !bodies.is_empty() && !bodies.contains(SECRET) && bodies.contains("Password");
    let count = sent.len() - before;
    [
        line("agentConfirmGate", began && asked && kept, why),
        line("passwordRedacted", hidden, format!("requests={count}")),
    ]
}

/// B-141: a click on a nine-pixel button through the client lands on it.
async fn click_exact(r: &Rig, h: &Value) -> Line {
    r.focus(h).await;
    let Some(dot) = r.node(h, "button", "Dot").await else {
        return line("clickExact", false, "no Dot button in the form".into());
    };
    let n = |k: &str| dot["rect"][k].as_f64().unwrap_or(0.0);
    let x = (n("x") + n("width") / 2.0).floor() as i64;
    let y = (n("y") + n("height") / 2.0).floor() as i64;
    let at = r.d.op("elementAt", json!({ "x": x, "y": y })).await;
    let named = at.is_ok_and(|a| a["node"]["name"] == "Dot");
    let opts = json!({ "button": "left", "count": 1, "modifiers": "" });
    let click = json!({ "t": { "x": x, "y": y }, "opts": opts });
    let clicked = r.d.op("click", click).await;
    let mut title = String::new();
    for _ in 0..20 {
        title = r.title(h).await;
        if title.ends_with(" dot") {
            break;
        }
        sleep(100).await;
    }
    let _ = r.d.op("restoreCursor", json!({})).await;
    let hit = title.ends_with(" dot");
    let done = clicked.is_ok();
    let ok = named && done && hit;
    let why = format!("named={named} clicked={done} title={title} at=({x},{y})");
    line("clickExact", ok, why)
}

async fn form_lines(r: &Rig, out: &mut Vec<Line>) {
    let tag = format!("aeval-form-{}-{:x}", std::process::id(), stamp());
    let (mut child, script) = match open_form(&tag) {
        Ok(opened) => opened,
        Err(why) => {
            out.extend(FORM_LABELS.map(|l| line(l, false, why.clone())));
            return;
        }
    };
    let form = r.window(20000, |w| str_of(&w["title"]) == tag).await;
    match form {
        Some(form) => {
            let h = form["handle"].clone();
            out.extend(confirm_gate(r, &h).await);
            out.push(click_exact(r, &h).await);
            out.push(cancel_stops_typing(r, &h).await);
            out.push(timeout_no_late_input(r, &h).await);
            let _ = r.d.op("closeWindow", json!({ "handle": h })).await;
        }
        None => {
            let exit = child.try_wait().ok().flatten();
            let why = format!("the check form never opened: exit={exit:?}");
            out.extend(FORM_LABELS.map(|l| line(l, false, why.clone())));
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(&script);
}

/// Every line of the desktop half. The helper is put back as it was found.
pub async fn lines(app: AppHandle, d: Arc<DesktopAdapter>) -> Vec<Line> {
    let was_running = d.inner().is_ready();
    let rig = Rig::new(app, d).await;
    let mut out = Vec::new();
    notepad_lines(&rig, &mut out).await;
    form_lines(&rig, &mut out).await;
    if !was_running {
        rig.d.inner().stop();
    }
    out
}
