//! Desktop perception and action through the C# helper sidecar (PARITY
//! DESK-*), plus the wiring for the rest of split (c): the IPC rows for
//! region, buddy and glass, and the CI checks.
//!
//! Callers use the one shared client: `desktop::client().list_windows(..)`.
//! Agent (b)'s find and agent modules get every op index.js exported, with
//! the same JSON shapes.

pub mod channels;
pub mod checks;
pub mod client;
pub mod protocol;
pub mod scoring;
pub mod selfcheck;
pub mod win;

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use tauri::Manager;

#[allow(unused_imports)]
pub use client::{Deps, Desktop, OwnWindow};
#[allow(unused_imports)]
pub use protocol::{DResult, DesktopError};

/// The process-wide helper client (lazy: nothing spawns until a call).
pub fn client() -> &'static Arc<Desktop> {
    static CLIENT: OnceLock<Arc<Desktop>> = OnceLock::new();
    CLIENT.get_or_init(Desktop::new)
}

/// Called once from `lib.rs` setup: lets the client read our windows, starts
/// the glass backdrop, and runs a CI check when one is asked for.
pub fn setup(app: &tauri::AppHandle) {
    client().set_app(app.clone());
    crate::backdrop::init(app.clone());
    watch_boot(app);
    checks::maybe_run(app);
}

static HELPER_AT_BOOT: OnceLock<bool> = OnceLock::new();

/// Under the smoke, notes whether the helper is already running once the
/// boot windows have reached Rust: the moment Electron's runSmoke took
/// `helperStartedBeforeSmoke`, before any lane uses the helper.
fn watch_boot(app: &tauri::AppHandle) {
    if !crate::selftest::smoke_enabled() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let want: Vec<&str> = crate::windows::SCAFFOLD_WINDOWS
            .iter()
            .map(|(l, _)| *l)
            .collect();
        let booted = || {
            let Some(seen) = app.try_state::<crate::ipc::Seen>() else {
                return false;
            };
            let seen = seen.snapshot();
            want.iter().all(|w| seen.contains_key(*w))
        };
        let end = Instant::now() + Duration::from_secs(30);
        while !booted() {
            if Instant::now() > end {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let c = client();
        let _ = HELPER_AT_BOOT.set(c.is_ready() || !c.status()["pid"].is_null());
    });
}

/// smoke-v2.js B29: whether the helper was running at boot, when it never
/// should be (it starts on first use). `None` outside the smoke, or when the
/// boot windows never reached Rust.
pub fn helper_at_boot() -> Option<bool> {
    HELPER_AT_BOOT.get().copied()
}

/// Called on quit: the helper tree must not outlive the app (it would hold
/// files under the install folder and block an update).
pub fn shutdown() {
    client().stop();
}
