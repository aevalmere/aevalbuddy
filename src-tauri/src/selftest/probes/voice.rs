//! The voice lane's smoke labels, listed in `tools/smoke-labels/voice.txt`.
//! CI has no microphone, so every recording here reads the dictation
//! fixture (`capture::set_fixture`), a person saying "Um, so I think we
//! should meet at three, no, four pm tomorrow, to go over the budget."

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::desktop;
use crate::selftest::{notepad, Cx, Input};
use crate::voice::{self, capture, engine, insert, model, pill, settings, wav, whisper, Origin};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/voice/fixtures/dictation.wav"
);
const CLIP: &str = "voice probe clipboard";

pub fn run(cx: &mut Cx) {
    runtime(cx);
    let before = crate::store::get().raw(settings::KEY);
    // The small model keeps the run quick; the default is base.
    let mut s = settings::get();
    s.engine = "whisper-tiny".into();
    s.language = "en".into();
    settings::save(&s);
    capture::set_fixture(Some(PathBuf::from(FIXTURE)));

    if transcribes(cx) {
        bar_field(cx);
        dictation(cx);
        speed(cx);
    }

    capture::set_fixture(None);
    crate::store::get().set_raw(settings::KEY, before.unwrap_or(Value::Null));
}

/// whisper.cpp is linked with the static C runtime, as the rest of the
/// exe, so it starts on a PC without the Visual C++ redistributable.
fn runtime(cx: &Cx) {
    let exe = std::env::current_exe().and_then(std::fs::read);
    let dlls = exe
        .map(|b| imports(&b).unwrap_or_default())
        .unwrap_or_default();
    let bad: Vec<&String> = dlls
        .iter()
        .filter(|d| d.starts_with("msvcp") || d.starts_with("vcomp") || d.starts_with("vcruntime"))
        .collect();
    cx.must(
        "voiceNoCppRuntime",
        !dlls.is_empty() && bad.is_empty(),
        format!("imports {bad:?} of {}", dlls.len()),
    );
}

/// The DLL names in a PE32+ file's import table, lowercased.
fn imports(b: &[u8]) -> Option<Vec<String>> {
    let u16_at = |o: usize| b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]));
    let u32_at = |o: usize| {
        b.get(o..o + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    };
    let pe = u32_at(0x3C)? as usize;
    let sections = u16_at(pe + 6)? as usize;
    let opt = pe + 24;
    let opt_size = u16_at(pe + 20)? as usize;
    let import_rva = u32_at(opt + 112 + 8)?;
    let table = pe + 24 + opt_size;
    let to_file = |rva: u32| -> Option<usize> {
        (0..sections).find_map(|i| {
            let s = table + i * 40;
            let (size, va, raw) = (u32_at(s + 8)?, u32_at(s + 12)?, u32_at(s + 20)?);
            (rva >= va && rva < va + size.max(1)).then(|| (rva - va + raw) as usize)
        })
    };
    let mut at = to_file(import_rva)?;
    let mut out = Vec::new();
    while let Some(name) = u32_at(at + 12).filter(|n| *n != 0) {
        let start = to_file(name)?;
        let end = b[start..].iter().position(|c| *c == 0)? + start;
        out.push(String::from_utf8_lossy(&b[start..end]).to_lowercase());
        at += 20;
    }
    Some(out)
}

/// The model downloads and checks out, and hears the fixture.
fn transcribes(cx: &Cx) -> bool {
    let spec = model::spec_for("tiny", "en");
    let got = cx.block_on(model::ensure(spec, &|_, _| {}));
    let ready = got.is_ok() && model::ready(spec);
    cx.must("voiceModel", ready, format!("{got:?}"));
    if !ready {
        return false;
    }
    let samples = std::fs::read(FIXTURE)
        .map_err(|e| e.to_string())
        .and_then(|b| wav::read(&b))
        .map(|(s, rate)| wav::resample(&s, rate, wav::RATE));
    let text = samples.and_then(|s| whisper::transcribe(&model::path(spec), &s, "en"));
    cx.log("voiceHeard", format!("{text:?}"));
    let heard = text
        .as_ref()
        .is_ok_and(|t| t.to_lowercase().contains("tomorrow"));
    cx.must("voiceTranscribe", heard, format!("{text:?}"));
    heard
}

/// The bar's mic: start, stop, and the words land in the field.
fn bar_field(cx: &Cx) {
    cx.on_main(|app| crate::windows::open_bar(app, None, None));
    cx.sleep(400);
    let field = || {
        cx.eval("overlay", "document.getElementById('query-input').value")
            .ok()
            .and_then(|v| v.as_str().map(str::to_lowercase))
            .unwrap_or_default()
    };
    let mic = |what: &str| {
        cx.eval(
            "overlay",
            &format!("document.getElementById('mic-btn').{what}"),
        )
    };
    let _ = mic("click()");
    let listening = cx.wait_for(Duration::from_secs(3), || {
        mic("dataset.state") == Ok(json!("listening"))
    });
    cx.sleep(600);
    let _ = mic("click()");
    let landed = cx.wait_for(Duration::from_secs(40), || field().contains("tomorrow"));
    cx.must(
        "voiceBarField",
        listening && landed,
        format!("listening={listening} field={:?}", field()),
    );
    let _ = cx.eval(
        "overlay",
        "(() => { const f = document.getElementById('query-input'); f.value = ''; \
         f.dispatchEvent(new Event('input', { bubbles: true })); return true; })()",
    );
    cx.on_main(crate::windows::collapse_panel);
    cx.sleep(300);
}

fn notepad_text(cx: &Cx, handle: &str) -> String {
    let h = json!(handle);
    let got = cx.block_on(desktop::client().get_text(Some(&h), &Value::Null, 50_000, false));
    got.ok()
        .and_then(|v| v["text"].as_str().map(str::to_string))
        .unwrap_or_default()
}

/// A key down or up as a person's keyboard sends it (not marked as ours,
/// so the dictation hook sees it).
fn key(vk: u16, up: bool) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse as km;
    let mut flags = if up { km::KEYEVENTF_KEYUP } else { 0 };
    if vk == 0x5B {
        flags |= km::KEYEVENTF_EXTENDEDKEY;
    }
    // SAFETY: a plain lookup, then one initialised INPUT.
    unsafe {
        let scan = km::MapVirtualKeyW(vk as u32, km::MAPVK_VK_TO_VSC) as u16;
        let i = km::INPUT {
            r#type: km::INPUT_KEYBOARD,
            Anonymous: km::INPUT_0 {
                ki: km::KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        km::SendInput(1, &i, std::mem::size_of::<km::INPUT>() as i32);
    }
}

/// Hold Ctrl+Win in Notepad: the pill shows, the words are typed at the
/// caret, and the clipboard is as it was.
fn dictation(cx: &mut Cx) {
    let Some(handle) = cx.notepad().map(|n| n.handle.clone()) else {
        return;
    };
    let _ = cx.block_on(desktop::client().focus_window(&json!(handle)));
    cx.sleep(300);
    let _ = cx.input(Input::Key("Ctrl+End"));
    let _ = insert::set_clipboard_text(CLIP);

    const CTRL: u16 = 0xA2;
    const WIN: u16 = 0x5B;
    key(CTRL, false);
    key(WIN, false);
    let shown = cx.wait_for(Duration::from_secs(3), || {
        let up = cx.on_main(pill::visible).unwrap_or(false);
        let state = cx.eval(pill::LABEL, "document.body.dataset.state");
        up && state == Ok(json!("listening"))
    });
    cx.sleep(900);
    key(WIN, true);
    key(CTRL, true);
    cx.must("voicePill", shown, "the pill did not show listening");

    let typed = cx.wait_for(Duration::from_secs(40), || {
        notepad_text(cx, &handle)
            .to_lowercase()
            .contains("tomorrow")
    });
    cx.must(
        "voiceDictation",
        typed,
        format!("{:?}", notepad_text(cx, &handle)),
    );
    cx.sleep(800);
    let clip = insert::clipboard_text();
    cx.must(
        "voiceClipboard",
        clip.as_deref() == Some(CLIP),
        format!("{clip:?}"),
    );

    // Put the file's text back for the lanes after.
    let _ = insert::set_clipboard_text(notepad::TEXT);
    let _ = cx.block_on(desktop::client().focus_window(&json!(handle)));
    let _ = cx.input(Input::Key("Ctrl+A"));
    let _ = cx.input(Input::Key("Ctrl+V"));
    cx.sleep(300);
    let _ = cx.on_main(|app| voice::cancel(app, true));
}

/// What the speed gate allows on the runner (4 vCPUs of an EPYC 9V74, no
/// GPU), in ms, about 1.5 times what it measured (a4a6634: 566 and 790):
/// Whisper on the fixture (7.9 s of speech) with the default model kept
/// loaded, and the whole release-to-text of a 17 s utterance with a pause.
const SPEED_BUDGET_MS: u64 = 900;
const E2E_BUDGET_MS: u64 = 1100;

fn fixture_16k() -> Result<Vec<f32>, String> {
    std::fs::read(FIXTURE)
        .map_err(|e| e.to_string())
        .and_then(|b| wav::read(&b))
        .map(|(s, rate)| wav::resample(&s, rate, wav::RATE))
}

/// A fixed piece of arithmetic on one core (about 0.1 s on an idle modern
/// core): how much of the processor the probe gets right now, so a slow
/// Whisper time can be told apart from a busy runner.
fn calibrate() -> u64 {
    let t = Instant::now();
    let mut acc = [1.0f32; 8];
    for i in 0..40_000_000u32 {
        let x = std::hint::black_box(i as f32 * 1e-9);
        for a in acc.iter_mut() {
            *a = *a * 0.999_999 + x;
        }
    }
    std::hint::black_box(acc);
    ms(t)
}

fn ms(since: Instant) -> u64 {
    since.elapsed().as_millis() as u64
}

/// Before and after on the same clip: earlier builds' path (the f16
/// base.en model loaded cold, Whisper's full 30 s window, no trimming) and
/// this build's (the q5_1 model kept loaded, silence cut, the window fitted
/// to the clip). Then the release-to-text time of a long utterance through
/// the real pipeline, which transcribes its first half while it is held.
fn speed(cx: &Cx) {
    let Ok(clip) = fixture_16k() else {
        cx.must("voiceSpeed", false, "the fixture did not read");
        return;
    };
    let lower =
        |r: &Result<String, String>| r.as_ref().map(|t| t.to_lowercase()).unwrap_or_default();

    // Before.
    let old = model::by_id("base.en").expect("the f16 base.en spec");
    let got = cx.block_on(model::ensure(old, &|_, _| {}));
    if let Ok(path) = got {
        let t0 = Instant::now();
        let ctx = whisper::cold(&path, false);
        let load_ms = ms(t0);
        if let Ok(ctx) = ctx {
            let t1 = Instant::now();
            let text = whisper::run(&ctx, &clip, "en", "", false);
            ctx.print_timings();
            cx.log(
                "voiceSpeedBefore",
                format!("load={load_ms}ms transcribe={}ms text={text:?}", ms(t1)),
            );
        }
    }

    // After: base.en q5_1, loaded once (at app start, in the app).
    let new = model::spec_for("base", "en");
    let path = match cx.block_on(model::ensure(new, &|_, _| {})) {
        Ok(p) => p,
        Err(e) => {
            cx.must("voiceSpeed", false, e);
            return;
        }
    };
    let t0 = Instant::now();
    whisper::preload(&path);
    let load_ms = ms(t0);
    whisper::print_timings();
    let t1 = Instant::now();
    let speech = engine::trim(&clip).unwrap_or(&clip);
    let trim_ms = ms(t1);
    let t2 = Instant::now();
    let text = whisper::transcribe(&path, speech, "en");
    let run_ms = ms(t2);
    whisper::print_timings();
    let total = trim_ms + run_ms;
    cx.log(
        "voiceSpeedAfter",
        format!(
            "load={load_ms}ms (once) trim={trim_ms}ms ({:.1}s of {:.1}s kept) transcribe={run_ms}ms text={text:?}",
            speech.len() as f32 / wav::RATE as f32,
            clip.len() as f32 / wav::RATE as f32
        ),
    );
    // A three-second phrase is most of dictation, and where a short encoder
    // window once made Whisper loop (tools/voice-bench on bench/parakeet).
    let short = &clip[..clip.len().min(3 * wav::RATE as usize)];
    let t3 = Instant::now();
    let heard = whisper::transcribe(&path, short, "en");
    cx.log(
        "voiceSpeedShort",
        format!("transcribe={}ms text={heard:?}", ms(t3)),
    );
    let tiny = model::usable(model::spec_for("tiny", "en"));
    if let Some(tp) = tiny {
        whisper::preload(&tp);
        let t = Instant::now();
        let tt = whisper::transcribe(&tp, speech, "en");
        cx.log(
            "voiceSpeedTiny",
            format!("transcribe={}ms text={tt:?}", ms(t)),
        );
        // Thread counts, on tiny to keep the run short.
        let mut sweep = Vec::new();
        for n in [1, 2, 4] {
            whisper::set_threads(n);
            let t = Instant::now();
            let _ = whisper::transcribe(&tp, speech, "en");
            sweep.push(format!("{n}={}ms", ms(t)));
        }
        whisper::set_threads(0);
        cx.log("voiceSpeedThreads", sweep.join(" "));
        if let Ok(ctx) = whisper::cold(&tp, true) {
            cx.log("voiceSpeedBreakdown", whisper::breakdown(&ctx, speech));
        }
        whisper::preload(&path);
    }
    cx.log("voiceCalibrate", format!("{}ms", calibrate()));
    cx.log(
        "voiceSystem",
        format!(
            "threads={} {}",
            whisper::threads(),
            whisper::system_info().trim()
        ),
    );
    cx.must(
        "voiceSpeed",
        lower(&text).contains("tomorrow") && total <= SPEED_BUDGET_MS,
        format!("{total}ms of {SPEED_BUDGET_MS}ms"),
    );

    e2e(cx, &clip);
}

/// The fixture, a second of silence and the fixture again, recorded through
/// the bar's mic path with the default engine.
fn e2e(cx: &Cx, clip: &[f32]) {
    let mut long = clip.to_vec();
    long.resize(long.len() + wav::RATE as usize, 0.0);
    long.extend_from_slice(clip);
    let file = std::env::temp_dir().join(format!("aeval-voice-long-{}.wav", std::process::id()));
    if std::fs::write(&file, wav::encode(&long, wav::RATE)).is_err() {
        cx.must("voiceSpeedE2e", false, "the long clip did not write");
        return;
    }
    let mut s = settings::get();
    let was = s.clone();
    s.engine = "whisper-base".into();
    s.cleanup = false;
    settings::save(&s);
    capture::set_fixture(Some(file.clone()));
    let _ = voice::take_timings();
    let started = cx.on_main(|app| voice::start(app, Origin::Bar));
    let held = long.len() as u64 * 1000 / wav::RATE as u64 + 300;
    cx.sleep(held);
    let _ = cx.on_main(|app| voice::finish(app, Origin::Bar));
    let mut timings = None;
    let _ = cx.wait_for(Duration::from_secs(30), || {
        timings = voice::take_timings();
        timings.is_some()
    });
    capture::set_fixture(Some(PathBuf::from(FIXTURE)));
    settings::save(&was);
    let _ = std::fs::remove_file(&file);
    let json = serde_json::to_string(&timings).unwrap_or_default();
    cx.log("voiceSpeedTimings", &json);
    let ok = matches!(started, Some(Ok(())))
        && timings
            .as_ref()
            .is_some_and(|t| t.total_ms <= E2E_BUDGET_MS);
    cx.must(
        "voiceSpeedE2e",
        ok,
        format!("started={started:?} {json} budget={E2E_BUDGET_MS}ms"),
    );
}
