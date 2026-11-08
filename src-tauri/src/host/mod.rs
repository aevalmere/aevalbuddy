//! The glue between the four splits (ARCHITECTURE.md sections 10 to 12):
//! the long-lived services split (b) built (the model client, skills, Find,
//! the agent) wired onto split (a)'s store, windows and hooks and split (c)'s
//! desktop helper, buddy, lens and region.
//!
//! Nothing here owns behaviour of its own: each function is the Rust form of
//! one piece of main.js that sat between modules (handleQuerySubmit,
//! startAgentRun, findEmit, contextMenu, the `init({...})` blocks).

pub mod agent;
pub mod ask;
pub mod desktop_ops;
pub mod find;
pub mod menu;
pub mod settings;

use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::agent::run::Agent;
use crate::find::Find;
use crate::llm::catalogue::Kind;
use crate::llm::client::Llm;
use crate::llm::settings::LlmSettings;
use crate::skills::Skills;
use crate::state::{shared, Rect};
use crate::windows::{self, outputs};
use crate::{buddy, hotkeys, region, store};

use self::desktop_ops::DesktopAdapter;
use self::settings::StoreSettings;

/// The services main.js built at `app.whenReady`.
pub struct Services {
    pub llm: Arc<Llm>,
    pub skills: Skills,
    pub settings: Arc<StoreSettings>,
    pub desktop: Arc<DesktopAdapter>,
    pub find: Find,
    pub agent: Agent,
}

static SERVICES: OnceLock<Services> = OnceLock::new();

/// The services, once `setup` has run (it runs before any window loads).
pub fn services() -> &'static Services {
    SERVICES
        .get()
        .expect("host::setup runs before the first IPC call")
}

/// `services()` when setup has run; `None` in unit tests and before boot.
pub fn try_services() -> Option<&'static Services> {
    SERVICES.get()
}

/// `online()` in main.js: AEVAL_OFFLINE forces offline; otherwise the OS.
pub fn online() -> bool {
    if std::env::var_os("AEVAL_OFFLINE").is_some_and(|v| !v.is_empty()) {
        return false;
    }
    crate::updater::online()
}

/// The area the bar's conversation is about (main.js `activeRegion`), in
/// DIPs. Split (c)'s region module holds it.
pub fn active_region() -> Option<Rect> {
    region::active_region().map(|r| Rect::new(r.x, r.y, r.width, r.height))
}

/// The same area in physical pixels (`screen.dipToScreenRect`), as a JSON
/// rect for the desktop helper and the agent.
pub fn active_region_px() -> Option<Value> {
    let r = active_region()?;
    let p = windows::screen::dip_to_px(r);
    Some(
        json!({ "x": p.x.round(), "y": p.y.round(), "width": p.width.round(), "height": p.height.round() }),
    )
}

/// Physical rect JSON -> DIP rect (`screen.screenToDipRect`).
pub fn px_rect_to_dip(v: &Value) -> Option<Rect> {
    let n = |k: &str| v.get(k).and_then(Value::as_f64).filter(|f| f.is_finite());
    let (x, y, w, h) = (n("x")?, n("y")?, n("width")?, n("height")?);
    Some(windows::screen::px_to_dip(Rect::new(x, y, w, h)))
}


/// Saves one conversation to history (main.js rememberChat).
pub fn remember_chat(app: &AppHandle, label: &str, model: &str) {
    let Some((id, messages)) =
        outputs::with(app, label, |o| (o.chat.id.clone(), o.chat.messages.clone()))
    else {
        return;
    };
    crate::history::get().save(&id, model, &messages);
    windows::notify_history_changed(app);
}

/// Copies the state other modules own into `AppState` so the core reads the
/// truth (buddy, region, Find, agent).
pub fn refresh_flags(app: &AppHandle) {
    let buddy = buddy::is_active();
    let region = region::is_active(app);
    let (find, agent, step, state) = match try_services() {
        Some(s) => {
            let cur = s.agent.current();
            (
                s.find.is_open(),
                s.agent.is_running(),
                cur.as_ref().map(|c| c.0 as u64).unwrap_or(0),
                cur.map(|c| c.1).unwrap_or_default(),
            )
        }
        None => (false, false, 0, String::new()),
    };
    let st = shared(app);
    let mut s = st.lock();
    s.buddy_active = buddy;
    s.region_active = region;
    s.find_open = find;
    s.agent.running = agent;
    if agent {
        s.agent.step = step;
        s.agent.state = state;
    }
}

/// `applyRegistrations` (the core refreshes the flags first).
pub fn apply_registrations(app: &AppHandle) {
    hotkeys::apply(app);
}

/// `sendOverlayState` with the flags refreshed first.
pub fn send_overlay_state(app: &AppHandle) {
    refresh_flags(app);
    windows::send_overlay_state(app);
}

/// main.js startWalkthrough.
pub fn start_walkthrough(app: &AppHandle) {
    if shared(app).lock().mode == crate::state::Mode::Hud {
        return;
    }
    let s = store::get();
    let (cq, fs, at) = (
        s.shortcut("captureQuery"),
        s.shortcut("findOnScreen"),
        s.shortcut("agentTask"),
    );
    let chords = buddy::Chords {
        capture_query: &cq,
        find_on_screen: &fs,
        agent_task: &at,
    };
    buddy::start(buddy::walkthrough(&chords));
    apply_registrations(app);
}

/// The vision model and brain this install should use right now, healing a
/// retired stored choice (main.js liveModels). `wait` false answers from the
/// cached catalogue and never blocks.
pub async fn live_models(wait: bool) -> (String, String) {
    let sv = services();
    let llm = &sv.llm;
    let s = store::get();
    let stored = (s.get_model(), s.get_brain());
    let (model, brain) = if wait {
        (
            llm.resolve_live(Kind::Vision, &stored.0).await,
            llm.resolve_live(Kind::Text, &stored.1).await,
        )
    } else {
        (
            llm.resolve_known(Kind::Vision, &stored.0),
            llm.resolve_known(Kind::Text, &stored.1),
        )
    };
    if model != stored.0 {
        eprintln!(
            "[aevalbuddy] {} left the catalogue; using {model}",
            stored.0
        );
        sv.settings.heal_model(Kind::Vision, &model);
    }
    if brain != stored.1 {
        eprintln!(
            "[aevalbuddy] {} left the catalogue; using {brain}",
            stored.1
        );
        sv.settings.heal_model(Kind::Text, &brain);
    }
    (model, brain)
}

/// The skill id the store accepts: a loaded skill, or `auto`.
pub fn skill_exists(id: &str) -> bool {
    id == crate::ask::AUTO_SKILL || try_services().is_some_and(|s| s.skills.has(id))
}

/// `settings:get` fields only this side knows: the healed model ids, the
/// skills, the glass tier (split c's backdrop).
fn settings_extra(_app: &AppHandle) -> Value {
    let s = store::get();
    let Some(sv) = try_services() else {
        return json!({});
    };
    let llm = &sv.llm;
    let model = llm.resolve_known(Kind::Vision, &s.get_model());
    let brain = llm.resolve_known(Kind::Text, &s.get_brain());
    json!({
        "model": model,
        "brain": brain,
        "findModel": llm.resolve_known(Kind::Find, &s.get_find_model()),
        "agentModel": llm.resolve_known(Kind::Agent, &s.get_agent_model()),
        "skills": sv.skills.list(),
        "skill": s.get_skill_id(skill_exists),
        "glass": { "tier": crate::backdrop::tier(), "reasons": crate::backdrop::reasons() },
    })
}

/// main.js settings:save-key's tour logic: the tour's first step moves on
/// once the key is saved and tested. A key the server rejects keeps the step
/// open and says so; a key that could not be tested offline is saved anyway.
fn key_saved(app: &AppHandle, provider: &str) {
    let touring = buddy::step_index() == 0;
    if !touring {
        buddy::notify("key-saved");
        return;
    }
    let Some(p) = crate::llm::provider::Provider::from_id(provider) else {
        buddy::notify("key-saved");
        return;
    };
    let key = store::get().get_provider_key(p.id());
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let t = services().llm.test_key(p, &key).await;
        if buddy::step_index() != 0 {
            return;
        }
        if t.ok {
            buddy::notify("key-saved");
        } else if t.message.to_lowercase().contains("reach") {
            buddy::notify("key-saved");
            let s = store::get();
            let (cq, fs, at) = (
                s.shortcut("captureQuery"),
                s.shortcut("findOnScreen"),
                s.shortcut("agentTask"),
            );
            let chords = buddy::Chords {
                capture_query: &cq,
                find_on_screen: &fs,
                agent_task: &at,
            };
            let next = buddy::walkthrough(&chords)
                .get(1)
                .map(|st| st.message.clone())
                .unwrap_or_default();
            buddy::set_message(&format!("Saved; it could not be tested offline. {next}"));
        } else {
            buddy::set_message(&t.message);
        }
        let _ = app;
    });
}

/// What the bar reads while the person types (main.js prefetchBarContext):
/// the foreground window's text, so the input can tell a word on screen
/// from a question, and so an Ask can answer from it without waiting.
#[derive(Clone)]
pub struct BarContext {
    pub at: std::time::Instant,
    pub region: Option<Rect>,
    pub read: Arc<tokio::sync::OnceCell<Option<crate::llm::prompt::ExactText>>>,
}

pub fn bar_context() -> &'static Mutex<Option<BarContext>> {
    static C: OnceLock<Mutex<Option<BarContext>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

/// Builds every service and registers every hook. Called once from lib.rs
/// setup, after the windows exist and before the first page can invoke.
pub fn setup(app: &AppHandle) {
    let llm = Arc::new(Llm::from_env());
    let skills_dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.join("skills")));
    let skills = Skills::load(skills_dir.as_deref());
    let settings = Arc::new(StoreSettings);
    let desktop = Arc::new(DesktopAdapter::new(crate::desktop::client().clone()));
    let rt = tokio::runtime::Handle::try_current()
        .unwrap_or_else(|_| tauri::async_runtime::handle().inner().clone());
    let find_host = Arc::new(find::Host::new(app.clone(), llm.clone()));
    let find = Find::new(desktop.clone(), llm.clone(), find_host, rt.clone());
    let agent_host = Arc::new(agent::Host::new(app.clone(), llm.clone()));
    let agent = Agent::new(desktop.clone(), llm.clone(), agent_host, rt);
    let _ = SERVICES.set(Services {
        llm,
        skills,
        settings,
        desktop,
        find,
        agent,
    });

    desktop_ops::init_desktop_deps(app);
    menu::init(app);

    let a = app.clone();
    buddy::init(
        app.clone(),
        buddy::Deps {
            ensure_step_prereq: Arc::new(move |step: &buddy::Step| {
                if step.needs == "settings-key" {
                    windows::open_settings(&a, Some("sec-key"));
                } else if step.win == "overlay" && shared(&a).lock().mode == crate::state::Mode::Hud
                {
                    windows::exit_hud(&a);
                }
            }),
            on_done: {
                let a = app.clone();
                Arc::new(move || apply_registrations(&a))
            },
            on_cancel: {
                let a = app.clone();
                Arc::new(move || apply_registrations(&a))
            },
            on_try: {
                let a = app.clone();
                Arc::new(move || {
                    let a2 = a.clone();
                    let _ = a.run_on_main_thread(move || {
                        windows::open_bar(&a2, Some("agent"), Some(String::new()))
                    });
                })
            },
        },
    );

    crate::backdrop::set_hooks(crate::backdrop::Hooks {
        pace: {
            let a = app.clone();
            Arc::new(move || backdrop_pace(&a))
        },
        user_surface: Arc::new(|| store::get().get_surface().to_string()),
    });
    crate::backdrop::set_intensity(store::get().get_glass_intensity());

    {
        let a = app.clone();
        let last: Mutex<Option<Rect>> = Mutex::new(active_region());
        region::on_change(move || on_region_change(&a, &last));
    }

    shared(app).set_hooks(|h| {
        h.stop_agent = Some(Box::new(|_| {
            services().agent.stop(None);
        }));
        h.close_find = Some(Box::new(|app| find::close_find(app, false)));
        h.refresh_flags = Some(Box::new(refresh_flags));
        h.cancel_region = Some(Box::new(region::cancel));
        h.cancel_tour = Some(Box::new(|_| buddy::cancel()));
        h.start_region = Some(Box::new(|app| {
            let a = app.clone();
            tauri::async_runtime::spawn(async move {
                apply_registrations(&a);
                region::pick(&a).await;
                apply_registrations(&a);
            });
        }));
        h.start_tour = Some(Box::new(start_walkthrough));
        h.abort_query = Some(Box::new(ask::abort));
        h.prefetch_bar_context = Some(Box::new(ask::prefetch_bar_context));
        h.warm_catalogue = Some(Box::new(|_| {
            tauri::async_runtime::spawn(async {
                live_models(true).await;
            });
        }));
        h.show_trail = Some(Box::new(|app| agent::show_trail(app, false)));
        h.tour_notify = Some(Box::new(|_, ev| buddy::notify(ev)));
        h.set_bar_mode = Some(Box::new(find::set_bar_mode));
        h.key_saved = Some(Box::new(key_saved));
        h.settings_extra = Some(Box::new(settings_extra));
    });
}

/// 2 Hz while anything you read or type into is up; the orb alone refreshes
/// every few seconds (main.js backdrop pace).
fn backdrop_pace(app: &AppHandle) -> crate::backdrop::Pace {
    use crate::backdrop::Pace;
    let busy = {
        let st = shared(app);
        let s = st.lock();
        s.mode != crate::state::Mode::Pill || s.outputs.values().any(|o| o.active)
    };
    let up = |l: &str| {
        windows::window(app, l)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
    };
    if busy
        || up(windows::SETTINGS)
        || up(windows::HISTORY)
        || buddy::is_active()
        || !crate::lens::windows(app).is_empty()
    {
        Pace::Fast
    } else {
        Pace::Slow
    }
}

/// After a pick or a clear (main.js triggerRegionCapture / clearRegion): a
/// new area opens the bar with its conversation; a cancel leaves the panel
/// as it was.
fn on_region_change(app: &AppHandle, last: &Mutex<Option<Rect>>) {
    let now = active_region();
    let picked = {
        let mut l = last.lock();
        let changed = *l != now;
        *l = now;
        changed && now.is_some()
    };
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        if picked {
            // A drawn area arrives with its shot; one set by Find's "Ask
            // about this" has none and is captured when the question goes.
            if let Some(shot) = region::take_pending_shot() {
                shared(&a).lock().capture_ok = true;
                ask::set_pending_shot(Some(shot));
            }
            shared(&a).lock().hidden = false;
            windows::expand_panel(&a);
            outputs::show_with_chat(&a);
        }
        send_overlay_state(&a);
        apply_registrations(&a);
    });
}
