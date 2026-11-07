//! AevalBuddy's Rust core. ARCHITECTURE.md is the map: each module below owns
//! one responsibility of the Electron main process, and PARITY.md lists what
//! each must keep doing.

mod agent;
mod ask;
mod autostart;
mod backdrop;
mod buddy;
mod capture;
mod crash;
mod desktop;
mod find;
mod history;
mod hotkeys;
mod instance;
mod ipc;
mod lens;
mod llm;
mod notify;
mod paths;
mod region;
mod selftest;
mod skills;
mod state;
mod store;
mod tray;
mod updater;
mod windows;

use tauri::webview::PageLoadEvent;

pub fn run() {
    tauri::Builder::default()
        .manage(ipc::Seen::default())
        .invoke_handler(tauri::generate_handler![ipc::aeval_invoke])
        .on_page_load(|webview, payload| {
            let loaded = matches!(payload.event(), PageLoadEvent::Finished);
            if loaded && selftest::smoke_enabled() {
                let _ = webview.eval(selftest::PING);
            }
        })
        .setup(|app| {
            for (label, page) in windows::SCAFFOLD_WINDOWS {
                windows::create_page(app.handle(), label, page)?;
            }
            if selftest::smoke_enabled() {
                let handle = app.handle().clone();
                std::thread::spawn(move || selftest::scaffold(handle));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("AevalBuddy failed to start");
}
