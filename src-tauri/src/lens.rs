//! The lens (lib/lens.js, PARITY LENS-01/02, GLASS-09): a sheet over each
//! display that draws Find's highlights and, while the agent runs, the
//! control frame and its chip. One window per display that has something to
//! draw, sized to the display, click-through, never focusable, and excluded
//! from capture, so a presenter can Find without the audience seeing it.
//!
//! Rects in are DIP screen coordinates; callers convert the desktop engine's
//! physical pixels first.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::capture::{self, screen, Rect};
use crate::desktop::win;
use crate::windows::flags;

/// Matches PAD in renderer/lens.js, the ring's inset.
pub const LOUPE_PAD: f64 = 3.0;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Hit {
    pub i: i64,
    #[serde(default)]
    pub kind: String,
    pub rect: Option<Rect>,
    #[serde(default)]
    pub why: Value,
    #[serde(default)]
    pub offscreen: bool,
}

#[derive(Default)]
struct State {
    hits: Vec<(Hit, u32)>,
    current: i64,
    frame: Option<(u32, String)>,
    hidden: bool,
    loupe_for: String,
    ready: HashSet<String>,
    /// Windows on their way out; each still holds its label until it is gone.
    closing: HashSet<String>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(|| State {
        current: -1,
        ..Default::default()
    }))
}

pub fn label_for(display_id: u32) -> String {
    format!("lens-{display_id}")
}

/// What one display's lens is told to draw (`lens:draw`).
pub fn draw_payload(
    hits: &[(Hit, u32)],
    current: i64,
    frame: Option<&(u32, String)>,
    display: &screen::Display,
) -> Value {
    let o = display.bounds;
    let list: Vec<Value> = hits
        .iter()
        .filter(|(_, d)| *d == display.id)
        .filter_map(|(h, _)| {
            let r = h.rect?;
            Some(json!({
                "i": h.i,
                "kind": h.kind,
                "why": h.why,
                "x": (r.x - o.x).round(),
                "y": (r.y - o.y).round(),
                "w": r.width.round().max(4.0),
                "h": r.height.round().max(4.0),
            }))
        })
        .collect();
    let cur = hits.iter().find(|(h, _)| h.i == current);
    json!({
        "hits": list,
        "current": if cur.map(|(_, d)| *d == display.id).unwrap_or(false) { current } else { -1 },
        "frame": match frame {
            Some((id, chip)) if *id == display.id => json!({ "chip": chip }),
            _ => Value::Null,
        },
    })
}

fn create(app: &AppHandle, d: &screen::Display) -> tauri::Result<tauri::WebviewWindow> {
    let label = label_for(d.id);
    let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("renderer/lens.html".into()))
        .title("AevalBuddy")
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
            // A reload starts over. `lens:ready` ends the wait, so hits that
            // came in while the page loaded are drawn once it can hear them.
            if matches!(p.event(), tauri::webview::PageLoadEvent::Started) {
                with(|s| s.ready.remove(w.label()));
            }
        })
        .build()?;
    let raw = capture::hwnd_of(&win);
    win::tool_window(raw, true);
    win::set_mouse_through(raw, true);
    win::place(&win, d.phys);
    capture::exclude_window(raw);
    win.on_window_event(move |e| {
        if matches!(e, tauri::WindowEvent::Destroyed) {
            with(|s| s.closing.remove(&label));
        }
    });
    crate::crash::guard_webview(&win, browser_gone);
    Ok(win)
}

/// The browser process died and the webview with it: the window goes, and
/// the next draw builds a new one.
fn browser_gone(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        with(|s| {
            s.ready.remove(label);
            s.closing.insert(label.to_string());
        });
        let _ = w.destroy();
    }
    redraw(app);
}

/// `lens:ready`: the page has its listeners, so what waited can be drawn.
pub fn page_ready(app: &AppHandle, label: &str) {
    with(|s| {
        s.ready.insert(label.to_string());
        s.loupe_for.clear();
    });
    redraw(app);
}

pub fn is_ready(label: &str) -> bool {
    with(|s| s.ready.contains(label))
}

/// Waits up to a second for a window on its way out to be gone. Only ever on
/// a `draw` thread.
fn wait_out_close(app: &AppHandle, label: &str) {
    for _ in 0..50 {
        if !with(|s| s.closing.contains(label)) || app.get_webview_window(label).is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    with(|s| s.closing.remove(label));
}

/// `draw` on its own thread. Windows are never built on the event loop
/// thread (WebView2 creation waits on it), and callers include sync IPC
/// handlers and page-load callbacks that run there.
pub fn redraw(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || draw(&app));
}

static DRAW: Mutex<()> = Mutex::new(());

/// Brings every display's lens in line with the current hits and frame.
pub fn draw(app: &AppHandle) {
    let _one = DRAW.lock().unwrap_or_else(|e| e.into_inner());
    let (hits, current, frame, hidden, ready) = with(|s| {
        (
            s.hits.clone(),
            s.current,
            s.frame.clone(),
            s.hidden,
            s.ready.clone(),
        )
    });
    let mut wanted: HashSet<u32> = hits.iter().map(|(_, d)| *d).collect();
    if let Some((id, _)) = &frame {
        wanted.insert(*id);
    }
    for d in screen::all_displays() {
        let label = label_for(d.id);
        let drawn = wanted.contains(&d.id) && !hidden;
        if drawn {
            wait_out_close(app, &label);
        }
        let existing = app.get_webview_window(&label);
        if !drawn {
            if let Some(w) = existing {
                let _ = w.emit_to(
                    label.as_str(),
                    "lens:draw",
                    json!({ "hits": [], "current": -1, "frame": null }),
                );
                // Win32, not `w.hide()`: a queued hide could land after the
                // next draw's show.
                win::hide(capture::hwnd_of(&w));
            }
            continue;
        }
        let win = match existing {
            Some(w) => {
                win::place(&w, d.phys);
                w
            }
            None => match create(app, &d) {
                Ok(w) => w,
                Err(_) => continue,
            },
        };
        let is_ready = ready.contains(&label);
        if is_ready {
            let _ = app.emit_to(
                label.as_str(),
                "lens:draw",
                draw_payload(&hits, current, frame.as_ref(), &d),
            );
        }
        let raw = capture::hwnd_of(&win);
        if !win.is_visible().unwrap_or(false) {
            win::show_inactive(raw);
        }
        win::raise(raw);
        let cur = hits.iter().find(|(h, id)| h.i == current && *id == d.id);
        if let (Some((h, _)), true) = (cur, is_ready) {
            loupe(app, &label, h, d.id);
        }
    }
}

/// The current hit through a magnifier: one full-resolution capture each
/// time the current hit changes, never on a timer. Our windows are excluded
/// from capture, so the cut never contains the lens's own marks. The cut
/// comes from the display the hit was placed on (B-005).
fn loupe(app: &AppHandle, label: &str, cur: &Hit, display: u32) {
    let Some(r) = cur.rect else { return };
    let key = format!(
        "{}:{},{},{},{}",
        cur.i,
        r.x.round(),
        r.y.round(),
        r.width.round(),
        r.height.round()
    );
    let fresh = with(|s| {
        if s.loupe_for == key {
            false
        } else {
            s.loupe_for = key.clone();
            true
        }
    });
    if !fresh {
        return;
    }
    let rect = Rect::new(
        (r.x - LOUPE_PAD).round(),
        (r.y - LOUPE_PAD).round(),
        (r.width + LOUPE_PAD * 2.0).round().max(4.0),
        (r.height + LOUPE_PAD * 2.0).round().max(4.0),
    );
    let app = app.clone();
    let label = label.to_string();
    let i = cur.i;
    std::thread::spawn(move || {
        let Some(url) = crate::backdrop::crop_url(rect, Some(display)) else {
            return;
        };
        if with(|s| s.loupe_for != key) {
            return;
        }
        let _ = app.emit_to(label.as_str(), "lens:loupe", json!({ "i": i, "img": url }));
    });
}

/// Find's hits; offscreen hits are listed in the results but not drawn.
pub fn set_hits(app: &AppHandle, list: Vec<Hit>, current: i64) {
    let displays = screen::all_displays();
    let hits: Vec<(Hit, u32)> = list
        .into_iter()
        .filter(|h| h.rect.is_some() && !h.offscreen)
        .map(|h| {
            let id = screen::matching(&displays, &h.rect.unwrap_or_default()).id;
            (h, id)
        })
        .collect();
    with(|s| {
        s.hits = hits;
        s.current = current;
    });
    redraw(app);
}

/// The agent's control frame on one display, or `None` to take it down.
pub fn set_frame(app: &AppHandle, display: Option<&screen::Display>, chip: &str) {
    with(|s| s.frame = display.map(|d| (d.id, chip.to_string())));
    redraw(app);
}

/// Find's hits are gone. With no control frame either, the windows go too:
/// a lens with nothing to draw must not keep the backdrop awake (B-030).
pub fn clear(app: &AppHandle) {
    let nothing = with(|s| {
        s.hits.clear();
        s.current = -1;
        s.loupe_for.clear();
        s.frame.is_none()
    });
    if nothing {
        destroy(app);
    } else {
        redraw(app);
    }
}

// lens.js export; Called by the smoke and shot harnesses (agent e), not merged yet.
#[allow(dead_code)]
pub fn set_hidden(app: &AppHandle, on: bool) {
    with(|s| s.hidden = on);
    redraw(app);
}

fn all_windows(app: &AppHandle) -> Vec<tauri::WebviewWindow> {
    app.webview_windows()
        .into_iter()
        .filter(|(l, _)| l.starts_with("lens-"))
        .map(|(_, w)| w)
        .collect()
}

/// The lens windows on screen. A hidden one must not count as something the
/// backdrop has to keep fresh (B-030).
pub fn windows(app: &AppHandle) -> Vec<tauri::WebviewWindow> {
    all_windows(app)
        .into_iter()
        .filter(|w| flags::is_visible(capture::hwnd_of(w)))
        .collect()
}

pub fn destroy(app: &AppHandle) {
    let list = all_windows(app);
    with(|s| {
        s.ready.clear();
        s.closing.extend(list.iter().map(|w| w.label().to_string()));
    });
    for w in list {
        let _ = w.destroy();
    }
}

/// Per-display DIP to display id map, for callers that place hits.
// lens.js export; Called by the smoke and shot harnesses (agent e), not merged yet.
#[allow(dead_code)]
pub fn display_ids() -> HashMap<u32, Rect> {
    screen::all_displays()
        .into_iter()
        .map(|d| (d.id, d.bounds))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d() -> screen::Display {
        screen::Display::from_phys(
            r"\\.\DISPLAY2",
            false,
            96,
            screen::PxRect {
                x: 1920,
                y: 0,
                width: 1920,
                height: 1080,
            },
            screen::PxRect {
                x: 1920,
                y: 0,
                width: 1920,
                height: 1080,
            },
        )
    }

    #[test]
    fn payload_is_display_relative_with_a_4px_floor() {
        let disp = d();
        let hits = vec![
            (
                Hit {
                    i: 0,
                    kind: "literal".into(),
                    rect: Some(Rect::new(2000.4, 10.0, 2.0, 30.0)),
                    why: json!("w"),
                    offscreen: false,
                },
                disp.id,
            ),
            (
                Hit {
                    i: 1,
                    kind: "related".into(),
                    rect: Some(Rect::new(10.0, 10.0, 20.0, 20.0)),
                    why: Value::Null,
                    offscreen: false,
                },
                7,
            ),
        ];
        let p = draw_payload(&hits, 0, Some(&(disp.id, "chip".into())), &disp);
        assert_eq!(p["hits"].as_array().unwrap().len(), 1);
        assert_eq!(p["hits"][0]["x"], json!(80.0));
        assert_eq!(p["hits"][0]["w"], json!(4.0));
        assert_eq!(p["current"], json!(0));
        assert_eq!(p["frame"]["chip"], "chip");
        let p = draw_payload(&hits, 1, None, &disp);
        assert_eq!(p["current"], json!(-1));
        assert_eq!(p["frame"], Value::Null);
    }

    #[test]
    fn hits_land_on_the_display_their_pixels_are_on() {
        // Find's hits arrive in physical pixels and are mapped through the
        // display holding them; in the canonical layout the DIP rect then
        // names that same display, even at mixed scales (B-005).
        let d = screen::tests::mixed();
        let rects = [
            screen::tests::pr(1500, 200, 300, 40),
            screen::tests::pr(1900, 200, 60, 40),
            screen::tests::pr(1925, 10, 90, 30),
            screen::tests::pr(4000, 1300, 200, 100),
        ];
        for p in rects {
            let dip = screen::screen_to_dip_rect(&d, p);
            let want = screen::matching_phys(&d, &p).id;
            assert_eq!(screen::matching(&d, &dip).id, want, "{p:?}");
        }
    }
}
