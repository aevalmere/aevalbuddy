//! Screen capture: DXGI Desktop Duplication with a GDI fallback, then the
//! JPEG pipeline main.js's `captureScreen` ran (PARITY CAP-01..12).
//!
//! Answer quality is bounded by capture fidelity, so the numbers are kept
//! exactly: the whole screen at its DIP size shrunk to a 1600 px long edge,
//! an area at the display's native pixels cropped by the size that arrived,
//! Lanczos3 for every resample, JPEG 88 stepping down by 8 to a floor of 62
//! while over 1.6 MB. Our own windows are kept out with
//! WDA_EXCLUDEFROMCAPTURE, never WDA_MONITOR (which would paint them black
//! into the shot and into screen shares).
//!
//! All native capture runs on one dedicated thread, so neither the event
//! loop nor an async worker ever blocks on DXGI (GLASS-06).

// Much of this API is for agents (a) and (b) to call as they port.
#![allow(dead_code)]

pub mod dxgi;
pub mod encode;
pub mod gdi;
pub mod screen;

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};

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

fn worker() -> &'static Mutex<mpsc::Sender<Job>> {
    static TX: OnceLock<Mutex<mpsc::Sender<Job>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("aeval-capture".into())
            .spawn(move || {
                let mut cache = dxgi::Cache::default();
                while let Ok(job) = rx.recv() {
                    // A panic inside one capture must not take the thread
                    // (and every later capture) with it.
                    let _ =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&mut cache)));
                }
            })
            .expect("spawn capture thread");
        Mutex::new(tx)
    })
}

/// Runs `f` on the capture thread and waits for it.
fn on_capture_thread<T: Send + 'static>(
    f: impl FnOnce(&mut dxgi::Cache) -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    let job: Job = Box::new(move |c| {
        let _ = tx.send(f(c));
    });
    worker().lock().ok()?.send(job).ok()?;
    rx.recv().ok()
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
    })
    .unwrap_or_else(|| Err("capture thread is gone".into()))
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

/// The pixels a shot is made from, before encoding.
pub fn shot_image(region: Option<Rect>) -> Result<(Rgb, Display), String> {
    let list = screen::all_displays();
    let target = match region {
        Some(r) => screen::matching(&list, &r),
        None => screen::active_display(&list),
    };
    let frame = grab(&target)?;
    let full = frame.rgb();
    let b = target.bounds;
    let img = match region {
        Some(want) => {
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
        None => {
            let (tw, th) = encode::whole_screen_size(b.width, b.height);
            full.resize(tw, th)
        }
    };
    Ok((img, target))
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

/// A full-resolution cut of what is under a DIP rect (the lens loupe).
pub fn crop_dip(rect: Rect) -> Option<Rgb> {
    let list = screen::all_displays();
    let d = screen::matching(&list, &rect);
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
