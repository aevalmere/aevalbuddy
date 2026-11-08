//! The Ask flow (PARITY ASK-*): main.js handleQuerySubmit, abortCurrentQuery,
//! prefetchBarContext, freshBarText, readExactText, showModelPoints,
//! resolveUiaPoint, and the models:list / settings:test-key handlers, on the
//! floating output windows: a question goes to the output `outputs::target`
//! gives, and every event of that turn goes to that window only.

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;

use crate::ask::{
    is_visual, point_fraction, AUTO_SKILL, CHAT_HISTORY_LIMIT, DEFAULT_PROMPT, TEXT_ONLY_CHARS,
};
use crate::capture::{self, Shot};
use crate::llm::catalogue::Kind;
use crate::llm::client::{ChatTurn, QueryOpts, ReaderOpts};
use crate::llm::elements::{match_element, parse_elements, View};
use crate::llm::errors::{friendly_error_for, ApiError};
use crate::llm::failover::{short_model, with_failover};
use crate::llm::points::{Point, PointExtractor};
use crate::llm::prompt::{extract_task, ExactText};
use crate::llm::provider::Provider;
use crate::llm::settings::{active, model_for, Active, LlmSettings};
use crate::llm::sse::Meter;
use crate::state::{shared, Mode};
use crate::windows::{self, outputs};
use crate::{buddy, store};

use super::{services, BarContext};

/// `query:submit` options.
#[derive(Clone, Copy, Debug, Default)]
pub struct SubmitOpts {
    /// Re-run the turn already at the end of the transcript.
    pub retry: bool,
    /// The bar guessed Ask; the answer offers Find instead.
    pub inferred: bool,
    /// Always send the picture (Find's "Ask about this").
    pub with_image: bool,
}

struct Current {
    gen: u64,
    cancel: CancellationToken,
}

fn current() -> &'static Mutex<Option<Current>> {
    static C: OnceLock<Mutex<Option<Current>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

/// The shot taken when an area was chosen, used by the next question.
fn pending_shot() -> &'static Mutex<Option<Shot>> {
    static P: OnceLock<Mutex<Option<Shot>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(None))
}

pub fn set_pending_shot(shot: Option<Shot>) {
    *pending_shot().lock() = shot;
}

/// The window an Ask read its exact text from, for pointing by name.
fn last_exact() -> &'static Mutex<Option<ExactText>> {
    static L: OnceLock<Mutex<Option<ExactText>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(None))
}

/// main.js abortCurrentQuery.
pub fn abort(_app: &AppHandle) {
    if let Some(c) = current().lock().take() {
        c.cancel.cancel();
    }
}

fn no_uia() -> bool {
    std::env::var_os("AEVAL_NO_UIA").is_some()
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// The window in front's own text, through UI Automation. Never waits
/// long: the first use starts the desktop helper, and an answer is worth more
/// than a perfect context.
pub async fn read_exact_text(region: Option<crate::state::Rect>) -> Option<ExactText> {
    if no_uia() {
        return None;
    }
    let d = services().desktop.inner().clone();
    let limit = if d.is_ready() { 1500 } else { 4000 };
    let work = async move {
        let fg = d.foreground_window().await.ok()?;
        if !fg.is_object() {
            return None;
        }
        let rect = region
            .map(|r| {
                let p = windows::screen::dip_to_px(r);
                json!({ "x": p.x.round(), "y": p.y.round(), "width": p.width.round(), "height": p.height.round() })
            })
            .unwrap_or(Value::Null);
        let handle = fg["handle"].clone();
        let title = text_of(&fg["title"]);
        let g = d.get_text(Some(&handle), &rect, 16000, false).await.ok()?;
        let text = text_of(&g["text"]);
        let text = if text.trim().is_empty() {
            String::new()
        } else {
            text
        };
        Some(ExactText {
            handle: text_of(&handle),
            title,
            text,
        })
    };
    tokio::time::timeout(Duration::from_millis(limit), work)
        .await
        .ok()
        .flatten()
}

/// main.js prefetchBarContext: starts the desktop engine and the catalogue,
/// reads the foreground window's text and sends `bar:context`.
pub fn prefetch_bar_context(app: &AppHandle) {
    if no_uia() {
        return;
    }
    let sv = services();
    let d = sv.desktop.inner().clone();
    let llm = sv.llm.clone();
    let ctx = BarContext {
        at: Instant::now(),
        region: super::active_region(),
        read: Arc::new(tokio::sync::OnceCell::new()),
    };
    *super::bar_context().lock() = Some(ctx.clone());
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let d2 = d.clone();
        tauri::async_runtime::spawn(async move {
            let _ = d2.start().await;
        });
        if store::get().has_api_key() {
            tauri::async_runtime::spawn(async move {
                llm.fetch_catalogue().await;
            });
        }
        let fg = d.foreground_window().await.ok();
        let region = ctx.region;
        let exact = ctx
            .read
            .get_or_init(|| read_exact_text(region))
            .await
            .clone();
        let still = super::bar_context()
            .lock()
            .as_ref()
            .is_some_and(|c| Arc::ptr_eq(&c.read, &ctx.read));
        if !still {
            return;
        }
        let process = fg
            .as_ref()
            .map(|f| text_of(&f["process"]))
            .unwrap_or_default();
        let title = exact
            .as_ref()
            .map(|e| e.title.clone())
            .filter(|t| !t.is_empty())
            .or_else(|| fg.as_ref().map(|f| text_of(&f["title"])))
            .unwrap_or_default();
        let text = exact.as_ref().map(|e| e.text.clone()).unwrap_or_default();
        let screen: String = text.chars().take(16000).collect();
        windows::emit(
            &app,
            windows::OVERLAY,
            "bar:context",
            json!({ "screen": screen, "title": title, "process": process, "chars": text.encode_utf16().count() }),
        );
    });
}

/// The text read when the bar opened, while it is recent (8 s) and for the
/// same area.
async fn fresh_bar_text(region: Option<crate::state::Rect>) -> Option<Option<ExactText>> {
    let ctx = super::bar_context().lock().clone()?;
    if ctx.at.elapsed() > Duration::from_secs(8) || ctx.region != region {
        return None;
    }
    Some(
        ctx.read
            .get_or_init(|| read_exact_text(region))
            .await
            .clone(),
    )
}

fn norm_name(s: &str) -> String {
    let mapped: String = s
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' {
                c
            } else {
                ' '
            }
        })
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_role_words(label: &str) -> String {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"(?i)\b(button|link|field|tab|menu|checkbox)\b").expect("role words")
    });
    re.replace_all(label, "").into_owned()
}

/// `[[point Save]]` against the real control: only a clear winner counts.
/// Returns a DIP screen point and the control's name.
async fn resolve_uia_point(label: &str, handle: &str) -> Option<(f64, f64, String)> {
    if label.is_empty() || handle.is_empty() {
        return None;
    }
    let d = services().desktop.inner().clone();
    let h = json!(handle);
    let f = d.find(label, Some(&h), 4, 55).await.ok()?;
    let nodes = f["nodes"].as_array()?;
    let a = nodes.first()?;
    let b = nodes.get(1);
    let score = |n: &Value| n["score"].as_f64().unwrap_or(0.0);
    let rect = &a["rect"];
    if !rect.is_object() || score(a) < 70.0 {
        return None;
    }
    let name_a = norm_name(&text_of(&a["name"]));
    let exact = name_a == norm_name(&strip_role_words(label));
    let tie = b.is_some_and(|b| norm_name(&text_of(&b["name"])) == name_a);
    if !(exact && !tie) {
        if let Some(b) = b {
            if score(a) - score(b) < 8.0 {
                return None;
            }
        }
    }
    let n = |k: &str| rect[k].as_f64().unwrap_or(0.0);
    let (x, y) =
        windows::screen::px_to_dip_point(n("x") + n("width") / 2.0, n("y") + n("height") / 2.0);
    Some((x.round(), y.round(), text_of(&a["name"])))
}

/// main.js showModelPoints: directives to screen points, on the display the
/// shot came from.
async fn show_model_points(
    app: &AppHandle,
    points: &[Point],
    shot: Option<&Shot>,
    report: Option<&str>,
) {
    let Some(shot) = shot.filter(|s| s.width > 0 && s.height > 0) else {
        return;
    };
    if points.is_empty() || shared(app).lock().mode == Mode::Hud {
        return;
    }
    let b = shot.region.unwrap_or(shot.display);
    let view = View {
        width: shot.width as f64,
        height: shot.height as f64,
    };
    let elements = parse_elements(report, Some(view));
    let to_screen = |fx: f64, fy: f64, label: &str| buddy::Point {
        x: (b.x + fx * b.width).round(),
        y: (b.y + fy * b.height).round(),
        label: if label.is_empty() {
            "Here".into()
        } else {
            label.to_string()
        },
    };
    let exact_handle = last_exact().lock().as_ref().map(|e| e.handle.clone());
    let mut mapped = Vec::new();
    for p in points {
        match (p.x, p.y) {
            (Some(x), Some(y)) => {
                if let Some((fx, fy)) = point_fraction(x, y, view.width, view.height) {
                    mapped.push(to_screen(fx, fy, &p.label));
                }
            }
            _ => {
                if let Some(h) = &exact_handle {
                    if let Some((x, y, name)) = resolve_uia_point(&p.label, h).await {
                        let label = if p.label.is_empty() {
                            name
                        } else {
                            p.label.clone()
                        };
                        mapped.push(buddy::Point { x, y, label });
                        continue;
                    }
                }
                if let Some(el) = match_element(&p.label, &elements) {
                    let label = if p.label.is_empty() {
                        el.label.clone()
                    } else {
                        p.label.clone()
                    };
                    mapped.push(to_screen(el.x / 1000.0, el.y / 1000.0, &label));
                }
            }
        }
    }
    if mapped.is_empty() {
        return;
    }
    mapped.truncate(3);
    let display = capture::screen::matching(&capture::screen::all_displays(), &b);
    buddy::show_points(mapped, Some(&display));
    super::apply_registrations(app);
}

/// The provider, the vision model and the brain for this question.
async fn models_now() -> (Option<Active>, String, String) {
    let settings = &*services().settings;
    let act = active(settings);
    match act.as_ref().map(|a| a.provider) {
        Some(p) if p.is_native() => (
            act.clone(),
            model_for(settings, p, Kind::Vision),
            model_for(settings, p, Kind::Text),
        ),
        _ => {
            let (m, b) = super::live_models(true).await;
            (act, m, b)
        }
    }
}

/// One conversation turn (main.js handleQuerySubmit). `label` is the output
/// the question came from (a retry), or None for the bar's target.
pub async fn submit_to(
    app: &AppHandle,
    label: Option<String>,
    question: &str,
    o: SubmitOpts,
) -> Value {
    let q = question.trim().to_string();
    let help = {
        static RE: OnceLock<regex::Regex> = OnceLock::new();
        RE.get_or_init(|| regex::Regex::new(r"(?i)^help me[.!]?$").expect("help"))
    };
    if !o.retry && help.is_match(&q) {
        super::start_walkthrough(app);
        return json!({ "status": "walkthrough" });
    }
    if !store::get().has_api_key() {
        super::send_overlay_state(app);
        return json!({ "status": "needs-key" });
    }
    abort(app);
    let gen = {
        let st = shared(app);
        let mut s = st.lock();
        s.query_gen += 1;
        s.query_gen
    };
    let label = match label.filter(|l| outputs::with(app, l, |_| ()).is_some()) {
        Some(l) => l,
        None => match outputs::target(app) {
            Some(l) => l,
            None => return json!({ "status": "error" }),
        },
    };
    shared(app).lock().focused_output = Some(label.clone());
    let text = if q.is_empty() {
        DEFAULT_PROMPT.to_string()
    } else {
        q
    };
    let region = super::active_region();
    let (resume, prior) = outputs::with(app, &label, |out| {
        out.chat.last_question = text.clone();
        out.chat.region = region;
        let last = out.chat.messages.last();
        let resume = o.retry
            && last
                .is_some_and(|m| m["role"] == "user" && m["text"].as_str() == Some(text.as_str()));
        let upto = if resume {
            out.chat.messages.len() - 1
        } else {
            out.chat.messages.len()
        };
        let from = upto.saturating_sub(CHAT_HISTORY_LIMIT);
        let prior: Vec<ChatTurn> = out.chat.messages[from..upto]
            .iter()
            .map(|m| ChatTurn {
                role: text_of(&m["role"]),
                text: text_of(&m["text"]),
                kind: m.get("kind").and_then(Value::as_str).map(str::to_string),
            })
            .collect();
        if !resume {
            out.chat
                .messages
                .push(json!({ "role": "user", "text": text }));
        }
        (resume, prior)
    })
    .unwrap_or((false, Vec::new()));
    let use_brain = store::get().get_brain_enabled();
    let send = {
        let app = app.clone();
        let label = label.clone();
        move |channel: &str, payload: Value| {
            let mut p = payload;
            if let Value::Object(m) = &mut p {
                m.insert("gen".into(), json!(gen));
            }
            windows::emit(&app, &label, channel, p);
        }
    };
    if !resume {
        send("chat:user", json!({ "text": text }));
    }
    send("query:start", json!({}));
    shared(app).lock().busy = true;
    super::send_overlay_state(app);
    outputs::show(app, &label);

    let cancel = CancellationToken::new();
    *current().lock() = Some(Current {
        gen,
        cancel: cancel.clone(),
    });
    let early = fresh_bar_text(region);
    let exact_f = async {
        match early.await {
            Some(Some(x)) => Some(x),
            _ => read_exact_text(region).await,
        }
    };
    let cap_region = region.map(|r| capture::Rect::new(r.x, r.y, r.width, r.height));
    let pending = if region.is_some() {
        pending_shot().lock().take()
    } else {
        None
    };
    let shot_f = async move {
        match pending {
            Some(s) => Some(s),
            None => capture::capture_screen_async(cap_region).await,
        }
    };
    let (shot_taken, (act, model, brain), exact) = tokio::join!(shot_f, models_now(), exact_f);
    *last_exact().lock() = exact.clone();
    shared(app).lock().capture_ok = shot_taken.is_some();
    super::send_overlay_state(app);

    let exact_len = exact
        .as_ref()
        .map(|e| e.text.encode_utf16().count())
        .unwrap_or(0);
    let text_only = shot_taken.is_some()
        && !use_brain
        && !o.with_image
        && exact_len >= TEXT_ONLY_CHARS
        && !is_visual(&text);
    let shot = if text_only { None } else { shot_taken };
    let via_region = shot.as_ref().is_some_and(|s| s.region.is_some());
    let provider = act
        .as_ref()
        .map(|a| a.provider)
        .unwrap_or(Provider::OpenRouter);
    let act = act.unwrap_or(Active {
        provider: Provider::OpenRouter,
        key: String::new(),
    });

    let ex = Arc::new(Mutex::new(PointExtractor::new()));
    let meter = Arc::new(Mutex::new(Meter::default()));
    let t0 = meter.lock().t0;
    let llm = services().llm.clone();
    let mut answering = model.clone();

    let result: Result<Option<String>, ApiError> = async {
        let mut report: Option<String> = None;
        if use_brain {
            if let Some(s) = &shot {
                send("query:stage", json!({ "label": "Reading your screen" }));
                let read = with_failover(
                    &model,
                    |tried| llm.next_free_model(Kind::Vision, tried),
                    || true,
                    |m| send("query:stage", json!({ "label": format!("Busy \u{2014} reading with {}", short_model(m)) })),
                    |m| {
                        let llm = llm.clone();
                        let act = act.clone();
                        let img = s.base64.clone();
                        let q = text.clone();
                        let exact = exact.clone();
                        let cancel = cancel.clone();
                        async move {
                            let opts = ReaderOpts {
                                model: &m,
                                image_base64: &img,
                                question: &q,
                                region: via_region,
                                exact_text: exact.as_ref(),
                            };
                            llm.read_screen(&act, opts, &cancel).await
                        }
                    },
                )
                .await;
                match read {
                    Ok((_, r)) => report = Some(r),
                    Err(e) if e.is_abort() => return Err(e),
                    Err(_) => report = None,
                }
            }
        }

        let skills = &services().skills;
        let pinned = store::get().get_skill_id(super::skill_exists);
        let chosen = if pinned == AUTO_SKILL {
            let task = extract_task(report.as_deref());
            skills.route(&text, Some(task.as_str()).filter(|t| !t.is_empty())).0
        } else {
            pinned
        };
        let skill = skills.get(&chosen).map(|s| s.text());

        // One streamed attempt on `m`, with or without the picture or the
        // reading.
        let attempt = |m: String, image: Option<String>, rep: Option<String>| {
            let llm = llm.clone();
            let act = act.clone();
            let q = text.clone();
            let exact = exact.clone();
            let skill = skill.clone();
            let prior = prior.clone();
            let cancel = cancel.clone();
            let ex = ex.clone();
            let meter = meter.clone();
            let send = send.clone();
            let (w, h) = shot.as_ref().map(|s| (s.width, s.height)).unwrap_or((0, 0));
            async move {
                let mut on_delta = move |t: &str| {
                    let pieces = ex.lock().push(t);
                    for p in pieces {
                        send("query:delta", json!({ "text": p }));
                    }
                };
                let opts = QueryOpts {
                    model: &m,
                    question: &q,
                    image_base64: image.as_deref(),
                    image_width: w,
                    image_height: h,
                    history: &prior,
                    skill: skill.as_ref(),
                    screen_report: rep.as_deref(),
                    via_region,
                    exact_text: exact.as_ref(),
                };
                let mut mtr = Meter::default();
                let r = llm.stream_query(&act, opts, &cancel, &mut on_delta, &mut mtr).await;
                let mut total = meter.lock();
                if total.ttft_ms == 0 && mtr.ttft_ms > 0 {
                    total.ttft_ms = (mtr.t0 - total.t0).as_millis() as u64 + mtr.ttft_ms;
                }
                total.prompt += mtr.prompt;
                total.completion += mtr.completion;
                if let Some(c) = mtr.cost {
                    total.cost = Some(total.cost.unwrap_or(0.0) + c);
                }
                r
            }
        };
        let nothing_shown = || ex.lock().text.is_empty();

        if let Some(rep) = report.clone() {
            answering = brain.clone();
            send("query:stage", json!({ "label": "Thinking" }));
            let spoke = with_failover(
                &brain,
                |tried| llm.next_free_model(Kind::Text, tried),
                nothing_shown,
                |m| send("query:stage", json!({ "label": format!("Busy \u{2014} thinking with {}", short_model(m)) })),
                |m| attempt(m, None, Some(rep.clone())),
            )
            .await;
            match spoke {
                Ok((m, _)) => answering = m,
                Err(e) => {
                    if e.is_abort() || !nothing_shown() {
                        return Err(e);
                    }
                    answering = model.clone();
                    send("query:stage", json!({ "label": "Answering directly" }));
                    let img = shot.as_ref().map(|s| s.base64.clone());
                    attempt(model.clone(), img, None).await?;
                }
            }
        } else {
            answering = model.clone();
            if use_brain && shot.is_some() {
                send("query:stage", json!({ "label": "Answering directly" }));
            }
            let img = shot.as_ref().map(|s| s.base64.clone());
            let (m, _) = with_failover(
                &model,
                |tried| llm.next_free_model(Kind::Vision, tried),
                nothing_shown,
                |m| send("query:stage", json!({ "label": format!("Busy \u{2014} asking {}", short_model(m)) })),
                |m| attempt(m, img.clone(), None),
            )
            .await?;
            answering = m;
        }
        Ok(report)
    }
    .await;

    for p in ex.lock().flush() {
        send("query:delta", json!({ "text": p }));
    }
    let (answer, points) = {
        let e = ex.lock();
        (e.text.clone(), e.points.clone())
    };
    let status = match result {
        Ok(report) => {
            outputs::with(app, &label, |out| {
                out.chat
                    .messages
                    .push(json!({ "role": "assistant", "text": answer }));
            });
            let m = meter.lock().clone();
            send(
                "query:done",
                json!({
                    "usage": {
                        "model": short_model(&answering),
                        "ttftMs": m.ttft_ms,
                        "ms": t0.elapsed().as_millis() as u64,
                        "prompt": m.prompt,
                        "completion": m.completion,
                        "cost": m.cost,
                        "textOnly": text_only,
                    },
                    "inferred": o.inferred,
                    "question": text,
                }),
            );
            super::remember_chat(app, &label, &answering);
            shared(app).lock().pill_error = false;
            buddy::notify("first-answer");
            show_model_points(app, &points, shot.as_ref(), report.as_deref()).await;
            "done"
        }
        Err(err) => {
            if !answer.is_empty() {
                outputs::with(app, &label, |out| {
                    out.chat
                        .messages
                        .push(json!({ "role": "assistant", "text": answer }));
                });
                super::remember_chat(app, &label, &answering);
            }
            let named = if err.stage == Some("reader") {
                &model
            } else {
                &answering
            };
            match friendly_error_for(provider, Some(&err), named) {
                Some(message) => send(
                    "query:error",
                    json!({ "message": message, "question": text }),
                ),
                None => send("query:done", json!({})),
            }
            "error"
        }
    };
    {
        let mut c = current().lock();
        if c.as_ref().is_some_and(|c| c.gen == gen) {
            *c = None;
        }
    }
    let newest = shared(app).lock().query_gen == gen;
    if newest {
        shared(app).lock().busy = false;
        super::send_overlay_state(app);
        crate::updater::idle(app);
    }
    json!({ "status": status })
}

/// The bar's question, to its target output.
pub async fn submit(app: &AppHandle, question: &str, o: SubmitOpts) -> Value {
    submit_to(app, None, question, o).await
}

/// `models:list`: the picker for a job, with the model in use now.
async fn models_list(p: &Value) -> Value {
    let sv = services();
    let k = p.get("kind").and_then(Value::as_str);
    let kind = Kind::from_list_kind(k);
    let settings = &*sv.settings;
    let provider = p
        .get("provider")
        .and_then(Value::as_str)
        .and_then(Provider::from_id)
        .or_else(|| active(settings).map(|a| a.provider))
        .unwrap_or(Provider::OpenRouter);
    let mut result = sv
        .llm
        .list_models(provider, kind, settings.show_paid())
        .await;
    let current = if provider.is_native() {
        model_for(settings, provider, kind)
    } else {
        let s = store::get();
        match kind {
            Kind::Vision => super::live_models(true).await.0,
            Kind::Text => super::live_models(true).await.1,
            Kind::Find => sv.llm.resolve_live(Kind::Find, &s.get_find_model()).await,
            Kind::Agent => sv.llm.resolve_live(Kind::Agent, &s.get_agent_model()).await,
        }
    };
    if let Value::Object(m) = &mut result {
        m.insert("kind".into(), json!(kind.as_str()));
        m.insert("current".into(), json!(current));
    }
    result
}

/// `settings:test-key`: the named provider's key, else the one in use.
async fn test_key(p: &Value) -> Value {
    let sv = services();
    let named = p
        .get("provider")
        .and_then(Value::as_str)
        .and_then(Provider::from_id);
    let provider = named
        .or_else(|| active(&*sv.settings).map(|a| a.provider))
        .unwrap_or(Provider::OpenRouter);
    let key = store::get().get_provider_key(provider.id());
    sv.llm.test_key(provider, &key).await.to_json()
}

/// IPC rows query:submit, query:cancel, query:retry, models:list,
/// settings:test-key; None for any other channel.
pub async fn route(
    app: &AppHandle,
    label: &str,
    channel: &str,
    p: &Value,
) -> Option<Result<Value, String>> {
    let out = match channel {
        "query:submit" => {
            let q = text_of(&p["question"]);
            let o = SubmitOpts {
                inferred: crate::store::truthy(p.get("inferred")),
                ..SubmitOpts::default()
            };
            submit(app, &q, o).await
        }
        "query:cancel" => {
            abort(app);
            Value::Null
        }
        "query:retry" => {
            let from = if outputs::is_output(label) {
                Some(label.to_string())
            } else {
                outputs::focused(app)
            };
            let last = from
                .as_ref()
                .and_then(|l| outputs::with(app, l, |o| o.chat.last_question.clone()))
                .unwrap_or_default();
            if last.is_empty() {
                json!({ "status": "nothing-to-retry" })
            } else {
                let o = SubmitOpts {
                    retry: true,
                    ..SubmitOpts::default()
                };
                submit_to(app, from, &last, o).await
            }
        }
        "models:list" => models_list(p).await,
        "settings:test-key" => test_key(p).await,
        _ => return None,
    };
    Some(Ok(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_compare_without_punctuation_or_role_words() {
        assert_eq!(norm_name("  Save   As… "), "save as");
        assert_eq!(norm_name(&strip_role_words("Save button")), "save");
    }
}
