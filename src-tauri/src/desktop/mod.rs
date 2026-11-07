//! Desktop perception and action through the C# helper sidecar (PARITY
//! DESK-*), plus the wiring for the rest of split (c): the IPC rows for
//! region, buddy and glass, and the CI checks.
//!
//! Callers use the one shared client: `desktop::client().list_windows(..)`.
//! Agent (b)'s find and agent modules get every op index.js exported, with
//! the same JSON shapes.

// Much of this API is for agents (a) and (b) to call as they port.
#![allow(dead_code)]

pub mod channels;
pub mod checks;
pub mod client;
pub mod protocol;
pub mod scoring;
pub mod selfcheck;
pub mod win;

use std::sync::{Arc, OnceLock};

pub use client::{Deps, Desktop, OwnWindow};
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
    checks::maybe_run(app);
}

/// Called on quit: the helper tree must not outlive the app (it would hold
/// files under the install folder and block an update).
pub fn shutdown() {
    client().stop();
}
