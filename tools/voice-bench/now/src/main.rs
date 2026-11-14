//! The app's current speech path, timed: whisper.cpp through whisper-rs with
//! the parameters of `run(.., fast = true)` in src-tauri/src/voice/whisper.rs
//! (the encoder window cut to the clip, one segment, no timestamps, flash
//! attention, greedy). Keep the two in step: this is the baseline every
//! candidate in ../next is compared with.
//!
//!   voice-bench-now <models dir> <clips dir>
//!
//! Prints one `[BENCH]` line per model and clip; tools/voice-bench/report.mjs
//! reads them.

use std::path::{Path, PathBuf};
use std::time::Instant;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const RATE: usize = 16_000;
const FRAMES_PER_SECOND: usize = 50;
const FULL_CTX: usize = 1500;
const CTX_MARGIN: usize = 64;
const CTX_MIN: usize = 256;
/// Runs after the first; the first is reported on its own.
const RUNS: usize = 2;

/// What is changed from the app's settings, to find where its slow passes
/// come from: Whisper decoding again at a higher temperature when it does
/// not trust a pass, or the encoder window being cut too close.
#[derive(Clone, Copy)]
struct Variant {
    name: &'static str,
    /// Decode once; no second try at a higher temperature.
    no_fallback: bool,
    /// The smallest encoder window, in frames (the app: 256).
    ctx_min: usize,
}

const VARIANTS: [Variant; 4] = [
    Variant {
        name: "now",
        no_fallback: false,
        ctx_min: CTX_MIN,
    },
    Variant {
        name: "now-nofb",
        no_fallback: true,
        ctx_min: CTX_MIN,
    },
    Variant {
        name: "now-ctx512",
        no_fallback: false,
        ctx_min: 512,
    },
    Variant {
        name: "now-nofb-ctx512",
        no_fallback: true,
        ctx_min: 512,
    },
];

fn threads() -> i32 {
    let n = std::thread::available_parallelism().map_or(4, |n| n.get());
    n.clamp(1, 8) as i32
}

fn audio_ctx(samples: usize, min: usize) -> i32 {
    let frames = samples * FRAMES_PER_SECOND / RATE + CTX_MARGIN;
    if frames >= FULL_CTX {
        0
    } else {
        frames.max(min) as i32
    }
}

fn run(ctx: &WhisperContext, samples: &[f32], v: Variant) -> Result<String, String> {
    let mut state = ctx.create_state().map_err(|e| format!("{e:?}"))?;
    let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    p.set_n_threads(threads());
    p.set_translate(false);
    p.set_language(Some("en"));
    p.set_no_context(true);
    p.set_print_special(false);
    p.set_print_progress(false);
    p.set_print_realtime(false);
    p.set_print_timestamps(false);
    p.set_suppress_blank(true);
    if v.no_fallback {
        p.set_temperature_inc(0.0);
    }
    let min = (RATE * 11) / 10;
    let mut pcm = samples.to_vec();
    if pcm.len() < min {
        pcm.resize(min, 0.0);
    }
    let frames = audio_ctx(pcm.len(), v.ctx_min);
    if frames > 0 {
        p.set_audio_ctx(frames);
        p.set_single_segment(true);
        p.set_no_timestamps(true);
    }
    state.full(p, &pcm).map_err(|e| format!("{e:?}"))?;
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

fn load_wav(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    if spec.sample_rate != RATE as u32 || spec.bits_per_sample != 16 {
        return Err(format!(
            "{} Hz, {} bit: only 16 kHz 16-bit is read",
            spec.sample_rate, spec.bits_per_sample
        ));
    }
    let ch = spec.channels.max(1) as usize;
    let all: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.map(|v| v as f32 / 32768.0).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    Ok(all
        .chunks(ch)
        .map(|c| c.iter().sum::<f32>() / ch as f32)
        .collect())
}

fn files(dir: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn stem(p: &Path) -> String {
    p.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Every clip, and the first three seconds of the dictation one: a short
/// phrase, which is most of what dictation is.
fn clips(dir: &str) -> Vec<(String, Vec<f32>)> {
    let mut out = Vec::new();
    for path in files(dir) {
        match load_wav(&path) {
            Ok(pcm) => {
                let name = stem(&path);
                if name == "dictation" && pcm.len() > 3 * RATE {
                    out.push(("dictation-3s".to_string(), pcm[..3 * RATE].to_vec()));
                }
                out.push((name, pcm));
            }
            Err(e) => println!("[BENCH] skip clip={} why={e:?}", stem(&path)),
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: voice-bench-now <models dir> <clips dir>");
        std::process::exit(2);
    }
    println!(
        "[BENCH] info engine=now threads={} system={:?}",
        threads(),
        whisper_rs::print_system_info()
    );
    let clips = clips(&args[1]);
    for model in files(&args[0]) {
        let name = stem(&model);
        let mb = std::fs::metadata(&model).map_or(0, |m| m.len() / 1_000_000);
        let Some(path) = model.to_str() else { continue };
        let t = Instant::now();
        let mut params = WhisperContextParameters::default();
        params.flash_attn(true);
        let ctx = match WhisperContext::new_with_params(path, params) {
            Ok(c) => c,
            Err(e) => {
                println!("[BENCH] fail engine=now model={name} why={:?}", format!("{e:?}"));
                continue;
            }
        };
        let load_ms = t.elapsed().as_millis();
        for v in VARIANTS {
            let engine = v.name;
            for (clip, pcm) in &clips {
                let audio_s = pcm.len() as f64 / RATE as f64;
                let t = Instant::now();
                let first = run(&ctx, pcm, v);
                let first_ms = t.elapsed().as_millis();
                let text = match first {
                    Ok(t) => t,
                    Err(e) => {
                        println!("[BENCH] fail engine={engine} model={name} clip={clip} why={e:?}");
                        continue;
                    }
                };
                let mut runs = Vec::new();
                for _ in 0..RUNS {
                    let t = Instant::now();
                    let _ = run(&ctx, pcm, v);
                    runs.push(t.elapsed().as_millis().to_string());
                }
                println!(
                    "[BENCH] run engine={engine} model={name} mb={mb} clip={clip} audio_s={audio_s:.2} load_ms={load_ms} first_ms={first_ms} runs_ms={} text={text:?}",
                    runs.join(",")
                );
            }
        }
    }
}
