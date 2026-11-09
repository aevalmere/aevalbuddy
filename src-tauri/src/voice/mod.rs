//! Voice input (entries/voice.md): the bar's mic and dictation in any app.
//!
//! One utterance at a time. The bar's mic and the dictation chord both
//! start a recording here. While it records, the part before each pause is
//! transcribed already (`stream`); on release the rest is (Whisper on this
//! PC by default), hesitations are taken out, AI cleanup runs when the
//! person turned it on, and the text is handed to the bar's field or typed
//! into the app that has the focus. Every utterance's stage timings are
//! kept for the speed probe ([`last_timings`]). Nothing is written to disk
//! and nothing outlives the utterance.

pub mod capture;
pub mod cleanup;
pub mod engine;
pub mod gemini;
pub mod insert;
pub mod ipc;
pub mod model;
pub mod pill;
pub mod ptt;
pub mod settings;
pub mod stream;
pub mod vad;
pub mod wav;
pub mod whisper;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::AppHandle;

pub const NOTHING_HEARD: &str = "I didn't hear anything. Hold the key and speak.";
/// A shorter hold is a tap of the chord, not dictation.
const MIN_HOLD: Duration = Duration::from_millis(250);
/// Below this, the loudest tenth of a second was room noise.
const QUIET: f32 = 0.12;

/// Where an utterance came from and where its text goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Bar,
    Dictation,
}

impl Origin {
    pub fn name(self) -> &'static str {
        match self {
            Origin::Bar => "bar",
            Origin::Dictation => "dictation",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Origin::Bar => crate::windows::OVERLAY,
            Origin::Dictation => pill::LABEL,
        }
    }
}

struct Live {
    id: u64,
    origin: Origin,
    recorder: capture::Recorder,
    started: Instant,
    handsfree: bool,
    streamer: Option<stream::Streamer>,
}

/// Where the time went in one utterance, from the release to the text being
/// in, in milliseconds.
#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timings {
    /// Seconds recorded.
    pub audio_s: f32,
    /// Seconds transcribed while the key was held, in `pieces` pieces.
    pub early_s: f32,
    pub pieces: usize,
    /// Whisper time spent on those pieces before the release (not part of
    /// the total).
    pub early_ms: u64,
    /// Stopping the microphone and resampling.
    pub stop_ms: u64,
    /// Waiting for a piece that was being transcribed at the release.
    pub wait_ms: u64,
    /// Transcribing what was left (and downloading the model, the first
    /// time).
    pub transcribe_ms: u64,
    pub cleanup_ms: u64,
    pub insert_ms: u64,
    pub total_ms: u64,
}

static LAST_TIMINGS: Mutex<Option<Timings>> = Mutex::new(None);

/// The last utterance's timings.
pub fn last_timings() -> Option<Timings> {
    LAST_TIMINGS.lock().clone()
}

/// The last utterance's timings, cleared (the speed probe waits for the
/// next).
pub fn take_timings() -> Option<Timings> {
    LAST_TIMINGS.lock().take()
}

static LIVE: Mutex<Option<Live>> = Mutex::new(None);
static NEXT: AtomicU64 = AtomicU64::new(1);
/// Bumped by every start and cancel: work for an older utterance stops
/// without delivering.
static GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn generation() -> u64 {
    GENERATION.load(Ordering::SeqCst)
}

fn moved(since: u64) -> bool {
    generation() != since
}

/// The state the bar or the pill shows.
pub fn state(origin: Origin, state: &str, extra: Value) -> Value {
    let mut v = json!({ "state": state, "origin": origin.name() });
    if let (Some(o), Value::Object(more)) = (v.as_object_mut(), extra) {
        o.extend(more);
    }
    v
}

fn emit_state(app: &AppHandle, origin: Origin, name: &str, extra: Value) {
    let v = state(origin, name, extra);
    match origin {
        Origin::Bar => crate::windows::emit(app, origin.label(), "voice:state", v),
        Origin::Dictation => pill::update(app, v),
    }
}

/// Whether an utterance is being recorded (or, with `origin`, one of its).
pub fn recording(origin: Option<Origin>) -> bool {
    match (LIVE.lock().as_ref(), origin) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(l), Some(o)) => l.origin == o,
    }
}

/// Starts listening. Refused while another utterance records.
pub fn start(app: &AppHandle, origin: Origin) -> Result<(), String> {
    let s = settings::get();
    let mut live = LIVE.lock();
    if live.is_some() {
        return Err("Already listening.".into());
    }
    let recorder = match capture::start(&s.device) {
        Ok(r) => r,
        Err(e) => {
            drop(live);
            if origin == Origin::Dictation {
                pill::show(app, state(origin, "error", json!({ "message": e })));
                pill::hide(app, Duration::from_millis(2600), generation());
            } else {
                emit_state(app, origin, "error", json!({ "message": e }));
            }
            return Err(e);
        }
    };
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    GENERATION.fetch_add(1, Ordering::SeqCst);
    // Pieces are transcribed early only with a model already on disk and a
    // processor that runs it; Gemini takes the whole clip.
    let streamer = engine::usable(&s)
        .filter(|_| whisper::cpu_ok())
        .and_then(|path| stream::Streamer::start(recorder.tap(), path, s.language.clone()));
    *live = Some(Live {
        id,
        origin,
        recorder,
        started: Instant::now(),
        handsfree: false,
        streamer,
    });
    drop(live);
    let listening = state(origin, "listening", json!({}));
    match origin {
        Origin::Bar => crate::windows::emit(app, origin.label(), "voice:state", listening),
        Origin::Dictation => pill::show(app, listening),
    }
    engine::warm(&s);
    tick(app.clone(), id, origin);
    Ok(())
}

/// The level meter, about 20 times a second, and the length limit.
fn tick(app: AppHandle, id: u64, origin: Origin) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(50));
        let (level, long) = {
            let live = LIVE.lock();
            let Some(l) = live.as_ref().filter(|l| l.id == id) else {
                return;
            };
            let long = l.started.elapsed().as_secs() >= capture::MAX_SECONDS;
            (l.recorder.level(), long)
        };
        let v = json!({ "origin": origin.name(), "level": level });
        crate::windows::emit(&app, origin.label(), "voice:level", v);
        if long {
            finish(&app, origin);
            return;
        }
    });
}

/// Hands-free: dictation keeps listening after the chord is let go.
pub fn latch(app: &AppHandle) {
    let mut live = LIVE.lock();
    if let Some(l) = live.as_mut().filter(|l| l.origin == Origin::Dictation) {
        l.handsfree = true;
        drop(live);
        emit_state(app, Origin::Dictation, "handsfree", json!({}));
    }
}

/// Stops listening and drops what was heard; work already under way stops
/// without delivering. `quiet` skips the message (a chord that turned out
/// to be another shortcut).
pub fn cancel(app: &AppHandle, quiet: bool) {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    let gone = LIVE.lock().take();
    ptt::reset();
    let origin = gone.as_ref().map(|l| l.origin);
    drop(gone);
    if let Some(o) = origin {
        emit_state(app, o, "idle", json!({ "cancelled": !quiet }));
        if o == Origin::Dictation {
            pill::hide(app, Duration::ZERO, generation());
        }
    }
}

/// Stops listening and turns what was heard into text.
pub fn finish(app: &AppHandle, origin: Origin) {
    let live = {
        let mut slot = LIVE.lock();
        match slot.as_ref() {
            Some(l) if l.origin == origin => slot.take(),
            _ => None,
        }
    };
    let Some(live) = live else { return };
    if origin == Origin::Dictation && !live.handsfree && live.started.elapsed() < MIN_HOLD {
        drop(live);
        emit_state(app, origin, "idle", json!({}));
        pill::hide(app, Duration::ZERO, generation());
        return;
    }
    let since = generation();
    let released = Instant::now();
    emit_state(app, origin, "working", json!({}));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Live {
            recorder, streamer, ..
        } = live;
        let mut t = Timings::default();
        let audio = tauri::async_runtime::spawn_blocking(move || recorder.finish()).await;
        let audio = match audio {
            Ok(Ok(a)) => a,
            Ok(Err(e)) => return fail(&app, origin, &e, since),
            Err(_) => return fail(&app, origin, "The recording stopped unexpectedly.", since),
        };
        t.stop_ms = released.elapsed().as_millis() as u64;
        let waited = Instant::now();
        let early = match streamer {
            Some(st) => tauri::async_runtime::spawn_blocking(move || st.finish())
                .await
                .unwrap_or_default(),
            None => stream::Done::default(),
        };
        t.wait_ms = waited.elapsed().as_millis() as u64;
        t.audio_s = audio.seconds();
        t.early_s = stream::seconds(early.upto);
        t.pieces = early.pieces;
        t.early_ms = early.busy_ms;
        let got = pipeline(&app, origin, audio, early, since, &mut t).await;
        match got {
            Ok(text) => deliver(&app, origin, text, since, released, t).await,
            Err(e) => fail(&app, origin, &e, since),
        }
    });
}

async fn pipeline(
    app: &AppHandle,
    origin: Origin,
    audio: capture::Audio,
    early: stream::Done,
    since: u64,
    t: &mut Timings,
) -> Result<String, String> {
    if audio.seconds() < 0.3 || audio.peak_level() < QUIET {
        return Err(NOTHING_HEARD.into());
    }
    let s = settings::get();
    let a = app.clone();
    let shown = Arc::new(AtomicBool::new(false));
    let on_progress = move |got: u64, total: u64| {
        let pct = (got * 100).checked_div(total).unwrap_or(0);
        if !shown.swap(true, Ordering::SeqCst) {
            emit_state(&a, origin, "preparing", json!({ "progress": pct }));
        }
        let v = json!({ "origin": origin.name(), "progress": pct });
        crate::windows::emit(&a, origin.label(), "voice:progress", v);
    };
    let began = Instant::now();
    let heard = engine::transcribe(&s, audio.samples, early, &on_progress).await?;
    t.transcribe_ms = began.elapsed().as_millis() as u64;
    if moved(since) {
        // Cancelled meanwhile: nothing to say.
        return Err(String::new());
    }
    let raw = cleanup::strip_noise(&heard.text);
    if raw.is_empty() {
        return Err(NOTHING_HEARD.into());
    }
    if heard.cleaned {
        return Ok(raw);
    }
    if !s.cleanup {
        return Ok(cleanup::tidy(&raw));
    }
    emit_state(app, origin, "cleaning", json!({}));
    let began = Instant::now();
    let cleaned = cleanup::clean(&raw).await.0;
    t.cleanup_ms = began.elapsed().as_millis() as u64;
    Ok(cleaned)
}

fn record(t: &mut Timings, released: Instant) {
    t.total_ms = released.elapsed().as_millis() as u64;
    eprintln!(
        "[voice] timings {}",
        serde_json::to_string(&t).unwrap_or_default()
    );
    *LAST_TIMINGS.lock() = Some(t.clone());
}

async fn deliver(
    app: &AppHandle,
    origin: Origin,
    text: String,
    since: u64,
    released: Instant,
    mut timing: Timings,
) {
    if moved(since) {
        return;
    }
    match origin {
        Origin::Bar => {
            record(&mut timing, released);
            emit_state(app, origin, "done", json!({ "text": text }));
        }
        Origin::Dictation => {
            let t = text.clone();
            let began = Instant::now();
            let put = tauri::async_runtime::spawn_blocking(move || insert::insert(&t)).await;
            timing.insert_ms = began.elapsed().as_millis() as u64;
            match put {
                Ok(Ok(_)) => {
                    record(&mut timing, released);
                    emit_state(app, origin, "done", json!({ "text": text }));
                    pill::hide(app, Duration::from_millis(700), since);
                }
                Ok(Err(e)) => fail(app, origin, &e, since),
                Err(_) => fail(app, origin, "Couldn't type the text.", since),
            }
        }
    }
}

fn fail(app: &AppHandle, origin: Origin, message: &str, since: u64) {
    if message.is_empty() || moved(since) {
        return;
    }
    emit_state(app, origin, "error", json!({ "message": message }));
    if origin == Origin::Dictation {
        pill::hide(app, Duration::from_millis(2600), since);
    }
}

/// The dictation chord settings ask for (none when dictation is off).
pub fn chord_of(s: &settings::VoiceSettings) -> Option<ptt::Chord> {
    if s.dictation {
        ptt::parse(&s.hotkey).ok()
    } else {
        None
    }
}

/// Whether `accelerator`, an app shortcut, is the dictation chord while
/// dictation is on (hotkeys::set refuses it; voice refuses the reverse).
pub fn dictation_uses(accelerator: &str) -> bool {
    let Some(chord) = chord_of(&settings::get()) else {
        return false;
    };
    ptt::parse(accelerator).is_ok_and(|c| c == chord)
}

fn chord_has_win() -> bool {
    chord_of(&settings::get()).is_some_and(|c| c.mods.win)
}

fn on_ptt(app: &AppHandle, e: ptt::Event) {
    match e {
        ptt::Event::Start => {
            if chord_has_win() {
                insert::mask_start_menu();
            }
            let _ = start(app, Origin::Dictation);
        }
        ptt::Event::Latch => latch(app),
        ptt::Event::Finish => {
            if chord_has_win() {
                insert::mask_start_menu();
            }
            finish(app, Origin::Dictation);
        }
        ptt::Event::Cancel | ptt::Event::Abort => {
            if recording(Some(Origin::Dictation)) {
                cancel(app, e == ptt::Event::Abort);
            }
        }
        ptt::Event::Mask => insert::mask_start_menu(),
    }
}

/// Keeps the model ready for the settings `s`: loaded and kept while
/// dictation is on, so the first words after a quiet spell are as quick as
/// any; the quantized model fetched in place of an earlier build's.
pub fn refresh(s: &settings::VoiceSettings) {
    whisper::pin(s.dictation);
    if s.dictation {
        engine::warm(s);
    }
    engine::upgrade(s);
}

/// Called once from the app's setup: the dictation hook, the pill and the
/// model.
pub fn setup(app: &AppHandle) {
    let s = settings::get();
    let a = app.clone();
    ptt::start(chord_of(&s), move |e| {
        let _ = crate::crash::guard("voice-ptt", || on_ptt(&a, e));
    });
    if s.dictation {
        pill::prepare(app);
    }
    refresh(&s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_carries_its_origin_and_extras() {
        let v = state(Origin::Dictation, "error", json!({ "message": "x" }));
        assert_eq!(
            v,
            json!({ "state": "error", "origin": "dictation", "message": "x" })
        );
        let done = state(Origin::Bar, "done", json!({}));
        assert_eq!(done, json!({ "state": "done", "origin": "bar" }));
    }
}
