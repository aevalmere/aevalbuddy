//! The harness's own labels (`tools/smoke-labels/harness.txt`): the version,
//! the scaffold pages reaching Rust, and the probe API checked end to end, so
//! a FAIL in another lane is about that lane's code and not the harness.
//! Then the desktop selfcheck (main.js:3833), whose labels the capture lane
//! lists.

use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::desktop::{self, protocol, selfcheck};
use crate::selftest::{input, probes, Cx, Input};
use crate::{capture, ipc, paths, windows};

pub fn run(cx: &mut Cx) {
    let app = cx.app.clone();
    let want: Vec<&str> = windows::SCAFFOLD_WINDOWS.iter().map(|(l, _)| *l).collect();
    let all_seen = || {
        let seen = app.state::<ipc::Seen>().snapshot();
        want.iter().all(|w| seen.contains_key(*w))
    };
    let ready = cx.wait_for(Duration::from_secs(30), all_seen);
    cx.log("version", &app.package_info().version);
    cx.log("userData", paths::user_data_dir().display());
    cx.log("webviewData", paths::webview_data_dir().display());
    let seen = app.state::<ipc::Seen>().snapshot();
    let pages: Vec<&str> = seen.keys().map(String::as_str).collect();
    cx.log("scaffold.pages", pages.join(","));
    cx.must("scaffold.bridge", ready, "a page never reached Rust");

    // A Promise in the overlay's page comes back as JSON.
    let js = "Promise.resolve(6 * 7).then((n) => [typeof window.aeval, n])";
    let got = cx.eval(windows::OVERLAY, js);
    let expected = Ok(json!(["object", 42]));
    cx.must("selftest.eval", got == expected, format!("{got:?}"));
    // SendInput reaches the real pointer.
    let to = (200, 200);
    let sent = cx.input(Input::Move(to.0, to.1));
    let at = input::cursor();
    let why = format!("{sent:?}, pointer at {at:?}");
    cx.must("selftest.input", sent.is_ok() && at == to, why);

    desktop_selfcheck(cx);
    // Throwaway (B-009): a failed must has to turn the Smoke step red.
    cx.must(
        "deliberateFailure",
        false,
        "throwaway commit proving the gate",
    );
}

/// The desktop helper starts, lists windows without ours and refuses a click
/// on the pill; on CI it also works a Notepad of its own (B-009 step 6).
fn desktop_selfcheck(cx: &mut Cx) {
    let own_point = pill_centre(&cx.app);
    cx.log("pillCentre", format!("{own_point:?}"));
    let own_handles: Vec<String> = cx
        .app
        .webview_windows()
        .values()
        .map(|w| protocol::handle_str(capture::hwnd_of(w) as u64))
        .collect();
    let options = selfcheck::Options {
        notepad: probes::notepad_enabled(),
        own_point,
        own_handles,
    };
    // It prints its own [SMOKE] lines; its failed musts count here too.
    let rep = selfcheck::Report::new("SMOKE");
    cx.block_on(selfcheck::run(desktop::client(), &rep, &options));
    cx.rep.absorb(rep.failures());
}

/// The pill's centre in physical pixels, while the overlay shows.
fn pill_centre(app: &AppHandle) -> Option<(i64, i64)> {
    let overlay = app.get_webview_window(windows::OVERLAY)?;
    if !overlay.is_visible().unwrap_or(false) {
        return None;
    }
    let r = windows::screen::dip_to_px(windows::bar_rect(app));
    let x = (r.x + r.width / 2.0).round() as i64;
    let y = (r.y + r.height / 2.0).round() as i64;
    Some((x, y))
}
