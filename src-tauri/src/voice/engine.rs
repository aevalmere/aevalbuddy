//! Turns a recording into text with the engine Settings chose: Whisper on
//! this PC (the default, free and offline) or Gemini with the person's key.

use super::settings::VoiceSettings;
use super::{gemini, model, whisper};

/// What an engine heard; `cleaned` when it already tidied the text.
pub struct Heard {
    pub text: String,
    pub cleaned: bool,
}

/// The Whisper model a `whisper-*` engine uses.
pub fn spec(s: &VoiceSettings) -> Option<&'static model::Spec> {
    s.whisper_size()
        .map(|size| model::spec_for(size, &s.language))
}

/// Whether the engine can run now without a download or a missing key.
pub fn ready(s: &VoiceSettings) -> bool {
    match spec(s) {
        Some(m) => model::ready(m),
        None => gemini::available(),
    }
}

pub async fn transcribe(
    s: &VoiceSettings,
    samples: Vec<f32>,
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
    let path = model::ensure(m, on_progress).await?;
    let language = s.language.clone();
    let text = tauri::async_runtime::spawn_blocking(move || {
        whisper::transcribe(&path, &samples, &language)
    })
    .await
    .map_err(|_| "Transcription stopped unexpectedly.".to_string())??;
    Ok(Heard {
        text,
        cleaned: false,
    })
}

/// While the person is still talking: load the model if it is on disk, so
/// the release is answered quickly. A missing model waits for the release,
/// which shows the download.
pub fn warm(s: &VoiceSettings) {
    if let Some(m) = spec(s) {
        if model::ready(m) && whisper::cpu_ok() {
            let path = model::path(m);
            std::thread::spawn(move || whisper::preload(&path));
        }
    }
}
