//! Find's side of main.js: setBarMode, setOutputView, openFind, closeFind,
//! findEmit, findToLens, findToAsk, the `find.init({...})` deps and the
//! bar:mode / find:* IPC rows.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::capture;
use crate::find::{self as engine, FindHost, FindSettings, Related};
use crate::lens;
use crate::llm::catalogue::Kind;
use crate::llm::client::Llm;
use crate::llm::settings::{self as llm_settings, model_for, Active, LlmSettings};
use crate::state::{shared, Rect};
use crate::windows::{self, outputs};
use crate::{buddy, region, store};

use super::ask::{self, SubmitOpts};
use super::settings::StoreSettings;
use super::{
    active_region_px, apply_registrations, px_rect_to_dip, send_overlay_state, services,
    try_services,
};

/// Room kept around a hit when it becomes an Ask's area, DIPs.
const ASK_PAD: f64 = 120.0;
/// Longest prefill and `why` text carried over, characters.
const CARRY_MAX: usize = 120;

/// The output window Find's results show in, and the last `find:hits`
/// (resent when a window is first picked, so it never misses a round).
struct Out {
    label: Option<String>,
    last_hits: Option<Value>,
    /// The conversation that was in front when Find took a window of its
    /// own; it is in front again once Find closes.
    prev: Option<String>,
}

static OUT: Mutex<Out> = Mutex::new(Out {
    label: None,
    last_hits: None,
    prev: None,
});

fn out() -> std::sync::MutexGuard<'static, Out> {
    OUT.lock().unwrap_or_else(|e| e.into_inner())
}

/// `find.init({...})`'s deps.
pub struct Host {
    app: AppHandle,
    llm: Arc<Llm>,
}

impl Host {
    pub fn new(app: AppHandle, llm: Arc<Llm>) -> Host {
        Host { app, llm }
    }
}

impl FindHost for Host {
    fn settings(&self) -> FindSettings {
        let v = store::get().get_find_settings();
        FindSettings {
            related: Related::from_value(&v["related"]),
            ocr: v["ocr"].as_bool().unwrap_or(true),
        }
    }

    fn online(&self) -> bool {
        super::online()
    }

    fn active(&self) -> Option<Active> {
        llm_settings::active(&StoreSettings)
    }

    fn model(&self) -> String {
        find_model(&StoreSettings, &self.llm)
    }

    fn emit(&self, channel: &str, payload: Value) {
        find_emit(&self.app, channel, payload);
    }

    fn on_lens(&self, hits: Value, current: i64) {
        lens::set_hits(&self.app, lens_hits(&hits), current);
    }

    fn notify(&self, event: &str) {
        buddy::notify(event);
    }
}

// ------------------------------------------------------------ pure helpers

/// The Find model for the provider that will answer: a native provider's
/// own pick or default (never an OpenRouter id, which it would refuse), else
/// the stored OpenRouter choice against the catalogue.
fn find_model(settings: &dyn LlmSettings, llm: &Llm) -> String {
    match llm_settings::active(settings).map(|a| a.provider) {
        Some(p) if p.is_native() => {
            llm.known_native(p, Kind::Find, &model_for(settings, p, Kind::Find))
        }
        _ => llm.resolve_known(Kind::Find, &settings.model(Kind::Find)),
    }
}

/// setBarMode's normalisation: find and agent stay, anything else is ask.
fn next_mode(mode: &str) -> &'static str {
    match mode {
        "find" => "find",
        "agent" => "agent",
        _ => "ask",
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// findToAsk's ` (it matched because ...)`, empty with no reason.
fn why_suffix(why: Option<&Value>) -> String {
    match why.and_then(Value::as_str) {
        Some(w) if !w.is_empty() => {
            format!(" (it matched because {})", truncate_chars(w, CARRY_MAX))
        }
        _ => String::new(),
    }
}

/// findEmit's test for a `find:state` worth showing the results for.
fn has_content(payload: &Value) -> bool {
    let open = payload["open"].as_bool().unwrap_or(false);
    let status = payload["status"].as_str().is_some_and(|s| !s.is_empty());
    let lines = payload["lines"].as_array().is_some_and(|l| !l.is_empty());
    open && (status || lines)
}

/// A hit's DIP rect with ASK_PAD on every side.
fn padded(r: Rect) -> capture::Rect {
    capture::Rect::new(
        r.x - ASK_PAD,
        r.y - ASK_PAD,
        r.width + ASK_PAD * 2.0,
        r.height + ASK_PAD * 2.0,
    )
}

/// `String(p.prefill)` for the truthy strings and numbers the renderer sends.
fn prefill_text(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.is_empty() => Some(truncate_chars(s, CARRY_MAX)),
        Value::Number(n) if n.as_f64() != Some(0.0) => {
            Some(truncate_chars(&n.to_string(), CARRY_MAX))
        }
        _ => None,
    }
}

/// find:to-ask's index: a finite number, or none. A fraction indexes nothing.
fn hit_index(v: Option<&Value>) -> Option<i64> {
    let f = v?.as_f64().filter(|f| f.is_finite())?;
    Some(if f.fract() == 0.0 { f as i64 } else { -1 })
}

fn toggle_scope(scope: Option<&str>) -> &'static str {
    if scope == Some("screen") {
        "window"
    } else {
        "screen"
    }
}

// --------------------------------------------------------------- windows

/// The stored Find output, if that window still exists.
fn stored_label(app: &AppHandle) -> Option<String> {
    let label = out().label.clone()?;
    if outputs::with(app, &label, |_| ()).is_some() {
        Some(label)
    } else {
        out().label = None;
        None
    }
}

/// The output to show results in: the stored one while it is open, else
/// `outputs::fresh`. The flag is true when the window changed.
fn ensure_label(app: &AppHandle) -> Option<(String, bool)> {
    let cur = stored_label(app);
    if let Some(l) = &cur {
        if outputs::with(app, l, |o| o.active).unwrap_or(false) {
            return Some((l.clone(), false));
        }
    }
    // Never the chat in front: results get a window of their own (B-020).
    // With no chat in front, any idle window will do, the one Find used
    // last time included.
    let is_active = |l: &String| outputs::with(app, l, |o| o.active).unwrap_or(false);
    let front = outputs::focused(app).filter(is_active);
    let label = match front {
        Some(_) => outputs::fresh(app)?,
        None => outputs::target(app)?,
    };
    let fresh = cur.as_deref() != Some(label.as_str());
    if let (true, Some(old)) = (fresh, &cur) {
        // The window Find used before goes back to being a chat; left in
        // the results view, the next answer sent to it would not show.
        set_view(app, old, "chat");
    }
    let mut o = out();
    o.label = Some(label.clone());
    o.prev = front;
    drop(o);
    Some((label, fresh))
}

fn set_view(app: &AppHandle, label: &str, view: &str) {
    outputs::with(app, label, |o| o.view = view.to_string());
    windows::emit(app, label, "view:set", json!({ "view": view }));
}

/// main.js setOutputView, on Find's output.
fn set_output_view(app: &AppHandle, view: &str) {
    if let Some(l) = stored_label(app) {
        set_view(app, &l, view);
    }
}

/// main.js hideOutput for one window.
fn hide_output(app: &AppHandle, label: &str) {
    outputs::with(app, label, |o| o.active = false);
    if let Some(w) = windows::window(app, label) {
        if let Some(h) = windows::flags::hwnd_of(&w) {
            windows::motion::stop(h);
        }
        windows::flags::hide(&w);
    }
}

/// findToLens: physical rects to DIPs; hits without one are dropped.
fn lens_hits(hits: &Value) -> Vec<lens::Hit> {
    let Some(list) = hits.as_array() else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|h| {
            let r = px_rect_to_dip(h.get("rect")?)?;
            Some(lens::Hit {
                i: h["i"].as_i64().unwrap_or(0),
                kind: h["kind"].as_str().unwrap_or_default().to_string(),
                rect: Some(capture::Rect::new(r.x, r.y, r.width, r.height)),
                why: h.get("why").cloned().unwrap_or(Value::Null),
                offscreen: h["offscreen"].as_bool().unwrap_or(false),
            })
        })
        .collect()
}

/// main.js findEmit: to Find's output and the overlay; the results show
/// once there is something to say.
fn find_emit(app: &AppHandle, channel: &str, payload: Value) {
    if channel == "find:hits" {
        out().last_hits = Some(payload.clone());
    }
    if channel == "find:state" {
        let open = payload["open"].as_bool().unwrap_or(false);
        let was = std::mem::replace(&mut shared(app).lock().find_open, open);
        if has_content(&payload) {
            if let Some((label, fresh)) = ensure_label(app) {
                if fresh {
                    let hits = out().last_hits.clone();
                    if let Some(h) = hits {
                        windows::emit(app, &label, "find:hits", h);
                    }
                }
                let (view, active) =
                    outputs::with(app, &label, |o| (o.view.clone(), o.active)).unwrap_or_default();
                if view != "find" {
                    set_view(app, &label, "find");
                }
                if !active {
                    outputs::show(app, &label);
                }
            }
        }
        if was != open {
            send_overlay_state(app);
        }
    }
    if let Some(l) = stored_label(app) {
        windows::emit(app, &l, channel, payload.clone());
    }
    windows::emit(app, windows::OVERLAY, channel, payload);
}

// ------------------------------------------------------------- the API

fn find_is_open() -> bool {
    try_services().is_some_and(|s| s.find.is_open())
}

/// main.js setBarMode; also the core's `set_bar_mode` hook.
pub fn set_bar_mode(app: &AppHandle, mode: &str) {
    let next = next_mode(mode);
    let prev = std::mem::replace(&mut shared(app).lock().bar_mode, next.to_string());
    if prev == "find" && next != "find" {
        close_find(app, true);
    }
    if next == "find" && !find_is_open() {
        open_find(app);
    }
    send_overlay_state(app);
    apply_registrations(app);
}

/// main.js openFind: reads the window in front (or the chosen area).
pub fn open_find(app: &AppHandle) {
    if !cfg!(windows) || try_services().is_none() {
        return;
    }
    let px = active_region_px();
    let region = px.as_ref().and_then(engine::Rect::from_value);
    set_output_view(app, "find");
    let settings = store::get().get_find_settings();
    let scope = settings["scope"].as_str().unwrap_or("window").to_string();
    // Open now, read in the background: a switch away from Find before the
    // window has been read must find something to close.
    services().find.begin(Some(scope.as_str()), region);
    tauri::async_runtime::spawn(async move {
        services().find.read().await;
    });
    tauri::async_runtime::spawn(heal_find_model());
    apply_registrations(app);
}

/// Opening Find checks its OpenRouter model against the live catalogue, so
/// the stages do not start on a retired id (B-044). Native picks are left
/// alone.
async fn heal_find_model() {
    let sv = services();
    let native = llm_settings::active(&*sv.settings).is_some_and(|a| a.provider.is_native());
    if native {
        return;
    }
    let stored = sv.settings.model(Kind::Find);
    let live = sv.llm.resolve_live(Kind::Find, &stored).await;
    if live != stored {
        sv.settings.heal_model(Kind::Find, &live);
    }
}

/// main.js closeFind.
pub fn close_find(app: &AppHandle, keep_mode: bool) {
    if let Some(s) = try_services() {
        if s.find.is_open() {
            s.find.close();
        }
    }
    lens::clear(app);
    set_output_view(app, "chat");
    if let Some(l) = stored_label(app) {
        let empty = outputs::with(app, &l, |o| o.chat.messages.is_empty()) == Some(true);
        let agent = try_services().is_some_and(|s| s.agent.is_running());
        if empty && !agent {
            hide_output(app, &l);
        }
    }
    // The conversation Find stepped in front of is the target again, so the
    // next question continues it.
    let prev = out().prev.take();
    if let Some(p) = prev {
        if outputs::with(app, &p, |o| o.active).unwrap_or(false) {
            shared(app).lock().focused_output = Some(p);
        }
    }
    if !keep_mode {
        let st = shared(app);
        let mut s = st.lock();
        if s.bar_mode == "find" {
            s.bar_mode = "ask".to_string();
        }
    }
    send_overlay_state(app);
    apply_registrations(app);
}

/// main.js findToAsk: a hit becomes "explain this", looking at the hit
/// with some room around it.
pub async fn find_to_ask(app: &AppHandle, index: Option<i64>) -> Value {
    let Some(sv) = try_services() else {
        return json!({ "status": "no-hit" });
    };
    let hit = match index {
        Some(i) => usize::try_from(i)
            .ok()
            .and_then(|i| sv.find.state()["hits"].get(i).cloned()),
        None => sv.find.current_hit().map(|h| h.to_json()),
    };
    let Some(h) = hit else {
        return json!({ "status": "no-hit" });
    };
    let why = why_suffix(h.get("why"));
    let rect = h
        .get("rect")
        .filter(|r| r["width"].as_f64().is_some_and(|w| w != 0.0))
        .and_then(px_rect_to_dip);
    let matched = h["match"].as_str().unwrap_or_default().to_string();
    close_find(app, false);
    set_bar_mode(app, "ask");
    if let Some(r) = rect {
        region::set_active(Some(padded(r)));
        send_overlay_state(app);
    }
    let question = format!("Explain this from my screen: \"{matched}\"{why}");
    let opts = SubmitOpts {
        with_image: true,
        ..Default::default()
    };
    ask::submit(app, &question, opts).await
}

/// IPC rows bar:mode, find:open, find:query, find:related, find:step,
/// find:scope, find:to-ask, find:close; `None` for any other channel.
pub async fn route(app: &AppHandle, channel: &str, p: &Value) -> Option<Result<Value, String>> {
    let v = match channel {
        "bar:mode" => {
            set_bar_mode(app, p["mode"].as_str().unwrap_or_default());
            let mode = shared(app).lock().bar_mode.clone();
            json!({ "mode": mode })
        }
        "find:open" => match prefill_text(p.get("prefill")) {
            Some(text) => {
                windows::open_bar(app, Some("find"), Some(text));
                json!({ "ok": true })
            }
            None => {
                set_bar_mode(app, "find");
                services().find.state()
            }
        },
        "find:query" => {
            let q = p["query"].as_str().unwrap_or_default();
            let literal = p["literal"].as_bool().unwrap_or(false);
            services().find.query(q, literal)
        }
        "find:related" => json!({ "ok": services().find.related() }),
        "find:step" => services().find.step(&p["by"]).await,
        "find:scope" => {
            let find = &services().find;
            let scope = match p["scope"].as_str().filter(|s| !s.is_empty()) {
                Some(s) => s.to_string(),
                None => toggle_scope(find.state()["scope"].as_str()).to_string(),
            };
            store::get().set_find_settings(&json!({ "scope": scope }));
            json!({ "scope": find.set_scope(&scope) })
        }
        "find:to-ask" => find_to_ask(app, hit_index(p.get("index"))).await,
        "find:close" => {
            close_find(app, false);
            Value::Null
        }
        _ => return None,
    };
    Some(Ok(v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::client::{Meta, Reply, Transport};
    use crate::llm::errors::ApiError;
    use crate::llm::fake::Fake;
    use crate::llm::provider::Provider;
    use crate::llm::settings::MemorySettings;
    use crate::llm::wire::HttpRequest;
    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    struct NoNetwork;

    #[async_trait]
    impl Transport for NoNetwork {
        async fn send(&self, _: HttpRequest, _: &CancellationToken) -> Result<Reply, ApiError> {
            Err(ApiError::network("no network in tests"))
        }
    }

    #[tokio::test]
    async fn native_only_find_body_uses_native_default() {
        let fake = Arc::new(Fake::from_fixtures(Vec::new()));
        let mut llm = Llm::new(Arc::new(NoNetwork), Some(fake.clone()));
        llm.set_offline(false);
        let s = MemorySettings::with_key("sk-ant-api03-x");
        let model = find_model(&s, &llm);
        assert_eq!(model, "claude-haiku-5-5");
        let act = llm_settings::active(&s).unwrap();
        let meta = Meta::purpose("find-expand");
        let cancel = CancellationToken::new();
        let sent = llm
            .stream_text(
                &act,
                &model,
                "sys",
                "user",
                &meta,
                None,
                &cancel,
                &mut |_| {},
            )
            .await;
        assert!(sent.is_ok());
        let log = fake.requests();
        assert_eq!(log[0].provider, "anthropic");
        assert_eq!(log[0].model.as_deref(), Some("claude-haiku-5-5"));
        assert!(log[0].url.contains("anthropic.com"), "{}", log[0].url);

        // The person's own pick wins over the default.
        let pick = (Provider::Anthropic, Kind::Find);
        s.native.lock().insert(pick, "claude-opus-5-5".into());
        assert_eq!(find_model(&s, &llm), "claude-opus-5-5");

        // OpenRouter reads its own stored choice.
        let s = MemorySettings::with_key("sk-or-v1-x");
        let want = crate::llm::catalogue::default_for(Kind::Find);
        assert_eq!(find_model(&s, &llm), want);
    }

    #[test]
    fn modes_fall_back_to_ask() {
        assert_eq!(next_mode("find"), "find");
        assert_eq!(next_mode("agent"), "agent");
        assert_eq!(next_mode(""), "ask");
        assert_eq!(next_mode("chat"), "ask");
    }

    #[test]
    fn why_is_cut_and_optional() {
        assert_eq!(why_suffix(None), "");
        assert_eq!(why_suffix(Some(&json!(""))), "");
        let long = "a".repeat(200);
        let s = why_suffix(Some(&json!(long)));
        assert_eq!(s, format!(" (it matched because {})", "a".repeat(120)));
    }

    #[test]
    fn results_show_once_there_is_something_to_say() {
        assert!(!has_content(&json!({ "open": false, "status": "Reading" })));
        assert!(!has_content(
            &json!({ "open": true, "status": "", "lines": [] })
        ));
        assert!(has_content(&json!({ "open": true, "status": "Reading" })));
        assert!(has_content(&json!({ "open": true, "lines": ["x"] })));
    }

    #[test]
    fn ask_area_is_padded() {
        let r = padded(Rect::new(200.0, 300.0, 50.0, 20.0));
        assert_eq!(r, capture::Rect::new(80.0, 180.0, 290.0, 260.0));
    }

    #[test]
    fn payload_fields() {
        assert_eq!(prefill_text(Some(&json!("hi"))), Some("hi".to_string()));
        assert_eq!(prefill_text(Some(&json!(""))), None);
        assert_eq!(prefill_text(None), None);
        let long = "é".repeat(130);
        let cut = prefill_text(Some(&json!(long)));
        assert_eq!(cut.map(|s| s.chars().count()), Some(120));
        assert_eq!(hit_index(Some(&json!(2))), Some(2));
        assert_eq!(hit_index(Some(&json!(1.5))), Some(-1));
        assert_eq!(hit_index(Some(&json!("2"))), None);
        assert_eq!(hit_index(None), None);
        assert_eq!(toggle_scope(Some("screen")), "window");
        assert_eq!(toggle_scope(Some("window")), "screen");
        assert_eq!(toggle_scope(None), "screen");
    }
}
