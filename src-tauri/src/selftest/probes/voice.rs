//! The voice lane's smoke labels, listed in `tools/smoke-labels/voice.txt`.
//! CI has no microphone, so every recording here reads the dictation
//! fixture (`capture::set_fixture`), a person saying "Um, so I think we
//! should meet at three, no, four pm tomorrow, to go over the budget."

use std::path::{Path, PathBuf};
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

/// transcribe.cpp is linked with the static C runtime, as the rest of the
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
/// GPU), in ms, about 1.5 times what whisper.cpp measured (a4a6634: 566 and
/// 790): Whisper on the fixture (7.9 s of speech) with the default model kept
/// loaded, and the whole release-to-text of a 17 s utterance with a pause.
/// transcribe.cpp measures 807 to 830 and 832 to 849 (3add819): inside both,
/// the first with little to spare (lanes/voice-transcribe.md).
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

/// Whether a pass heard the fixture's "tomorrow".
fn tomorrow(text: &Result<String, String>) -> bool {
    match text {
        Ok(t) => t.to_lowercase().contains("tomorrow"),
        Err(_) => false,
    }
}

/// Seconds of 16 kHz audio, to one place.
fn secs(samples: &[f32]) -> String {
    format!("{:.1}s", samples.len() as f32 / wav::RATE as f32)
}

/// One pass with the model at `path`: how long it took and what it heard.
fn pass(path: &Path, samples: &[f32]) -> (u64, Result<String, String>) {
    let t = Instant::now();
    let text = whisper::transcribe(path, samples, "en");
    (ms(t), text)
}

/// The pass just done, as a log line: its time, where the engine spent it
/// and the text.
fn said(took: u64, text: &Result<String, String>) -> String {
    let stages = whisper::stages();
    format!("transcribe={took}ms {stages} text={text:?}")
}

/// The default model on the fixture, kept loaded and with the silence cut;
/// then a three-second phrase, the smallest model, Parakeet on the same two
/// clips and a file an earlier build fetched. Last, the release-to-text time
/// of a long utterance through the real pipeline, which transcribes its first
/// half while it is held, with the default engine and with Parakeet.
fn speed(cx: &Cx) {
    let Ok(clip) = fixture_16k() else {
        cx.must("voiceSpeed", false, "the fixture did not read");
        return;
    };

    // Whisper base.en, loaded once (at app start, in the app).
    let base = model::spec_for("base", "en");
    let path = match cx.block_on(model::ensure(base, &|_, _| {})) {
        Ok(p) => p,
        Err(e) => {
            cx.must("voiceSpeed", false, e);
            return;
        }
    };
    let t0 = Instant::now();
    whisper::preload(&path);
    let load_ms = ms(t0);
    let t1 = Instant::now();
    let speech = engine::trim(&clip).unwrap_or(&clip);
    let trim_ms = ms(t1);
    let (run_ms, text) = pass(&path, speech);
    let total = trim_ms + run_ms;
    let kept = format!("{} of {} kept", secs(speech), secs(&clip));
    let heard = said(run_ms, &text);
    let line = format!("load={load_ms}ms (once) trim={trim_ms}ms ({kept}) {heard}");
    cx.log("voiceSpeedAfter", line);
    // A three-second phrase is most of dictation.
    let short = &clip[..clip.len().min(3 * wav::RATE as usize)];
    let (short_ms, phrase) = pass(&path, short);
    cx.log("voiceSpeedShort", said(short_ms, &phrase));

    let tiny = model::usable(model::spec_for("tiny", "en"));
    if let Some(tp) = tiny {
        whisper::preload(&tp);
        let (tiny_ms, tt) = pass(&tp, speech);
        cx.log("voiceSpeedTiny", said(tiny_ms, &tt));
        // Thread counts, on tiny to keep the run short.
        let mut sweep = Vec::new();
        for n in [1, 2, 4] {
            whisper::set_threads(n);
            let (took, _) = pass(&tp, speech);
            sweep.push(format!("{n}={took}ms"));
        }
        whisper::set_threads(0);
        cx.log("voiceSpeedThreads", sweep.join(" "));
    }
    parakeet(cx, &clip, short);
    legacy(cx, speech);
    // Back to the default model, as the app keeps it.
    whisper::preload(&path);
    cx.log("voiceCalibrate", format!("{}ms", calibrate()));
    let threads = whisper::threads();
    let system = whisper::system_info();
    cx.log("voiceSystem", format!("threads={threads} {system}"));
    cx.must(
        "voiceSpeed",
        tomorrow(&text) && total <= SPEED_BUDGET_MS,
        format!("{total}ms of {SPEED_BUDGET_MS}ms"),
    );

    let (started, timings, field) = e2e(cx, &clip, "whisper-base");
    let json = serde_json::to_string(&timings).unwrap_or_default();
    cx.log("voiceSpeedTimings", &json);
    cx.log("voiceSpeedHeard", format!("{field:?}"));
    let in_time = timings.is_some_and(|t| t.total_ms <= E2E_BUDGET_MS);
    cx.must(
        "voiceSpeedE2e",
        started && in_time,
        format!("started={started} {json} budget={E2E_BUDGET_MS}ms"),
    );

    let (_, timings, field) = e2e(cx, &clip, settings::PARAKEET);
    let json = serde_json::to_string(&timings).unwrap_or_default();
    cx.log("voiceSpeedParakeetTimings", &json);
    cx.log("voiceSpeedParakeetHeard", format!("{field:?}"));
}

/// Parakeet on the fixture and on its first three seconds, kept loaded as
/// the default model is. It is handed the clip as the app hands it, silence
/// left on; the same clip cut to the speech, and cut then given half a second
/// of quiet either side, are logged beside it.
fn parakeet(cx: &Cx, clip: &[f32], short: &[f32]) {
    let spec = model::parakeet();
    let path = match cx.block_on(model::ensure(spec, &|_, _| {})) {
        Ok(p) => p,
        Err(e) => {
            cx.must("voiceParakeet", false, e);
            return;
        }
    };
    let t0 = Instant::now();
    whisper::preload(&path);
    let load_ms = ms(t0);
    let whole = engine::piece(&path, clip).unwrap_or(clip);
    let (run_ms, text) = pass(&path, whole);
    let heard = said(run_ms, &text);
    // Once more: a slow first pass is not the model's speed.
    let (again_ms, _) = pass(&path, whole);
    let line = format!("load={load_ms}ms {heard} again={again_ms}ms");
    cx.log("voiceSpeedParakeet", line);
    let (short_ms, phrase) = pass(&path, short);
    cx.log("voiceSpeedParakeetShort", said(short_ms, &phrase));
    if let Some(speech) = engine::trim(clip) {
        let (cut_ms, cut) = pass(&path, speech);
        cx.log("voiceSpeedParakeetTrimmed", said(cut_ms, &cut));
        let quiet = vec![0.0; wav::RATE as usize / 2];
        let padded = [&quiet[..], speech, &quiet[..]].concat();
        let (pad_ms, pad) = pass(&path, &padded);
        cx.log("voiceSpeedParakeetPadded", said(pad_ms, &pad));
    }
    cx.must("voiceParakeet", tomorrow(&text), format!("{text:?}"));
}

/// A file an earlier build fetched for whisper.cpp: it is what runs until
/// its GGUF replacement has arrived, so it has to load and hear the fixture.
fn legacy(cx: &Cx, speech: &[f32]) {
    let Some(old) = model::by_id("tiny.en-q5_1") else {
        return;
    };
    let path = match cx.block_on(model::ensure(old, &|_, _| {})) {
        Ok(p) => p,
        Err(e) => {
            cx.log("voiceLegacyBin", format!("download: {e}"));
            return;
        }
    };
    let t0 = Instant::now();
    let engine = whisper::cold(&path);
    let load_ms = ms(t0);
    let t1 = Instant::now();
    let text = engine.and_then(|e| whisper::run(&e, speech, "en", ""));
    let run_ms = ms(t1);
    let line = format!("load={load_ms}ms transcribe={run_ms}ms text={text:?}");
    cx.log("voiceLegacyBin", line);
}

/// What the bar's field holds.
fn bar_text(cx: &Cx) -> String {
    cx.eval("overlay", "document.getElementById('query-input').value")
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The fixture, a second of silence and the fixture again, recorded through
/// the bar's mic path with `engine`: whether it started, its timings and the
/// text that reached the bar's field.
fn e2e(cx: &Cx, clip: &[f32], engine: &str) -> (bool, Option<voice::Timings>, String) {
    let mut long = clip.to_vec();
    long.resize(long.len() + wav::RATE as usize, 0.0);
    long.extend_from_slice(clip);
    let file = std::env::temp_dir().join(format!("aeval-voice-long-{}.wav", std::process::id()));
    if std::fs::write(&file, wav::encode(&long, wav::RATE)).is_err() {
        return (false, None, "the long clip did not write".into());
    }
    let mut s = settings::get();
    let was = s.clone();
    s.engine = engine.into();
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
    cx.sleep(300);
    let field = bar_text(cx);
    let _ = cx.eval(
        "overlay",
        "(() => { const f = document.getElementById('query-input'); f.value = ''; \
         f.dispatchEvent(new Event('input', { bubbles: true })); return true; })()",
    );
    (matches!(started, Some(Ok(()))), timings, field)
}
