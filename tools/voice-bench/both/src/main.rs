//! whisper.cpp and transcribe.cpp in one process: loads a model with each and
//! transcribes the same clip with both, one after the other, twice over.
//!
//!   voice-bench-both <whisper ggml .bin> <transcribe .gguf> <clip.wav>

use std::path::Path;

use transcribe_cpp::{Model, RunOptions};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

fn load_wav(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    reader
        .samples::<i16>()
        .map(|s| s.map(|v| v as f32 / 32768.0).map_err(|e| e.to_string()))
        .collect()
}

fn whisper(ctx: &WhisperContext, pcm: &[f32]) -> Result<String, String> {
    let mut state = ctx.create_state().map_err(|e| format!("{e:?}"))?;
    let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    p.set_language(Some("en"));
    p.set_print_progress(false);
    p.set_print_realtime(false);
    p.set_print_timestamps(false);
    state.full(p, pcm).map_err(|e| format!("{e:?}"))?;
    let n = state.full_n_segments().map_err(|e| format!("{e:?}"))?;
    let mut out = String::new();
    for i in 0..n {
        out.push_str(&state.full_get_segment_text(i).map_err(|e| format!("{e:?}"))?);
    }
    Ok(out.trim().to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: voice-bench-both <whisper ggml .bin> <transcribe .gguf> <clip.wav>");
        std::process::exit(2);
    }
    let pcm = match load_wav(Path::new(&args[2])) {
        Ok(p) => p,
        Err(e) => {
            println!("[BENCH] fail engine=both why={e:?}");
            std::process::exit(1);
        }
    };
    let _ = transcribe_cpp::init_backends_default();
    let ctx = WhisperContext::new_with_params(&args[0], WhisperContextParameters::default());
    let model = Model::load(&args[1]);
    let (ctx, model) = match (ctx, model) {
        (Ok(c), Ok(m)) => (c, m),
        (c, m) => {
            println!(
                "[BENCH] fail engine=both why={:?}",
                format!("whisper={:?} transcribe={:?}", c.err(), m.err().map(|e| e.to_string()))
            );
            std::process::exit(1);
        }
    };
    let mut session = match model.session() {
        Ok(s) => s,
        Err(e) => {
            println!("[BENCH] fail engine=both why={:?}", e.to_string());
            std::process::exit(1);
        }
    };
    let mut ok = true;
    for round in 1..=2 {
        let w = whisper(&ctx, &pcm);
        let t = session
            .run(&pcm, &RunOptions::default())
            .map(|r| r.text.trim().to_string())
            .map_err(|e| e.to_string());
        ok &= w.is_ok() && t.is_ok();
        println!("[BENCH] both round={round} whisper={w:?} transcribe={t:?}");
    }
    println!("[BENCH] info both linked=true ran={ok}");
    if !ok {
        std::process::exit(1);
    }
}
