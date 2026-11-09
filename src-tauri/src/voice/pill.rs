//! The dictation pill: a small glass capsule at the bottom centre of the
//! screen the person is working on, there while dictation listens and
//! works. It never takes focus (the text goes to the app that has it), lets
//! clicks through, and stays out of screenshots.

use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::capture::{self, screen, PxRect};
use crate::desktop::win;

pub const LABEL: &str = "voice-pill";
const WIDTH: f64 = 200.0;
const HEIGHT: f64 = 52.0;
/// Above the taskbar, clear of it.
const MARGIN: f64 = 20.0;

/// The last state sent, so a page that loads late still shows it.
static LAST: Mutex<Option<Value>> = Mutex::new(None);

fn last() -> std::sync::MutexGuard<'static, Option<Value>> {
    LAST.lock().unwrap_or_else(|e| e.into_inner())
}

fn create(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    let win = WebviewWindowBuilder::new(
        app,
        LABEL,
        WebviewUrl::App("renderer/voice-pill.html".into()),
    )
    .title("AevalBuddy dictation")
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .resizable(false)
    .focusable(false)
    .focused(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .content_protected(true)
    .visible(false)
    .data_directory(crate::paths::webview_data_dir())
    .initialization_script(crate::ipc::bridge_script())
    .on_page_load(|w, p| {
        if matches!(p.event(), tauri::webview::PageLoadEvent::Finished) {
            if let Some(v) = last().clone() {
                let _ = tauri::Emitter::emit_to(&w, LABEL, "voice:state", v);
            }
        }
    })
    .build()?;
    let raw = capture::hwnd_of(&win);
    win::tool_window(raw, true);
    let _ = win.set_ignore_cursor_events(true);
    capture::exclude_window(raw);
    crate::crash::guard_webview(&win, |app, label| {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.destroy();
        }
    });
    Ok(win)
}

fn window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    app.get_webview_window(LABEL).or_else(|| match create(app) {
        Ok(w) => Some(w),
        Err(e) => {
            crate::crash::write("voice-pill", &e.to_string());
            None
        }
    })
}

/// Bottom centre of the work area, in physical pixels.
pub fn place_on(work: PxRect, scale: f64) -> PxRect {
    let w = (WIDTH * scale).round() as i32;
    let h = (HEIGHT * scale).round() as i32;
    PxRect {
        x: work.x + (work.width - w) / 2,
        y: work.y + work.height - h - (MARGIN * scale).round() as i32,
        width: w,
        height: h,
    }
}

/// Builds the window ahead of the first hold, so it shows at once.
pub fn prepare(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        let a = app.clone();
        let _ = app.run_on_main_thread(move || {
            window(&a);
        });
    });
}

/// Shows the pill with `state` on the active screen.
pub fn show(app: &AppHandle, state: Value) {
    *last() = Some(state.clone());
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(w) = window(&a) else { return };
        let d = screen::active_display(&screen::all_displays());
        win::place(&w, place_on(d.work_phys, d.scale_factor));
        let raw = capture::hwnd_of(&w);
        win::show_inactive(raw);
        win::raise(raw);
        crate::windows::emit(&a, LABEL, "voice:state", state);
    });
}

/// Sends `state` to the pill if it is up.
pub fn update(app: &AppHandle, state: Value) {
    *last() = Some(state.clone());
    crate::windows::emit(app, LABEL, "voice:state", state);
}

/// Hides it after `after` (a done or error state stays briefly).
pub fn hide(app: &AppHandle, after: Duration, generation: u64) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(after);
        if super::generation() != generation {
            return;
        }
        *last() = None;
        let a = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(w) = a.get_webview_window(LABEL) {
                win::hide(capture::hwnd_of(&w));
            }
        });
    });
}

/// Whether the pill shows now (the selfcheck asks).
pub fn visible(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_sits_at_the_bottom_centre() {
        let work = PxRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
        };
        let r = place_on(work, 1.0);
        assert_eq!((r.x, r.width, r.y + r.height), (860, 200, 1020));
        let r2 = place_on(
            PxRect {
                x: -2560,
                y: 0,
                width: 2560,
                height: 1400,
            },
            2.0,
        );
        assert_eq!((r2.x, r2.width), (-2560 + 1080, 400));
    }
}
