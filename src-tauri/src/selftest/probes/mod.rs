//! One probe per lane. `probes/<lane>.rs` belongs to that lane, as does
//! `tools/smoke-labels/<lane>.txt`, which lists every label it prints. The
//! probes run in the order of [`LANES`], each inside its own `catch_unwind`;
//! the `v2desktop` lanes run last, with the smoke's Notepad open.

pub mod agent;
pub mod bar;
pub mod capture;
pub mod harness;
pub mod llm;
pub mod platform;
pub mod settings;
pub mod sheet;
// voice lane
pub mod voice;
pub mod windows;

use std::collections::BTreeMap;
use std::panic::{self, AssertUnwindSafe};

use super::{panic_text, Cx};

/// A lane's entry point.
pub type Probe = fn(&mut Cx);

/// Every lane's probe, in the order they run: the harness first (it checks
/// the API the others use, then runs the desktop selfcheck), the `v2desktop`
/// lanes last.
pub const LANES: [(&str, Probe); 10] = [
    ("harness", harness::run),
    ("platform", platform::run),
    ("windows", windows::run),
    ("capture", capture::run),
    ("bar", bar::run),
    ("sheet", sheet::run),
    ("settings", settings::run),
    ("agent", agent::run),
    ("llm", llm::run),
    // voice lane: works in the smoke's Notepad, so it runs last.
    ("voice", voice::run),
];

/// Electron's four block labels (main.js:3881-3914), each standing for a
/// group of lanes: `=ok` when every probe in it returned, and
/// `=FAIL:<lane>: <message>` when one panicked. `v2desktop` is the Notepad
/// run, so off CI it prints `skipped (not on CI)` instead; on CI the
/// Notepad opens before its first lane (`v2NotepadOpen`) and closes after
/// every lane has run (`v2NotepadClosed`).
pub const BLOCKS: [(&str, &[&str]); 4] = [
    ("v2basics", &["platform", "windows", "bar"]),
    ("v21basics", &["agent"]),
    ("glassGates", &["capture", "sheet", "settings"]),
    ("v2desktop", NOTEPAD_LANES),
];

/// The `v2desktop` block: the lanes that work in the smoke's Notepad.
const NOTEPAD_LANES: &[&str] = &["llm", "voice"]; // voice lane

/// The Notepad half of the smoke runs on CI, or when asked (main.js:3837).
pub fn notepad_enabled() -> bool {
    let asked = std::env::var("AEVAL_DESKTOP_NOTEPAD");
    asked.is_ok_and(|v| v != "0") || std::env::var_os("GITHUB_ACTIONS").is_some()
}

/// Runs every lane's probe, then prints the block labels.
pub fn run_all(cx: &mut Cx) {
    let mut panicked = BTreeMap::new();
    for (lane, probe) in LANES {
        if NOTEPAD_LANES.contains(&lane) {
            let _ = cx.notepad();
        }
        let label = format!("probe.{lane}");
        match panic::catch_unwind(AssertUnwindSafe(|| probe(cx))) {
            Ok(()) => cx.must(&label, true, ""),
            Err(e) => {
                let message = panic_text(&*e);
                cx.must(&label, false, &message);
                panicked.insert(lane, message);
            }
        }
    }
    cx.close_notepad();
    for (block, lanes) in BLOCKS {
        if block == "v2desktop" && !notepad_enabled() {
            cx.log(block, "skipped (not on CI)");
            continue;
        }
        let why: Vec<String> = lanes
            .iter()
            .filter_map(|l| panicked.get(l).map(|m| format!("{l}: {m}")))
            .collect();
        cx.must(block, why.is_empty(), why.join("; "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_lane_runs_once_and_sits_in_at_most_one_block() {
        let lanes: Vec<&str> = LANES.iter().map(|(l, _)| *l).collect();
        let mut sorted = lanes.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 10); // voice lane
        assert_eq!(lanes[0], "harness");
        let grouped: Vec<&str> = BLOCKS.iter().flat_map(|b| b.1.iter().copied()).collect();
        for lane in &lanes[1..] {
            let n = grouped.iter().filter(|g| *g == lane).count();
            assert_eq!(n, 1, "{lane} is in {n} blocks");
        }
        // The Notepad lanes are the last ones, so it opens and closes once.
        let tail = &lanes[lanes.len() - NOTEPAD_LANES.len()..];
        assert!(NOTEPAD_LANES.iter().all(|l| tail.contains(l)), "{tail:?}");
    }
}
