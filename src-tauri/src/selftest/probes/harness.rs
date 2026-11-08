//! The harness's own labels (`tools/smoke-labels/harness.txt`): the version,
//! the scaffold pages reaching Rust, and the probe API checked end to end, so
//! a FAIL in another lane is about that lane's code and not the harness.

use std::time::Duration;

use serde_json::json;
use tauri::Manager;

use crate::selftest::{input, Cx, Input};
use crate::{ipc, paths, windows};

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
}
