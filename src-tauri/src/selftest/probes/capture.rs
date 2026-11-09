//! The capture lane's smoke labels, listed in `tools/smoke-labels/capture.txt`.
//! `selftest/mod.rs` documents the probe API, with a worked example.
//!
//! Each label keeps the value Electron printed (main.js runSmoke,
//! lib/smoke-v2.js, lib/smoke-glass.js, lib/smoke-v2-desktop.js). Where that
//! value has a pass condition, it prints as is when the condition holds and
//! as `=FAIL:<value>` when not. The desktop selfcheck's labels are printed by
//! the harness probe, which runs it once; they are listed here.

use std::fmt::Display;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::Manager;

use crate::capture::{self, encode, screen, Rect};
use crate::llm::elements::{match_element, parse_elements};
use crate::selftest::{Cx, Input};
use crate::state::shared;
use crate::{backdrop, buddy, desktop, ipc, lens, region, store, windows};

pub fn run(cx: &mut Cx) {
    helper_not_at_boot(cx);
    whole_screen(cx);
    area(cx);
    selector_cancels(cx);
    selector_escape(cx);
    fail_null(cx);
    dpi_round_trip(cx);
    glass(cx);
    lens_first_draw(cx);
    backdrop_idle(cx);
    tour(cx);
}

/// `value` under `label` when `ok`, else `label=FAIL:<value>`.
fn check(cx: &Cx, label: &str, ok: bool, value: impl Display) {
    if ok {
        cx.log(label, value);
    } else {
        cx.must(label, false, value);
    }
}

/// smoke-v2.js B29: the helper starts on first use, never at boot.
fn helper_not_at_boot(cx: &Cx) {
    let at_boot = desktop::helper_at_boot();
    let why = match at_boot {
        Some(true) => "the desktop helper was running before anything used it",
        Some(false) => "",
        None => "the boot windows never reached Rust, so nothing was noted",
    };
    cx.must("helperNotAtBoot", at_boot == Some(false), why);
}

/// `capture` (main.js:3446) at physical pixels within the 1600 px budget
/// (B-032), and `captureActiveDisplay`: the shot is of the display the
/// person is on (D-09: the foreground window's, else the pointer's).
fn whole_screen(cx: &Cx) {
    let list = screen::all_displays();
    let active = screen::latched_display(&list);
    let Some(shot) = cx.block_on(capture::capture_screen_async(None)) else {
        cx.must("capture", false, "failed");
        cx.must("captureActiveDisplay", false, "no shot");
        return;
    };
    let p = active.phys;
    let want = encode::whole_screen_size(p.width as u32, p.height as u32);
    let value = format!("ok:{}bytes:{}x{}", shot.bytes, shot.width, shot.height);
    if (shot.width, shot.height) == want {
        cx.log("capture", value);
    } else {
        let why = format!("{value} want {}x{}", want.0, want.1);
        cx.must("capture", false, why);
    }
    let d = shot.display;
    let at = format!("{},{} {}x{}", d.x, d.y, d.width, d.height);
    check(cx, "captureActiveDisplay", d == active.bounds, at);
}

/// main.js:3672-3697: an area of the primary comes back tagged and at its
/// own pixels, it spends the budget on more pixels per point than the whole
/// screen, and a point in it is a fraction of the area.
fn area(cx: &Cx) {
    let d = screen::primary(&screen::all_displays());
    let bx = Rect::new(d.bounds.x + 40.0, d.bounds.y + 40.0, 320.0, 200.0);
    let area = cx.block_on(capture::capture_screen_async(Some(bx)));
    let full = cx.block_on(capture::capture_screen_async(None));
    let Some(a) = area else {
        cx.must("regionCapture", false, "failed");
        cx.must("regionSharper", false, "no area shot");
        cx.must("regionPointTarget", false, "null");
        return;
    };
    let s = d.scale_factor;
    let native = ((320.0 * s).round() as u32, (200.0 * s).round() as u32);
    let tagged = if a.region.is_some() {
        "tagged"
    } else {
        "untagged"
    };
    let ok = a.region == Some(bx) && (a.width, a.height) == native;
    let value = format!("{}x{}:{tagged}", a.width, a.height);
    check(cx, "regionCapture", ok, value);

    match full {
        Some(f) => {
            let full_ppd = f.width as f64 / d.bounds.width;
            let area_ppd = a.width as f64 / bx.width;
            let value = format!("{full_ppd:.2}x->{area_ppd:.2}x");
            check(cx, "regionSharper", area_ppd >= full_ppd, value);
        }
        None => cx.must("regionSharper", false, "no whole-screen shot"),
    }

    let r = a.region.unwrap_or_default();
    let x = (r.x + 0.5 * r.width).round() as i64;
    let y = (r.y + 0.5 * r.height).round() as i64;
    let want_x = (bx.x + 160.0).round() as i64;
    let want_y = (bx.y + 100.0).round() as i64;
    let centred = (x, y) == (want_x, want_y);
    let value = json!({ "x": x, "y": y });
    check(cx, "regionPointTarget", centred, value);
}

/// The area selector on the primary, picked on a thread of its own.
fn open_selector(cx: &Cx) -> JoinHandle<Option<Rect>> {
    let app = cx.app.clone();
    let d = screen::primary(&screen::all_displays());
    thread::spawn(move || {
        let pick = region::select(&app, Some(d));
        tauri::async_runtime::block_on(pick)
    })
}

/// The pick's answer, or `None` while it is still open after `within`.
fn answer(cx: &Cx, pick: JoinHandle<Option<Rect>>, within: Duration) -> Option<Option<Rect>> {
    if !cx.wait_for(within, || pick.is_finished()) {
        region::cancel(&cx.app);
        let _ = cx.wait_for(Duration::from_secs(2), || pick.is_finished());
        return None;
    }
    pick.join().ok()
}

/// main.js:3699-3701: the selector opens and cancels without stranding a
/// full-screen scrim.
fn selector_cancels(cx: &Cx) {
    let pick = open_selector(cx);
    let opened = cx.wait_for(Duration::from_secs(5), || region::is_active(&cx.app));
    region::cancel(&cx.app);
    let got = answer(cx, pick, Duration::from_secs(3));
    let gone = cx.wait_for(Duration::from_secs(2), || !region::is_active(&cx.app));
    let ok = opened && got == Some(None) && gone;
    let why = format!("opened={opened} pick={got:?} closed={gone}");
    let value = if ok { "true".to_string() } else { why };
    check(cx, "regionSelectorCancels", ok, value);
}

/// main.js:3702-3711, with a real Escape key: it dismisses the scrim, not
/// the panel behind it (B-028 claims Escape once the selector exists).
fn selector_escape(cx: &Cx) {
    let pick = open_selector(cx);
    let _ = cx.wait_for(Duration::from_secs(8), || region::ready(&cx.app));
    cx.sleep(300);
    let was = shared(&cx.app).lock().mode;
    let sent = cx.input(Input::Key("Escape"));
    let got = answer(cx, pick, Duration::from_secs(3));
    let now = shared(&cx.app).lock().mode;
    let (cancelled, kept) = (got == Some(None), now == was);
    let state = if cancelled { "cancelled" } else { "stuck" };
    let panel = if kept { "panel-kept" } else { "panel-lost" };
    let value = match sent {
        Ok(()) => format!("{state}:{panel}"),
        Err(e) => format!("{state}:{panel} (Escape not sent: {e})"),
    };
    check(cx, "regionEscape", cancelled && kept, value);
}

/// CAP-06 and B-110: a capture thread that hangs makes the capture come back
/// empty within its limit (the question goes on as text), and the next
/// capture runs on a fresh thread. The glass stands still meanwhile, so the
/// hung thread is ours alone to find.
fn fail_null(cx: &Cx) {
    backdrop::pause();
    cx.sleep(300);
    capture::stall_worker(Duration::from_secs(8));
    let t = Instant::now();
    let hung = cx.block_on(capture::capture_screen_async(None));
    let ms = t.elapsed().as_millis();
    let next = cx.block_on(capture::capture_screen_async(None));
    backdrop::resume();
    let first = if hung.is_some() { "a shot" } else { "null" };
    let then = if next.is_some() { "a shot" } else { "null" };
    let value = format!("{first} in {ms}ms, then {then}");
    let ok = hung.is_none() && ms < 4500 && next.is_some();
    check(cx, "captureFailNull", ok, value);
}

/// DESK-08: physical rects go to DIP and back to the same pixels, on the
/// live displays and on two-display layouts at 125 % and 150 %.
fn dpi_round_trip(cx: &Cx) {
    let px = |x, y, width, height| screen::PxRect {
        x,
        y,
        width,
        height,
    };
    let pair = |dpi: u32| {
        let main = screen::Display::from_phys(
            r"\\.\DISPLAY1",
            true,
            96,
            px(0, 0, 1920, 1080),
            px(0, 0, 1920, 1040),
        );
        let side = screen::Display::from_phys(
            r"\\.\DISPLAY2",
            false,
            dpi,
            px(1920, 0, 2560, 1440),
            px(1920, 0, 2560, 1400),
        );
        screen::with_layout(vec![main, side])
    };
    let layouts = [
        ("live", screen::all_displays()),
        ("125", pair(120)),
        ("150", pair(144)),
    ];
    let (mut n, mut bad) = (0, Vec::new());
    for (name, list) in &layouts {
        for d in list {
            let p = d.phys;
            let r = px(p.x + p.width / 4, p.y + p.height / 3, 301, 167);
            let dip = screen::screen_to_dip_rect(list, r);
            let back = screen::dip_to_screen_rect(list, dip);
            let off = [
                back.x - r.x,
                back.y - r.y,
                back.width - r.width,
                back.height - r.height,
            ];
            n += 1;
            if off.iter().any(|v| v.abs() > 1) {
                bad.push(format!("{name}:{}:{r:?}->{back:?}", d.device));
            }
        }
    }
    let value = if bad.is_empty() {
        format!("ok:{n} rects within 1px")
    } else {
        bad.join(" ")
    };
    check(cx, "dpiRoundTrip", bad.is_empty(), value);
}

/// smoke-glass.js: the glass pages have glass.js and a tier, a backdrop
/// capture costs the event loop under 25 ms at p95, and the process CPU with
/// the glass live and then solid.
fn glass(cx: &Cx) {
    let js = "(() => (window.AevalGlass ? 'glass' : 'none') + ':' \
              + (document.documentElement.dataset.glass || 'notier'))()";
    let mut seen = Vec::new();
    for (name, label) in [("overlay", windows::OVERLAY), ("output", "output-1")] {
        let r = match cx.eval(label, js) {
            Ok(Value::String(s)) => s,
            Ok(v) => v.to_string(),
            Err(e) => format!("error:{e}"),
        };
        seen.push(format!("{name}={r}"));
    }
    let tier = backdrop::tier();
    let mut reasons = backdrop::reasons().join("+");
    if reasons.is_empty() {
        reasons = "none".into();
    }
    let can_hide = capture::can_hide_from_capture();
    let value = format!("{tier} reasons={reasons} canHide={can_hide}");
    cx.log("glassTier", value);
    let has_tier = |s: &String| s.ends_with("=glass:live") || s.ends_with("=glass:solid");
    let loaded = seen.iter().all(has_tier);
    cx.must("glassLoaded", loaded, seen.join(" "));

    if tier == "live" {
        let m = backdrop::measure(50);
        let f = |k: &str| (m[k].as_f64().unwrap_or(f64::NAN) * 10.0).round() / 10.0;
        let block = f("blockP95");
        cx.log(
            "snapCost",
            format!(
                "n={} wallP50={}ms wallP95={}ms encodeP95={}ms blockP95={block}ms",
                m["n"],
                f("wallP50"),
                f("wallP95"),
                f("encodeP95")
            ),
        );
        let why = format!("main thread blocked {block}ms p95, budget 25ms");
        cx.must("snapCost", block < 25.0, why);
    } else {
        let why = format!("the glass is {tier} ({reasons}), so there is nothing to measure");
        cx.must("snapCost", false, why);
    }

    let live = cpu_over(Duration::from_secs(3));
    let was = backdrop::intensity();
    backdrop::set_intensity(0.0);
    cx.sleep(600);
    let solid = cpu_over(Duration::from_secs(3));
    backdrop::set_intensity(was);
    let how = "percent of one core over 3 s, the app process; solid is glassIntensity 0";
    let value = format!("live total={live:.1}% | solid total={solid:.1}% ({how})");
    cx.log("glassCost", value);
}

/// Kernel plus user CPU time of this process, in milliseconds.
fn cpu_ms() -> f64 {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let zero = || FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut c, mut e, mut k, mut u) = (zero(), zero(), zero(), zero());
    // SAFETY: four valid out pointers and the pseudo-handle of this process.
    let ok = unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
    if ok == 0 {
        return 0.0;
    }
    let ms = |f: FILETIME| (((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64) as f64;
    (ms(k) + ms(u)) / 10_000.0
}

/// This process's CPU over `d`, in percent of one core.
fn cpu_over(d: Duration) -> f64 {
    let (c0, t0) = (cpu_ms(), Instant::now());
    thread::sleep(d);
    (cpu_ms() - c0) / t0.elapsed().as_secs_f64() / 10.0
}

/// smoke-v2-desktop.js: a fresh lens told its hits and frame once, with
/// nothing sent after, draws them (`lensFirstDraw`), and the current hit's
/// why chip comes to rest clear of the other hits (`lensWhyClear`).
fn lens_first_draw(cx: &Cx) {
    let d = screen::primary(&screen::all_displays());
    let label = lens::label_for(d.id);
    let at = |x: f64, y: f64, width: f64| Rect::new(d.bounds.x + x, d.bounds.y + y, width, 18.0);
    let hit = |i: i64, kind: &str, rect: Rect, why: &str| {
        let why = match why {
            "" => Value::Null,
            w => json!(w),
        };
        lens::Hit {
            i,
            kind: kind.to_string(),
            rect: Some(rect),
            why,
            offscreen: false,
        }
    };
    lens::destroy(&cx.app);
    let first = vec![
        hit(0, "literal", at(140.0, 120.0, 64.0), ""),
        hit(1, "literal", at(260.0, 168.0, 64.0), ""),
        hit(2, "related", at(140.0, 216.0, 64.0), "smoke"),
    ];
    lens::set_hits(&cx.app, first, 1);
    lens::set_frame(&cx.app, Some(&d), "smoke");
    let read_js = "(() => document.querySelectorAll('.hit').length + ':' \
                   + document.getElementById('ring').classList.contains('on') + ':' \
                   + document.getElementById('frame').classList.contains('on'))()";
    let read = || match cx.eval(&label, read_js) {
        Ok(Value::String(s)) => s,
        Ok(v) => v.to_string(),
        Err(e) => format!("error:{e}"),
    };
    let drew = cx.wait_for(Duration::from_secs(5), || read() == "3:true:true");
    let last = read();
    cx.log("lensFirstDraw", format!("hits:ring:frame={last}"));
    let why = format!("hits:ring:frame={last} want 3:true:true");
    cx.must("lensFirstDraw", drew, why);
    // The page said so itself, once its listeners were up (B-034).
    let seen = cx.app.state::<ipc::Seen>().snapshot();
    let said = seen.get(&label).is_some_and(|c| c.contains("lens:ready"));
    let ready = said && lens::is_ready(&label);
    check(cx, "lensReady", ready, format!("said={said} ready={ready}"));

    let invoice = "\"Invoice\" relates to \"payment\": an invoice is a request for payment";
    let owe = "the total due is what you owe";
    let second = vec![
        hit(0, "literal", at(260.0, 168.0, 64.0), ""),
        hit(1, "related", at(140.0, 216.0, 92.0), invoice),
        hit(2, "concept", at(140.0, 264.0, 150.0), owe),
    ];
    lens::set_hits(&cx.app, second, 1);
    let why_js = "(() => { const w = document.getElementById('why'); \
                  if (!w.classList.contains('on')) return null; \
                  const a = w.getBoundingClientRect(); \
                  const over = [...document.querySelectorAll('.hit')] \
                  .filter((e) => e.dataset.i !== '1').filter((e) => { \
                  const b = e.getBoundingClientRect(); \
                  return b.left < a.right && b.right > a.left \
                  && b.top < a.bottom && b.bottom > a.top; }).length; \
                  return 'covers=' + over + ' at=' + Math.round(a.left) + ',' \
                  + Math.round(a.top); })()";
    let probe = || cx.eval(&label, why_js).ok().filter(|v| v.is_string());
    let _ = cx.wait_for(Duration::from_secs(3), || probe().is_some());
    // The chip glides into place; measure where it comes to rest.
    cx.sleep(400);
    let settled = probe().and_then(|v| v.as_str().map(String::from));
    let value = settled.clone().unwrap_or_else(|| "null".into());
    cx.log("lensWhyClear", &value);
    let clear = settled.is_some_and(|s| s.starts_with("covers=0 "));
    cx.must("lensWhyClear", clear, value);
    lens::clear(&cx.app);
    lens::set_frame(&cx.app, None, "");
    lens::destroy(&cx.app);
}

/// B-030: Find's hits hold the backdrop at its fast pace; once they are gone
/// and only the orb shows, it is back at 5 s within 6 s.
fn backdrop_idle(cx: &Cx) {
    let d = screen::primary(&screen::all_displays());
    let hit = lens::Hit {
        i: 0,
        kind: "literal".into(),
        rect: Some(Rect::new(
            d.bounds.x + 140.0,
            d.bounds.y + 120.0,
            64.0,
            18.0,
        )),
        why: Value::Null,
        offscreen: false,
    };
    lens::set_hits(&cx.app, vec![hit], 0);
    let fast = cx.wait_for(Duration::from_secs(5), || !backdrop::slow_paced());
    lens::clear(&cx.app);
    let t = Instant::now();
    let slow = cx.wait_for(Duration::from_secs(6), backdrop::slow_paced);
    let ms = t.elapsed().as_millis();
    let value = format!("fast={fast} slow={slow} after {ms}ms");
    check(cx, "backdropIdle", fast && slow, value);
}

fn point_at(x: f64, y: f64, label: &str) -> buddy::Point {
    buddy::Point {
        x,
        y,
        label: label.into(),
    }
}

/// Whether the pointer page is showing `message` within `within`.
fn pointer_shows(cx: &Cx, message: &str, within: Duration) -> bool {
    let js = format!(
        "document.getElementById('buddy').classList.contains('visible') && message === {}",
        json!(message)
    );
    cx.wait_for(within, || cx.eval(buddy::LABEL, &js) == Ok(json!(true)))
}

/// Kills the page's renderer through DevTools, as a crash would.
fn crash_renderer(cx: &Cx, label: &str) -> Result<(), String> {
    use ::windows::core::w;
    use webview2_com::CallDevToolsProtocolMethodCompletedHandler;

    let window = cx
        .app
        .get_webview_window(label)
        .ok_or_else(|| format!("no window {label}"))?;
    let sent = window.with_webview(|pw| {
        let done = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(|_, _| Ok(())));
        // SAFETY: COM calls on the webview's own thread, inside `with_webview`.
        unsafe {
            if let Ok(core) = pw.controller().CoreWebView2() {
                let _ = core.CallDevToolsProtocolMethod(w!("Page.crash"), w!("{}"), &done);
            }
        }
    });
    sent.map_err(|e| e.to_string())
}

/// main.js:3653-3658: a point the model named resolves through the reader's
/// elements, an unknown name is dropped, and the pointer appears on a window
/// built for it from the one send (B-034).
fn point_resolved(cx: &Cx) {
    buddy::destroy();
    let d = screen::primary(&screen::all_displays());
    let b = d.bounds;
    let report = "ELEMENTS:\n- Terminal tab | 480,905\n- Problems tab | 600,905";
    let elements = parse_elements(Some(report), None);
    let points: Vec<buddy::Point> = ["terminal tab", "nothing here"]
        .iter()
        .filter_map(|name| {
            let e = match_element(name, &elements)?;
            let x = (b.x + e.x / 1000.0 * b.width).round();
            let y = (b.y + e.y / 1000.0 * b.height).round();
            Some(point_at(x, y, name))
        })
        .collect();
    let named = points.len();
    buddy::show_points(points, Some(&d));
    let shown = pointer_shows(cx, "terminal tab", Duration::from_secs(8));
    let ok = named == 1 && shown && buddy::is_active();
    let value = if ok {
        "true".to_string()
    } else {
        format!("named={named} shown={shown}")
    };
    check(cx, "pointResolved", ok, value);
    buddy::cancel();
}

/// smoke-v2.js B23 on the tour's new order: what works without a key, the
/// card asking for one, the first question once there is one, the bar's
/// token (D-06). Runs without a key and then with a fake one; what was
/// stored is put back, and so is the bar, which the last step opens.
fn onboarding(cx: &Cx) {
    buddy::cancel();
    let s = store::get();
    let had = s.get_api_key();
    let mode = shared(&cx.app).lock().mode;
    s.clear_api_key();
    shared(&cx.app).call(&cx.app, |h| h.start_tour.as_ref());
    let _ = cx.wait_for(Duration::from_secs(2), buddy::is_active);
    let first = buddy::step_index();
    buddy::notify("find-hit");
    let second = buddy::step_index();
    s.set_api_key("sk-or-smoke-fake");
    buddy::notify("key-saved");
    let third = buddy::step_index();
    buddy::notify("first-answer");
    let fourth = buddy::step_index();
    buddy::cancel();
    if had.is_empty() {
        s.clear_api_key();
    } else {
        s.set_api_key(&had);
    }
    cx.sleep(300);
    if shared(&cx.app).lock().mode != mode {
        cx.on_main(windows::collapse_panel);
        let _ = cx.wait_for(Duration::from_secs(2), || {
            shared(&cx.app).lock().mode == mode
        });
    }
    let value = format!("{first}>{second}>{third}>{fourth}");
    check(cx, "onboardingSteps", value == "0>1>2>3", value);
}

/// B-031: the window is kept, so a new answer's points show at once, and the
/// agent's first cursor after a tour has no page to wait for.
fn reuse(cx: &Cx) {
    buddy::show_points(vec![point_at(120.0, 140.0, "first point")], None);
    let first = pointer_shows(cx, "first point", Duration::from_secs(5));
    let t = Instant::now();
    buddy::show_points(vec![point_at(160.0, 180.0, "second point")], None);
    let second = pointer_shows(cx, "second point", Duration::from_millis(500));
    let shown_ms = t.elapsed().as_millis();
    buddy::cancel();
    shared(&cx.app).call(&cx.app, |h| h.start_tour.as_ref());
    let _ = cx.wait_for(Duration::from_secs(2), buddy::is_active);
    buddy::cancel();
    let d = screen::primary(&screen::all_displays());
    let t = Instant::now();
    cx.block_on(buddy::agent_point((200.0, 200.0), "smoke", &d));
    let glide = Duration::from_millis(buddy::GLIDE_WAIT_MS);
    let waited = t.elapsed().saturating_sub(glide).as_millis();
    buddy::agent_clear();
    let value = format!("points in {shown_ms}ms, agent waited {waited}ms");
    check(cx, "buddyReuse", first && second && waited < 300, value);
}

/// B-114: a pointer page whose renderer dies comes back, and the window has
/// stopped eating clicks.
fn reload(cx: &Cx) {
    // The point goes in the quarter of the display away from the real
    // cursor, which earlier probes leave wherever their input ended: a page
    // whose bubble is under the cursor takes the mouse, as it should.
    let d = screen::primary(&screen::all_displays());
    let b = d.bounds;
    let cursor = screen::cursor_phys().unwrap_or((d.phys.x, d.phys.y));
    let left = f64::from(cursor.0 - d.phys.x) / d.scale_factor < b.width / 2.0;
    let top = f64::from(cursor.1 - d.phys.y) / d.scale_factor < b.height / 2.0;
    let x = if left {
        b.x + b.width - 360.0
    } else {
        b.x + 120.0
    };
    let y = if top {
        b.y + b.height - 260.0
    } else {
        b.y + 140.0
    };
    buddy::show_points(vec![point_at(x, y, "reload point")], Some(&d));
    let shown = pointer_shows(cx, "reload point", Duration::from_secs(5));
    buddy::set_click_through(false);
    let _ = cx.eval(buddy::LABEL, "(window.smokeBefore = 1)");
    let crashed = crash_renderer(cx, buddy::LABEL);
    let t = Instant::now();
    let new_page = "window.smokeBefore === undefined";
    // Each condition on its own, so a failure says which one held out. The
    // page check has a short timeout: a script started in a page that is
    // being replaced never settles, and a 5 s eval would use up the budget.
    let (mut through, mut ready, mut fresh) = (false, false, false);
    let back = cx.wait_for(Duration::from_secs(5), || {
        through = buddy::lets_mouse_through();
        ready = buddy::is_ready();
        if through && ready && !fresh {
            let short = Duration::from_millis(400);
            fresh = cx.eval_for(buddy::LABEL, new_page, short) == Ok(json!(true));
        }
        through && ready && fresh
    });
    let ms = t.elapsed().as_millis();
    let value = format!(
        "shown={shown} crash={crashed:?} back={back} in {ms}ms \
         (clickThrough={through} ready={ready} freshPage={fresh} cursor={cursor:?})"
    );
    check(cx, "buddyReload", shown && crashed.is_ok() && back, value);
    buddy::cancel();
}

/// main.js:3921-3926: model points show and finish, and the walkthrough
/// starts.
fn tour(cx: &Cx) {
    buddy::cancel();
    point_resolved(cx);
    let point = buddy::Point {
        x: 80.0,
        y: 80.0,
        label: "smoke point".into(),
    };
    buddy::show_points(vec![point], None);
    let shown = buddy::is_active();
    check(cx, "buddyPoints", shown, shown);
    buddy::advance();
    let done = !buddy::is_active();
    check(cx, "buddyPointsDone", done, done);
    shared(&cx.app).call(&cx.app, |h| h.start_tour.as_ref());
    let started = cx.wait_for(Duration::from_secs(2), buddy::is_active);
    check(cx, "walkthrough", started, started);
    buddy::cancel();
    onboarding(cx);
    reuse(cx);
    reload(cx);
}
