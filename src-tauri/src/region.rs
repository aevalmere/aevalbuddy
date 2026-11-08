//! Drag-to-choose-an-area (lib/region.js, PARITY CAP-10/11, IPC-12..15).
//!
//! The whole screen squeezed into a 1600 px long edge turns 12 px text into
//! about seven pixels; an area spends the same budget on a fraction of the
//! pixels, at 1:1. It is the largest single lever on answer quality.
//!
//! The scrim is live, not a frozen screenshot: nothing is transferred to the
//! page, and the capture that follows sees the screen as it is then. The
//! window is excluded from capture like every other, so it never appears in
//! the shot it triggers.

#![allow(dead_code)]

use std::sync::Mutex;

use serde_json::Value;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tokio::sync::oneshot;

use crate::capture::{self, screen, Rect};
use crate::desktop::win;

/// A drag shorter than this on either axis is a click that missed.
pub const MIN_EDGE: f64 = 12.0;
pub const LABEL: &str = "region";

struct Pending {
    tx: oneshot::Sender<Option<Rect>>,
    origin: (f64, f64),
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

fn pending() -> std::sync::MutexGuard<'static, Option<Pending>> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn is_active(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL).is_some()
}

fn settle(rect: Option<Rect>) {
    if let Some(p) = pending().take() {
        let _ = p.tx.send(rect);
    }
}

fn close(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.destroy();
    }
}

/// Closes the selector; the pick resolves with `None`.
pub fn cancel(app: &AppHandle) {
    close(app);
    settle(None);
}

/// The page reports window coordinates; the display origin (DIP) is added
/// here. Under 12 px on either axis is a cancel.
pub fn finish_rect(origin: (f64, f64), r: &Value) -> Option<Rect> {
    let n = |k: &str| {
        r.get(k)
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite())
            .unwrap_or(0.0)
    };
    let width = n("width").round();
    let height = n("height").round();
    if width < MIN_EDGE || height < MIN_EDGE {
        return None;
    }
    Some(Rect::new(
        (origin.0 + n("x")).round(),
        (origin.1 + n("y")).round(),
        width,
        height,
    ))
}

/// `region:done`.
pub fn finish(app: &AppHandle, r: &Value) {
    let origin = pending().as_ref().map(|p| p.origin).unwrap_or((0.0, 0.0));
    close(app);
    settle(finish_rect(origin, r));
}

/// Opens the selector on one display and resolves with the chosen DIP
/// rectangle in screen coordinates, or `None` if the user cancelled. A
/// second request while one is open replaces it.
pub async fn select(app: &AppHandle, display: Option<screen::Display>) -> Option<Rect> {
    if is_active(app) {
        cancel(app);
    }
    let target = display.unwrap_or_else(|| screen::primary(&screen::all_displays()));
    let (tx, rx) = oneshot::channel();
    *pending() = Some(Pending {
        tx,
        origin: (target.bounds.x, target.bounds.y),
    });
    // Built off the event loop thread: WebView2 creation waits on it.
    if let Err(e) = open_window(app, target.phys) {
        eprintln!("region selector failed to open: {e}");
        settle(None);
    }
    rx.await.ok().flatten()
}

fn open_window(app: &AppHandle, phys: screen::PxRect) -> tauri::Result<()> {
    // Unlike every other surface this one wants the keyboard and the mouse:
    // it is a modal gesture, not an overlay.
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("renderer/region.html".into()))
        .title("AevalBuddy")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .content_protected(true)
        .visible(false)
        .data_directory(crate::paths::webview_data_dir())
        .initialization_script(crate::ipc::bridge_script())
        .on_page_load(|w, p| {
            if matches!(p.event(), tauri::webview::PageLoadEvent::Finished) {
                let raw = capture::hwnd_of(&w);
                capture::exclude_window(raw);
                let _ = w.show();
                win::raise(raw);
                let _ = w.set_focus();
            }
        })
        .build()?;
    win::tool_window(capture::hwnd_of(&win), false);
    win::place(&win, phys);
    capture::exclude_window(capture::hwnd_of(&win));
    // Losing focus means the user clicked into something else; a full-screen
    // scrim they did not ask to keep must not survive that.
    let a = app.clone();
    let had_focus = std::sync::atomic::AtomicBool::new(false);
    win.on_window_event(move |e| match e {
        WindowEvent::Focused(true) => had_focus.store(true, std::sync::atomic::Ordering::Relaxed),
        WindowEvent::Focused(false) if had_focus.load(std::sync::atomic::Ordering::Relaxed) => {
            cancel(&a)
        }
        WindowEvent::Destroyed => settle(None),
        _ => {}
    });
    Ok(())
}

// ------------------------------------------------- the conversation's area

/// The chosen area sticks to the conversation (CAP-09) until cleared or a
/// new chat; the shot taken at pick time is used by the next message.
static ACTIVE: Mutex<Option<Rect>> = Mutex::new(None);
static PENDING_SHOT: Mutex<Option<capture::Shot>> = Mutex::new(None);

type Hook = Box<dyn Fn() + Send + Sync>;
static ON_CHANGE: Mutex<Option<Hook>> = Mutex::new(None);

/// Agent (a) passes what main.js did after a pick or a clear
/// (`expandPanel`, `showOutput`, `sendOverlayState`, `applyRegistrations`).
pub fn on_change(f: impl Fn() + Send + Sync + 'static) {
    *ON_CHANGE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(f));
}

fn changed() {
    if let Some(f) = ON_CHANGE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        f();
    }
}

pub fn active_region() -> Option<Rect> {
    *ACTIVE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The shot taken when the area was chosen, once.
pub fn take_pending_shot() -> Option<capture::Shot> {
    PENDING_SHOT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
}

/// Sets the area outright (Find's "Ask about this" looks at the hit with
/// room around it), as main.js assigned `activeRegion`.
pub fn set_active(rect: Option<Rect>) {
    let changed_now = {
        let mut a = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
        let was = *a;
        *a = rect;
        was != rect
    };
    if changed_now {
        changed();
    }
}

/// `region:clear` and New chat: back to the whole screen.
pub fn clear_active() {
    let had = ACTIVE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .is_some();
    if had {
        changed();
    }
}

/// `region:pick` (main.js triggerRegionCapture): selector on the active
/// display; on a pick the area is captured at once. A cancel leaves
/// everything as it was. Returns whether the capture worked.
pub async fn pick(app: &AppHandle) -> Option<bool> {
    if is_active(app) {
        return None;
    }
    let display = screen::active_display(&screen::all_displays());
    changed();
    let rect = select(app, Some(display)).await;
    let Some(rect) = rect else {
        changed();
        return None;
    };
    *ACTIVE.lock().unwrap_or_else(|e| e.into_inner()) = Some(rect);
    let shot = capture::capture_screen_async(Some(rect)).await;
    let ok = shot.is_some();
    *PENDING_SHOT.lock().unwrap_or_else(|e| e.into_inner()) = shot;
    changed();
    Some(ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn small_drags_cancel() {
        assert_eq!(
            finish_rect((0.0, 0.0), &json!({"x":1,"y":1,"width":11,"height":100})),
            None
        );
        assert_eq!(
            finish_rect((0.0, 0.0), &json!({"x":1,"y":1,"width":100,"height":11.4})),
            None
        );
        assert_eq!(finish_rect((0.0, 0.0), &json!({})), None);
    }

    #[test]
    fn origin_is_added_and_rounded() {
        assert_eq!(
            finish_rect(
                (1280.0, 0.0),
                &json!({"x":10.4,"y":20.6,"width":100.5,"height":50})
            ),
            Some(Rect::new(1290.0, 21.0, 101.0, 50.0))
        );
    }
}
