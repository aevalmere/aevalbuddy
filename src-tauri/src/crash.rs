//! Errors that would otherwise end the app without a trace (PARITY LIFE-05,
//! LIFE-12, FILE-04, WIN-16).
//!
//! Every window here is a tool window: when it dies there is no taskbar
//! button left to notice. So a panic on any thread is written to
//! `errors.log` beside the settings and announced once through a toast. The
//! log keeps its newest 128 KB once it passes 256 KB.
//!
//! - `guard` and `guard_async` (K-01) run a body, catch its panic, log it
//!   with the caller's scope and keep the app running.
//! - `guard_webview` (K-02) reloads a page whose WebView2 renderer died and
//!   calls back when the browser process itself is gone.
//! - A panic on the main thread outside a guard cannot be survived: it
//!   unwinds into a Win32 window procedure, and Rust aborts the process
//!   there. The toast says so, and is shown before the abort.

use std::any::Any;
use std::cell::Cell;
use std::fs;
use std::future::Future;
use std::io::Write;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::task::Poll;
use std::thread::{self, ThreadId};
use std::time::Duration;

const MAX_BYTES: u64 = 256 * 1024;

static LOG: OnceLock<PathBuf> = OnceLock::new();
static ANNOUNCED: AtomicBool = AtomicBool::new(false);
static ANNOUNCED_FATAL: AtomicBool = AtomicBool::new(false);
static MAIN: OnceLock<ThreadId> = OnceLock::new();

thread_local! {
    /// How many guards are running on this thread; a panic inside one is
    /// caught, so the hook leaves the toast to the guard.
    static GUARDED: Cell<u32> = const { Cell::new(0) };
}

pub const STILL_RUNNING: (&str, &str) = (
    "AevalBuddy hit an error",
    "It is still running. The details were written to errors.log in its settings folder.",
);

pub const HAS_TO_RESTART: (&str, &str) = (
    "AevalBuddy hit a problem and has to restart.",
    "Open it again from the Start menu. The details were written to errors.log in its settings folder.",
);

/// ISO 8601 with milliseconds and Z, as JavaScript's toISOString.
pub fn iso_now() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

/// Appends `[ISO] scope\ndetail\n\n`, keeping the tail of a big log.
pub fn write_to(path: &Path, scope: &str, detail: &str) {
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() > MAX_BYTES {
            if let Ok(bytes) = fs::read(path) {
                let keep = (MAX_BYTES / 2) as usize;
                let tail = &bytes[bytes.len().saturating_sub(keep)..];
                let _ = fs::write(path, String::from_utf8_lossy(tail).as_bytes());
            }
        }
    }
    let line = format!("[{}] {scope}\n{detail}\n\n", iso_now());
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
    eprintln!("[aevalbuddy] {scope}: {detail}");
}

/// Logs to errors.log in the user-data folder.
pub fn write(scope: &str, detail: &str) {
    if let Some(p) = LOG.get() {
        write_to(p, scope, detail);
    } else {
        eprintln!("[aevalbuddy] {scope}: {detail}");
    }
}

pub fn file() -> Option<&'static PathBuf> {
    LOG.get()
}

/// Once per session: something that throws every frame must not notify
/// every frame.
pub fn announce() {
    if ANNOUNCED.swap(true, Ordering::SeqCst) || cfg!(test) {
        return;
    }
    crate::notify::show(STILL_RUNNING.0, STILL_RUNNING.1, None);
}

/// The process is about to end: say so, and wait for the toast to be handed
/// to Windows, since nothing runs after the abort.
fn announce_fatal() {
    if ANNOUNCED_FATAL.swap(true, Ordering::SeqCst) || cfg!(test) {
        return;
    }
    crate::notify::show_and_wait(HAS_TO_RESTART.0, HAS_TO_RESTART.1, Duration::from_secs(3));
}

/// Whether the caller is on the thread that called `install`: the main
/// thread, where Tauri's event loop runs.
pub fn on_main_thread() -> bool {
    MAIN.get() == Some(&thread::current().id())
}

/// The text of a panic payload.
pub fn message(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic".to_string()
    }
}

/// Which toast an uncaught panic gets. Inside a guard the guard announces;
/// on the main thread (which is also where boot runs) the process aborts;
/// anywhere else only that thread's work is lost.
#[derive(Debug, PartialEq, Eq)]
enum Toast {
    ByGuard,
    HasToRestart,
    StillRunning,
}

fn toast_for(guarded: bool, main_thread: bool) -> Toast {
    if guarded {
        Toast::ByGuard
    } else if main_thread {
        Toast::HasToRestart
    } else {
        Toast::StillRunning
    }
}

/// Installed first thing in `run()`, on the main thread. A panic on any
/// thread (Tauri's main thread, tokio tasks, our own threads) is logged and
/// announced with the toast that fits where it happened.
pub fn install(user_data: &Path) {
    let _ = LOG.set(user_data.join("errors.log"));
    let _ = MAIN.set(thread::current().id());
    let _ = fs::create_dir_all(user_data);
    std::panic::set_hook(Box::new(|info| {
        let msg = message(info.payload());
        let at = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let guarded = GUARDED.with(Cell::get) > 0;
        let trace = std::backtrace::Backtrace::force_capture();
        let current = thread::current();
        let name = current.name().unwrap_or("unnamed");
        let detail = format!("{msg}\n    at {at}\n    thread {name}, guarded {guarded}");
        write("panic", &format!("{detail}\n{trace}"));
        match toast_for(guarded, on_main_thread()) {
            Toast::ByGuard => {}
            Toast::HasToRestart => announce_fatal(),
            Toast::StillRunning => announce(),
        }
    }));
}

/// Counts a running guard on this thread, also while unwinding.
struct Depth;

impl Depth {
    fn enter() -> Depth {
        GUARDED.with(|d| d.set(d.get() + 1));
        Depth
    }
}

impl Drop for Depth {
    fn drop(&mut self) {
        GUARDED.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

fn recovered(scope: &str, payload: &(dyn Any + Send)) {
    write(
        "guard",
        &format!(
            "{scope} panicked and was stopped; the app keeps running: {}",
            message(payload)
        ),
    );
    announce();
}

/// K-01: runs `f`; a panic inside it is logged under `scope`, announced once
/// and turned into None, so a main-thread callback never takes the process
/// down.
pub fn guard<R>(scope: &str, f: impl FnOnce() -> R) -> Option<R> {
    let out = catch_unwind(AssertUnwindSafe(|| {
        let _depth = Depth::enter();
        f()
    }));
    match out {
        Ok(v) => Some(v),
        Err(payload) => {
            recovered(scope, payload.as_ref());
            None
        }
    }
}

/// K-01 for async bodies: every poll of `fut` runs inside a guard.
pub async fn guard_async<T>(scope: &str, fut: impl Future<Output = T>) -> Option<T> {
    let mut fut = std::pin::pin!(fut);
    let polled = std::future::poll_fn(|cx| {
        let step = catch_unwind(AssertUnwindSafe(|| {
            let _depth = Depth::enter();
            fut.as_mut().poll(cx)
        }));
        match step {
            Ok(Poll::Ready(v)) => Poll::Ready(Ok(v)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(payload) => Poll::Ready(Err(payload)),
        }
    })
    .await;
    match polled {
        Ok(v) => Some(v),
        Err(payload) => {
            recovered(scope, payload.as_ref());
            None
        }
    }
}

/// What a WebView2 `ProcessFailed` event asks of us.
#[derive(Debug, PartialEq, Eq)]
enum Failure {
    /// The page's renderer died or hung: reload it.
    Reload,
    /// The browser process is gone and the webview with it: the owner
    /// recreates the window.
    BrowserGone,
    /// A helper process (GPU, utility) that WebView2 restarts by itself.
    Other,
}

/// COREWEBVIEW2_PROCESS_FAILED_KIND values.
fn failure_of(kind: i32) -> (Failure, &'static str) {
    match kind {
        0 => (Failure::BrowserGone, "browser-process-exited"),
        1 => (Failure::Reload, "render-process-exited"),
        2 => (Failure::Reload, "render-process-unresponsive"),
        3 => (Failure::Reload, "frame-render-process-exited"),
        4 => (Failure::Other, "utility-process-exited"),
        5 => (Failure::Other, "sandbox-helper-process-exited"),
        6 => (Failure::Other, "gpu-process-exited"),
        _ => (Failure::Other, "other-process-exited"),
    }
}

/// COREWEBVIEW2_PROCESS_FAILED_REASON values.
fn reason_of(reason: i32) -> &'static str {
    match reason {
        0 => "unexpected",
        1 => "unresponsive",
        2 => "terminated",
        3 => "crashed",
        4 => "launch-failed",
        5 => "out-of-memory",
        6 => "profile-deleted",
        _ => "unknown",
    }
}

pub type OnBrowserExit = Arc<dyn Fn(&tauri::AppHandle, &str) + Send + Sync + 'static>;

/// K-02: watches `window`'s WebView2 for dead processes. A renderer that
/// exited, hung or lost a frame is logged, announced once and reloaded, so
/// the window never sits there as a blank rectangle that eats clicks. When
/// the browser process exits, the webview is beyond reloading:
/// `on_browser_exit(app, label)` runs on the main thread so the owner can
/// recreate the window.
pub fn guard_webview(
    window: &tauri::WebviewWindow,
    on_browser_exit: impl Fn(&tauri::AppHandle, &str) + Send + Sync + 'static,
) {
    use tauri::Manager;
    let app = window.app_handle().clone();
    let label = window.label().to_string();
    let on_exit: OnBrowserExit = Arc::new(on_browser_exit);
    let l2 = label.clone();
    let res = window.with_webview(move |pw| {
        if let Err(e) = webview::watch(&pw, app, l2.clone(), on_exit) {
            write("renderer", &format!("{l2} not watched for crashes: {e}"));
        }
    });
    if let Err(e) = res {
        write("renderer", &format!("{label} not watched for crashes: {e}"));
    }
}

mod webview {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2ProcessFailedEventArgs2, COREWEBVIEW2_PROCESS_FAILED_KIND,
        COREWEBVIEW2_PROCESS_FAILED_REASON,
    };
    use webview2_com::ProcessFailedEventHandler;
    use windows_core::Interface;

    use super::{announce, failure_of, guard, reason_of, write, Failure, OnBrowserExit};

    /// Adds the ProcessFailed handler. Runs on the main thread, inside
    /// `with_webview`, as every WebView2 call must.
    pub fn watch(
        pw: &tauri::webview::PlatformWebview,
        app: tauri::AppHandle,
        label: String,
        on_exit: OnBrowserExit,
    ) -> windows_core::Result<()> {
        // SAFETY: COM calls on the webview's own thread; the controller is
        // alive for the duration of `with_webview`.
        let core = unsafe { pw.controller().CoreWebView2()? };
        let handler = ProcessFailedEventHandler::create(Box::new(move |sender, args| {
            let Some(args) = args else {
                return Ok(());
            };
            let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
            // SAFETY: an out pointer to a live local.
            unsafe { args.ProcessFailedKind(&mut kind)? };
            let mut reason = COREWEBVIEW2_PROCESS_FAILED_REASON::default();
            if let Ok(a2) = args.cast::<ICoreWebView2ProcessFailedEventArgs2>() {
                // SAFETY: as above.
                let _ = unsafe { a2.Reason(&mut reason) };
            }
            let (what, kind_name) = failure_of(kind.0);
            let detail = format!("{label} {kind_name} {}", reason_of(reason.0));
            guard("renderer", || match what {
                Failure::Reload => {
                    write("renderer", &detail);
                    announce();
                    if let Some(webview) = sender.as_ref() {
                        // SAFETY: the event's own webview, on its thread.
                        if let Err(e) = unsafe { webview.Reload() } {
                            write("renderer", &format!("{label} reload failed: {e}"));
                        }
                    }
                }
                Failure::BrowserGone => {
                    write("renderer", &detail);
                    announce();
                    let (a, l, f) = (app.clone(), label.clone(), on_exit.clone());
                    let _ = app.run_on_main_thread(move || {
                        guard("renderer.recreate", || f(&a, &l));
                    });
                }
                Failure::Other => eprintln!("[aevalbuddy] renderer: {detail}"),
            });
            Ok(())
        }));
        let mut token = 0i64;
        // SAFETY: a live handler and an out pointer to a live local.
        unsafe { core.add_ProcessFailed(&handler, &mut token)? };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aeval-crash-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn log_lines_and_trimming() {
        let dir = temp("log");
        let p = dir.join("errors.log");
        write_to(&p, "uncaughtException", "Error: boom");
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.starts_with('['));
        assert!(text.contains("Z] uncaughtException\nError: boom\n\n"));
        fs::write(&p, "x".repeat(300 * 1024)).unwrap();
        write_to(&p, "s", "d");
        let len = fs::metadata(&p).unwrap().len();
        assert!(len < 140 * 1024, "kept {len}");
        assert!(iso_now().ends_with('Z'));
    }

    /// The errors.log the guards write to in this test binary.
    fn test_log() -> PathBuf {
        let _ = LOG.set(temp("guard").join("errors.log"));
        LOG.get().unwrap().clone()
    }

    fn boom(text: &str) -> i32 {
        panic!("{text}")
    }

    #[test]
    fn guard_catches_and_logs() {
        let log = test_log();
        assert_eq!(guard("test.ok", || 5), Some(5));
        assert_eq!(guard("test.scope", || boom("boom-in-guard")), None);
        assert_eq!(GUARDED.with(Cell::get), 0, "depth unwinds with the panic");
        let text = fs::read_to_string(&log).unwrap();
        assert!(text.contains("] guard\ntest.scope panicked"), "{text}");
        assert!(text.contains("boom-in-guard"), "{text}");
        // Nested guards: the inner one catches, the outer one carries on.
        let inner = || guard("test.inner", || boom("inner")).is_none();
        assert_eq!(guard("test.outer", inner), Some(true));
    }

    #[test]
    fn guard_async_catches_and_logs() {
        let log = test_log();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let ok = rt.block_on(guard_async("test.async.ok", async {
            tokio::time::sleep(Duration::from_millis(5)).await;
            7
        }));
        assert_eq!(ok, Some(7));
        let bad = rt.block_on(guard_async("test.async.scope", async {
            tokio::time::sleep(Duration::from_millis(5)).await;
            boom("boom-after-await")
        }));
        assert_eq!(bad, None);
        let text = fs::read_to_string(&log).unwrap();
        assert!(text.contains("test.async.scope panicked"), "{text}");
        assert!(text.contains("boom-after-await"), "{text}");
    }

    #[test]
    fn toast_follows_the_thread() {
        assert_eq!(toast_for(true, true), Toast::ByGuard);
        assert_eq!(toast_for(false, true), Toast::HasToRestart);
        assert_eq!(toast_for(false, false), Toast::StillRunning);
        assert_eq!(HAS_TO_RESTART.0, "AevalBuddy hit a problem and has to restart.");
    }

    #[test]
    fn renderer_failures_reload_and_browser_exit_recreates() {
        assert_eq!(failure_of(1).0, Failure::Reload);
        assert_eq!(failure_of(2).0, Failure::Reload);
        assert_eq!(failure_of(3).0, Failure::Reload);
        assert_eq!(failure_of(0).0, Failure::BrowserGone);
        assert_eq!(failure_of(6).0, Failure::Other);
        assert_eq!(failure_of(1).1, "render-process-exited");
        assert_eq!(reason_of(3), "crashed");
    }
}
