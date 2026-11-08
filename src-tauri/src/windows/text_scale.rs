//! Text size (design-ux U8, DESIGN Accessibility, D-24). Pages scale their
//! type by `--text-scale`: the Windows text size (Settings, Accessibility,
//! Text size) times the person's own factor, at most 2.5. Rust sends it as
//! `ui:text-scale {scale}` to every window when either changes, and to each
//! page when it says hello. The person's factor is the stored `textScale`.

use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Manager};
use windows::core::IInspectable;
use windows::Foundation::TypedEventHandler;
use windows::UI::ViewManagement::UISettings;

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

/// Windows' text size, 1.0 to 2.25: UISettings.TextScaleFactor, or where
/// Settings writes it when WinRT cannot be reached.
pub fn os_factor() -> f64 {
    ui_settings_factor()
        .or_else(registry_factor)
        .filter(|v| (1.0..=2.25).contains(v))
        .unwrap_or(1.0)
}

fn ui_settings_factor() -> Option<f64> {
    UISettings::new().ok()?.TextScaleFactor().ok()
}

fn registry_factor() -> Option<f64> {
    use winreg::enums::HKEY_CURRENT_USER;
    winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Accessibility")
        .and_then(|k| k.get_value::<u32, _>("TextScaleFactor"))
        .map(|p| f64::from(p) / 100.0)
        .ok()
}

/// The person's own factor (`settings:get.textScale`), stored as
/// `textScale` (K-08).
pub fn user_factor() -> f64 {
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

/// `settings:set-text-scale`: stores the person's factor and tells every
/// window. Returns the factor as kept.
pub fn set_user(app: &AppHandle, v: f64) -> f64 {
    let v = clamp_user(v);
    store::get().set_raw("textScale", json!(v));
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

/// Follows the Windows text size: UISettings' TextScaleFactorChanged tells
/// every window; where it cannot be had, a look every 2 s does.
pub fn start_watch(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        // The settings object lives on this thread, which keeps it; the
        // event comes on a pool thread.
        if let Some(_settings) = watch_ui_settings(&app) {
            loop {
                std::thread::park();
            }
        }
        poll(&app);
    });
}

fn watch_ui_settings(app: &AppHandle) -> Option<UISettings> {
    let settings = UISettings::new().ok()?;
    let a = app.clone();
    let changed = TypedEventHandler::<UISettings, IInspectable>::new(move |_, _| {
        broadcast(&a);
        Ok(())
    });
    settings.TextScaleFactorChanged(&changed).ok()?;
    Some(settings)
}

fn poll(app: &AppHandle) {
    let mut last = os_factor();
    loop {
        std::thread::sleep(Duration::from_secs(2));
        let now = os_factor();
        if (now - last).abs() > 0.001 {
            last = now;
            broadcast(app);
        }
    }
}
