//! The agent lane's smoke labels, listed in `tools/smoke-labels/agent.txt`.
//! `selftest/mod.rs` documents the probe API, with a worked example.
//!
//! The gate tables and the runs on a canned desktop live in
//! `agent/smoke.rs`, where a unit test holds them to the same bar.

use super::notepad_enabled;
use crate::agent::{smoke, smoke_live};
use crate::selftest::Cx;

pub fn run(cx: &mut Cx) {
    let (count, mut lines) = cx.block_on(smoke::gate_lines());
    lines.extend(cx.block_on(smoke::seam_lines()));
    cx.log("gateBypasses", count);
    for l in lines {
        cx.must(l.label, l.ok, l.why);
    }
    if !notepad_enabled() {
        cx.log("agentDesktop", "skipped (not on CI)");
        return;
    }
    let Some(sv) = crate::host::try_services() else {
        cx.must("agentDesktop", false, "the app has no services");
        return;
    };
    let live = smoke_live::lines(cx.app.clone(), sv.desktop.clone());
    for l in cx.block_on(live) {
        cx.must(l.label, l.ok, l.why);
    }
}
