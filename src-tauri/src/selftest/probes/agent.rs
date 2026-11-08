//! The agent lane's smoke labels, listed in `tools/smoke-labels/agent.txt`.
//! `selftest/mod.rs` documents the probe API, with a worked example.
//!
//! The gate tables and the runs on a canned desktop live in
//! `agent/smoke.rs`, where a unit test holds them to the same bar.

use crate::agent::smoke;
use crate::selftest::Cx;

pub fn run(cx: &mut Cx) {
    let (count, mut lines) = cx.block_on(smoke::gate_lines());
    lines.extend(cx.block_on(smoke::seam_lines()));
    cx.log("gateBypasses", count);
    for l in lines {
        cx.must(l.label, l.ok, l.why);
    }
}
