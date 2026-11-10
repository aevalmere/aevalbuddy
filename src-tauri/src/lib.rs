//! AevalBuddy's Rust core. ARCHITECTURE.md is the map: each module below owns
//! one responsibility of the Electron main process, and PARITY.md lists what
//! each must keep doing.

// serde_json::json! recurses once per field; the store defaults,
// settings:get and overlay:state are bigger than the default limit allows.
#![recursion_limit = "256"]

mod agent;
// agents lane: watching other coding agents (Claude Code, Codex, ...).
mod agents;
mod ask;
pub mod autostart;
mod backdrop;
mod buddy;
mod capture;
pub mod crash;
mod desktop;
mod find;
pub mod history;
mod host;
pub mod hotkeys;
pub mod instance;
pub mod ipc;
mod lens;
mod llm;
pub mod notify;
pub mod paths;
mod region;
mod selftest;
mod skills;
pub mod state;
pub mod store;
pub mod tray;
pub mod updater;
// voice lane
mod voice;
pub mod windows;

use tauri::webview::PageLoadEvent;
use tauri::{Manager, RunEvent};

fn quiet_run() -> bool {
    // The CI checks too: a first-run tour would put the buddy's full-screen
    // window over the points the desktop selfcheck probes.
    [
        "AEVAL_SMOKE",
        "AEVAL_SHOT",
        "AEVAL_CAPTURE_CHECK",
        "AEVAL_DESKTOP_CHECK",
    ]
    .iter()
    .any(|k| std::env::var_os(k).is_some())
}

/// The self-test's `Cx::eval` answers through an event, which pages may not
/// send outside a test run (B-078).
const TEST_CAPABILITY: &str =
    r#"{"identifier":"selftest","windows":["*"],"permissions":["core:event:allow-emit"]}"#;

/// Whether a webview may go to `url`: only the app's own pages (B-078).
fn stays_in_app(url: &url::Url) -> bool {
    match url.scheme() {
        "http" | "https" => url.host_str() == Some("tauri.localhost"),
        "tauri" => url.host_str() == Some("localhost"),
        _ => url.as_str() == "about:blank",
    }
}

/// Refuses every navigation away from the app, in every webview (B-078).
fn navigation_guard() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("aeval-navigation")
        .on_navigation(|_webview, url| stays_in_app(url))
        .build()
}

/// main.js presenceToast (LIFE-12): once per fresh install, or whenever the
/// main chord is taken by another app.
fn presence_toast(app: &tauri::AppHandle) {
    if std::env::var_os("AEVAL_SMOKE").is_some() {
        return;
    }
    let s = store::get();
    let chord = s.shortcut("captureQuery");
    let failed = hotkeys::main_chord_failed(app);
    let fresh = s.take_presence_notice();
    if !failed && !fresh {
        return;
    }
    let (title, body) = if failed {
        (
            format!("{chord} is taken by another app"),
            "Open AevalBuddy from the tray icon by the clock, then pick another shortcut in Settings."
                .to_string(),
        )
    } else {
        (
            "AevalBuddy is running".to_string(),
            format!("Press {chord} anywhere to ask, find, or do. It starts with Windows; turn that off in Settings."),
        )
    };
    let a = app.clone();
    notify::show(
        &title,
        &body,
        Some(Box::new(move || {
            let a2 = a.clone();
            let _ = a.run_on_main_thread(move || {
                crash::guard("presence-toast", || {
                    if failed {
                        windows::open_settings(&a2, Some("sec-shortcuts"));
                    } else {
                        windows::open_bar(&a2, None, None);
                    }
                });
            });
        })),
    );
}

/// LIFE-03: a shots run's forced scale reaches WebView2 as well as the
/// window maths. Runs before any webview exists; keeps any arguments
/// already set.
fn force_shot_scale() {
    const ARGS: &str = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";
    let scale = std::env::var("AEVAL_SHOT_SCALE")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|s| *s > 0.0);
    let Some(scale) = scale else {
        return;
    };
    let flag = format!("--force-device-scale-factor={scale}");
    let args = match std::env::var(ARGS) {
        Ok(old) if !old.trim().is_empty() => format!("{} {flag}", old.trim()),
        _ => flag,
    };
    std::env::set_var(ARGS, args);
}

/// LIFE-10: a task still running at quit is told why it stopped.
const QUIT_REASON: &str = "Stopped because AevalBuddy quit.";

/// Stops a running task with the quit reason. False when none was running.
pub(crate) fn stop_for_exit() -> bool {
    host::try_services().is_some_and(|s| s.agent.stop(Some(QUIT_REASON)))
}

/// LIFE-09, LIFE-10, LIFE-11: what before-quit and will-quit did, on the
/// way out.
fn on_exit(app: &tauri::AppHandle) {
    stop_for_exit();
    state::shared(app).call(app, |k| k.close_find.as_ref());
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let _ = app.global_shortcut().unregister_all();
    history::get().flush_now();
    desktop::shutdown();
    updater::destroy();
    updater::install_on_quit();
    tray::destroy(app);
}

pub fn run() {
    // agents lane: a coding agent's hook call forwards its event to the
    // running app and exits, before anything else starts.
    if agents::hook::run_if_asked() {
        return;
    }
    force_shot_scale();
    windows::mark_start();
    notify::set_app_user_model_id();
    let user_data = paths::user_data_dir();
    let _ = std::fs::create_dir_all(&user_data);
    crash::install(&user_data);
    // LIFE-01: a second launch opens the first copy's bar and exits.
    let Some(lock) = instance::acquire(&user_data) else {
        return;
    };
    // Before the app is built, so a launch during a slow boot is heard; its
    // `open` is answered when setup is done.
    let doorbell = instance::serve(lock.name.clone());
    store::init(&user_data);
    history::init(&user_data).set_enabled(store::get().get_save_history());

    let shortcuts = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            crash::guard("hotkey", || hotkeys::on_shortcut(app, shortcut, event));
        })
        .build();

    let app = tauri::Builder::default()
        .plugin(navigation_guard())
        .plugin(shortcuts)
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .manage(ipc::Seen::default())
        .manage(state::Shared::default())
        .invoke_handler(tauri::generate_handler![ipc::aeval_invoke])
        // Glass frames and the lens loupe, served from memory (agent c, D4).
        .register_uri_scheme_protocol(backdrop::SCHEME, |_ctx, request| {
            backdrop::serve_request(&request)
        })
        .on_page_load(|webview, payload| {
            if !matches!(payload.event(), PageLoadEvent::Finished) {
                return;
            }
            crash::guard("page-load", || {
                if selftest::smoke_enabled() {
                    let _ = webview.eval(selftest::PING);
                }
                windows::on_page_loaded(webview.app_handle(), webview.label());
            });
        })
        .on_window_event(|window, event| {
            crash::guard("window-event", || windows::on_window_event(window, event));
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            if quiet_run() {
                handle.add_capability(TEST_CAPABILITY)?;
            }
            windows::boot(&handle)?;
            // Desktop helper client, glass backdrop, CI checks (split c).
            desktop::setup(&handle);
            // Ask, Find, the agent and the hooks between the splits.
            host::setup(&handle);
            // voice lane: the dictation chord and its pill.
            voice::setup(&handle);
            // agents lane: the hook pipe, keep-awake and the card.
            agents::setup(&handle);
            store::get().decide_presence();
            // Not apply: boot leaves Task Manager's StartupApproved alone.
            autostart::sync();
            // Before hotkeys::apply, which refreshes the tray labels.
            if let Err(e) = tray::init(&handle) {
                crash::write("tray", &e.to_string());
            }
            hotkeys::apply(&handle);
            hotkeys::report_failures(&handle);
            updater::init(&handle, &paths::user_data_dir());
            presence_toast(&handle);
            let s = store::get();
            if !s.has_api_key() && !s.get_walkthrough_auto_shown() && !quiet_run() {
                s.set_walkthrough_auto_shown();
                let h = handle.clone();
                let tour = async move {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    if state::shared(&h).lock().mode != state::Mode::Hud {
                        state::shared(&h).call(&h, |k| k.start_tour.as_ref());
                        hotkeys::apply(&h);
                    }
                };
                tauri::async_runtime::spawn(crash::guard_async("first-tour", tour));
            }
            if std::env::var_os("AEVAL_SMOKE").is_none() {
                state::shared(&handle).call(&handle, |k| k.warm_catalogue.as_ref());
            }
            if !quiet_run() {
                windows::start_keep_on_top(&handle);
            }
            if selftest::smoke_enabled() {
                let h = handle.clone();
                std::thread::spawn(move || selftest::scaffold(h));
            }
            // LIFE-01: a second launch opens the bar; one that came during
            // boot is answered now.
            let h = handle.clone();
            doorbell.answer(move || {
                let h2 = h.clone();
                let _ = h.run_on_main_thread(move || {
                    crash::guard("second-instance", || windows::trigger_capture(&h2));
                });
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("AevalBuddy failed to start");

    // LIFE-09: the last window closing quits, as window-all-closed did.
    app.run(|app, event| {
        if matches!(event, RunEvent::Exit) {
            crash::guard("exit", || on_exit(app));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::stays_in_app;

    #[test]
    fn navigation_stays_in_the_app() {
        let allowed = |u: &str| stays_in_app(&u.parse().unwrap());
        assert!(allowed("http://tauri.localhost/renderer/index.html"));
        assert!(allowed("http://tauri.localhost/a/b.html?kind=white"));
        assert!(allowed("tauri://localhost/settings/settings.html"));
        assert!(allowed("about:blank"));
        assert!(!allowed("https://example.com/"));
        assert!(!allowed("http://tauri.localhost.example.com/"));
        assert!(!allowed("http://aeval-frame.localhost/frame"));
        assert!(!allowed("file:///C:/Windows/notepad.exe"));
        assert!(!allowed("data:text/html,<script>alert(1)</script>"));
        assert!(!allowed("javascript:alert(1)"));
    }
}
