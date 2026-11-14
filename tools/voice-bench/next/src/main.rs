//! Candidate speech models, timed through transcribe.cpp on the CPU: Whisper
//! again (the same weights as ../now, another runtime), Parakeet, Moonshine.
//!
//!   voice-bench-next <models dir> <clips dir>
//!
//! Prints one `[BENCH]` line per model and clip, in the shape ../now prints;
//! tools/voice-bench/report.mjs reads both.

use std::path::{Path, PathBuf};
use std::time::Instant;

use transcribe_cpp::{Model, RunOptions, Session, SessionOptions, TimestampKind, Transcript};

const RATE: usize = 16_000;
/// Runs after the first; the first is reported on its own.
const RUNS: usize = 3;

/// The thread count the app gives Whisper (voice/whisper.rs `threads`).
fn threads() -> i32 {
    let n = std::thread::available_parallelism().map_or(4, |n| n.get());
    n.clamp(1, 8) as i32
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

/// One pass. English is named where the model takes a language, as the app
/// does; a model that refuses those options gets its own defaults.
fn run(session: &mut Session, pcm: &[f32], hint: bool) -> Result<Transcript, String> {
    let asked = RunOptions {
        timestamps: TimestampKind::None,
        language: if hint { Some("en".to_string()) } else { None },
        ..Default::default()
    };
    match session.run(pcm, &asked) {
        Ok(t) => Ok(t),
        Err(first) => session
            .run(pcm, &RunOptions::default())
            .map_err(|second| format!("{first}; with defaults: {second}")),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: voice-bench-next <models dir> <clips dir>");
        std::process::exit(2);
    }
    if let Err(e) = transcribe_cpp::init_backends_default() {
        println!("[BENCH] note backends={:?}", e.to_string());
    }
    println!(
        "[BENCH] info engine=next threads={} lib={:?} commit={:?}",
        threads(),
        transcribe_cpp::version().to_string(),
        transcribe_cpp::version_commit().to_string()
    );
    for d in transcribe_cpp::devices() {
        println!(
            "[BENCH] device kind={:?} name={:?} description={:?}",
            d.kind, d.name, d.description
        );
    }
    let clips = clips(&args[1]);
    for path in files(&args[0]) {
        let name = stem(&path);
        let mb = std::fs::metadata(&path).map_or(0, |m| m.len() / 1_000_000);
        let t = Instant::now();
        let model = match Model::load(&path) {
            Ok(m) => m,
            Err(e) => {
                println!("[BENCH] fail engine=next model={name} why={:?}", e.to_string());
                continue;
            }
        };
        let load_ms = t.elapsed().as_millis();
        let hint = model
            .capabilities()
            .map(|c| c.languages.len() > 1 && c.languages.iter().any(|l| l == "en"))
            .unwrap_or(false);
        println!(
            "[BENCH] model engine=next model={name} arch={:?} backend={:?} hint={hint}",
            model.arch().to_string(),
            model.backend().to_string()
        );
        let options = SessionOptions {
            n_threads: threads(),
            ..Default::default()
        };
        let mut session = match model.session_with(&options) {
            Ok(s) => s,
            Err(e) => {
                println!("[BENCH] fail engine=next model={name} why={:?}", e.to_string());
                continue;
            }
        };
        for (clip, pcm) in &clips {
            let audio_s = pcm.len() as f64 / RATE as f64;
            let t = Instant::now();
            let first = run(&mut session, pcm, hint);
            let first_ms = t.elapsed().as_millis();
            let first = match first {
                Ok(t) => t,
                Err(e) => {
                    println!("[BENCH] fail engine=next model={name} clip={clip} why={e:?}");
                    continue;
                }
            };
            let mut runs = Vec::new();
            let mut stages = String::new();
            for _ in 0..RUNS {
                let t = Instant::now();
                let again = run(&mut session, pcm, hint);
                runs.push(t.elapsed().as_millis().to_string());
                if let Ok(a) = again {
                    stages = format!(
                        "mel={:.0},encode={:.0},decode={:.0}",
                        a.timings.mel_ms, a.timings.encode_ms, a.timings.decode_ms
                    );
                }
            }
            println!(
                "[BENCH] run engine=next model={name} mb={mb} clip={clip} audio_s={audio_s:.2} load_ms={load_ms} first_ms={first_ms} runs_ms={} stages={stages} text={:?}",
                runs.join(","),
                first.text.trim()
            );
        }
    }
}
