//! The backdrop behind the glass (lib/backdrop.js, PARITY GLASS-01..09).
//!
//! A transparent window has nothing behind it inside the webview, so
//! backdrop-filter has nothing to blur. Every one of our windows is excluded
//! from capture, so a small frame of the display is the desktop exactly as it
//! is behind us, without us in it. Each glass page gets that frame and draws
//! it, blurred, inside its own shapes (renderer/glass.js).
//!
//! The frame is a sixth of the display, JPEG q72, held only in memory and
//! served to our own pages over `aeval-frame://` (ARCHITECTURE D4); it is
//! never written anywhere. 2 Hz while something you read or type into is up,
//! every 5 s while only the orb is, none when nothing is visible.
//!
//! Tiers: 'live' draws the frame, 'solid' the same shapes on an opaque fill.
//! Solid when the user chose it, when our windows cannot be hidden from
//! capture, in high contrast, and after three capture failures (retried).
//! REQUESTS.md 5: the Windows transparency-effects switch no longer forces
//! solid. The glass is drawn by us from a captured frame, not by DWM, so it
//! does not depend on that setting; the renderer's own reasons
//! (prefers-reduced-transparency, forced-colors) still apply.

#![allow(dead_code)]

pub mod contrast;

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

use crate::capture::{self, encode, screen, Rect, Rgb};

pub const SCALE: f64 = 6.0;
pub const FAST_MS: u64 = 500;
pub const SLOW_MS: u64 = 5000;
pub const RETRY_MS: u64 = 30000;
pub const FRAME_QUALITY: u8 = 72;
pub const LOUPE_QUALITY: u8 = 90;
pub const SCHEME: &str = "aeval-frame";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pace {
    Fast,
    Slow,
}

type Hook<T> = Arc<dyn Fn() -> T + Send + Sync>;

/// What main.js passed to `backdrop.init`.
#[derive(Clone)]
pub struct Hooks {
    pub pace: Hook<Pace>,
    /// `'auto'` or `'solid'` (the stored surface setting).
    pub user_surface: Hook<String>,
}

struct Fixture {
    name: String,
    small: Arc<Vec<u8>>,
    full: Arc<Vec<u8>>,
    full_image: Rgb,
    display: screen::Display,
}

#[derive(Default)]
struct Stats {
    frames: u64,
    wall: Vec<f64>,
    encode: Vec<f64>,
    last_error: String,
}

struct Inner {
    app: Option<AppHandle>,
    hooks: Option<Hooks>,
    glass: BTreeSet<String>,
    paused: u32,
    fails: u32,
    high_contrast: bool,
    last_tier: Option<&'static str>,
    fixture: Option<Fixture>,
    stats: Stats,
    due: Option<Instant>,
    busy: bool,
}

struct Backdrop {
    inner: Mutex<Inner>,
    wake: Condvar,
    /// Newest bytes per key (`<display id>`, `loupe`, `fixture-full`).
    frames: RwLock<HashMap<String, Arc<Vec<u8>>>>,
    seq: AtomicU64,
}

fn me() -> &'static Backdrop {
    static B: OnceLock<Backdrop> = OnceLock::new();
    B.get_or_init(|| Backdrop {
        inner: Mutex::new(Inner {
            app: None,
            hooks: None,
            glass: BTreeSet::new(),
            paused: 0,
            fails: 0,
            high_contrast: false,
            last_tier: None,
            fixture: None,
            stats: Stats::default(),
            due: None,
            busy: false,
        }),
        wake: Condvar::new(),
        frames: RwLock::new(HashMap::new()),
        seq: AtomicU64::new(0),
    })
}

fn lock() -> std::sync::MutexGuard<'static, Inner> {
    me().inner.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn high_contrast_on() -> bool {
    let mut hc = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            hc.cbSize,
            Some(&mut hc as *mut HIGHCONTRASTW as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    ok.is_ok() && hc.dwFlags.contains(HCF_HIGHCONTRASTON)
}

/// The tier's inputs, so the rule can be unit-tested.
#[derive(Clone, Copy, Debug, Default)]
pub struct TierInputs {
    pub surface_solid: bool,
    pub can_hide: bool,
    pub high_contrast: bool,
    pub fails: u32,
    pub fixture: bool,
}

pub fn reasons_for(i: TierInputs) -> Vec<&'static str> {
    let mut r = Vec::new();
    if i.surface_solid {
        r.push("setting");
    }
    if !i.can_hide {
        r.push("capture-visible");
    }
    if i.high_contrast {
        r.push("high-contrast");
    }
    if i.fails >= 3 && !i.fixture {
        r.push("capture-failed");
    }
    r
}

fn inputs(g: &Inner) -> TierInputs {
    let solid = g
        .hooks
        .as_ref()
        .map(|h| (h.user_surface)() == "solid")
        .unwrap_or(false);
    TierInputs {
        surface_solid: solid,
        can_hide: capture::can_hide_from_capture(),
        high_contrast: g.high_contrast,
        fails: g.fails,
        fixture: g.fixture.is_some(),
    }
}

pub fn reasons() -> Vec<&'static str> {
    reasons_for(inputs(&lock()))
}

pub fn tier() -> &'static str {
    if reasons().is_empty() {
        "live"
    } else {
        "solid"
    }
}

fn tier_of(g: &Inner) -> (&'static str, Vec<&'static str>) {
    let r = reasons_for(inputs(g));
    (if r.is_empty() { "live" } else { "solid" }, r)
}

/// The URL a page loads a frame from. WebView2 serves custom schemes as
/// `http://<scheme>.localhost/`; the sequence number defeats caching.
pub fn frame_url(key: &str) -> String {
    let seq = me().seq.fetch_add(1, Ordering::Relaxed);
    format!("http://{SCHEME}.localhost/{key}/{seq}.jpg")
}

fn store(key: &str, bytes: Arc<Vec<u8>>) -> String {
    if let Ok(mut f) = me().frames.write() {
        f.insert(key.to_string(), bytes);
    }
    frame_url(key)
}

/// The `aeval-frame` protocol handler (registered in lib.rs): the newest
/// bytes for the key in the path, or 404. Frames are never cached.
pub fn serve(path: &str) -> tauri::http::Response<Vec<u8>> {
    let key = path.trim_start_matches('/').split('/').next().unwrap_or("");
    let bytes = me().frames.read().ok().and_then(|f| f.get(key).cloned());
    let builder = tauri::http::Response::builder()
        .header("Cache-Control", "no-store")
        .header("Access-Control-Allow-Origin", "*");
    match bytes {
        Some(b) => builder
            .status(200)
            .header("Content-Type", "image/jpeg")
            .body(b.as_ref().clone()),
        None => builder.status(404).body(Vec::new()),
    }
    .unwrap_or_else(|_| tauri::http::Response::new(Vec::new()))
}

fn default_hooks(app: AppHandle) -> Hooks {
    // Until agent (a) passes the overlay mode: fast while any glass window
    // other than the orb's overlay is visible.
    let a = app.clone();
    Hooks {
        pace: Arc::new(move || {
            let fast = a
                .webview_windows()
                .iter()
                .any(|(label, w)| label != "overlay" && w.is_visible().unwrap_or(false));
            if fast {
                Pace::Fast
            } else {
                Pace::Slow
            }
        }),
        user_surface: Arc::new(|| "auto".to_string()),
    }
}

/// Starts the frame loop. Safe to call once; later calls only replace the app.
pub fn init(app: AppHandle) {
    let first = {
        let mut g = lock();
        let first = g.app.is_none();
        if g.hooks.is_none() {
            g.hooks = Some(default_hooks(app.clone()));
        }
        g.app = Some(app);
        g.high_contrast = high_contrast_on();
        first
    };
    if first {
        std::thread::Builder::new()
            .name("aeval-backdrop".into())
            .spawn(run_loop)
            .ok();
    }
    schedule(Some(0));
}

pub fn set_hooks(h: Hooks) {
    lock().hooks = Some(h);
    broadcast_tier();
}

/// `schedule(ms)`: `Some(0)` runs now, replacing a pending tick; anything
/// else only arms the timer when none is pending (backdrop.js semantics).
fn schedule(ms: Option<u64>) {
    let mut g = lock();
    if g.due.is_some() && ms != Some(0) {
        return;
    }
    let pace = match ms {
        Some(v) => v,
        None => match g.hooks.as_ref().map(|h| (h.pace)()) {
            Some(Pace::Slow) => SLOW_MS,
            _ => FAST_MS,
        },
    };
    g.due = Some(Instant::now() + Duration::from_millis(pace));
    me().wake.notify_all();
}

fn run_loop() {
    loop {
        {
            let mut g = lock();
            loop {
                match g.due {
                    Some(t) if Instant::now() >= t => break,
                    Some(t) => {
                        let wait = t.saturating_duration_since(Instant::now());
                        g = me().wake.wait_timeout(g, wait).map(|r| r.0).unwrap_or_else(|e| e.into_inner().0);
                    }
                    None => {
                        g = me().wake.wait(g).unwrap_or_else(|e| e.into_inner());
                    }
                }
            }
            g.due = None;
        }
        let _ = std::panic::catch_unwind(tick);
    }
}

/// Labels of glass pages that are visible now.
fn visible_glass() -> (Option<AppHandle>, Vec<(String, tauri::WebviewWindow)>) {
    let (app, labels) = {
        let g = lock();
        (g.app.clone(), g.glass.clone())
    };
    let Some(app) = app else {
        return (None, Vec::new());
    };
    let shown = labels
        .into_iter()
        .filter_map(|l| app.get_webview_window(&l).map(|w| (l, w)))
        .filter(|(_, w)| w.is_visible().unwrap_or(false))
        .collect();
    (Some(app), shown)
}

fn display_for(win: &tauri::WebviewWindow, list: &[screen::Display]) -> screen::Display {
    let pos = win.outer_position().ok();
    let size = win.outer_size().ok();
    match (pos, size) {
        (Some(p), Some(s)) => screen::matching_phys(
            list,
            &screen::PxRect {
                x: p.x,
                y: p.y,
                width: s.width as i32,
                height: s.height as i32,
            },
        ),
        _ => screen::primary(list),
    }
}

/// One capture of every display at a sixth of its DIP size, JPEG q72.
/// JPEG bytes per display id.
pub type Frames = HashMap<u32, Arc<Vec<u8>>>;

pub fn grab_all() -> Result<(Frames, f64, f64), String> {
    let list = screen::all_displays();
    let mut out = HashMap::new();
    let mut wall = 0.0;
    let mut enc = 0.0;
    let mut last_err = String::new();
    for d in &list {
        let t0 = Instant::now();
        let frame = match capture::grab(d) {
            Ok(f) => f,
            Err(e) => {
                last_err = e;
                continue;
            }
        };
        let t1 = Instant::now();
        let small = frame.rgb().resize(
            (d.bounds.width / SCALE).round().max(1.0) as u32,
            (d.bounds.height / SCALE).round().max(1.0) as u32,
        );
        if let Some(j) = encode::jpeg(&small, FRAME_QUALITY) {
            out.insert(d.id, Arc::new(j));
        }
        wall += (t1 - t0).as_secs_f64() * 1000.0;
        enc += t1.elapsed().as_secs_f64() * 1000.0;
    }
    if out.is_empty() {
        return Err(if last_err.is_empty() { "no screen sources".into() } else { last_err });
    }
    Ok((out, wall, enc))
}

fn send_frame(app: &AppHandle, label: &str, img: &str, full: Option<&str>, display: Rect) {
    let (tier, reasons, fixture) = {
        let g = lock();
        let (t, r) = tier_of(&g);
        (t, r, g.fixture.as_ref().map(|f| f.name.clone()).unwrap_or_default())
    };
    let _ = app.emit_to(
        label,
        "glass:frame",
        json!({
            "img": img,
            "full": full,
            "display": display,
            "tier": tier,
            "reasons": reasons,
            "fixture": fixture,
        }),
    );
}

fn push(v: &mut Vec<f64>, x: f64) {
    v.push(x);
    if v.len() > 200 {
        v.remove(0);
    }
}

fn tick() {
    // Theme changes (high contrast on or off) are noticed here.
    let hc = high_contrast_on();
    let changed = {
        let mut g = lock();
        let c = g.high_contrast != hc;
        g.high_contrast = hc;
        c
    };
    if changed {
        broadcast_tier();
    }
    {
        let g = lock();
        if g.busy || g.paused > 0 {
            drop(g);
            return schedule(None);
        }
    }
    let (app, shown) = visible_glass();
    let Some(app) = app else {
        return schedule(None);
    };
    let solid = tier() == "solid";
    if shown.is_empty() || solid {
        // Solid needs no frames; capture failures are retried slowly in case
        // whatever broke it has passed.
        let retry = {
            let mut g = lock();
            if g.fails >= 3 && g.fixture.is_none() && !shown.is_empty() {
                g.fails = 2;
            }
            g.fails >= 3
        };
        return schedule(if retry { Some(RETRY_MS) } else { None });
    }
    lock().busy = true;
    let fixture = {
        let g = lock();
        g.fixture
            .as_ref()
            .map(|f| (f.small.clone(), f.full.clone(), f.display.bounds))
    };
    if let Some((small, full, bounds)) = fixture {
        let img = store("fixture-small", small);
        let full_url = store("fixture-full", full);
        for (label, _) in &shown {
            send_frame(&app, label, &img, Some(&full_url), bounds);
        }
    } else {
        match grab_all() {
            Ok((frames, wall, enc)) => {
                {
                    let mut g = lock();
                    g.stats.frames += 1;
                    push(&mut g.stats.wall, wall);
                    push(&mut g.stats.encode, enc);
                }
                let list = screen::all_displays();
                let mut urls: HashMap<u32, String> = HashMap::new();
                for (label, w) in &shown {
                    let d = display_for(w, &list);
                    let id = if frames.contains_key(&d.id) {
                        d.id
                    } else {
                        *frames.keys().next().expect("non-empty")
                    };
                    let url = urls
                        .entry(id)
                        .or_insert_with(|| store(&id.to_string(), frames[&id].clone()))
                        .clone();
                    send_frame(&app, label, &url, None, d.bounds);
                }
                let was_failed = {
                    let mut g = lock();
                    let was = tier_of(&g).0;
                    let had = g.fails > 0;
                    g.fails = 0;
                    had && was != tier_of(&g).0
                };
                if was_failed {
                    broadcast_tier();
                }
            }
            Err(e) => {
                let third = {
                    let mut g = lock();
                    g.stats.last_error = e;
                    g.fails += 1;
                    g.fails == 3
                };
                if third {
                    broadcast_tier();
                }
            }
        }
    }
    lock().busy = false;
    schedule(None);
}

/// Tier and reasons to every glass window (on change, on load, on theme).
pub fn broadcast_tier() {
    let (app, labels, tier, reasons) = {
        let mut g = lock();
        let (t, r) = tier_of(&g);
        g.last_tier = Some(t);
        (g.app.clone(), g.glass.clone(), t, r)
    };
    if let Some(app) = app {
        for l in labels {
            let _ = app.emit_to(l.as_str(), "glass:tier", json!({ "tier": tier, "reasons": reasons }));
        }
    }
    schedule(Some(0));
}

/// `glass:hello`: the page loaded glass.js and wants frames. It gets the
/// tier now (the load-time broadcast) and a frame at once.
pub fn hello(label: &str) {
    let (app, tier, reasons) = {
        let mut g = lock();
        g.glass.insert(label.to_string());
        let (t, r) = tier_of(&g);
        (g.app.clone(), t, r)
    };
    if let Some(app) = app {
        let _ = app.emit_to(label, "glass:tier", json!({ "tier": tier, "reasons": reasons }));
    }
    schedule(Some(0));
}

/// Something changed that the windows should see now (a window was shown).
pub fn kick() {
    schedule(Some(0));
}

pub fn forget(label: &str) {
    lock().glass.remove(label);
}

/// While content protection is off for a live photograph, no frames.
pub fn pause() {
    lock().paused += 1;
}

pub fn resume() {
    {
        let mut g = lock();
        g.paused = g.paused.saturating_sub(1);
    }
    schedule(Some(0));
}

/// A rendered page stands in for the desktop (the visual self-check,
/// GLASS-05): lib/backdrop-fixture.html at the primary display's size,
/// photographed through the real capture path, then closed. `None` returns
/// to the live desktop. Blocking; never call it on the event loop thread.
pub fn use_fixture(name: Option<&str>) -> Result<(), String> {
    let Some(name) = name else {
        lock().fixture = None;
        broadcast_tier();
        return Ok(());
    };
    let app = lock().app.clone().ok_or("backdrop not initialised")?;
    let list = screen::all_displays();
    let display = screen::primary(&list);
    let shot = render_fixture(&app, name, &display)?;
    let (w, h) = (display.bounds.width.round() as u32, display.bounds.height.round() as u32);
    let full = shot.resize(w, h);
    let small = shot.resize(
        (display.bounds.width / SCALE).round() as u32,
        (display.bounds.height / SCALE).round() as u32,
    );
    let small_j = encode::jpeg(&small, 80).ok_or("encode small")?;
    let full_j = encode::jpeg(&full, 88).ok_or("encode full")?;
    lock().fixture = Some(Fixture {
        name: name.to_string(),
        small: Arc::new(small_j),
        full: Arc::new(full_j),
        full_image: full,
        display,
    });
    broadcast_tier();
    Ok(())
}

/// Shows the fixture page full-screen on `display` (an ordinary, capturable
/// window: everything of ours stays excluded), captures it with the
/// production pipeline, and closes it.
pub fn render_fixture(app: &AppHandle, kind: &str, display: &screen::Display) -> Result<Rgb, String> {
    let label = "backdrop-fixture";
    if let Some(old) = app.get_webview_window(label) {
        let _ = old.destroy();
    }
    let url = format!("http://tauri.localhost/lib/backdrop-fixture.html?kind={kind}");
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let tx = Mutex::new(Some(tx));
    let win = tauri::WebviewWindowBuilder::new(
        app,
        label,
        tauri::WebviewUrl::External(url.parse().map_err(|e| format!("url: {e}"))?),
    )
    .title("backdrop fixture")
    .decorations(false)
    .shadow(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .focused(false)
    .visible(false)
    .data_directory(crate::paths::webview_data_dir())
    .on_page_load(move |_, p| {
        if matches!(p.event(), tauri::webview::PageLoadEvent::Finished) {
            if let Some(tx) = tx.lock().ok().and_then(|mut t| t.take()) {
                let _ = tx.send(());
            }
        }
    })
    .build()
    .map_err(|e| format!("fixture window: {e}"))?;
    crate::desktop::win::place(&win, display.phys);
    let _ = win.show();
    let loaded = rx.recv_timeout(Duration::from_secs(15)).is_ok();
    std::thread::sleep(Duration::from_millis(if loaded { 400 } else { 0 }));
    let result = if loaded {
        // Give DWM a frame or two after the show, then read it back.
        std::thread::sleep(Duration::from_millis(300));
        capture::grab(display).map(|f| f.rgb())
    } else {
        Err("fixture page did not load".into())
    };
    let _ = win.destroy();
    result
}

/// A crop of what is behind a DIP rect at full resolution, for the Find
/// lens's magnifier. One capture per current-hit change, never on a timer.
pub fn crop(rect: Rect) -> Option<Vec<u8>> {
    {
        let g = lock();
        if let Some(f) = &g.fixture {
            let b = f.display.bounds;
            let img = &f.full_image;
            let x = ((rect.x - b.x).round().max(0.0) as u32).min(img.width.saturating_sub(1));
            let y = ((rect.y - b.y).round().max(0.0) as u32).min(img.height.saturating_sub(1));
            let w = (rect.width.round().max(1.0) as u32).min(img.width - x);
            let h = (rect.height.round().max(1.0) as u32).min(img.height - y);
            return encode::jpeg(&img.crop(x, y, w, h), LOUPE_QUALITY);
        }
    }
    if tier() == "solid" {
        return None;
    }
    let cut = capture::crop_dip(rect)?;
    encode::jpeg(&cut, LOUPE_QUALITY)
}

/// The loupe as a URL the lens page can load.
pub fn crop_url(rect: Rect) -> Option<String> {
    crop(rect).map(|b| store("loupe", Arc::new(b)))
}

/// The fixture's full picture, for photographing a window that draws no glass.
pub fn fixture_url() -> String {
    let full = lock().fixture.as_ref().map(|f| f.full.clone());
    match full {
        Some(b) => store("fixture-full", b),
        None => String::new(),
    }
}

fn pct(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    s[((p / 100.0) * s.len() as f64).floor().min((s.len() - 1) as f64) as usize]
}

/// The snapCost gate: n captures back to back. Wall time, the encode, and
/// the longest the event loop thread waited to run a no-op while each
/// capture ran (capture runs off that thread, so this should stay tiny).
pub fn measure(n: usize) -> Value {
    let app = lock().app.clone();
    let mut wall = Vec::new();
    let mut enc = Vec::new();
    let mut block = Vec::new();
    for _ in 0..n {
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worst = Arc::new(Mutex::new(0f64));
        let probe = app.clone().map(|a| {
            let stop = stop.clone();
            let worst = worst.clone();
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let t = Instant::now();
                    if a.run_on_main_thread(move || {
                        let _ = tx.send(());
                    })
                    .is_err()
                    {
                        break;
                    }
                    let _ = rx.recv_timeout(Duration::from_secs(2));
                    let ms = t.elapsed().as_secs_f64() * 1000.0;
                    if let Ok(mut w) = worst.lock() {
                        *w = w.max(ms);
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
            })
        });
        let g = grab_all();
        stop.store(true, Ordering::Relaxed);
        if let Some(p) = probe {
            let _ = p.join();
        }
        if let Ok((_, w, e)) = g {
            wall.push(w + e);
            enc.push(e);
            block.push(worst.lock().map(|w| *w).unwrap_or(0.0));
        }
    }
    json!({
        "n": n,
        "wallP50": pct(&wall, 50.0),
        "wallP95": pct(&wall, 95.0),
        "encodeP95": pct(&enc, 95.0),
        "blockP95": pct(&block, 95.0),
    })
}

pub fn status() -> Value {
    let g = lock();
    let (t, r) = tier_of(&g);
    json!({
        "tier": t,
        "reasons": r,
        "windows": g.glass.len(),
        "frames": g.stats.frames,
        "wallP95": pct(&g.stats.wall, 95.0),
        "encodeP95": pct(&g.stats.encode, 95.0),
        "fails": g.fails,
        "lastError": g.stats.last_error,
        "fixture": g.fixture.as_ref().map(|f| f.name.clone()).unwrap_or_default(),
        "backend": capture::last_backend(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparency_setting_is_not_a_reason() {
        // REQUESTS.md 5: only these four can force solid.
        let all = reasons_for(TierInputs {
            surface_solid: true,
            can_hide: false,
            high_contrast: true,
            fails: 3,
            fixture: false,
        });
        assert_eq!(all, vec!["setting", "capture-visible", "high-contrast", "capture-failed"]);
        let none = reasons_for(TierInputs {
            can_hide: true,
            ..Default::default()
        });
        assert!(none.is_empty());
    }

    #[test]
    fn fixture_hides_capture_failures() {
        let r = reasons_for(TierInputs {
            can_hide: true,
            fails: 5,
            fixture: true,
            ..Default::default()
        });
        assert!(r.is_empty());
        let r = reasons_for(TierInputs {
            can_hide: true,
            fails: 2,
            ..Default::default()
        });
        assert!(r.is_empty());
    }

    #[test]
    fn frames_are_served_by_key_and_never_cached() {
        let url = store("test-key", Arc::new(vec![1, 2, 3]));
        assert!(url.starts_with("http://aeval-frame.localhost/test-key/"));
        let path = url.trim_start_matches("http://aeval-frame.localhost");
        let r = serve(path);
        assert_eq!(r.status(), 200);
        assert_eq!(r.body(), &vec![1, 2, 3]);
        assert_eq!(r.headers()["Cache-Control"], "no-store");
        assert_eq!(serve("/missing/1.jpg").status(), 404);
    }

    #[test]
    fn percentiles() {
        assert_eq!(pct(&[], 95.0), 0.0);
        let v: Vec<f64> = (1..=100).map(|x| x as f64).collect();
        assert_eq!(pct(&v, 50.0), 51.0);
        assert_eq!(pct(&v, 95.0), 96.0);
    }
}
