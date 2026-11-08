//! Screen capture: DXGI Desktop Duplication with a GDI fallback, then the
//! JPEG pipeline main.js's `captureScreen` ran (PARITY CAP-01..12).
//!
//! Answer quality is bounded by capture fidelity (D-09, B-032): the whole
//! screen at its physical pixels shrunk to a 1600 px long edge, an area at
//! the display's native pixels cropped by the size that arrived, Lanczos3
//! for every resample, JPEG 88 with 4:4:4 chroma stepping down by 8 to a
//! floor of 62 while over 1.6 MB. The whole screen is the display the bar
//! was opened on (`screen::latched_display`), an area the display it was
//! picked on (B-005). Our own windows are kept out with
//! WDA_EXCLUDEFROMCAPTURE, never WDA_MONITOR (which would paint them black
//! into the shot and into screen shares).
//!
//! All native capture runs on one dedicated thread, so neither the event
//! loop nor an async worker ever blocks on DXGI (GLASS-06).

pub mod dxgi;
pub mod encode;
pub mod gdi;
pub mod screen;

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use base64::Engine as _;
use serde::Serialize;
use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};

pub use encode::Rgb;
pub use screen::{Display, PxRect, Rect};

/// One captured output: physical pixels, top-down BGRA, `width * 4` stride.
#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

impl Frame {
    pub fn rgb(&self) -> Rgb {
        Rgb::from_bgra(self.width, self.height, self.width as usize * 4, &self.bgra)
    }
}

/// Which backend produced the last frame (for smoke lines).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Dxgi = 1,
    Gdi = 2,
}

static LAST_BACKEND: AtomicU8 = AtomicU8::new(0);

pub fn last_backend() -> &'static str {
    match LAST_BACKEND.load(Ordering::Relaxed) {
        1 => "dxgi",
        2 => "gdi",
        _ => "none",
    }
}

type Job = Box<dyn FnOnce(&mut dxgi::Cache) + Send>;

/// How long a capture may take before its thread is presumed hung (B-110).
const JOB_LIMIT: Duration = Duration::from_secs(3);

/// A capture thread with its own DXGI cache. If the thread cannot start, the
/// receiver is dropped with it and every send fails, so captures fail
/// rather than panic.
fn spawn_worker() -> mpsc::Sender<Job> {
    let (tx, rx) = mpsc::channel::<Job>();
    let _ = std::thread::Builder::new()
        .name("aeval-capture".into())
        .spawn(move || {
            let mut cache = dxgi::Cache::default();
            while let Ok(job) = rx.recv() {
                // A panic inside one capture must not take the thread (and
                // every later capture) with it.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&mut cache)));
            }
        });
    tx
}

fn worker() -> &'static Mutex<mpsc::Sender<Job>> {
    static TX: OnceLock<Mutex<mpsc::Sender<Job>>> = OnceLock::new();
    TX.get_or_init(|| Mutex::new(spawn_worker()))
}

fn replace_worker(slot: &Mutex<mpsc::Sender<Job>>) {
    *slot.lock().unwrap_or_else(|e| e.into_inner()) = spawn_worker();
}

/// Runs `f` on the capture thread and waits for it.
fn on_capture_thread<T: Send + 'static>(
    f: impl FnOnce(&mut dxgi::Cache) -> T + Send + 'static,
) -> Result<T, String> {
    run_on(worker(), JOB_LIMIT, f)
}

/// Runs `f` on the worker in `slot`, waiting at most `limit`. A worker that
/// does not answer in time is presumed hung: a fresh one takes its place for
/// every later capture, and this one fails so the query falls back to text
/// (CAP-06, B-110). The hung thread exits once its job returns, because its
/// sender is gone.
fn run_on<T: Send + 'static>(
    slot: &Mutex<mpsc::Sender<Job>>,
    limit: Duration,
    f: impl FnOnce(&mut dxgi::Cache) -> T + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = mpsc::channel();
    let job: Job = Box::new(move |c| {
        let _ = tx.send(f(c));
    });
    let sent = slot.lock().unwrap_or_else(|e| e.into_inner()).send(job);
    if sent.is_err() {
        replace_worker(slot);
        return Err("capture thread is gone".into());
    }
    match rx.recv_timeout(limit) {
        Ok(v) => Ok(v),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            replace_worker(slot);
            Err(format!("capture took over {} ms", limit.as_millis()))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("capture panicked".into()),
    }
}

fn forced_backend() -> Option<Backend> {
    match std::env::var("AEVAL_CAPTURE_BACKEND").ok()?.as_str() {
        "gdi" => Some(Backend::Gdi),
        "dxgi" => Some(Backend::Dxgi),
        _ => None,
    }
}

/// The display as composed now, in physical pixels: DXGI, else GDI.
pub fn grab(display: &Display) -> Result<Arc<Frame>, String> {
    let device = display.device.clone();
    let phys = display.phys;
    let forced = forced_backend();
    on_capture_thread(move |cache| {
        let mut dxgi_err = String::new();
        if forced != Some(Backend::Gdi) {
            match cache.grab(&device) {
                Ok(f) if f.width as i32 == phys.width && f.height as i32 == phys.height => {
                    LAST_BACKEND.store(Backend::Dxgi as u8, Ordering::Relaxed);
                    return Ok(f);
                }
                Ok(f) => dxgi_err = format!("size {}x{} != monitor", f.width, f.height),
                Err(e) => dxgi_err = e,
            }
            if forced == Some(Backend::Dxgi) {
                return Err(dxgi_err);
            }
        }
        gdi::grab(phys)
            .map(|f| {
                LAST_BACKEND.store(Backend::Gdi as u8, Ordering::Relaxed);
                Arc::new(f)
            })
            .map_err(|e| format!("dxgi: {dxgi_err}; gdi: {e}"))
    })?
}

/// What a query sends to the model (CAP-06), same keys as main.js.
#[derive(Clone, Debug, Serialize)]
pub struct Shot {
    pub base64: String,
    pub bytes: usize,
    pub width: u32,
    pub height: u32,
    pub display: Rect,
    pub region: Option<Rect>,
    /// Not sent by main.js; useful to smoke and logs. Skipped in JSON.
    #[serde(skip)]
    pub quality: u8,
    #[serde(skip)]
    pub jpeg: Vec<u8>,
}

/// The display a shot reads: for an area, the display it was chosen on when
/// that is known (`device`), else the one the rect matches; for the whole
/// screen, `whole()`.
pub fn target_display(
    list: &[Display],
    region: Option<&Rect>,
    device: Option<&str>,
    whole: impl FnOnce() -> Display,
) -> Display {
    match region {
        Some(r) => device
            .and_then(|dev| list.iter().find(|d| d.device == dev).cloned())
            .unwrap_or_else(|| screen::matching(list, r)),
        None => whole(),
    }
}

/// The pixels a shot is made from, before encoding.
pub fn shot_image(region: Option<Rect>) -> Result<(Rgb, Display), String> {
    let list = screen::all_displays();
    let device = region.as_ref().and_then(crate::region::device_for);
    let whole = || screen::latched_display(&list);
    let target = target_display(&list, region.as_ref(), device.as_deref(), whole);
    let frame = grab(&target)?;
    let full = frame.rgb();
    let img = match region {
        Some(want) => crop_area(&full, &target, want),
        None => {
            // Sized from the pixels that arrived, not the DIP size (B-032).
            let (tw, th) = encode::whole_screen_size(full.width, full.height);
            full.resize(tw, th)
        }
    };
    Ok((img, target))
}

/// An area of one display's frame (CAP-03): cut by the size that arrived,
/// then held to the long-edge budget.
fn crop_area(full: &Rgb, display: &Display, want: Rect) -> Rgb {
    let b = display.bounds;
    let (x, y, w, h) = encode::crop_plan(
        (want.x, want.y, want.width, want.height),
        (b.x, b.y, b.width, b.height),
        (full.width, full.height),
    );
    let cut = full.crop(x, y, w, h);
    match encode::fit_long_edge(cut.width, cut.height) {
        Some((tw, th)) => cut.resize(tw, th),
        None => cut,
    }
}

/// A JPEG of the screen, or of one area of it (DIP), plus what the pointer
/// needs to map an answer back. `None` on any failure: the query degrades to
/// text-only, never crashes (CAP-06).
pub fn capture_screen(region: Option<Rect>) -> Option<Shot> {
    let (img, target) = shot_image(region).ok()?;
    let (jpeg, quality) = encode::jpeg_within_budget(&img)?;
    Some(Shot {
        base64: base64::engine::general_purpose::STANDARD.encode(&jpeg),
        bytes: jpeg.len(),
        width: img.width,
        height: img.height,
        display: target.bounds,
        region,
        quality,
        jpeg,
    })
}

/// `capture_screen` off the async runtime's worker threads.
pub async fn capture_screen_async(region: Option<Rect>) -> Option<Shot> {
    tauri::async_runtime::spawn_blocking(move || capture_screen(region))
        .await
        .ok()
        .flatten()
}

/// A full-resolution cut of what is under a DIP rect (the lens loupe), from
/// the display the rect was placed on (`display`, a `Display::id`), else the
/// one it matches (B-005).
pub fn crop_dip(rect: Rect, display: Option<u32>) -> Option<Rgb> {
    let list = screen::all_displays();
    let on = display.and_then(|id| list.iter().find(|d| d.id == id).cloned());
    let d = on.unwrap_or_else(|| screen::matching(&list, &rect));
    let frame = grab(&d).ok()?;
    let full = frame.rgb();
    let k = full.width as f64 / d.bounds.width.max(1.0);
    let x = (((rect.x - d.bounds.x) * k).round().max(0.0) as u32).min(full.width - 1);
    let y = (((rect.y - d.bounds.y) * k).round().max(0.0) as u32).min(full.height - 1);
    let w = ((rect.width * k).round().max(1.0) as u32).min(full.width - x);
    let h = ((rect.height * k).round().max(1.0) as u32).min(full.height - y);
    Some(full.crop(x, y, w, h))
}

/// Windows 10 2004 (build 19041) is where WDA_EXCLUDEFROMCAPTURE exists;
/// before it the only affinity blanks the window, which would put a black
/// box into every shot, so nothing is set there (main.js
/// CAN_HIDE_FROM_CAPTURE).
pub fn can_hide_from_capture() -> bool {
    static CAN: OnceLock<bool> = OnceLock::new();
    *CAN.get_or_init(|| windows_build() >= 19041)
}

pub fn windows_build() -> u32 {
    let mut buf = [0u16; 32];
    let mut len = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"),
            w!("CurrentBuildNumber"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut len),
        )
    };
    if r.0 != 0 {
        return 0;
    }
    screen::wide(&buf).trim().parse().unwrap_or(0)
}

/// Keeps one of our windows out of every capture (CAP-08, WIN D10). Called
/// at creation and again on every show: Windows can drop a hidden window's
/// affinity. Returns whether the affinity is now set.
pub fn exclude_window(hwnd: isize) -> bool {
    if !can_hide_from_capture() || hwnd == 0 {
        return false;
    }
    unsafe { SetWindowDisplayAffinity(HWND(hwnd as *mut _), WDA_EXCLUDEFROMCAPTURE).is_ok() }
}

/// A Tauri window's HWND as a plain integer (independent of the windows-rs
/// version Tauri itself links).
pub fn hwnd_of<R: tauri::Runtime>(win: &tauri::WebviewWindow<R>) -> isize {
    win.hwnd().map(|h| h.0 as isize).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame whose pixels say where they are.
    fn coded(w: u32, h: u32) -> Rgb {
        let mut data = Vec::with_capacity(w as usize * h as usize * 3);
        for y in 0..h {
            for x in 0..w {
                let hi = (x / 256 * 16 + y / 256) as u8;
                data.extend_from_slice(&[(x % 256) as u8, (y % 256) as u8, hi]);
            }
        }
        Rgb {
            width: w,
            height: h,
            data,
        }
    }

    #[test]
    fn area_on_primary_crops_primary() {
        let d = screen::tests::mixed();
        // In the band both displays claimed under the old layout.
        let area = Rect::new(1500.0, 200.0, 300.0, 150.0);
        let t = target_display(&d, Some(&area), None, || d[1].clone());
        assert_eq!(t.device, r"\\.\DISPLAY1");
        // A remembered display that is gone falls back to matching.
        let gone = Some(r"\\.\DISPLAY9");
        let t = target_display(&d, Some(&area), gone, || d[1].clone());
        assert_eq!(t.device, r"\\.\DISPLAY1");
        // Cut from the primary's own pixels.
        let frame = coded(1920, 1080);
        let cut = crop_area(&frame, &t, area);
        assert_eq!((cut.width, cut.height), (300, 150));
        assert_eq!(cut.pixel(0, 0), frame.pixel(1500, 200));
        assert_eq!(cut.pixel(299, 149), frame.pixel(1799, 349));
    }

    #[test]
    fn area_on_the_scaled_display_crops_its_pixels() {
        let d = screen::tests::mixed();
        let area = Rect::new(1930.0, 10.0, 100.0, 100.0);
        let t = target_display(&d, Some(&area), None, || d[0].clone());
        assert_eq!(t.device, r"\\.\DISPLAY2");
        // Straddling the edge evenly: the display it was picked on wins.
        let picked = Some(r"\\.\DISPLAY2");
        let edge = Rect::new(1900.0, 10.0, 40.0, 40.0);
        let t2 = target_display(&d, Some(&edge), picked, || d[0].clone());
        assert_eq!(t2.device, r"\\.\DISPLAY2");
        let frame = coded(2560, 1440);
        let cut = crop_area(&frame, &t, area);
        assert_eq!((cut.width, cut.height), (150, 150));
        assert_eq!(cut.pixel(0, 0), frame.pixel(15, 15));
    }

    #[test]
    fn hung_worker_is_replaced() {
        let slot = Mutex::new(spawn_worker());
        let limit = Duration::from_millis(300);
        // A job that outlives the limit fails the call...
        let hung = run_on(&slot, limit, |_| std::thread::sleep(limit * 10));
        assert!(hung.is_err());
        // ...and the next one runs at once on a fresh thread.
        let t = std::time::Instant::now();
        assert_eq!(run_on(&slot, limit, |_| 7), Ok(7));
        assert!(t.elapsed() < Duration::from_secs(2));
        // A job that panics fails its call and leaves the thread working.
        let boom: Result<(), String> = run_on(&slot, limit, |_| panic!("capture failed"));
        assert!(boom.is_err());
        assert_eq!(run_on(&slot, limit, |_| 8), Ok(8));
    }

    #[test]
    fn whole_screen_is_the_whole_target() {
        let d = screen::tests::mixed();
        let t = target_display(&d, None, Some(r"\\.\DISPLAY1"), || d[1].clone());
        assert_eq!(t.device, r"\\.\DISPLAY2");
    }
}
