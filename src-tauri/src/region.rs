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

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tokio::sync::oneshot;

use crate::capture::{self, screen, Rect};
use crate::desktop::win;
use crate::state::{shared, Mode};

/// A drag shorter than this on either axis is a click that missed.
pub const MIN_EDGE: f64 = 12.0;
pub const LABEL: &str = "region";

/// How long the selector page may take to load before the pick gives up
/// (B-028): an invisible modal pick must never stay open.
const LOAD_LIMIT: Duration = Duration::from_secs(5);

/// One open pick. `gen` tells it apart from the pick it replaced, whose
/// window may still be closing.
struct Pending {
    tx: oneshot::Sender<Option<Rect>>,
    origin: (f64, f64),
    gen: u64,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
/// The newest pick's generation, and the newest whose page has loaded.
static GEN: AtomicU64 = AtomicU64::new(0);
static LOADED: AtomicU64 = AtomicU64::new(0);

fn pending() -> std::sync::MutexGuard<'static, Option<Pending>> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn is_active(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL).is_some()
}

/// The newest pick's selector is open and its page has loaded.
pub fn ready(app: &AppHandle) -> bool {
    is_active(app) && LOADED.load(Ordering::Relaxed) == GEN.load(Ordering::Relaxed)
}

/// Opens a pick and returns its generation. A pick still open resolves
/// `None`: a second request replaces the first.
fn begin(tx: oneshot::Sender<Option<Rect>>, origin: (f64, f64)) -> u64 {
    let gen = GEN.fetch_add(1, Ordering::Relaxed) + 1;
    let old = pending().replace(Pending { tx, origin, gen });
    if let Some(p) = old {
        let _ = p.tx.send(None);
    }
    gen
}

fn current() -> Option<u64> {
    pending().as_ref().map(|p| p.gen)
}

/// Resolves the open pick, whichever it is.
fn settle(rect: Option<Rect>) {
    if let Some(p) = pending().take() {
        let _ = p.tx.send(rect);
    }
}

/// Resolves pick `gen` only, so a closing selector's late `Destroyed` or
/// focus loss never touches the pick that replaced it.
fn settle_gen(gen: u64, rect: Option<Rect>) {
    let mine = pending().take_if(|p| p.gen == gen);
    if let Some(p) = mine {
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
    settle(None);
    close(app);
}

/// `cancel` for pick `gen` only.
fn cancel_gen(app: &AppHandle, gen: u64) {
    if current() == Some(gen) {
        settle_gen(gen, None);
        close(app);
    }
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

/// `region:done`. The answer is settled before the window closes: closing
/// fires `Destroyed`, which would otherwise resolve the pick as a cancel
/// first and lose the area (B-028).
pub fn finish(app: &AppHandle, r: &Value) {
    let origin = pending().as_ref().map(|p| p.origin).unwrap_or((0.0, 0.0));
    settle(finish_rect(origin, r));
    close(app);
}

/// Opens the selector on one display and resolves with the chosen DIP
/// rectangle in screen coordinates, or `None` if the user cancelled. A
/// second request while one is open replaces it.
pub async fn select(app: &AppHandle, display: Option<screen::Display>) -> Option<Rect> {
    if is_active(app) {
        cancel(app);
        // The label is free again only once the old window is gone.
        for _ in 0..50 {
            if !is_active(app) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    let target = display.unwrap_or_else(|| screen::primary(&screen::all_displays()));
    let (tx, rx) = oneshot::channel();
    let gen = begin(tx, (target.bounds.x, target.bounds.y));
    // Built off the event loop thread: WebView2 creation waits on it.
    match open_window(app, target.phys, gen) {
        Ok(()) => {
            // Only now is there a selector: Escape is claimed and the
            // overlay hears it (CAP-10).
            changed();
            give_up_unless_loaded(app, gen);
        }
        Err(e) => {
            eprintln!("region selector failed to open: {e}");
            settle_gen(gen, None);
        }
    }
    rx.await.ok().flatten()
}

/// After `LOAD_LIMIT`, a pick whose page never loaded is closed and
/// resolves `None`.
fn give_up_unless_loaded(app: &AppHandle, gen: u64) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(LOAD_LIMIT).await;
        if LOADED.load(Ordering::Relaxed) < gen && current() == Some(gen) {
            let s = LOAD_LIMIT.as_secs();
            eprintln!("region selector did not load in {s} s");
            cancel_gen(&app, gen);
        }
    });
}

fn open_window(app: &AppHandle, phys: screen::PxRect, gen: u64) -> tauri::Result<()> {
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
        .on_page_load(move |w, p| {
            if matches!(p.event(), tauri::webview::PageLoadEvent::Finished) {
                LOADED.fetch_max(gen, Ordering::Relaxed);
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
    let seen = AtomicBool::new(false);
    win.on_window_event(move |e| match e {
        WindowEvent::Focused(true) => seen.store(true, Ordering::Relaxed),
        WindowEvent::Focused(false) if seen.load(Ordering::Relaxed) => cancel_gen(&a, gen),
        WindowEvent::Destroyed => settle_gen(gen, None),
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

/// The display the area was chosen on (its device name). A DIP rect alone
/// can sit where two displays' DIP ranges meet; the device says which one.
static ACTIVE_DEVICE: Mutex<Option<String>> = Mutex::new(None);

fn store_active(rect: Option<Rect>, device: Option<String>) {
    let changed_now = {
        let mut a = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
        let was = *a;
        *a = rect;
        *ACTIVE_DEVICE.lock().unwrap_or_else(|e| e.into_inner()) = device;
        was != rect
    };
    if changed_now {
        changed();
    }
}

/// Sets the area outright (Find's "Ask about this" looks at the hit with
/// room around it), as main.js assigned `activeRegion`. The display is the
/// one the rect matches.
pub fn set_active(rect: Option<Rect>) {
    store_active(rect, None);
}

/// K-12: sets the area on a known display (`Display::device`), for callers
/// that start from physical pixels, where the display is unambiguous.
// host/find.rs (llm lane) moves to this from `set_active`.
#[allow(dead_code)]
pub fn set_active_on(rect: Rect, device: &str) {
    store_active(Some(rect), Some(device.to_string()));
}

/// The display a capture of `rect` should read: the one the area was chosen
/// on, when `rect` is the conversation's area and that is known.
pub fn device_for(rect: &Rect) -> Option<String> {
    if active_region().as_ref() != Some(rect) {
        return None;
    }
    ACTIVE_DEVICE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// `region:clear` and New chat: back to the whole screen.
pub fn clear_active() {
    let had = ACTIVE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .is_some();
    *ACTIVE_DEVICE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    if had {
        changed();
    }
}

/// `region:pick` (main.js triggerRegionCapture): selector on the active
/// display; on a pick the area is captured at once. A cancel leaves
/// everything as it was. Returns whether the capture worked.
pub async fn pick(app: &AppHandle) -> Option<bool> {
    // As main.js: never over the HUD, never two at once.
    if is_active(app) || shared(app).lock().mode == Mode::Hud {
        return None;
    }
    let display = screen::active_display(&screen::all_displays());
    let device = display.device.clone();
    let rect = select(app, Some(display)).await;
    let Some(rect) = rect else {
        changed();
        return None;
    };
    *ACTIVE.lock().unwrap_or_else(|e| e.into_inner()) = Some(rect);
    *ACTIVE_DEVICE.lock().unwrap_or_else(|e| e.into_inner()) = Some(device);
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
    fn the_area_remembers_its_display() {
        let r = Rect::new(1500.0, 200.0, 300.0, 150.0);
        set_active_on(r, r"\\.\DISPLAY1");
        assert_eq!(device_for(&r).as_deref(), Some(r"\\.\DISPLAY1"));
        // Only the conversation's own area carries it.
        assert_eq!(device_for(&Rect::new(0.0, 0.0, 10.0, 10.0)), None);
        // An area set without a display leaves matching to the capture.
        set_active(Some(r));
        assert_eq!(device_for(&r), None);
        set_active_on(r, r"\\.\DISPLAY2");
        clear_active();
        assert_eq!(device_for(&r), None);
        assert_eq!(active_region(), None);
    }

    #[test]
    fn stale_close_does_not_settle_new_pick() {
        let (tx1, mut rx1) = oneshot::channel();
        let first = begin(tx1, (0.0, 0.0));
        // A second request replaces the first, which resolves as a cancel.
        let (tx2, mut rx2) = oneshot::channel();
        let second = begin(tx2, (1280.0, 0.0));
        assert_eq!(rx1.try_recv(), Ok(None));
        // The first selector's window closing late leaves the new pick open.
        settle_gen(first, None);
        assert!(rx2.try_recv().is_err());
        assert_eq!(current(), Some(second));
        // Its own answer resolves it.
        let area = Rect::new(1290.0, 10.0, 100.0, 50.0);
        settle_gen(second, Some(area));
        assert_eq!(rx2.try_recv(), Ok(Some(area)));
        assert_eq!(current(), None);
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
