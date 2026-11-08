//! `[SMOKE] label=value` lines, and the musts that decide the exit code
//! (main.js runSmoke's `log` and `must`).

use std::fmt::Display;
use std::io::Write;
use std::sync::Arc;

use parking_lot::Mutex;

/// Prints `[<prefix>] <label>=<value>` lines and remembers which musts
/// failed. Clones share that list.
#[derive(Clone)]
pub struct Report {
    prefix: &'static str,
    failed: Arc<Mutex<Vec<String>>>,
}

impl Report {
    pub fn new(prefix: &'static str) -> Report {
        Report {
            prefix,
            failed: Arc::default(),
        }
    }

    /// An observation: `[SMOKE] <label>=<value>`, always on one line.
    pub fn log(&self, label: &str, value: impl Display) {
        println!("{}", self.line(label, &value.to_string()));
    }

    /// An invariant: `[SMOKE] <label>=ok`, or `[SMOKE] <label>=FAIL:<why>`,
    /// which fails the run.
    pub fn must(&self, label: &str, ok: bool, why: impl Display) {
        if ok {
            self.log(label, "ok");
            return;
        }
        let why = why.to_string();
        if why.is_empty() {
            self.log(label, "FAIL");
        } else {
            self.log(label, format!("FAIL:{why}"));
        }
        self.failed.lock().push(label.to_string());
    }

    /// Counts musts another reporter printed and failed (the desktop
    /// selfcheck has its own), so they decide the exit code too.
    pub fn absorb(&self, labels: impl IntoIterator<Item = String>) {
        self.failed.lock().extend(labels);
    }

    /// The labels whose must failed, in order.
    pub fn failures(&self) -> Vec<String> {
        self.failed.lock().clone()
    }

    /// The last line, `[SMOKE] done` or `[SMOKE] failed: a, b`, and the exit
    /// code that goes with it.
    pub fn finish(&self) -> i32 {
        let failed = self.failures();
        let code = if failed.is_empty() {
            println!("[{}] done", self.prefix);
            0
        } else {
            println!("[{}] failed: {}", self.prefix, failed.join(", "));
            1
        };
        let _ = std::io::stdout().flush();
        code
    }

    /// The runner itself panicked: `[SMOKE] error=<message>`, exit code 1.
    pub fn error(&self, message: &str) -> i32 {
        self.log("error", message);
        let _ = std::io::stdout().flush();
        1
    }

    fn line(&self, label: &str, value: &str) -> String {
        let value = value.replace(['\r', '\n'], " ");
        format!("[{}] {label}={value}", self.prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn musts_decide_the_exit_code() {
        let rep = Report::new("TEST");
        rep.must("a", true, "");
        rep.log("v", 1);
        assert!(rep.failures().is_empty());
        let clone = rep.clone();
        clone.must("b", false, "why");
        rep.must("c", false, "");
        assert_eq!(rep.failures(), ["b", "c"]);
        rep.absorb(vec!["desktopReady".to_string()]);
        assert_eq!(rep.failures(), ["b", "c", "desktopReady"]);
        assert_eq!(rep.finish(), 1);
        assert_eq!(Report::new("TEST").finish(), 0);
        assert_eq!(Report::new("TEST").error("boom"), 1);
    }

    #[test]
    fn a_line_stays_one_line() {
        let rep = Report::new("SMOKE");
        assert_eq!(rep.line("x", "a\r\nb"), "[SMOKE] x=a  b");
        assert_eq!(rep.line("version", "1.1.1"), "[SMOKE] version=1.1.1");
    }
}
