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
//! Solid when our windows cannot be hidden from capture, in high contrast,
//! and after three capture failures (retried every 30 s, and the pages hear
//! 'live' again when a capture succeeds). Choosing solid is `glassIntensity`
//! 0, drawn by the page; the stored `surface` is kept for rollback only
//! (B-035).
//! REQUESTS.md 5: the Windows transparency-effects switch no longer forces
//! solid. The glass is drawn by us from a captured frame, not by DWM, so it
//! does not depend on that setting; the renderer's own reasons
//! (prefers-reduced-transparency, forced-colors) still apply.

pub mod contrast;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock, Weak};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

use crate::capture::{self, encode, screen, Frame, Rect, Rgb};

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

#[derive(Default)]
struct Inner {
    app: Option<AppHandle>,
    hooks: Option<Hooks>,
    glass: BTreeSet<String>,
    paused: u32,
    fails: u32,
    high_contrast: bool,
    /// The tier the pages were last told.
    last_tier: Option<&'static str>,
    fixture: Option<Fixture>,
    stats: Stats,
    due: Option<Instant>,
    busy: bool,
    /// While capture failures hold the glass solid: when to try again.
    retry_at: Option<Instant>,
    /// Per display, the last frame encoded and its JPEG.
    last_jpeg: HashMap<u32, (Weak<Frame>, Arc<Vec<u8>>)>,
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
        inner: Mutex::new(Inner::default()),
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
    /// The stored `surface` is 'solid'. Reported by `status`, never a
    /// reason: `glassIntensity` 0 is the solid path, so a profile migrated
    /// as solid follows the glass slider (B-035, NEW-12).
    pub surface_solid: bool,
    pub can_hide: bool,
    pub high_contrast: bool,
    pub fails: u32,
    pub fixture: bool,
}

pub fn reasons_for(i: TierInputs) -> Vec<&'static str> {
    let mut r = Vec::new();
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

fn tier_for(i: TierInputs) -> (&'static str, Vec<&'static str>) {
    let r = reasons_for(i);
    (if r.is_empty() { "live" } else { "solid" }, r)
}

fn tier_of(g: &Inner) -> (&'static str, Vec<&'static str>) {
    tier_for(inputs(g))
}

/// The `glass:tier` payload.
fn tier_payload(i: TierInputs, intensity: f64) -> Value {
    let (tier, reasons) = tier_for(i);
    json!({ "tier": tier, "reasons": reasons, "intensity": intensity })
}

/// What a tick does about frames (B-111).
#[derive(Debug, PartialEq, Eq)]
enum Plan {
    /// Nothing visible, or solid for a reason a capture cannot change.
    Idle,
    /// Capture failures alone hold the glass solid; the next try is due in
    /// this long.
    Wait(Duration),
    /// Make and send frames.
    Frames,
}

fn plan(i: TierInputs, shown: bool, retry_at: Option<Instant>, now: Instant) -> Plan {
    let reasons = reasons_for(i);
    if !shown {
        return Plan::Idle;
    }
    if reasons.is_empty() {
        return Plan::Frames;
    }
    if reasons != ["capture-failed"] {
        return Plan::Idle;
    }
    match retry_at {
        Some(t) if t > now => Plan::Wait(t - now),
        _ => Plan::Frames,
    }
}

/// After a capture went through: whether the pages must hear the tier
/// again, because the last one they were told is not the one now (B-111).
fn must_rebroadcast(last_told: Option<&str>, now: &str) -> bool {
    last_told.is_some_and(|t| t != now)
}

/// Marks a tick running and clears the mark however the tick ends, a panic
/// included, so one bad tick cannot freeze the glass (B-111).
struct Busy;

impl Busy {
    fn take() -> Option<Busy> {
        let mut g = lock();
        if g.busy {
            return None;
        }
        g.busy = true;
        Some(Busy)
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        lock().busy = false;
    }
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

/// The only origin our pages load from (Tauri's custom protocol on Windows).
pub const APP_ORIGIN: &str = "http://tauri.localhost";

/// The `aeval-frame` protocol handler (registered in lib.rs): the newest
/// bytes for the key in the path, or 404. Frames are never cached. The
/// protocol handler uses `serve_request`; this stays for the tests.
#[allow(dead_code)]
pub fn serve(path: &str) -> tauri::http::Response<Vec<u8>> {
    serve_from(path, None)
}

/// `serve` for a whole request, so its `Origin` is checked too (B-112).
pub fn serve_request(req: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    let origin = req.headers().get("Origin").and_then(|v| v.to_str().ok());
    serve_from(req.uri().path(), origin)
}

/// The screen is readable only by our own pages (B-112): CORS names the app
/// origin, never `*`, and a request from any other origin is refused.
fn serve_from(path: &str, origin: Option<&str>) -> tauri::http::Response<Vec<u8>> {
    let builder = tauri::http::Response::builder()
        .header("Cache-Control", "no-store")
        .header("Access-Control-Allow-Origin", APP_ORIGIN)
        .header("Vary", "Origin");
    if origin.is_some_and(|o| o != APP_ORIGIN) {
        return builder
            .status(403)
            .body(Vec::new())
            .unwrap_or_else(|_| tauri::http::Response::new(Vec::new()));
    }
    let key = path.trim_start_matches('/').split('/').next().unwrap_or("");
    let bytes = me().frames.read().ok().and_then(|f| f.get(key).cloned());
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
    // The pace hook reads window state, which can wait on the main thread;
    // it runs with the lock released so the main thread can always take it.
    let hooks = {
        let g = lock();
        if g.due.is_some() && ms != Some(0) {
            return;
        }
        g.hooks.clone()
    };
    let pace = match ms {
        Some(v) => v,
        None => match hooks.map(|h| (h.pace)()) {
            Some(Pace::Slow) => SLOW_MS,
            _ => FAST_MS,
        },
    };
    let mut g = lock();
    if g.due.is_some() && ms != Some(0) {
        return;
    }
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
                        g = me()
                            .wake
                            .wait_timeout(g, wait)
                            .map(|r| r.0)
                            .unwrap_or_else(|e| e.into_inner().0);
                    }
                    None => {
                        g = me().wake.wait(g).unwrap_or_else(|e| e.into_inner());
                    }
                }
            }
            g.due = None;
        }
        if std::panic::catch_unwind(tick).is_err() {
            // A tick that panicked never armed the next one (B-111).
            schedule(None);
        }
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
    grab_displays(&screen::all_displays())
}

/// One display's backdrop image. A frame DXGI says has not changed is neither
/// converted nor encoded again (B-030).
pub fn encode_frame(d: &screen::Display, frame: &Arc<Frame>) -> Option<Arc<Vec<u8>>> {
    let last = lock().last_jpeg.get(&d.id).cloned();
    if let Some((seen, jpeg)) = last {
        if seen.upgrade().is_some_and(|f| Arc::ptr_eq(&f, frame)) {
            return Some(jpeg);
        }
    }
    let small = frame.scaled(
        (d.bounds.width / SCALE).round().max(1.0) as u32,
        (d.bounds.height / SCALE).round().max(1.0) as u32,
    );
    let jpeg = Arc::new(encode::jpeg(&small, FRAME_QUALITY)?);
    let seen = Arc::downgrade(frame);
    lock().last_jpeg.insert(d.id, (seen, jpeg.clone()));
    Some(jpeg)
}

/// The displays holding a visible glass window, each once (B-030).
fn hosting<T>(on: &[(T, screen::Display)]) -> Vec<screen::Display> {
    let mut seen = HashSet::new();
    on.iter()
        .filter(|(_, d)| seen.insert(d.id))
        .map(|(_, d)| d.clone())
        .collect()
}

fn grab_displays(list: &[screen::Display]) -> Result<(Frames, f64, f64), String> {
    let mut out = HashMap::new();
    let mut wall = 0.0;
    let mut enc = 0.0;
    let mut last_err = String::new();
    for d in list {
        let t0 = Instant::now();
        let frame = match capture::grab(d) {
            Ok(f) => f,
            Err(e) => {
                last_err = e;
                continue;
            }
        };
        let t1 = Instant::now();
        if let Some(j) = encode_frame(d, &frame) {
            out.insert(d.id, j);
        }
        wall += (t1 - t0).as_secs_f64() * 1000.0;
        enc += t1.elapsed().as_secs_f64() * 1000.0;
    }
    if out.is_empty() {
        return Err(if last_err.is_empty() {
            "no screen sources".into()
        } else {
            last_err
        });
    }
    Ok((out, wall, enc))
}

fn send_frame(app: &AppHandle, label: &str, img: &str, full: Option<&str>, display: Rect) {
    let (tier, reasons, fixture) = {
        let g = lock();
        let (t, r) = tier_of(&g);
        (
            t,
            r,
            g.fixture
                .as_ref()
                .map(|f| f.name.clone())
                .unwrap_or_default(),
        )
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
            "intensity": intensity(),
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
    if lock().paused > 0 {
        return schedule(None);
    }
    let Some(busy) = Busy::take() else {
        return schedule(None);
    };
    let (app, shown) = visible_glass();
    let Some(app) = app else {
        return schedule(None);
    };
    let (i, retry_at) = {
        let g = lock();
        (inputs(&g), g.retry_at)
    };
    // Solid needs no frames. Capture failures are retried every 30 s, not at
    // the fast pace, in case whatever broke capture has passed.
    match plan(i, !shown.is_empty(), retry_at, Instant::now()) {
        Plan::Idle => return schedule(None),
        Plan::Wait(d) => return schedule(Some(d.as_millis() as u64)),
        Plan::Frames => {}
    }
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
        let list = screen::all_displays();
        let on: Vec<(&str, screen::Display)> = shown
            .iter()
            .map(|(label, w)| (label.as_str(), display_for(w, &list)))
            .collect();
        match grab_displays(&hosting(&on)) {
            Ok((frames, wall, enc)) => {
                {
                    let mut g = lock();
                    g.stats.frames += 1;
                    push(&mut g.stats.wall, wall);
                    push(&mut g.stats.encode, enc);
                }
                let mut urls: HashMap<u32, String> = HashMap::new();
                for (label, d) in sendable(&frames, on) {
                    let url = urls
                        .entry(d.id)
                        .or_insert_with(|| store(&d.id.to_string(), frames[&d.id].clone()))
                        .clone();
                    send_frame(&app, label, &url, None, d.bounds);
                }
                let recovered = {
                    let mut g = lock();
                    g.fails = 0;
                    g.retry_at = None;
                    must_rebroadcast(g.last_tier, tier_of(&g).0)
                };
                if recovered {
                    broadcast_tier();
                }
            }
            Err(e) => {
                let third = {
                    let mut g = lock();
                    g.stats.last_error = e;
                    g.fails += 1;
                    if g.fails >= 3 {
                        g.retry_at = Some(Instant::now() + Duration::from_millis(RETRY_MS));
                    }
                    g.fails == 3
                };
                if third {
                    broadcast_tier();
                }
            }
        }
    }
    drop(busy);
    schedule(None);
}

/// The windows that get a frame this tick: those whose display has one. A
/// window whose display has none keeps its last frame rather than another
/// display's at the wrong bounds (B-113).
fn sendable<T>(frames: &Frames, on: Vec<(T, screen::Display)>) -> Vec<(T, screen::Display)> {
    on.into_iter()
        .filter(|(_, d)| frames.contains_key(&d.id))
        .collect()
}

/// Whether the backdrop is at its 5 s pace (`backdropIdle`).
pub fn slow_paced() -> bool {
    // The hook runs with the lock released, as in `schedule`.
    let hooks = lock().hooks.clone();
    hooks.is_some_and(|h| (h.pace)() == Pace::Slow)
}

/// Tier and reasons to every glass window (on change, on load, on theme).
pub fn broadcast_tier() {
    let (app, labels, payload) = {
        let mut g = lock();
        let i = inputs(&g);
        g.last_tier = Some(tier_for(i).0);
        let payload = tier_payload(i, intensity());
        (g.app.clone(), g.glass.clone(), payload)
    };
    if let Some(app) = app {
        for l in labels {
            let _ = app.emit_to(l.as_str(), "glass:tier", payload.clone());
        }
    }
    schedule(Some(0));
}

/// Stored `glassIntensity` (0..1, default 0.75), as bits. The page renders
/// glass from it (`--glass-k`, ARCHITECTURE.md 10); settings owns the value
/// and calls `set_intensity`, which rebroadcasts the tier.
static INTENSITY: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0x3FE8_0000_0000_0000);

pub fn intensity() -> f64 {
    f64::from_bits(INTENSITY.load(std::sync::atomic::Ordering::Relaxed))
}

pub fn set_intensity(k: f64) {
    let k = if k.is_finite() {
        (k.clamp(0.0, 1.0) * 100.0).round() / 100.0
    } else {
        crate::store::DEFAULT_GLASS_INTENSITY
    };
    INTENSITY.store(k.to_bits(), std::sync::atomic::Ordering::Relaxed);
    broadcast_tier();
}

/// `glass:hello`: the page loaded glass.js and wants frames. It gets the
/// tier now (the load-time broadcast) and a frame at once.
pub fn hello(label: &str) {
    let (app, payload) = {
        let mut g = lock();
        g.glass.insert(label.to_string());
        let i = inputs(&g);
        g.last_tier = Some(tier_for(i).0);
        (g.app.clone(), tier_payload(i, intensity()))
    };
    if let Some(app) = app {
        let _ = app.emit_to(label, "glass:tier", payload);
    }
    schedule(Some(0));
}

/// Something changed that the windows should see now (a window was shown).
// backdrop.js export; Called by the smoke and shot harnesses (agent e), not merged yet.
#[allow(dead_code)]
pub fn kick() {
    schedule(Some(0));
}

pub fn forget(label: &str) {
    lock().glass.remove(label);
}

/// While content protection is off for a live photograph, no frames.
// backdrop.js export; Called by the smoke and shot harnesses (agent e), not merged yet.
#[allow(dead_code)]
pub fn pause() {
    lock().paused += 1;
}

// backdrop.js export; Called by the smoke and shot harnesses (agent e), not merged yet.
#[allow(dead_code)]
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
    let (w, h) = (
        display.bounds.width.round() as u32,
        display.bounds.height.round() as u32,
    );
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
pub fn render_fixture(
    app: &AppHandle,
    kind: &str,
    display: &screen::Display,
) -> Result<Rgb, String> {
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
/// `display` is the `Display::id` the rect was placed on (B-005).
pub fn crop(rect: Rect, display: Option<u32>) -> Option<Vec<u8>> {
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
    let cut = capture::crop_dip(rect, display)?;
    encode::jpeg(&cut, LOUPE_QUALITY)
}

/// The loupe as a URL the lens page can load.
pub fn crop_url(rect: Rect, display: Option<u32>) -> Option<String> {
    crop(rect, display).map(|b| store("loupe", Arc::new(b)))
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
    s[((p / 100.0) * s.len() as f64)
        .floor()
        .min((s.len() - 1) as f64) as usize]
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
    let i = inputs(&g);
    let (t, r) = tier_for(i);
    let surface = if i.surface_solid { "solid" } else { "auto" };
    json!({
        "tier": t,
        "reasons": r,
        "surface": surface,
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
    fn intensity_defaults_to_liquid() {
        assert_eq!(intensity(), 0.75);
    }

    #[test]
    fn transparency_setting_is_not_a_reason() {
        // REQUESTS.md 5: only these three can force solid (B-035 dropped
        // the stored surface setting).
        let all = reasons_for(TierInputs {
            surface_solid: true,
            can_hide: false,
            high_contrast: true,
            fails: 3,
            fixture: false,
        });
        let want = ["capture-visible", "high-contrast", "capture-failed"];
        assert_eq!(all, want);
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
    fn solid_profile_at_07_is_live() {
        // A profile migrated as `surface: 'solid'` keeps the key for
        // rollback; the slider decides, and at 0.7 the glass is live.
        let i = TierInputs {
            surface_solid: true,
            can_hide: true,
            ..Default::default()
        };
        let p = tier_payload(i, 0.7);
        assert_eq!(p["tier"], "live");
        assert_eq!(p["reasons"], json!([]));
        assert_eq!(p["intensity"], json!(0.7));
    }

    #[test]
    fn busy_resets_after_panic() {
        let r = std::panic::catch_unwind(|| {
            let _busy = Busy::take().expect("no tick is running");
            panic!("a tick failed");
        });
        assert!(r.is_err());
        assert!(!lock().busy);
        assert!(Busy::take().is_some());
        assert!(!lock().busy);
    }

    #[test]
    fn recovery_rebroadcasts_live() {
        let now = Instant::now();
        let failed = TierInputs {
            can_hide: true,
            fails: 3,
            ..Default::default()
        };
        // Held solid by capture failures alone: tried again every 30 s, not
        // at the fast pace.
        let later = Some(now + Duration::from_millis(RETRY_MS));
        let wait = Plan::Wait(Duration::from_millis(RETRY_MS));
        assert_eq!(plan(failed, true, later, now), wait);
        assert_eq!(plan(failed, true, Some(now), now), Plan::Frames);
        // Solid for a reason a capture cannot change: no capture at all.
        let hc = TierInputs {
            high_contrast: true,
            ..failed
        };
        assert_eq!(plan(hc, true, None, now), Plan::Idle);
        assert_eq!(plan(failed, false, None, now), Plan::Idle);
        // The capture that succeeds tells the pages, who were told solid.
        let told = tier_for(failed).0;
        let mut ok = failed;
        ok.fails = 0;
        assert!(must_rebroadcast(Some(told), tier_for(ok).0));
        assert!(!must_rebroadcast(Some("live"), tier_for(ok).0));
    }

    #[test]
    fn missing_display_frame_skips_window() {
        let d = screen::tests::mixed();
        let mut frames = Frames::new();
        frames.insert(d[0].id, Arc::new(vec![1]));
        let on = vec![("a", d[0].clone()), ("b", d[1].clone())];
        let sent = sendable(&frames, on);
        let got: Vec<&str> = sent.iter().map(|(l, _)| *l).collect();
        assert_eq!(got, ["a"]);
    }

    #[test]
    fn frame_protocol_cors_is_app_only() {
        store("cors-key", Arc::new(vec![4, 5]));
        let r = serve_from("/cors-key/1.jpg", Some(APP_ORIGIN));
        assert_eq!(r.status(), 200);
        assert_eq!(r.headers()["Access-Control-Allow-Origin"], APP_ORIGIN);
        let r = serve_from("/cors-key/1.jpg", Some("https://example.com"));
        assert_eq!(r.status(), 403);
        assert!(r.body().is_empty());
        // A plain image load sends no Origin and still gets the bytes.
        let r = serve("/cors-key/1.jpg");
        assert_eq!(r.status(), 200);
        assert_ne!(r.headers()["Access-Control-Allow-Origin"], "*");
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
