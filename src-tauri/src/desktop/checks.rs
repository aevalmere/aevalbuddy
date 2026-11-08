//! CI checks for split (c), run by the built exe on the runner:
//!
//! - `AEVAL_CAPTURE_CHECK=1`: lib/backdrop-fixture.html fills the screen, a
//!   content-protected magenta window sits on top, and the production
//!   capture path must show the fixture through it (excluded, not blanked)
//!   with both backends; then the whole-screen and area shots are checked for
//!   size, encoding and budget, the backdrop frames for size, and the event
//!   loop for stalls (CAP-02..08, GLASS-01/05/06).
//! - `AEVAL_DESKTOP_CHECK=1`: the desktop selfcheck (lib/desktop/selfcheck.js
//!   port) with the Notepad half, against a visible window of ours.
//!
//! Lines are `[CAPTURE] k=v` / `[DESKTOP] k=v`; the run ends with `done` or
//! `failed: <labels>` and exits 0 or 1.

use std::io::Write;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::json;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use super::selfcheck::{self, Report};
use crate::capture::{self, encode, screen, Rect, Rgb};
use crate::{backdrop, desktop};

pub fn maybe_run(app: &AppHandle) {
    let capture_check = std::env::var_os("AEVAL_CAPTURE_CHECK").is_some();
    let desktop_check = std::env::var_os("AEVAL_DESKTOP_CHECK").is_some();
    if !capture_check && !desktop_check {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        // Let the event loop settle and the scaffold windows load.
        std::thread::sleep(Duration::from_millis(1500));
        let rep = Report::new(if capture_check { "CAPTURE" } else { "DESKTOP" });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if capture_check {
                capture_checks(&app, &rep);
            } else {
                tauri::async_runtime::block_on(desktop_checks(&app, &rep));
            }
        }));
        if result.is_err() {
            rep.must("panic", false, "the check panicked");
        }
        desktop::shutdown();
        let fails = rep.failures();
        if fails.is_empty() {
            println!("[{}] done", rep.prefix);
        } else {
            println!("[{}] failed: {}", rep.prefix, fails.join(","));
        }
        let _ = std::io::stdout().flush();
        app.exit(if fails.is_empty() { 0 } else { 1 });
    });
}

/// A plain, capturable window showing the fixture page on `r` (physical).
fn page_window(
    app: &AppHandle,
    label: &str,
    kind: &str,
    r: screen::PxRect,
    protected: bool,
) -> Option<tauri::WebviewWindow> {
    let url = format!("http://tauri.localhost/lib/backdrop-fixture.html?kind={kind}");
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let tx = Mutex::new(Some(tx));
    let win = WebviewWindowBuilder::new(app, label, WebviewUrl::External(url.parse().ok()?))
        .title(label)
        .decorations(false)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .focused(false)
        .content_protected(protected)
        .visible(false)
        .data_directory(crate::paths::webview_data_dir())
        .on_page_load(move |_, p| {
            if matches!(p.event(), tauri::webview::PageLoadEvent::Finished) {
                if let Some(tx) = tx.lock().ok().and_then(|mut t| t.take()) {
                    let _ = tx.send(());
                }
            }
        })
        .build()
        .ok()?;
    super::win::place(&win, r);
    let _ = win.show();
    let _ = rx.recv_timeout(Duration::from_secs(15));
    if protected {
        capture::exclude_window(capture::hwnd_of(&win));
    }
    Some(win)
}

struct Probe {
    magenta: f64,
    black: f64,
    mean: f64,
}

fn probe(img: &Rgb, r: screen::PxRect, origin: screen::PxRect) -> Probe {
    let (mut m, mut b, mut sum, mut n) = (0u64, 0u64, 0f64, 0u64);
    let x0 = (r.x - origin.x).max(0) as u32;
    let y0 = (r.y - origin.y).max(0) as u32;
    let x1 = ((r.x + r.width - origin.x).max(0) as u32).min(img.width);
    let y1 = ((r.y + r.height - origin.y).max(0) as u32).min(img.height);
    for y in (y0..y1).step_by(2) {
        for x in (x0..x1).step_by(2) {
            let [pr, pg, pb] = img.pixel(x, y);
            if pr > 200 && pg < 60 && pb > 200 {
                m += 1;
            }
            if pr < 16 && pg < 16 && pb < 16 {
                b += 1;
            }
            sum += (pr as f64 + pg as f64 + pb as f64) / 3.0;
            n += 1;
        }
    }
    let n = n.max(1) as f64;
    Probe {
        magenta: m as f64 / n,
        black: b as f64 / n,
        mean: sum / n,
    }
}

/// Horizontal edge energy: how much fine detail an image keeps.
fn edge_energy(img: &Rgb) -> f64 {
    let mut e = 0f64;
    for y in (0..img.height).step_by(3) {
        for x in 1..img.width {
            let a = img.pixel(x, y)[1] as f64;
            let b = img.pixel(x - 1, y)[1] as f64;
            e += (a - b).abs();
        }
    }
    e / (img.width as f64 * (img.height as f64 / 3.0)).max(1.0)
}

/// RMSE (0..255) of Lanczos3 and of `box_resize` against the ideal samples
/// of a 1-D sine (period 8 px, amplitude 100 around 128) shrunk by 0.625.
fn stripe_errors() -> (f64, f64) {
    let (sw, sh, tw, th) = (512u32, 16u32, 320u32, 10u32);
    let wave = |x: f64| 128.0 + 100.0 * (x * std::f64::consts::TAU / 8.0).sin();
    let mut data = Vec::with_capacity((sw * sh * 3) as usize);
    for _ in 0..sh {
        for x in 0..sw {
            let v = wave(x as f64 + 0.5).round() as u8;
            data.extend_from_slice(&[v, v, v]);
        }
    }
    let src = Rgb {
        width: sw,
        height: sh,
        data,
    };
    let rmse = |img: &Rgb| {
        let (mut e, mut n) = (0f64, 0f64);
        let y = th / 2;
        // Skip the borders, where every filter clamps.
        for x in 8..tw - 8 {
            let ideal = wave((x as f64 + 0.5) * sw as f64 / tw as f64);
            let d = img.pixel(x, y)[1] as f64 - ideal;
            e += d * d;
            n += 1.0;
        }
        (e / n).sqrt()
    };
    (rmse(&src.resize(tw, th)), rmse(&box_resize(&src, tw, th)))
}

/// Area-average downscale (the blurry baseline Lanczos must beat).
fn box_resize(img: &Rgb, w: u32, h: u32) -> Rgb {
    let mut data = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        let sy0 = y * img.height / h;
        let sy1 = ((y + 1) * img.height / h).max(sy0 + 1);
        for x in 0..w {
            let sx0 = x * img.width / w;
            let sx1 = ((x + 1) * img.width / w).max(sx0 + 1);
            let mut acc = [0u64; 3];
            let mut n = 0u64;
            for yy in sy0..sy1 {
                for xx in sx0..sx1 {
                    let p = img.pixel(xx, yy);
                    for (a, v) in acc.iter_mut().zip(p) {
                        *a += v as u64;
                    }
                    n += 1;
                }
            }
            for a in acc {
                data.push((a / n.max(1)) as u8);
            }
        }
    }
    Rgb {
        width: w,
        height: h,
        data,
    }
}

fn decode_dims(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut d = jpeg_decoder::Decoder::new(bytes);
    d.decode().ok()?;
    let i = d.info()?;
    Some((i.width as u32, i.height as u32))
}

fn capture_checks(app: &AppHandle, rep: &Report) {
    let list = screen::all_displays();
    let d = screen::primary(&list);
    rep.log("windowsBuild", capture::windows_build());
    rep.must(
        "canHideFromCapture",
        capture::can_hide_from_capture(),
        "Windows older than 2004",
    );
    rep.log("displays", list.len());
    rep.log(
        "display",
        format!(
            "{}x{} phys, {}x{} dip, scale {}",
            d.phys.width, d.phys.height, d.bounds.width, d.bounds.height, d.scale_factor
        ),
    );

    // ---------------------------------------------- fixture + protected probe
    let fixture = page_window(app, "capture-fixture", "white", d.phys, false);
    rep.must(
        "fixtureShown",
        fixture.is_some(),
        "fixture window did not open",
    );
    let pr = screen::PxRect {
        x: d.phys.x + d.phys.width / 2 - 200,
        y: d.phys.y + d.phys.height / 2 - 120,
        width: 400,
        height: 240,
    };
    let probe_win = page_window(app, "capture-probe", "probe", pr, true);
    if let Some(w) = &probe_win {
        let _ = w.eval("document.documentElement.style.background='#ff00ff';document.body.style.background='#ff00ff';document.body.innerHTML='';");
    }
    rep.must(
        "probeShown",
        probe_win.is_some(),
        "protected probe window did not open",
    );
    std::thread::sleep(Duration::from_millis(1200));

    // A control strip of the fixture next to the probe: proves the capture
    // sees the page at all.
    let control = screen::PxRect {
        x: pr.x - 300,
        y: pr.y,
        width: 200,
        height: pr.height,
    };
    let mut passed_any = false;
    for backend in ["dxgi", "gdi"] {
        std::env::set_var("AEVAL_CAPTURE_BACKEND", backend);
        let t = Instant::now();
        match capture::grab(&d) {
            Ok(f) => {
                let img = f.rgb();
                let p = probe(&img, pr, d.phys);
                let c = probe(&img, control, d.phys);
                rep.log(
                    &format!("{backend}.frame"),
                    format!("{}x{} in {}ms", f.width, f.height, t.elapsed().as_millis()),
                );
                rep.log(
                    &format!("{backend}.probe"),
                    format!(
                        "magenta={:.3} black={:.3} mean={:.0} controlMean={:.0}",
                        p.magenta, p.black, p.mean, c.mean
                    ),
                );
                let excluded = p.magenta < 0.01;
                let not_blanked = p.black < 0.05 && p.mean > 150.0;
                let sees_page = c.mean > 150.0;
                let ok = excluded && not_blanked && sees_page;
                if backend == "gdi" {
                    rep.must(
                        "gdiExcludesOwnWithoutBlanking",
                        ok,
                        &format!(
                            "excluded={excluded} notBlanked={not_blanked} seesPage={sees_page}"
                        ),
                    );
                } else {
                    rep.log(
                        "dxgiExcludesOwnWithoutBlanking",
                        if ok { "ok" } else { "no" },
                    );
                }
                passed_any |= ok;
            }
            Err(e) => {
                if backend == "gdi" {
                    rep.must("gdiCapture", false, &e);
                } else {
                    rep.log("dxgi.unavailable", e);
                }
            }
        }
    }
    std::env::remove_var("AEVAL_CAPTURE_BACKEND");
    rep.must(
        "captureExcludesOwn",
        passed_any,
        "no backend left our window out cleanly",
    );

    // ---------------------------------------------- whole screen (CAP-02/05)
    // Point the "active display" at the primary for a stable answer.
    let t = Instant::now();
    match capture::capture_screen(None) {
        Some(shot) => {
            let active = screen::active_display(&screen::all_displays());
            let want = encode::whole_screen_size(active.bounds.width, active.bounds.height);
            let dims = decode_dims(&shot.jpeg);
            let b64 = base64::engine::general_purpose::STANDARD
                .decode(&shot.base64)
                .ok();
            rep.log(
                "capture",
                format!(
                    "{}x{} bytes={} q={} backend={} ms={}",
                    shot.width,
                    shot.height,
                    shot.bytes,
                    shot.quality,
                    capture::last_backend(),
                    t.elapsed().as_millis()
                ),
            );
            rep.must(
                "captureSize",
                (shot.width, shot.height) == want && shot.width.max(shot.height) <= 1600,
                &format!(
                    "got {}x{} want {}x{}",
                    shot.width, shot.height, want.0, want.1
                ),
            );
            rep.must(
                "captureJpeg",
                dims == Some((shot.width, shot.height)),
                &format!("decoded {dims:?}"),
            );
            rep.must(
                "captureBudget",
                shot.bytes <= encode::SHOT_BUDGET && shot.quality >= 56,
                &format!("bytes={} q={}", shot.bytes, shot.quality),
            );
            rep.must(
                "captureBase64",
                b64.map(|b| b.len()) == Some(shot.bytes),
                "base64 does not decode to the bytes",
            );
            rep.must(
                "captureDisplay",
                shot.display == active.bounds && shot.region.is_none(),
                "display bounds or region wrong",
            );
            let v = serde_json::to_value(&shot).unwrap_or_default();
            // The field set main.js returned (serde_json's Value orders keys
            // alphabetically; the page reads fields by name).
            let mut keys: Vec<&str> = v
                .as_object()
                .map(|o| o.keys().map(String::as_str).collect())
                .unwrap_or_default();
            keys.sort_unstable();
            rep.must(
                "captureShape",
                keys == ["base64", "bytes", "display", "height", "region", "width"],
                &keys.join(","),
            );
        }
        None => rep.must("capture", false, "capture_screen returned None"),
    }

    // ---------------------------------------------- area at native pixels
    let area = Rect::new(d.bounds.x + 40.0, d.bounds.y + 40.0, 600.0, 300.0);
    match capture::capture_screen(Some(area)) {
        Some(shot) => {
            let (ew, eh) = (
                (600.0 * d.scale_factor).round() as u32,
                (300.0 * d.scale_factor).round() as u32,
            );
            rep.log(
                "regionCapture",
                format!(
                    "{}x{} bytes={} q={}",
                    shot.width, shot.height, shot.bytes, shot.quality
                ),
            );
            rep.must(
                "regionNative",
                (shot.width, shot.height) == (ew, eh),
                &format!("got {}x{} want {ew}x{eh}", shot.width, shot.height),
            );
            rep.must(
                "regionEchoed",
                shot.region == Some(area),
                "region not echoed",
            );
            rep.must(
                "regionJpeg",
                decode_dims(&shot.jpeg) == Some((shot.width, shot.height)),
                "area JPEG does not decode",
            );
        }
        None => rep.must("regionCapture", false, "area capture returned None"),
    }

    // ---------------------------------------------- fidelity (CAP-13 proxy)
    if let Ok(f) = capture::grab(&d) {
        let full = f.rgb();
        // 0.625x, the shrink a 2560-wide desktop gets to reach 1600.
        let (tw, th) = (full.width * 5 / 8, full.height * 5 / 8);
        let lz = full.resize(tw, th);
        let bx = box_resize(&full, tw, th);
        let (el, eb) = (edge_energy(&lz), edge_energy(&bx));
        rep.log(
            "fidelity",
            format!(
                "{}x{} -> {tw}x{th} lanczosEdge={el:.2} boxEdge={eb:.2}",
                full.width, full.height
            ),
        );
    }
    // Edge energy on a real screen rewards aliasing as much as detail, so the
    // gate is a signal with a known answer: fine stripes (8 px period, 4 px
    // at 1600/2560 is still above Nyquist) shrunk by 0.625. Lanczos3 must
    // land close to the ideal samples and beat the crude area average.
    let (lz_err, bx_err) = stripe_errors();
    rep.log(
        "fidelityStripes",
        format!("lanczosRmse={lz_err:.2} boxRmse={bx_err:.2}"),
    );
    rep.must(
        "fidelityLanczos",
        lz_err < 6.0 && lz_err < bx_err,
        &format!("lanczos rmse {lz_err:.2}, box {bx_err:.2}"),
    );

    // ---------------------------------------------- backdrop (GLASS-01/06)
    match backdrop::grab_all() {
        Ok((frames, wall, enc)) => {
            let small = frames.get(&d.id).cloned();
            let dims = small.as_ref().and_then(|b| decode_dims(b));
            let want = (
                (d.bounds.width / 6.0).round() as u32,
                (d.bounds.height / 6.0).round() as u32,
            );
            rep.log(
                "glassFrame",
                format!("{dims:?} wall={wall:.1}ms encode={enc:.1}ms"),
            );
            rep.must(
                "glassFrameSize",
                dims == Some(want),
                &format!("got {dims:?} want {want:?}"),
            );
        }
        Err(e) => rep.must("glassFrameSize", false, &e),
    }
    let m = backdrop::measure(10);
    rep.log("snapCost", &m);
    let block = m["blockP95"].as_f64().unwrap_or(999.0);
    rep.must("snapCost", block < 25.0, &format!("blockP95={block:.1}ms"));
    rep.log("glassTier", backdrop::tier());

    // Fixture mode (GLASS-05) goes through the same window + capture path.
    if let Some(w) = fixture {
        let _ = w.destroy();
    }
    match backdrop::use_fixture(Some("wallpaper")) {
        Ok(()) => {
            let s = backdrop::status();
            rep.must(
                "glassFixture",
                s["fixture"] == json!("wallpaper"),
                &s.to_string(),
            );
            let url = backdrop::fixture_url();
            rep.must(
                "glassFixtureUrl",
                url.starts_with("http://aeval-frame.localhost/fixture-full/"),
                &url,
            );
            let _ = backdrop::use_fixture(None);
        }
        Err(e) => rep.must("glassFixture", false, &e),
    }
    if let Some(w) = probe_win {
        let _ = w.destroy();
    }
}

async fn desktop_checks(app: &AppHandle, rep: &Report) {
    let list = screen::all_displays();
    let d = screen::primary(&list);
    // A visible window of ours for the BLOCKED_OWN_WINDOW check.
    let r = screen::PxRect {
        x: d.phys.x + 60,
        y: d.phys.y + 60,
        width: 360,
        height: 220,
    };
    let own = page_window(app, "desktop-probe", "white", r, true);
    rep.must("ownProbeShown", own.is_some(), "probe window did not open");
    tokio::time::sleep(Duration::from_millis(800)).await;
    let handles: Vec<String> = app
        .webview_windows()
        .values()
        .map(|w| super::protocol::handle_str(capture::hwnd_of(w) as u64))
        .collect();
    rep.log("ownHandles", handles.len());
    let notepad = std::env::var("AEVAL_DESKTOP_NOTEPAD")
        .map(|v| v != "0")
        .unwrap_or(true);
    selfcheck::run(
        desktop::client(),
        rep,
        &selfcheck::Options {
            notepad,
            own_point: own
                .as_ref()
                .map(|_| ((r.x + r.width / 2) as i64, (r.y + r.height / 2) as i64)),
            own_handles: handles,
        },
    )
    .await;
    rep.log("desktopStatus", desktop::client().status());
    if let Some(w) = own {
        let _ = w.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lanczos_keeps_fine_stripes() {
        let (lz, bx) = stripe_errors();
        assert!(lz < 6.0 && lz < bx, "lanczos {lz:.2} box {bx:.2}");
    }
}