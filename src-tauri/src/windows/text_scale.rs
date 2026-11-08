//! Text size (design-ux U8, DESIGN Accessibility, D-24). Pages scale their
//! type by `--text-scale`: the Windows text size (Settings, Accessibility,
//! Text size) times the person's own factor, at most 2.5. Rust sends it as
//! `ui:text-scale {scale}` to every window when either changes, and to each
//! page when it says hello.

use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::store;

/// The person's own factor: 100% to 200% in steps of 10%.
pub const USER_MIN: f64 = 1.0;
pub const USER_MAX: f64 = 2.0;
/// The effective scale never goes past this.
pub const MAX: f64 = 2.5;

/// A requested factor as kept: 1.0 to 2.0, to one decimal.
pub fn clamp_user(v: f64) -> f64 {
    if !v.is_finite() {
        return USER_MIN;
    }
    (v.clamp(USER_MIN, USER_MAX) * 10.0).round() / 10.0
}

/// What pages apply: the Windows factor times the person's, at most 2.5,
/// to three decimals.
pub fn effective(os: f64, user: f64) -> f64 {
    let os = if os.is_finite() { os.max(1.0) } else { 1.0 };
    ((os * clamp_user(user)).min(MAX) * 1000.0).round() / 1000.0
}

/// Windows' text size, 1.0 to 2.25: what UISettings.TextScaleFactor
/// reports, read where Settings writes it.
pub fn os_factor() -> f64 {
    use winreg::enums::HKEY_CURRENT_USER;
    winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Accessibility")
        .and_then(|k| k.get_value::<u32, _>("TextScaleFactor"))
        .map(|p| f64::from(p) / 100.0)
        .ok()
        .filter(|v| (1.0..=2.25).contains(v))
        .unwrap_or(1.0)
}

/// The factor set this session; None reads the stored `textScale`. Kept
/// in memory until the store can write a raw key (K-08).
fn session() -> &'static Mutex<Option<f64>> {
    static S: OnceLock<Mutex<Option<f64>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// The person's own factor (`settings:get.textScale`).
pub fn user_factor() -> f64 {
    if let Some(v) = *session().lock() {
        return v;
    }
    let stored = store::get()
        .raw("textScale")
        .and_then(|v| v.as_f64())
        .unwrap_or(USER_MIN);
    clamp_user(stored)
}

/// The scale pages apply now.
pub fn scale() -> f64 {
    effective(os_factor(), user_factor())
}

/// `settings:set-text-scale`: keeps the person's factor and tells every
/// window. Returns the factor as kept.
pub fn set_user(app: &AppHandle, v: f64) -> f64 {
    let v = clamp_user(v);
    *session().lock() = Some(v);
    broadcast(app);
    v
}

/// `ui:text-scale` to one window (its hello).
pub fn send_to(app: &AppHandle, label: &str) {
    super::emit(app, label, "ui:text-scale", json!({ "scale": scale() }));
}

/// `ui:text-scale` to every window.
pub fn broadcast(app: &AppHandle) {
    let s = scale();
    for (label, _) in app.webview_windows() {
        super::emit(app, &label, "ui:text-scale", json!({ "scale": s }));
    }
}

/// Follows the Windows text size: reads it every 2 s and tells every window
/// when it changed.
pub fn start_watch(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let mut last = os_factor();
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let now = os_factor();
            if (now - last).abs() > 0.001 {
                last = now;
                broadcast(&app);
            }
        }
    });
}
