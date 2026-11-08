//! The agent's host: main.js agentEmit, onAgentState, showTrail,
//! setControlFrame, agentPointAt, dodgeAnswer, openTarget, finishAgentRun,
//! startAgentRun, the `agent.init({...})` block and the agent:* IPC rows.
//!
//! A run's events go to the one output window `outputs::target` gave when
//! it started; that window's conversation gets the trail and the answer.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use parking_lot::Mutex;
use regex::Regex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::agent::run::{trail_text, AgentHost, AgentSettings, Point, RunReport, StartOpts};
use crate::agent::seams::{DesktopOps, Emitter};
use crate::capture::screen as cap;
use crate::desktop::client::system_tool;
use crate::llm::catalogue::Kind;
use crate::llm::client::Llm;
use crate::llm::failover::short_model;
use crate::llm::provider::Provider;
use crate::llm::settings::{candidates_for, keyed, model_for, Active, LlmSettings};
use crate::llm::{recipients_with, Recipient};
use crate::state::{shared, OutputKind, Rect};
use crate::windows::{self, flags, layout, motion, outputs, screen};
use crate::{buddy, lens, store};

use super::settings::StoreSettings;

const FRAME_CHIP: &str = "AevalBuddy is using your computer \u{00b7} Esc to stop";
const OFFLINE: &str = "No connection. The agent did not start; nothing was done.";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// What `startAgentRun(o)` took.
#[derive(Clone, Debug, Default)]
pub struct StartReq {
    pub task: String,
    pub context: Option<String>,
    pub plan_gate: bool,
    pub json_mode: bool,
    pub max_steps: Option<u32>,
    pub pin: Pin,
}

/// The provider and model an `agent:start` or `agent:do-it` named.
#[derive(Clone, Debug, Default)]
pub struct Pin {
    pub provider: Option<Provider>,
    pub model: Option<String>,
}

/// What a run starts with, chosen once: it keeps this provider for its
/// whole life, and a pinned model never hands over to another.
pub(crate) struct RunChoice {
    pub active: Active,
    pub model: String,
    pub pinned: bool,
}

/// main.js module state: the run's output window, `agentModelNow`,
/// `pendingAgent`.
#[derive(Default)]
struct Slots {
    label: Option<String>,
    model: Option<String>,
    pending: Option<StartReq>,
    /// The window showing the first run's consent card, which becomes that
    /// run's trail.
    card: Option<String>,
}

fn slots() -> &'static Mutex<Slots> {
    static S: OnceLock<Mutex<Slots>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Slots::default()))
}

fn run_label() -> Option<String> {
    slots().lock().label.clone()
}

/// openTarget's check: letters, digits, spaces, dots, dashes, underscores.
fn is_app_name(t: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[\p{L}\p{N} ._-]{1,40}$").expect("valid pattern"))
        .is_match(t)
}

fn is_link(t: &str) -> bool {
    let lower = t.to_ascii_lowercase();
    ["http://", "https://", "mailto:"]
        .iter()
        .any(|p| lower.starts_with(p))
}

/// The last path segment of a page URL (`output.html`), or `window`.
fn page_name(path: &str) -> String {
    path.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("window")
        .to_string()
}

/// setOutputView('chat') on one output. `force` sends even when the view
/// was already chat, as startAgentRun did.
fn set_view_chat(app: &AppHandle, label: &str, force: bool) {
    let changed = outputs::with(app, label, |o| {
        let changed = o.view != "chat";
        o.view = "chat".into();
        changed
    })
    .unwrap_or(false);
    if changed || force {
        windows::emit(app, label, "view:set", json!({ "view": "chat" }));
    }
}

/// main.js showTrail: the run's output in chat view, shown; focused when
/// the run needs an answer.
pub fn show_trail(app: &AppHandle, focus: bool) {
    let label = run_label().filter(|l| outputs::with(app, l, |_| ()).is_some());
    let Some(label) = label else {
        outputs::show_trail(app, focus);
        return;
    };
    set_view_chat(app, &label, false);
    outputs::show(app, &label);
    if focus {
        if let Some(w) = windows::window(app, &label) {
            flags::show_focused(&w);
        }
    }
}

fn status(s: &str) -> Value {
    json!({ "status": s })
}

/// The provider and model a run starts with (K-16): what the request
/// pinned, else the first candidate, with the person's pick for tasks or
/// its default on a native provider, and OpenRouter's stored agent model
/// checked against the cached catalogue. `None` is no usable key for it.
pub(crate) fn run_choice(s: &dyn LlmSettings, llm: &Llm, pin: &Pin) -> Option<RunChoice> {
    let found = match pin.provider {
        Some(p) => keyed(s).into_iter().find(|c| c.provider == p),
        None => candidates_for(s, Kind::Agent)
            .into_iter()
            .next()
            .or_else(|| keyed(s).into_iter().next()),
    };
    let active = found?.active();
    let model = match (&pin.model, active.provider) {
        (Some(m), _) => m.clone(),
        (None, Provider::OpenRouter) => llm.resolve_known(Kind::Agent, &s.model(Kind::Agent)),
        (None, p) => model_for(s, p, Kind::Agent),
    };
    Some(RunChoice {
        active,
        model,
        pinned: pin.model.is_some(),
    })
}

/// OpenRouter's stored model checked against the live catalogue, and healed
/// if it has left it. A native id or a pin is used as it stands.
async fn settle_model(choice: &mut RunChoice, s: &dyn LlmSettings, llm: &Llm) {
    if choice.active.provider.is_native() || choice.pinned {
        return;
    }
    let stored = s.model(Kind::Agent);
    choice.model = llm.resolve_live(Kind::Agent, &stored).await;
    s.heal_model(Kind::Agent, &choice.model);
}

/// `agent:consent-request` (K-21): the task, and who receives each step's
/// screenshot and window text, so the card names the real recipient.
fn consent_payload(task: &str, s: &dyn LlmSettings, llm: &Llm, pin: &Pin) -> Value {
    let all = llm.known_catalogue();
    let mut recipients = recipients_with(s, all.as_deref().map(Vec::as_slice));
    let Some(choice) = run_choice(s, llm, pin) else {
        return json!({ "task": task, "recipients": recipients });
    };
    let (provider, model) = (choice.active.provider, choice.model);
    let name = short_model(&model);
    let free = model.ends_with(":free");
    // The list is who a screen question can go to; the run goes to one of
    // them, on its own model, and a pinned run to that one alone.
    if pin.provider.is_some() {
        recipients.clear();
    }
    recipients.retain(|r| r.id != provider.id());
    recipients.insert(
        0,
        Recipient {
            id: provider.id(),
            name: provider.name(),
            model: model.clone(),
            model_name: name.clone(),
            free,
        },
    );
    let mut out = json!({ "task": task, "recipients": recipients });
    out["provider"] = json!(provider.id());
    out["providerName"] = json!(provider.name());
    out["model"] = json!(model);
    out["modelName"] = json!(name);
    out["free"] = json!(free);
    out
}

/// main.js startAgentRun.
pub async fn start_run(app: &AppHandle, req: StartReq) -> Value {
    let task = req.task.trim().to_string();
    if task.is_empty() {
        return status("empty");
    }
    if !store::get().has_api_key() {
        super::send_overlay_state(app);
        return status("needs-key");
    }
    let sv = super::services();
    // Held until the run exists, so a second start during the waits below
    // is turned away (B-130).
    let Some(claim) = sv.agent.claim() else {
        return status("busy");
    };
    let Some(mut choice) = run_choice(&StoreSettings, &sv.llm, &req.pin) else {
        super::send_overlay_state(app);
        return status("needs-key");
    };
    if store::get().get_agent_settings()["consent"].as_bool() != Some(true) {
        let payload = consent_payload(&task, &StoreSettings, &sv.llm, &req.pin);
        let card = outputs::target(app);
        {
            let mut s = slots().lock();
            s.pending = Some(req);
            s.card = card.clone();
        }
        if let Some(label) = card {
            set_view_chat(app, &label, true);
            outputs::show(app, &label);
            windows::emit(app, &label, "agent:consent-request", payload);
            if let Some(w) = windows::window(app, &label) {
                flags::show_focused(&w);
            }
        }
        return status("consent");
    }
    if sv.find.is_open() {
        shared(app).call(app, |k| k.close_find.as_ref());
    }
    // Its own window (K-04), or the one the consent card just asked in.
    let card = slots().lock().card.take();
    let open = |l: &String| outputs::with(app, l, |o| o.active).unwrap_or(false);
    let label = card.filter(open).or_else(|| outputs::fresh(app));
    slots().lock().label = label.clone();
    if let Some(l) = &label {
        outputs::with(app, l, |o| {
            o.kind = OutputKind::Trail;
            o.chat
                .messages
                .push(json!({ "role": "user", "text": task }));
        });
        let gen = {
            let st = shared(app);
            let mut s = st.lock();
            s.query_gen += 1;
            s.query_gen
        };
        windows::emit(app, l, "chat:user", json!({ "gen": gen, "text": task }));
        windows::emit(app, l, "agent:begin", json!({ "task": task }));
        set_view_chat(app, l, true);
    }
    if !super::online() {
        if let Some(l) = &label {
            let done = json!({ "state": "offline", "text": OFFLINE, "rows": [] });
            windows::emit(app, l, "agent:done", done);
        }
        return status("offline");
    }
    settle_model(&mut choice, &StoreSettings, &sv.llm).await;
    slots().lock().model = Some(choice.model.clone());
    // The bar is not needed while the agent works: back to the pill, with
    // the trail beside it, shown without focus.
    windows::collapse_panel(app, true);
    if let Some(l) = &label {
        outputs::show(app, l);
    }
    let context = req.context.filter(|c| !c.is_empty());
    let r = claim.start(StartOpts {
        task,
        plan_gate: context.is_some() || req.plan_gate,
        context,
        region: super::active_region_px(),
        json_mode: req.json_mode,
        max_steps: req.max_steps,
        active: Some(choice.active),
        model: Some(choice.model),
        pinned: choice.pinned,
    });
    super::send_overlay_state(app);
    super::apply_registrations(app);
    crate::tray::refresh(app);
    if r["ok"] == true {
        json!({ "status": "started", "runId": r["runId"] })
    } else {
        status("busy")
    }
}

/// `agent:consent`: the first run's consent card was answered.
fn consent(app: &AppHandle, allow: bool) -> Value {
    let pending = slots().lock().pending.take();
    if !allow {
        slots().lock().card = None;
        return json!({ "ok": true, "started": false });
    }
    store::get().set_agent_settings(&json!({ "consent": true }));
    let started = pending.is_some();
    if let Some(req) = pending {
        let a = app.clone();
        tauri::async_runtime::spawn(async move {
            start_run(&a, req).await;
        });
    }
    json!({ "ok": true, "started": started })
}

fn str_of(p: &Value, k: &str) -> String {
    p[k].as_str().unwrap_or_default().to_string()
}

/// The optional `{provider, model}` on `agent:start` and `agent:do-it`.
/// `None` for a provider that does not exist: the screen must not go to
/// some other provider instead.
fn pin_of(p: &Value) -> Option<Pin> {
    let provider = match p["provider"].as_str() {
        Some(id) => Some(Provider::from_id(id)?),
        None => None,
    };
    let model = p["model"].as_str().map(str::trim).filter(|m| !m.is_empty());
    Some(Pin {
        provider,
        model: model.map(str::to_string),
    })
}

fn id_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// IPC rows agent:start/stop/confirm/continue/consent/do-it; None for any
/// other channel.
pub async fn route(app: &AppHandle, channel: &str, p: &Value) -> Option<Result<Value, String>> {
    let v = match channel {
        "agent:start" => {
            let Some(pin) = pin_of(p) else {
                return Some(Ok(status("needs-key")));
            };
            let req = StartReq {
                task: str_of(p, "task"),
                context: p["context"].as_str().map(str::to_string),
                pin,
                ..StartReq::default()
            };
            start_run(app, req).await
        }
        "agent:stop" => json!({ "ok": super::services().agent.stop(None) }),
        "agent:confirm" => {
            let allow = p["allow"].as_bool().unwrap_or(false);
            json!({ "ok": super::services().agent.confirm(&id_of(&p["id"]), allow) })
        }
        "agent:continue" => json!({ "ok": super::services().agent.resume(&str_of(p, "text")) }),
        "agent:consent" => consent(app, p["allow"].as_bool().unwrap_or(false)),
        "agent:do-it" => {
            let Some(pin) = pin_of(p) else {
                return Some(Ok(status("needs-key")));
            };
            let question = str_of(p, "question");
            let task = if question.is_empty() {
                "Do what the answer describes".to_string()
            } else {
                question
            };
            let context = format!(
                "Your earlier answer, which the user wants carried out:\n{}",
                str_of(p, "answer")
            );
            let req = StartReq {
                task,
                context: Some(context),
                pin,
                ..StartReq::default()
            };
            start_run(app, req).await
        }
        _ => return None,
    };
    Some(Ok(v))
}

/// Everything `agent.init({...})` took besides the desktop and the model
/// client.
pub struct Host {
    app: AppHandle,
    llm: Arc<Llm>,
}

impl Host {
    pub fn new(app: AppHandle, llm: Arc<Llm>) -> Host {
        Host { app, llm }
    }
}

impl Emitter for Host {
    /// main.js agentEmit: to the run's output; state changes refresh the
    /// pill and the tray.
    fn emit(&self, channel: &str, payload: Value) {
        if let Some(label) = run_label() {
            windows::emit(&self.app, &label, channel, payload);
        }
        if channel == "agent:state" || channel == "agent:done" {
            super::send_overlay_state(&self.app);
            crate::tray::refresh(&self.app);
        }
    }
}

#[async_trait::async_trait]
impl AgentHost for Host {
    fn settings(&self) -> AgentSettings {
        let v = store::get().get_agent_settings();
        AgentSettings {
            every: v["permission"] == "every",
            max_steps: v["maxSteps"].as_u64().unwrap_or(0) as u32,
            screenshots: v["screenshots"].as_bool().unwrap_or(true),
            return_cursor: v["returnCursor"].as_bool().unwrap_or(true),
        }
    }

    fn active(&self) -> Option<Active> {
        crate::llm::settings::active(&StoreSettings)
    }

    fn model(&self) -> String {
        let now = slots().lock().model.clone();
        now.unwrap_or_else(|| {
            self.llm
                .resolve_known(Kind::Agent, &store::get().get_agent_model())
        })
    }

    fn online(&self) -> bool {
        super::online()
    }

    /// main.js agentPointAt.
    async fn point_at(&self, point: Point, label: &str) {
        let p = screen::px_to_dip_point(point.x as f64, point.y as f64);
        let display = cap::nearest_point(&cap::all_displays(), p.0, p.1);
        buddy::agent_point(p, label, &display).await;
    }

    /// main.js setControlFrame.
    fn frame(&self, on: bool) {
        if !on {
            lens::set_frame(&self.app, None, "");
            return;
        }
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let Some(sv) = super::try_services() else {
                return;
            };
            let fg = sv.desktop.op("foregroundWindow", json!({})).await;
            let rect = fg.ok().and_then(|w| super::px_rect_to_dip(&w["rect"]));
            let list = cap::all_displays();
            let display = match rect {
                Some(r) => cap::matching(&list, &cap::Rect::new(r.x, r.y, r.width, r.height)),
                None => cap::active_display(&list),
            };
            if sv.agent.is_running() {
                lens::set_frame(&app, Some(&display), FRAME_CHIP);
            }
        });
    }

    fn show_trail(&self, focus: bool) {
        show_trail(&self.app, focus);
    }

    /// main.js dodgeAnswer: the answer window steps aside if the agent is
    /// about to act under it.
    async fn dodge(&self, point: Point) {
        let Some(label) = run_label() else {
            return;
        };
        if !outputs::with(&self.app, &label, |o| o.active).unwrap_or(false) {
            return;
        }
        let Some(h) = windows::window(&self.app, &label).and_then(|w| flags::hwnd_of(&w)) else {
            return;
        };
        let (x, y) = screen::px_to_dip_point(point.x as f64, point.y as f64);
        let b = motion::glass(h);
        if !b.contains(x, y) {
            return;
        }
        let wa = screen::matching(b).work_area;
        let to = Rect::new(layout::dodge_x(wa, b), b.y, b.width, b.height);
        outputs::with(&self.app, &label, |o| o.rect = Some(to));
        windows::mark_app(&label);
        motion::animate(h, None, to, layout::T_OUT_GROW, None);
        tokio::time::sleep(Duration::from_millis(layout::T_OUT_GROW + 30)).await;
    }

    /// main.js openTarget: links through the browser, apps by name through
    /// the shell. Every open is confirmed before it gets here.
    async fn open_target(&self, target: &str) -> Result<(), String> {
        let t = target.trim();
        if is_link(t) {
            use tauri_plugin_opener::OpenerExt;
            return self
                .app
                .opener()
                .open_url(t, None::<&str>)
                .map_err(|e| e.to_string());
        }
        if !is_app_name(t) {
            return Err("not an app name".into());
        }
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new(system_tool("cmd.exe"));
        cmd.args(["/d", "/c", "start"])
            .raw_arg("\"\"")
            .raw_arg(format!("\"{t}\""))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        match cmd.spawn() {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// The OS's word, not the webview's: a window still marked focused
    /// after a blur does not count when another app is in front.
    async fn own_focused(&self) -> Option<String> {
        let page = self
            .app
            .webview_windows()
            .into_values()
            .find(|w| w.is_focused().unwrap_or(false))
            .map(|w| {
                w.url()
                    .map(|u| page_name(u.path()))
                    .unwrap_or_else(|_| "window".into())
            })?;
        if let Some(sv) = super::try_services() {
            if let Ok(list) = sv.desktop.op("listWindows", json!({})).await {
                let front = list
                    .as_array()
                    .is_some_and(|a| a.iter().any(|w| w["isForeground"] == true));
                if front {
                    return None;
                }
            }
        }
        Some(page)
    }

    fn clipboard_read(&self) -> Option<String> {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        self.app.clipboard().read_text().ok()
    }

    fn clipboard_write(&self, text: &str) -> bool {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        self.app.clipboard().write_text(text).is_ok()
    }

    /// main.js finishAgentRun.
    fn on_finish(&self, run: &RunReport) {
        let app = &self.app;
        if run.focus_violations > 0 {
            eprintln!(
                "[agent] run {} ({:?}) left the target window {} time(s): {}",
                run.id,
                run.task,
                run.focus_violations,
                run.focus_detail.as_deref().unwrap_or("")
            );
        }
        buddy::agent_clear();
        lens::set_frame(app, None, "");
        if let Some(label) = run_label() {
            let trail = trail_text(&run.rows);
            outputs::with(app, &label, |o| {
                if !trail.is_empty() {
                    let row = json!({ "role": "assistant", "kind": "trail", "text": trail });
                    o.chat.messages.push(row);
                }
                if !run.final_text.is_empty() {
                    let row = json!({ "role": "assistant", "text": run.final_text });
                    o.chat.messages.push(row);
                }
            });
            super::remember_chat(app, &label, &run.model);
        }
        let failed = matches!(run.end_state.as_str(), "error" | "limit" | "offline");
        shared(app).lock().pill_error = failed;
        let a = app.clone();
        tauri::async_runtime::spawn(async move {
            super::apply_registrations(&a);
            super::send_overlay_state(&a);
            crate::tray::refresh(&a);
            crate::updater::idle(&a);
        });
    }

    /// main.js onAgentState.
    fn on_state(&self, state: &str) {
        super::apply_registrations(&self.app);
        if state == "confirm" || state == "asking" {
            show_trail(&self.app, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::client::{Reply, Transport};
    use crate::llm::errors::ApiError;
    use crate::llm::settings::MemorySettings;
    use crate::llm::wire::HttpRequest;
    use tokio_util::sync::CancellationToken;

    struct NoNet;

    #[async_trait::async_trait]
    impl Transport for NoNet {
        async fn send(&self, _: HttpRequest, _: &CancellationToken) -> Result<Reply, ApiError> {
            Err(ApiError::network("no network in tests"))
        }
    }

    fn llm() -> Llm {
        Llm::new(Arc::new(NoNet), None)
    }

    #[test]
    fn consent_payload_names_provider() {
        let llm = llm();
        let ant = MemorySettings::with_key("sk-ant-test");
        let none = Pin::default();
        let v = consent_payload("tidy the desktop", &ant, &llm, &none);
        assert_eq!(v["task"], "tidy the desktop");
        assert_eq!(v["provider"], "anthropic");
        assert_eq!(v["providerName"], "Anthropic");
        let native = Provider::Anthropic.default_model(Kind::Agent);
        assert_eq!(v["model"], native);
        assert_eq!(v["free"], false);
        assert_eq!(v["recipients"][0]["id"], "anthropic");
        assert_eq!(v["recipients"][0]["modelName"], v["modelName"]);
        // A native pick for tasks is the model the card names.
        let key = (Provider::Anthropic, Kind::Agent);
        ant.native.lock().insert(key, "claude-test-pick".into());
        let v = consent_payload("x", &ant, &llm, &none);
        assert_eq!(v["model"], "claude-test-pick");
        let or = MemorySettings::with_key("sk-or-test");
        let v = consent_payload("x", &or, &llm, &none);
        assert_eq!(v["provider"], "openrouter");
        let free = crate::llm::catalogue::default_for(Kind::Agent);
        assert_eq!(v["model"], free);
        assert_eq!(v["free"], free.ends_with(":free"));
        let keyless = consent_payload("x", &MemorySettings::default(), &llm, &none);
        assert_eq!(keyless["recipients"], json!([]));
        assert!(keyless.get("provider").is_none());
    }

    /// Two keys: the card lists who a question can reach, led by the run's
    /// own provider on the run's own model. A pin names only that one.
    #[test]
    fn consent_recipients_come_from_the_ready_list() {
        let llm = llm();
        let s = MemorySettings::with_key("sk-ant-test");
        s.keys.lock().insert(Provider::Gemini, "AIzaTest".into());
        let v = consent_payload("x", &s, &llm, &Pin::default());
        let first = &v["recipients"][0];
        assert_eq!(first["model"], v["model"]);
        assert_eq!(first["id"], v["provider"]);
        assert_eq!(v["recipients"].as_array().unwrap().len(), 2);
        let pin = Pin {
            provider: Some(Provider::Gemini),
            model: None,
        };
        let v = consent_payload("x", &s, &llm, &pin);
        assert_eq!(v["provider"], "gemini");
        assert_eq!(v["recipients"].as_array().unwrap().len(), 1);
    }

    /// Attack: a native-only account handed an OpenRouter id, or its id
    /// healed into the OpenRouter slot.
    #[tokio::test]
    async fn native_only_run_uses_native_model() {
        let llm = llm();
        let s = MemorySettings::with_key("sk-ant-test");
        let mut choice = run_choice(&s, &llm, &Pin::default()).unwrap();
        settle_model(&mut choice, &s, &llm).await;
        assert_eq!(choice.active.provider, Provider::Anthropic);
        assert_eq!(choice.model, Provider::Anthropic.default_model(Kind::Agent));
        assert!(!choice.pinned);
        assert!(s.healed.lock().is_empty());
        let key = (Provider::Anthropic, Kind::Agent);
        s.native.lock().insert(key, "claude-test-pick".into());
        let mut choice = run_choice(&s, &llm, &Pin::default()).unwrap();
        settle_model(&mut choice, &s, &llm).await;
        assert_eq!(choice.model, "claude-test-pick");
        assert!(s.healed.lock().is_empty());
    }

    #[test]
    fn a_pin_for_a_provider_with_no_key_does_not_run_elsewhere() {
        let llm = llm();
        let s = MemorySettings::with_key("sk-ant-test");
        let pin = Pin {
            provider: Some(Provider::Gemini),
            model: None,
        };
        assert!(run_choice(&s, &llm, &pin).is_none());
        assert!(pin_of(&json!({ "provider": "nobody" })).is_none());
        let named = pin_of(&json!({ "provider": "gemini", "model": " m " })).unwrap();
        assert_eq!(named.provider, Some(Provider::Gemini));
        assert_eq!(named.model.as_deref(), Some("m"));
        assert!(pin_of(&json!({})).unwrap().model.is_none());
    }

    /// Attack: the pinned free model is busy. An unpinned run hands over to
    /// the next free one; this one must fail on the model it was given.
    #[tokio::test(flavor = "multi_thread")]
    async fn pinned_model_never_fails_over() {
        use crate::agent::smoke;
        let client = llm();
        let s = MemorySettings::with_key("sk-or-test");
        let pin = Pin {
            provider: None,
            model: Some("vendor/pinned:free".into()),
        };
        let choice = run_choice(&s, &client, &pin).unwrap();
        assert!(choice.pinned);
        let turns = json!([{ "status": 429 }, { "text": "Not reached." }]);
        let busy = json!({ "purpose": "agent", "task": "pinned", "turns": turns });
        let fake = Arc::new(crate::llm::fake::Fake::from_fixtures(vec![busy]));
        let (agent, host) = smoke::agent_on(smoke::notepad(), fake.clone());
        agent.start(StartOpts {
            task: "pinned".into(),
            active: Some(choice.active),
            model: Some(choice.model),
            pinned: choice.pinned,
            ..StartOpts::default()
        });
        assert!(smoke::until(10000, || !agent.is_running()).await);
        assert_eq!(host.last("agent:done").unwrap()["state"], "error");
        let sent = fake.requests();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].model.as_deref(), Some("vendor/pinned:free"));
    }

    #[test]
    fn app_names_are_held_to_safe_characters() {
        assert!(is_app_name("notepad"));
        assert!(is_app_name("Visual Studio Code"));
        assert!(is_app_name("calc.exe"));
        assert!(is_app_name("Bloc-notes_2"));
        assert!(is_app_name("Блокнот"));
        assert!(!is_app_name(""));
        assert!(!is_app_name("calc & del x"));
        assert!(!is_app_name("a\"b"));
        assert!(!is_app_name("C:\\Windows\\notepad"));
        assert!(!is_app_name(&"a".repeat(41)));
        assert!(is_app_name(&"a".repeat(40)));
    }

    #[test]
    fn links_open_through_the_browser() {
        assert!(is_link("https://example.com"));
        assert!(is_link("HTTP://example.com"));
        assert!(is_link("mailto:a@b.c"));
        assert!(!is_link("file:///c:/x"));
        assert!(!is_link("notepad"));
    }

    #[test]
    fn page_names_come_from_the_url_path() {
        assert_eq!(page_name("/renderer/output.html"), "output.html");
        assert_eq!(page_name("/"), "window");
        assert_eq!(page_name(""), "window");
    }
}
