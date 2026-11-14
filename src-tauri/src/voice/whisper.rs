//! Speech to text on this PC, with transcribe.cpp through transcribe-cpp.
//! One library runs both kinds of model the app offers, Whisper and
//! Parakeet. whisper.cpp could not stay beside it: each brings its own ggml
//! and the two do not link into one exe (tools/voice-bench on branch
//! bench/parakeet).
//!
//! Speed (entries/voice-speed.md):
//! - The model stays loaded, with one session. While dictation is on it is
//!   loaded when the app starts and kept ([`pin`]); otherwise it is let go ten
//!   minutes after its last use.
//! - No timestamps: a dictated clip is one piece of text, and none of the
//!   timing is used.
//! - One pass for Whisper. It decodes again at a higher temperature when it
//!   does not trust what it wrote, and those second tries ran for seconds.
//!   They are off. A pass that comes back with more words than the clip could
//!   hold has gone round in a loop, and only that one is done again with the
//!   library's own settings ([`looped`]).
//! - Whisper encodes its whole 30 s window whatever the clip's length:
//!   transcribe.cpp has no shorter window to ask for. Parakeet's work grows
//!   with the clip, which is why it is several times faster on a phrase.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use transcribe_cpp::{
    Error, Feature, Model, RunExtension, RunOptions, Session, SessionOptions, TimestampKind,
    Transcript, WhisperRunOptions,
};

use super::wav;

/// How long a model stays loaded after its last use when it is not pinned.
const KEEP: Duration = Duration::from_secs(600);

pub const OLD_CPU: &str =
    "This PC's processor can't run voice on the device. Pick Gemini under Settings, Voice, Transcription.";

/// More words a second than anyone dictates. With [`SPARE_WORDS`], the most
/// a clip can hold before its text counts as a loop.
const WORDS_PER_SECOND: usize = 5;
const SPARE_WORDS: usize = 4;

/// A model in memory and the session that runs it.
pub struct Engine {
    model: Model,
    /// Whisper takes the text so far as a prompt and has second tries to
    /// turn off. Parakeet has neither.
    whisper: bool,
    /// The languages the model can be told to expect.
    languages: Vec<String>,
    slot: Mutex<Slot>,
}

/// One pass at a time: a session is not shared between threads.
struct Slot {
    session: Session,
    threads: i32,
}

struct Loaded {
    path: PathBuf,
    engine: Arc<Engine>,
    used: Instant,
}

static LOADED: Mutex<Option<Loaded>> = parking_lot::const_mutex(None);
static PINNED: AtomicBool = AtomicBool::new(false);
static EVICTOR: AtomicBool = AtomicBool::new(false);
/// Threads for the engine; 0 picks them from the processor.
static THREADS: AtomicI32 = AtomicI32::new(0);
/// Where the last pass spent its time, for the speed probe.
static STAGES: Mutex<String> = parking_lot::const_mutex(String::new());

/// transcribe.cpp is built for AVX2, FMA and F16C (whisper.cmake).
pub fn cpu_ok() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx2")
            && std::arch::is_x86_feature_detected!("fma")
            && std::arch::is_x86_feature_detected!("f16c")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        true
    }
}

/// Keeps the loaded model for as long as `on` (dictation is on).
pub fn pin(on: bool) {
    PINNED.store(on, Ordering::SeqCst);
}

/// The library's devices are registered once, before the first model.
fn backends() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = transcribe_cpp::init_backends_default();
    });
}

fn session(model: &Model, threads: i32) -> Result<Session, String> {
    let options = SessionOptions {
        n_threads: threads,
        ..Default::default()
    };
    model
        .session_with(&options)
        .map_err(|e| format!("The voice model did not start ({e})."))
}

/// A model loaded from disk and not kept: the speed probe's cold start.
pub fn cold(path: &Path) -> Result<Engine, String> {
    if !cpu_ok() {
        return Err(OLD_CPU.into());
    }
    if path.to_str().is_none() {
        return Err("The voice model's folder has a name the engine can't read.".into());
    }
    backends();
    let model = Model::load(path).map_err(|e| format!("The voice model did not load ({e})."))?;
    let whisper = model.supports(Feature::InitialPrompt);
    let languages = match model.capabilities() {
        Ok(c) => c.languages,
        Err(_) => Vec::new(),
    };
    let threads = threads();
    let session = session(&model, threads)?;
    Ok(Engine {
        model,
        whisper,
        languages,
        slot: Mutex::new(Slot { session, threads }),
    })
}

fn load(path: &Path) -> Result<Arc<Engine>, String> {
    let mut slot = LOADED.lock();
    if let Some(l) = slot.as_mut().filter(|l| l.path == path) {
        l.used = Instant::now();
        return Ok(l.engine.clone());
    }
    *slot = None;
    let engine = Arc::new(cold(path)?);
    *slot = Some(Loaded {
        path: path.to_path_buf(),
        engine: engine.clone(),
        used: Instant::now(),
    });
    drop(slot);
    if !EVICTOR.swap(true, Ordering::SeqCst) {
        std::thread::spawn(|| loop {
            std::thread::sleep(Duration::from_secs(30));
            let mut slot = LOADED.lock();
            let idle = slot.as_ref().is_some_and(|l| l.used.elapsed() >= KEEP);
            if idle && !PINNED.load(Ordering::SeqCst) {
                *slot = None;
            }
        });
    }
    Ok(engine)
}

/// Loads the model ahead of the first words: at app start while dictation
/// is on, and when a recording starts. Whether it loaded.
pub fn preload(path: &Path) -> bool {
    load(path).is_ok()
}

/// Lets the loaded model go, so its file can be replaced or deleted.
pub fn unload() {
    *LOADED.lock() = None;
}

/// Whether `path` is the model in memory now.
pub fn loaded(path: &Path) -> bool {
    LOADED.lock().as_ref().is_some_and(|l| l.path == path)
}

/// Sets the thread count (the speed probe); 0 goes back to the default.
pub fn set_threads(n: i32) {
    THREADS.store(n, Ordering::SeqCst);
}

pub fn threads() -> i32 {
    match THREADS.load(Ordering::SeqCst) {
        0 => {
            let n = std::thread::available_parallelism().map_or(4, |n| n.get());
            n.clamp(1, 8) as i32
        }
        n => n,
    }
}

/// The library's version and the devices it runs on.
pub fn system_info() -> String {
    backends();
    let devices: Vec<String> = transcribe_cpp::devices()
        .iter()
        .map(|d| format!("{}: {}", d.kind, d.description.trim()))
        .collect();
    format!(
        "transcribe.cpp {} on {}",
        transcribe_cpp::version(),
        devices.join(", ")
    )
}

/// Where the last pass spent its time: `mel=6ms encode=327ms decode=16ms`.
pub fn stages() -> String {
    STAGES.lock().clone()
}

/// The words in `samples` (16 kHz mono) with the model at `path`, kept
/// loaded. `language` is a Whisper code, or `auto` to detect it.
pub fn transcribe(path: &Path, samples: &[f32], language: &str) -> Result<String, String> {
    transcribe_after(path, samples, language, "")
}

/// [`transcribe`] for the next piece of an utterance: `before` is what was
/// heard so far, which keeps names and casing consistent across pieces with
/// Whisper. Parakeet takes no prompt and hears each piece on its own.
pub fn transcribe_after(
    path: &Path,
    samples: &[f32],
    language: &str,
    before: &str,
) -> Result<String, String> {
    if !cpu_ok() {
        return Err(OLD_CPU.into());
    }
    let engine = load(path)?;
    let out = run(&engine, samples, language, before);
    if let Some(l) = LOADED.lock().as_mut() {
        l.used = Instant::now();
    }
    out
}

/// The last `n` words of `text`.
fn last_words(text: &str, n: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    words[words.len().saturating_sub(n)..].join(" ")
}

/// Whether `text` holds more words than `samples` of speech could: Whisper
/// went round in a loop.
fn looped(text: &str, samples: usize) -> bool {
    let seconds = samples / wav::RATE as usize + 1;
    text.split_whitespace().count() > seconds * WORDS_PER_SECOND + SPARE_WORDS
}

/// The language to name to a model: one it lists among several. `auto`, and
/// a model made for one language, are told nothing.
fn hint(languages: &[String], language: &str) -> Option<String> {
    let known = languages.len() > 1 && languages.iter().any(|l| l == language);
    known.then(|| language.to_string())
}

/// What a pass that stopped early had written, if the error carries it.
fn partial(e: &Error) -> Option<Transcript> {
    e.partial().cloned()
}

/// The words in `samples`. A Whisper pass that looped is done again with the
/// library's own settings, second tries included: slow and seldom needed.
pub fn run(
    engine: &Engine,
    samples: &[f32],
    language: &str,
    before: &str,
) -> Result<String, String> {
    let text = pass(engine, samples, language, before, true)?;
    if engine.whisper && looped(&text, samples.len()) {
        return pass(engine, samples, language, "", false);
    }
    Ok(text)
}

/// One pass. `fast` is this lane's Whisper settings: the prompt, and no
/// second tries.
fn pass(
    engine: &Engine,
    samples: &[f32],
    language: &str,
    before: &str,
    fast: bool,
) -> Result<String, String> {
    // Neither model is handed less than about a second: a lone word gets
    // silence after it.
    let min = (wav::RATE as usize * 11) / 10;
    let mut pcm = samples.to_vec();
    if pcm.len() < min {
        pcm.resize(min, 0.0);
    }
    let family = (engine.whisper && fast).then(|| {
        let prompt = last_words(before, 40);
        RunExtension::Whisper(WhisperRunOptions {
            initial_prompt: Some(prompt).filter(|p| !p.is_empty()),
            // One greedy pass: no second tries at a higher temperature.
            temperature_inc: Some(0.0),
            ..Default::default()
        })
    });
    let options = RunOptions {
        timestamps: TimestampKind::None,
        language: hint(&engine.languages, language),
        family,
        ..Default::default()
    };
    let mut slot = engine.slot.lock();
    let want = threads();
    if slot.threads != want {
        slot.session = session(&engine.model, want)?;
        slot.threads = want;
    }
    let heard = match slot.session.run(&pcm, &options) {
        Ok(t) => t,
        // The library stops a text that began to repeat, or one that ran
        // out of room, and hands back what it had (one copy of a repeat).
        Err(e) => match partial(&e) {
            Some(t) => t,
            None => return Err(format!("That could not be transcribed ({e}).")),
        },
    };
    drop(slot);
    let t = &heard.timings;
    let (mel, encode, decode) = (t.mel_ms, t.encode_ms, t.decode_ms);
    *STAGES.lock() = format!("mel={mel:.0}ms encode={encode:.0}ms decode={decode:.0}ms");
    Ok(heard.text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_is_the_last_words() {
        assert_eq!(last_words("a b c d", 2), "c d");
        assert_eq!(last_words("", 2), "");
    }

    #[test]
    fn more_words_than_the_clip_could_hold_is_a_loop() {
        let s = wav::RATE as usize;
        let said = "So I think we should meet at three.";
        assert!(!looped(said, 3 * s));
        assert!(looped(&format!("{said} ").repeat(12), 3 * s));
        assert!(!looped("", 0));
    }

    #[test]
    fn a_language_is_named_only_to_a_model_that_knows_several() {
        let many: Vec<String> = ["en", "fr", "de"].iter().map(|l| l.to_string()).collect();
        let one = vec!["en".to_string()];
        assert_eq!(hint(&many, "fr"), Some("fr".to_string()));
        assert_eq!(hint(&many, "auto"), None, "auto is left to the model");
        assert_eq!(hint(&one, "en"), None, "an English model is not told");
        assert_eq!(hint(&[], "en"), None);
    }
}
