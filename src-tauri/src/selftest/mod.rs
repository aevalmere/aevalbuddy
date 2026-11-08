//! The smoke runner, and the probe API each lane uses to prove its behaviour
//! inside the real exe (BACKLOG K-30).
//!
//! `AEVAL_SMOKE=1` boots the app as usual; then [`scaffold`] runs on a thread
//! of its own. It runs each lane's probe in turn, prints one line per check,
//! and exits 0 when every `must` held or 1 when one did not:
//!
//! ```text
//! [SMOKE] version=1.1.1
//! [SMOKE] scaffold.bridge=ok
//! [SMOKE] outputMulti=FAIL:2 outputs, want 3
//! [SMOKE] probe.windows=ok
//! [SMOKE] failed: outputMulti
//! ```
//!
//! The rust workflow's Smoke step (`tools/smoke-gate.mjs`) turns red on any
//! `=FAIL`, on a missing `[SMOKE] done`, and on any label listed in
//! `tools/smoke-labels/*.txt` that never printed.
//!
//! # Writing a probe
//!
//! A lane owns `probes/<lane>.rs`, whose `pub fn run(cx: &mut Cx)` prints that
//! lane's labels, and `tools/smoke-labels/<lane>.txt`, which lists them, one
//! per line. List a label in the same commit that makes it print; never
//! remove one to get green.
//!
//! - `cx.must(label, ok, why)` is an invariant: `label=ok`, or
//!   `label=FAIL:<why>`, which fails the run.
//! - `cx.log(label, value)` is an observation: `label=<value>`.
//! - `cx.eval(window, js)` runs `js`, one expression, in that window's page and
//!   returns its value as JSON; a Promise is awaited first. Wrap statements in
//!   `(() => { ...; return x; })()`. It gives up after 5 s ([`Cx::eval_for`]
//!   takes another timeout).
//! - `cx.input(Input::...)` sends real mouse and keyboard input through
//!   `SendInput` at physical pixels: move, click, double and right click,
//!   drag, wheel, key chords and text. `input::cursor()` reads the pointer.
//! - `cx.wait_for(timeout, || cond)` polls every 50 ms until `cond` holds.
//! - `cx.block_on(fut)` runs async code (the llm, desktop and agent APIs);
//!   `cx.on_main(|app| ..)` runs a closure on the main thread and returns
//!   its value.
//! - `cx.notepad()` is a real Notepad holding [`notepad::TEXT`], for probes
//!   that need an app to read, find in, type into or focus. It opens on first
//!   use (on CI, or with `AEVAL_DESKTOP_NOTEPAD=1`; None otherwise), always
//!   before the `v2desktop` lanes, and closes once every lane has run. Leave
//!   it open, and put back any text you change.
//!
//! A probe runs on the smoke thread, never the main thread, so it may block.
//! A panic ends only that probe: it prints `probe.<lane>=FAIL:<message>` and
//! the next lane runs. Each lane also rolls up into one of Electron's block
//! labels ([`probes::BLOCKS`]). The app is the real one with a scratch
//! `AEVAL_USERDATA`, shared by every lane in turn: leave it as you found it
//! (bar collapsed, extra windows closed, settings you changed put back).
//!
//! ```ignore
//! use std::time::Duration;
//!
//! use serde_json::json;
//!
//! use crate::selftest::{Cx, Input};
//!
//! pub fn run(cx: &mut Cx) {
//!     cx.log("model", crate::store::get().get_model());
//!     let found = cx.eval("output-1", "document.querySelectorAll('input, textarea').length");
//!     cx.must("noOutputInput", found == Ok(json!(0)), format!("{found:?}"));
//!     // Three notches down over the answer, then wait for it to scroll.
//!     let sent = cx.input(Input::Wheel { at: (400, 300), notches: -3 });
//!     let moved = cx.wait_for(Duration::from_secs(2), || {
//!         let top = cx.eval("output-1", "document.getElementById('chat').scrollTop");
//!         top.is_ok_and(|v| v.as_f64() > Some(0.0))
//!     });
//!     cx.must("transcriptWheel", sent.is_ok() && moved, format!("{sent:?}"));
//! }
//! ```

mod eval;
pub mod input;
pub mod notepad;
pub mod probes;
mod report;

use std::any::Any;
use std::fmt::Display;
use std::future::Future;
use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::AppHandle;

pub use input::Input;
pub use notepad::Notepad;
pub use report::Report;

/// The script each page runs once loaded under smoke: one call through the
/// bridge, which `ipc::Seen` records.
pub const PING: &str = "window.aeval && window.aeval.invoke('glass:hello').catch(() => {});";

/// `AEVAL_SMOKE` is set: lib.rs pings every page and starts [`scaffold`].
pub fn smoke_enabled() -> bool {
    std::env::var_os("AEVAL_SMOKE").is_some()
}

/// The smoke run, on its own thread: every lane's probe, then the verdict.
/// Exits 0 when every must held, else 1; a panic outside the probes prints
/// `[SMOKE] error=<message>` and exits 1.
pub fn scaffold(app: AppHandle) {
    let rep = Report::new("SMOKE");
    let run = panic::catch_unwind(AssertUnwindSafe(|| {
        let mut cx = Cx::new(app.clone(), rep.clone());
        probes::run_all(&mut cx);
    }));
    let code = match run {
        Ok(()) => rep.finish(),
        Err(e) => rep.error(&panic_text(&*e)),
    };
    app.exit(code);
}

/// The message a caught panic carried.
pub fn panic_text(e: &(dyn Any + Send)) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "a panic with no message".to_string()
    }
}

/// What a probe gets: the app, the report, and the means to drive the app as
/// a person would.
pub struct Cx {
    pub app: AppHandle,
    pub rep: Report,
    notepad: Option<Notepad>,
    notepad_tried: bool,
}

// The lanes' probes are the callers; the harness's own probe uses part.
#[allow(dead_code)]
impl Cx {
    pub fn new(app: AppHandle, rep: Report) -> Cx {
        Cx {
            app,
            rep,
            notepad: None,
            notepad_tried: false,
        }
    }

    /// `[SMOKE] <label>=ok`, or `=FAIL:<why>`, which fails the run.
    pub fn must(&self, label: &str, ok: bool, why: impl Display) {
        self.rep.must(label, ok, why);
    }

    /// `[SMOKE] <label>=<value>`.
    pub fn log(&self, label: &str, value: impl Display) {
        self.rep.log(label, value);
    }

    /// Runs `js`, one expression, in the page of window `label` and returns
    /// its value as JSON (a Promise is awaited first). Gives up after 5 s.
    pub fn eval(&self, label: &str, js: &str) -> Result<Value, String> {
        self.eval_for(label, js, Duration::from_secs(5))
    }

    /// [`Cx::eval`] with its own timeout.
    pub fn eval_for(&self, label: &str, js: &str, timeout: Duration) -> Result<Value, String> {
        eval::eval(&self.app, label, js, timeout)
    }

    /// Real mouse or keyboard input through `SendInput`, at physical pixels.
    pub fn input(&self, gesture: Input<'_>) -> Result<(), String> {
        input::send(gesture)
    }

    /// Polls `cond` every 50 ms: true as soon as it holds, false once
    /// `timeout` has passed without it.
    pub fn wait_for(&self, timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
        let end = Instant::now() + timeout;
        loop {
            if cond() {
                return true;
            }
            if Instant::now() >= end {
                return false;
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// Runs async code to completion on Tauri's runtime.
    pub fn block_on<F: Future>(&self, fut: F) -> F::Output {
        tauri::async_runtime::block_on(fut)
    }

    /// Runs `f` on the main thread and returns its value; None when the
    /// event loop did not run it within 10 s.
    pub fn on_main<R, F>(&self, f: F) -> Option<R>
    where
        R: Send + 'static,
        F: FnOnce(&AppHandle) -> R + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let app = self.app.clone();
        self.app
            .run_on_main_thread(move || {
                let _ = tx.send(f(&app));
            })
            .ok()?;
        rx.recv_timeout(Duration::from_secs(10)).ok()
    }

    pub fn sleep(&self, ms: u64) {
        thread::sleep(Duration::from_millis(ms));
    }

    /// The smoke's own Notepad, opened on first use and printing
    /// `v2NotepadOpen`; None off CI (unless `AEVAL_DESKTOP_NOTEPAD` asks for
    /// it) or when it did not open, which is not tried twice.
    pub fn notepad(&mut self) -> Option<&Notepad> {
        if self.notepad.is_none() && !self.notepad_tried && probes::notepad_enabled() {
            self.notepad_tried = true;
            let opened = self.block_on(notepad::open());
            let why = opened.as_ref().err().cloned().unwrap_or_default();
            self.must("v2NotepadOpen", opened.is_ok(), why);
            self.notepad = opened.ok();
        }
        self.notepad.as_ref()
    }

    /// Closes the Notepad, if one opened: `v2NotepadClosed=true|false`.
    fn close_notepad(&mut self) {
        if let Some(np) = self.notepad.take() {
            let closed = self.block_on(np.close());
            self.log("v2NotepadClosed", closed);
        }
    }
}
