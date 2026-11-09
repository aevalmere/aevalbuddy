//! Speech to text on this PC, with whisper.cpp through whisper-rs.
//!
//! Speed (entries/voice-speed.md):
//! - The model stays loaded. While dictation is on it is loaded when the app
//!   starts and kept ([`pin`]); otherwise it is let go ten minutes after its
//!   last use. Loading costs a few hundred milliseconds that used to land on
//!   the first words after every idle spell.
//! - Whisper encodes a fixed 30 s window by default, so three seconds of
//!   speech cost as much as thirty. [`run`] sets `audio_ctx` to the clip's own
//!   length plus a margin, which makes the encoder several times faster on a
//!   short clip.
//! - One segment, no timestamps and flash attention: a dictated clip is one
//!   piece of text, and none of the timing is used.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::wav;

/// How long a model stays loaded after its last use when it is not pinned.
const KEEP: Duration = Duration::from_secs(600);

pub const OLD_CPU: &str =
    "This PC's processor can't run voice on the device. Pick Gemini under Settings, Voice, Transcription.";

/// Encoder frames per second of audio (a 10 ms mel hop, halved by the
/// first convolution).
const FRAMES_PER_SECOND: usize = 50;
/// The most Whisper encodes at once: 30 s.
const FULL_CTX: usize = 1500;
/// Frames past the end of the speech, so the last word is never clipped.
const CTX_MARGIN: usize = 64;
/// Below this the encoder has too little to go on and gets words wrong.
const CTX_MIN: usize = 256;

struct Loaded {
    path: PathBuf,
    ctx: Arc<WhisperContext>,
    used: Instant,
}

static LOADED: Mutex<Option<Loaded>> = parking_lot::const_mutex(None);
static PINNED: AtomicBool = AtomicBool::new(false);
static EVICTOR: AtomicBool = AtomicBool::new(false);
/// Threads for Whisper; 0 picks them from the processor.
static THREADS: AtomicI32 = AtomicI32::new(0);

/// whisper.cpp is built for AVX2, FMA and F16C (whisper.cmake).
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

/// A model loaded from disk and not kept: the speed probe's cold start.
/// `fast` turns flash attention on, as [`load`] does.
pub fn cold(path: &Path, fast: bool) -> Result<WhisperContext, String> {
    if !cpu_ok() {
        return Err(OLD_CPU.into());
    }
    let p = path
        .to_str()
        .ok_or("The voice model's folder has a name Whisper can't read.")?;
    let mut params = WhisperContextParameters::default();
    params.flash_attn(fast);
    WhisperContext::new_with_params(p, params)
        .map_err(|e| format!("The voice model did not load ({e:?})."))
}

fn load(path: &Path) -> Result<Arc<WhisperContext>, String> {
    let mut slot = LOADED.lock();
    if let Some(l) = slot.as_mut().filter(|l| l.path == path) {
        l.used = Instant::now();
        return Ok(l.ctx.clone());
    }
    *slot = None;
    let ctx = Arc::new(cold(path, true)?);
    *slot = Some(Loaded {
        path: path.to_path_buf(),
        ctx: ctx.clone(),
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
    Ok(ctx)
}

/// Loads the model ahead of the first words: at app start while dictation
/// is on, and when a recording starts.
pub fn preload(path: &Path) {
    let _ = load(path);
}

/// Prints whisper.cpp's own stage times for the loaded model to stderr and
/// starts them again (the speed probe).
pub fn print_timings() {
    if let Some(l) = LOADED.lock().as_ref() {
        l.ctx.print_timings();
        l.ctx.reset_timings();
    }
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

/// whisper.cpp's build: the instruction sets it was compiled with.
pub fn system_info() -> &'static str {
    whisper_rs::print_system_info()
}

/// The encoder window for `samples` at 16 kHz: the clip plus a margin, or
/// 0 (Whisper's own 30 s) for a clip that long.
pub fn audio_ctx(samples: usize) -> i32 {
    let frames = samples * FRAMES_PER_SECOND / wav::RATE as usize + CTX_MARGIN;
    if frames >= FULL_CTX {
        0
    } else {
        frames.max(CTX_MIN) as i32
    }
}

/// The words in `samples` (16 kHz mono) with the model at `path`, kept
/// loaded. `language` is a Whisper code, or `auto` to detect it.
pub fn transcribe(path: &Path, samples: &[f32], language: &str) -> Result<String, String> {
    transcribe_after(path, samples, language, "")
}

/// [`transcribe`] for the next piece of an utterance: `before` is what was
/// heard so far, which keeps names and casing consistent across pieces.
pub fn transcribe_after(
    path: &Path,
    samples: &[f32],
    language: &str,
    before: &str,
) -> Result<String, String> {
    if !cpu_ok() {
        return Err(OLD_CPU.into());
    }
    let ctx = load(path)?;
    let out = run(&ctx, samples, language, before, true);
    if let Some(l) = LOADED.lock().as_mut() {
        l.used = Instant::now();
    }
    out
}

/// Where the time goes in one pass, for the speed probe: making a state,
/// the mel spectrogram, one full-window encode, then a whole run and its
/// token count.
pub fn breakdown(ctx: &WhisperContext, samples: &[f32]) -> String {
    let n = threads() as usize;
    let t = Instant::now();
    let Ok(mut state) = ctx.create_state() else {
        return "no state".into();
    };
    let state_ms = t.elapsed().as_millis();
    let t = Instant::now();
    let mel = state.pcm_to_mel(samples, n).is_ok();
    let mel_ms = t.elapsed().as_millis();
    let t = Instant::now();
    let enc = state.encode(0, n).is_ok();
    let enc_ms = t.elapsed().as_millis();
    let t = Instant::now();
    let text = run(ctx, samples, "en", "", true).unwrap_or_default();
    let run_ms = t.elapsed().as_millis();
    format!(
        "state={state_ms}ms mel={mel_ms}ms({mel}) encode1500={enc_ms}ms({enc}) run={run_ms}ms words={}",
        text.split_whitespace().count()
    )
}

/// The last `n` words of `text`.
fn last_words(text: &str, n: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    words[words.len().saturating_sub(n)..].join(" ")
}

/// One pass of Whisper. `fast` is this lane's settings; without it, the
/// parameters earlier builds used (the speed probe's "before").
pub fn run(
    ctx: &WhisperContext,
    samples: &[f32],
    language: &str,
    before: &str,
    fast: bool,
) -> Result<String, String> {
    let mut state = ctx
        .create_state()
        .map_err(|e| format!("Whisper did not start ({e:?})."))?;
    let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    p.set_n_threads(threads());
    p.set_translate(false);
    p.set_language(Some(language));
    p.set_no_context(true);
    p.set_print_special(false);
    p.set_print_progress(false);
    p.set_print_realtime(false);
    p.set_print_timestamps(false);
    p.set_suppress_blank(true);
    // Whisper skips anything under a second; pad short words with silence.
    let min = (wav::RATE as usize * 11) / 10;
    let mut pcm = samples.to_vec();
    if pcm.len() < min {
        pcm.resize(min, 0.0);
    }
    let frames = audio_ctx(pcm.len());
    if fast && frames > 0 {
        p.set_audio_ctx(frames);
        p.set_single_segment(true);
        p.set_no_timestamps(true);
    }
    let prompt = if fast {
        last_words(before, 40)
    } else {
        String::new()
    };
    if !prompt.is_empty() {
        p.set_initial_prompt(&prompt);
    }
    state
        .full(p, &pcm)
        .map_err(|e| format!("Whisper could not transcribe that ({e:?})."))?;
    let n = state.full_n_segments().map_err(|e| format!("{e:?}"))?;
    let mut out = String::new();
    for i in 0..n {
        let seg = state
            .full_get_segment_text(i)
            .map_err(|e| format!("{e:?}"))?;
        out.push_str(&seg);
    }
    Ok(out.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_encoder_window_follows_the_clip() {
        let s = wav::RATE as usize;
        assert_eq!(audio_ctx(s), CTX_MIN as i32, "a short clip gets the floor");
        assert_eq!(audio_ctx(8 * s), (8 * 50 + CTX_MARGIN) as i32);
        assert_eq!(
            audio_ctx(29 * s),
            0,
            "close to 30 s is Whisper's own window"
        );
        assert_eq!(audio_ctx(120 * s), 0);
        assert_eq!(last_words("a b c d", 2), "c d");
        assert_eq!(last_words("", 2), "");
    }
}
