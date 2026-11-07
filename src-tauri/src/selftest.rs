//! Self-checks run by `AEVAL_SMOKE=1` and `AEVAL_SHOT=<dir>` (agent e). The
//! full port prints every `[SMOKE]` label in PARITY section 24 with the same
//! pass conditions. Phase 1 proves the shell: the exe starts, every page loads
//! from the staged frontend, and each one reaches Rust through `window.aeval`.

use std::io::Write;
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::{ipc, paths, windows};

/// The script each page runs once loaded under smoke: one call through the
/// bridge, which `ipc::Seen` records.
pub const PING: &str = "window.aeval && window.aeval.invoke('glass:hello').catch(() => {});";

pub fn smoke_enabled() -> bool {
    std::env::var_os("AEVAL_SMOKE").is_some()
}

fn line(label: &str, value: impl std::fmt::Display) {
    println!("[SMOKE] {label}={value}");
}

/// Waits for every scaffold page to call through the bridge, prints the
/// result, and exits 0 or 1.
pub fn scaffold(app: AppHandle) {
    let want: Vec<&str> = windows::SCAFFOLD_WINDOWS.iter().map(|(l, _)| *l).collect();
    let deadline = Instant::now() + Duration::from_secs(30);
    let seen = loop {
        let seen = app.state::<ipc::Seen>().snapshot();
        if want.iter().all(|w| seen.contains_key(*w)) || Instant::now() > deadline {
            break seen;
        }
        thread::sleep(Duration::from_millis(200));
    };
    let ok = want.iter().all(|w| seen.contains_key(*w));
    line("version", &app.package_info().version);
    line("userData", paths::user_data_dir().display());
    line("webviewData", paths::webview_data_dir().display());
    let pages: Vec<&str> = seen.keys().map(String::as_str).collect();
    line("scaffold.pages", pages.join(","));
    let verdict = if ok {
        "ok"
    } else {
        "FAIL:a page never reached Rust"
    };
    line("scaffold.bridge", verdict);
    if ok {
        println!("[SMOKE] done");
    } else {
        println!("[SMOKE] failed: scaffold.bridge");
    }
    let _ = std::io::stdout().flush();
    app.exit(if ok { 0 } else { 1 });
}
