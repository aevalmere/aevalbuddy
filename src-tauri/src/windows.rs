//! Window manager (agent a). Phase 1 creates the two windows every session
//! has, the overlay (pill and bar) and an answer window, from the unchanged
//! pages, with the frame rules every AevalBuddy window shares (PARITY WIN-*).
//! Positioning, the stage margin, motion, the HUD editor and the floating
//! output windows of REQUESTS.md 8-11 arrive with the port.

use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

use crate::{ipc, paths};

/// A page's first invoke (`glass:hello`): split (a)'s `windows::on_hello`
/// sends the page its first state. This no-op stands in on the scaffold so
/// `desktop::channels` can call it now; (a)'s windows/mod.rs replaces this
/// file at the merge. (agent c)
pub fn on_hello(_app: &AppHandle, _label: &str) {}

/// Label and page of each window the scaffold opens.
pub const SCAFFOLD_WINDOWS: [(&str, &str); 2] = [
    ("overlay", "renderer/index.html"),
    ("output", "renderer/output.html"),
];

/// One page in a frameless, transparent, topmost tool window that is hidden
/// from screen capture and sees only `window.aeval`.
pub fn create_page(app: &AppHandle, label: &str, page: &str) -> tauri::Result<()> {
    let bridge = ipc::bridge_script();
    WebviewWindowBuilder::new(app, label, WebviewUrl::App(page.into()))
        .title("AevalBuddy")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .content_protected(true)
        .focused(false)
        .visible(false)
        .inner_size(460.0, 220.0)
        .data_directory(paths::webview_data_dir())
        .initialization_script(&bridge)
        .build()?;
    Ok(())
}
