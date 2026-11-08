//! AevalBuddy's Rust core. ARCHITECTURE.md is the map: each module below owns
//! one responsibility of the Electron main process, and PARITY.md lists what
//! each must keep doing.

mod agent;
mod ask;
pub mod autostart;
mod backdrop;
mod buddy;
mod capture;
pub mod crash;
mod desktop;
mod find;
pub mod history;
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
pub mod windows;

use tauri::webview::PageLoadEvent;
use tauri::{Manager, RunEvent};

fn quiet_run() -> bool {
    std::env::var_os("AEVAL_SMOKE").is_some() || std::env::var_os("AEVAL_SHOT").is_some()
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
                if failed {
                    windows::open_settings(&a2, Some("sec-shortcuts"));
                } else {
                    windows::open_bar(&a2, None, None);
                }
            });
        })),
    );
}

pub fn run() {
    windows::mark_start();
    notify::set_app_user_model_id();
    let user_data = paths::user_data_dir();
    let _ = std::fs::create_dir_all(&user_data);
    crash::install(&user_data);
    // LIFE-01: a second launch opens the first copy's bar and exits.
    let Some(lock) = instance::acquire(&user_data) else {
        return;
    };
    store::init(&user_data);
    history::init(&user_data).set_enabled(store::get().get_save_history());

    let shortcuts = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| hotkeys::on_shortcut(app, shortcut, event))
        .build();

    let app = tauri::Builder::default()
        .plugin(shortcuts)
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .manage(ipc::Seen::default())
        .manage(state::Shared::default())
        .invoke_handler(tauri::generate_handler![ipc::aeval_invoke])
        .on_page_load(|webview, payload| {
            if !matches!(payload.event(), PageLoadEvent::Finished) {
                return;
            }
            if selftest::smoke_enabled() {
                let _ = webview.eval(selftest::PING);
            }
            windows::on_page_loaded(webview.app_handle(), webview.label());
        })
        .on_window_event(windows::on_window_event)
        .setup(move |app| {
            let handle = app.handle().clone();
            windows::boot(&handle)?;
            let h2 = handle.clone();
            instance::serve(lock.name.clone(), move || {
                let h3 = h2.clone();
                let _ = h2.run_on_main_thread(move || windows::trigger_capture(&h3));
            });
            store::get().decide_presence();
            autostart::apply();
            // Before hotkeys::apply, which refreshes the tray labels.
            if let Err(e) = tray::init(&handle) {
                crash::write("tray", &e.to_string());
            }
            hotkeys::apply(&handle);
            updater::init(&handle, &paths::user_data_dir());
            presence_toast(&handle);
            let s = store::get();
            if !s.has_api_key() && !s.get_walkthrough_auto_shown() && !quiet_run() {
                s.set_walkthrough_auto_shown();
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    if state::shared(&h).lock().mode != state::Mode::Hud {
                        state::shared(&h).call(&h, |k| k.start_tour.as_ref());
                        hotkeys::apply(&h);
                    }
                });
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
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("AevalBuddy failed to start");

    app.run(|app, event| match event {
        // LIFE-17: windows closing never quits; the tray's Quit does.
        RunEvent::ExitRequested {
            code: None, api, ..
        } => api.prevent_exit(),
        RunEvent::Exit => {
            let st = state::shared(app);
            if st.lock().agent.running {
                st.call(app, |k| k.stop_agent.as_ref());
            }
            st.call(app, |k| k.close_find.as_ref());
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            let _ = app.global_shortcut().unregister_all();
            history::get().flush_now();
            updater::destroy();
            updater::install_on_quit();
            tray::destroy(app);
        }
        _ => {}
    });
}
