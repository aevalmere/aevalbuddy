//! Speech to text on this PC, with whisper.cpp through whisper-rs. The model
//! stays loaded for two minutes after the last use (loading base.en takes a
//! few hundred milliseconds; keeping it costs about 200 MB), then is let go.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::wav;

/// How long a model stays loaded after its last use.
const KEEP: Duration = Duration::from_secs(120);

pub const OLD_CPU: &str =
    "This PC's processor can't run voice on the device. Pick Gemini under Settings, Voice, Transcription.";

struct Loaded {
    path: PathBuf,
    ctx: Arc<WhisperContext>,
    used: Instant,
}

static LOADED: Mutex<Option<Loaded>> = parking_lot::const_mutex(None);

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

fn load(path: &Path) -> Result<Arc<WhisperContext>, String> {
    if !cpu_ok() {
        return Err(OLD_CPU.into());
    }
    let mut slot = LOADED.lock();
    if let Some(l) = slot.as_mut().filter(|l| l.path == path) {
        l.used = Instant::now();
        return Ok(l.ctx.clone());
    }
    *slot = None;
    let p = path
        .to_str()
        .ok_or("The voice model's folder has a name Whisper can't read.")?;
    let ctx = WhisperContext::new_with_params(p, WhisperContextParameters::default())
        .map_err(|e| format!("The voice model did not load ({e:?})."))?;
    let ctx = Arc::new(ctx);
    *slot = Some(Loaded {
        path: path.to_path_buf(),
        ctx: ctx.clone(),
        used: Instant::now(),
    });
    drop(slot);
    std::thread::spawn(|| loop {
        std::thread::sleep(KEEP / 4);
        let mut slot = LOADED.lock();
        match slot.as_ref() {
            Some(l) if l.used.elapsed() >= KEEP => {
                *slot = None;
                return;
            }
            Some(_) => {}
            None => return,
        }
    });
    Ok(ctx)
}

/// Loads the model ahead of the first words (while the person is still
/// talking).
pub fn preload(path: &Path) {
    let _ = load(path);
}

fn threads() -> i32 {
    let n = std::thread::available_parallelism().map_or(4, |n| n.get());
    n.clamp(1, 8) as i32
}

/// The words in `samples` (16 kHz mono). `language` is a Whisper code, or
/// `auto` to detect it.
pub fn transcribe(path: &Path, samples: &[f32], language: &str) -> Result<String, String> {
    let ctx = load(path)?;
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
    if let Some(l) = LOADED.lock().as_mut() {
        l.used = Instant::now();
    }
    Ok(out.trim().to_string())
}
