//! The cloud engine: Gemini hears the recording, with the person's own
//! Gemini key, and cleans it up in the same call when cleanup is on. Chosen
//! in Settings; the recording leaves the PC only on this path.

use std::time::Duration;

use base64::Engine;
use serde_json::{json, Value};

use super::wav;
use crate::host::settings::StoreSettings;
use crate::llm::catalogue::Kind;
use crate::llm::provider::Provider;
use crate::llm::settings::{model_for, usable_key};

pub const NO_KEY: &str =
    "Gemini needs its key. Add it under Settings, Providers, or switch Voice to this PC.";
const FAILED: &str = "Gemini could not transcribe that. Try again, or switch Voice to this PC.";

/// Whether a Gemini key is saved.
pub fn available() -> bool {
    usable_key(&StoreSettings, Provider::Gemini).is_some()
}

pub fn prompt(language: &str, clean: bool) -> String {
    let lang = if language == "auto" {
        "in the language spoken".to_string()
    } else {
        format!("in the language with code \"{language}\"")
    };
    let mut p = format!("Transcribe this recording of someone dictating, {lang}. ");
    if clean {
        p.push_str(
            "Clean it up as you go: punctuation, capitals and small grammar slips fixed; filler words (um, uh, like, \
             you know) left out; where the speaker corrects themselves, only the correction kept. Keep their own \
             words and meaning, and add nothing. ",
        );
    } else {
        p.push_str("Write exactly what is said, with punctuation. ");
    }
    p.push_str("Reply with the text only. If nothing is said, reply with nothing.");
    p
}

/// The text of a generateContent reply.
pub fn reply_text(v: &Value) -> String {
    let parts = v["candidates"][0]["content"]["parts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    parts
        .iter()
        .filter_map(|p| p["text"].as_str())
        .collect::<String>()
        .trim()
        .to_string()
}

pub async fn transcribe(samples: &[f32], language: &str, clean: bool) -> Result<String, String> {
    let key = usable_key(&StoreSettings, Provider::Gemini).ok_or(NO_KEY)?;
    let model = model_for(&StoreSettings, Provider::Gemini, Kind::Text);
    let audio = base64::engine::general_purpose::STANDARD.encode(wav::encode(samples, wav::RATE));
    let body = json!({
        "contents": [{ "role": "user", "parts": [
            { "text": prompt(language, clean) },
            { "inline_data": { "mime_type": "audio/wav", "data": audio } },
        ] }],
        "generationConfig": { "temperature": 0 },
    });
    let url =
        format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| FAILED.to_string())?;
    let resp = client
        .post(url)
        .header("x-goog-api-key", key)
        .json(&body)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| FAILED.to_string())?;
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        crate::crash::write(
            "voice-gemini",
            &format!("HTTP {status}: {}", v["error"]["message"]),
        );
        return Err(match status.as_u16() {
            400 | 401 | 403 => "Gemini refused the key. Check it under Settings, Providers.".into(),
            429 => "Gemini is busy or out of free requests for now. Try again in a minute.".into(),
            _ => FAILED.into(),
        });
    }
    Ok(reply_text(&v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_and_the_reply() {
        assert!(prompt("en", true).contains("only the correction kept"));
        assert!(prompt("auto", false).contains("language spoken"));
        let v = json!({ "candidates": [{ "content": { "parts": [{ "text": "Hello " }, { "text": "there.\n" }] } }] });
        assert_eq!(reply_text(&v), "Hello there.");
        assert_eq!(reply_text(&json!({})), "");
    }
}
