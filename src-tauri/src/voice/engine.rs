//! Turns a recording into text with the engine Settings chose: Whisper on
//! this PC (the default, free and offline), Parakeet on this PC, or Gemini
//! with the person's key.

use std::path::PathBuf;

use super::settings::VoiceSettings;
use super::{gemini, model, settings, stream, vad, whisper};

/// What an engine heard; `cleaned` when it already tidied the text.
pub struct Heard {
    pub text: String,
    pub cleaned: bool,
}

/// The model an engine on this PC uses; None for Gemini. Parakeet hears
/// English only, so with another language chosen its place is taken by
/// Whisper's base model, which keeps the Language setting true.
pub fn spec(s: &VoiceSettings) -> Option<&'static model::Spec> {
    if s.engine == settings::PARAKEET {
        if s.language == "en" {
            return Some(model::parakeet());
        }
        return Some(model::spec_for("base", &s.language));
    }
    s.whisper_size()
        .map(|size| model::spec_for(size, &s.language))
}

/// The model file to run now, if one is on disk (a file an earlier build
/// fetched counts until its replacement arrives).
pub fn usable(s: &VoiceSettings) -> Option<PathBuf> {
    spec(s).and_then(model::usable)
}

/// Whether the engine can run now without a download or a missing key.
pub fn ready(s: &VoiceSettings) -> bool {
    match spec(s) {
        Some(m) => model::usable(m).is_some(),
        None => gemini::available(),
    }
}

/// Fetches the model in the background when only a file an earlier build
/// fetched is on disk, then deletes the old one.
pub fn upgrade(s: &VoiceSettings) {
    let Some(m) = spec(s) else { return };
    if model::ready(m) {
        model::drop_legacy(m);
        return;
    }
    if model::usable(m).is_none() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if model::ensure(m, &|_, _| {}).await.is_err() {
            return;
        }
        // The old file cannot be deleted while its model is loaded: the new
        // one takes its place in memory first.
        let old = model::legacy_for(m).any(|l| whisper::loaded(&model::path(l)));
        if old {
            let new = model::path(m);
            let swap = tauri::async_runtime::spawn_blocking(move || {
                whisper::unload();
                whisper::preload(&new)
            });
            let _ = swap.await;
        }
        model::drop_legacy(m);
    });
}

/// `samples` with the silence before and after the speech cut off; None
/// when nothing was said.
pub fn trim(samples: &[f32]) -> Option<&[f32]> {
    vad::speech(samples).map(|r| &samples[r])
}

pub async fn transcribe(
    s: &VoiceSettings,
    samples: Vec<f32>,
    early: stream::Done,
    on_progress: &(dyn Fn(u64, u64) + Send + Sync),
) -> Result<Heard, String> {
    let Some(m) = spec(s) else {
        let text = gemini::transcribe(&samples, &s.language, s.cleanup).await?;
        return Ok(Heard {
            text,
            cleaned: s.cleanup,
        });
    };
    if !whisper::cpu_ok() {
        return Err(whisper::OLD_CPU.into());
    }
    let mut path = match model::usable(m) {
        Some(p) => p,
        None => model::ensure(m, on_progress).await?,
    };
    if path != model::path(m) {
        // An earlier build's file. Should this engine not read it, its
        // replacement is fetched now rather than in the background.
        let old = path.clone();
        let reads = tauri::async_runtime::spawn_blocking(move || whisper::preload(&old));
        if !reads.await.unwrap_or(false) {
            path = model::ensure(m, on_progress).await?;
        }
    }
    let language = s.language.clone();
    let text = tauri::async_runtime::spawn_blocking(move || {
        let rest = samples.get(early.upto..).unwrap_or_default();
        let tail = match trim(rest) {
            Some(speech) => whisper::transcribe_after(&path, speech, &language, &early.text)?,
            None => String::new(),
        };
        Ok::<String, String>(stream::join(&early.text, &tail))
    })
    .await
    .map_err(|_| "Transcription stopped unexpectedly.".to_string())??;
    Ok(Heard {
        text,
        cleaned: false,
    })
}

/// Loads the model if it is on disk and not in memory, on a thread of its
/// own. A missing model waits for the first release, which shows the
/// download.
pub fn warm(s: &VoiceSettings) {
    if !whisper::cpu_ok() {
        return;
    }
    if let Some(path) = usable(s).filter(|p| !whisper::loaded(p)) {
        std::thread::spawn(move || whisper::preload(&path));
    }
}
