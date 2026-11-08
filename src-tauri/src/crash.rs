//! Errors that would otherwise end the app without a trace (PARITY LIFE-05,
//! FILE-04, WIN-16).
//!
//! Every window here is a tool window: when it dies there is no taskbar
//! button left to notice. So a panic on any thread is written to
//! `errors.log` beside the settings, announced once through a toast, and the
//! app carries on. The log keeps its newest 128 KB once it passes 256 KB.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

const MAX_BYTES: u64 = 256 * 1024;

static LOG: OnceLock<PathBuf> = OnceLock::new();
static ANNOUNCED: AtomicBool = AtomicBool::new(false);

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
    if ANNOUNCED.swap(true, Ordering::SeqCst) {
        return;
    }
    crate::notify::show(
        "AevalBuddy hit an error",
        "It is still running. The details were written to errors.log in its settings folder.",
        None,
    );
}

/// Installed first thing in `run()`. A panic on any thread (Tauri's main
/// thread, tokio tasks, our own threads) is logged and announced; with
/// `panic = "unwind"` the thread's work is lost but the process stays.
pub fn install(user_data: &Path) {
    let _ = LOG.set(user_data.join("errors.log"));
    let _ = fs::create_dir_all(user_data);
    std::panic::set_hook(Box::new(|info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "panic".to_string()
        };
        let at = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let trace = std::backtrace::Backtrace::force_capture();
        write("panic", &format!("{msg}\n    at {at}\n{trace}"));
        announce();
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_lines_and_trimming() {
        let dir = std::env::temp_dir().join(format!("aeval-crash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
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
}
