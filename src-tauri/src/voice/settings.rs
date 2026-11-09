//! Voice settings, kept under `voice` in aevalbuddy.json:
//!
//! ```json
//! "voice": { "dictation": true, "hotkey": "Ctrl+Super", "cleanup": true,
//!            "language": "en", "engine": "whisper-base", "device": "" }
//! ```
//!
//! Anything missing or unreadable reads as its default.

use serde_json::{json, Value};

use super::ptt;

pub const KEY: &str = "voice";

/// Transcription engines, the default first: Whisper on this PC at three
/// sizes, or Gemini with the person's own key.
pub const ENGINES: [&str; 4] = ["whisper-base", "whisper-tiny", "whisper-small", "gemini"];

/// Languages Settings offers (Whisper codes); `auto` detects.
pub const LANGUAGES: [(&str, &str); 14] = [
    ("en", "English"),
    ("auto", "Detect automatically"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("de", "German"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("tr", "Turkish"),
    ("hi", "Hindi"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("zh", "Chinese"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceSettings {
    /// The hold-to-talk chord works in every app.
    pub dictation: bool,
    pub hotkey: String,
    pub cleanup: bool,
    pub language: String,
    pub engine: String,
    /// An input device's name; empty is the Windows default.
    pub device: String,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        VoiceSettings {
            dictation: true,
            hotkey: ptt::DEFAULT.into(),
            cleanup: true,
            language: "en".into(),
            engine: ENGINES[0].into(),
            device: String::new(),
        }
    }
}

impl VoiceSettings {
    pub fn to_json(&self) -> Value {
        json!({
            "dictation": self.dictation,
            "hotkey": self.hotkey,
            "cleanup": self.cleanup,
            "language": self.language,
            "engine": self.engine,
            "device": self.device,
        })
    }

    /// The Whisper size for a `whisper-*` engine.
    pub fn whisper_size(&self) -> Option<&str> {
        self.engine.strip_prefix("whisper-")
    }
}

/// Reads one stored object, field by field; a bad field is its default.
pub fn from_value(v: Option<&Value>) -> VoiceSettings {
    let d = VoiceSettings::default();
    let Some(v) = v.filter(|v| v.is_object()) else {
        return d;
    };
    let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let flag = |k: &str, def: bool| v.get(k).and_then(Value::as_bool).unwrap_or(def);
    let hotkey = text("hotkey")
        .and_then(|h| ptt::parse(&h).ok().map(|c| ptt::accelerator(&c)))
        .unwrap_or(d.hotkey);
    let language = text("language")
        .filter(|l| LANGUAGES.iter().any(|(id, _)| id == l))
        .unwrap_or(d.language);
    let engine = text("engine")
        .filter(|e| ENGINES.contains(&e.as_str()))
        .unwrap_or(d.engine);
    VoiceSettings {
        dictation: flag("dictation", d.dictation),
        hotkey,
        cleanup: flag("cleanup", d.cleanup),
        language,
        engine,
        device: text("device").unwrap_or_default(),
    }
}

pub fn get() -> VoiceSettings {
    from_value(crate::store::get().raw(KEY).as_ref())
}

/// `cur` with the fields of `patch` applied, or a sentence saying why not.
/// `taken` lists the app's other shortcuts, which the chord may not equal.
pub fn merge(
    cur: &VoiceSettings,
    patch: &Value,
    taken: &[String],
) -> Result<VoiceSettings, String> {
    let mut next = cur.clone();
    if let Some(b) = patch.get("dictation").and_then(Value::as_bool) {
        next.dictation = b;
    }
    if let Some(b) = patch.get("cleanup").and_then(Value::as_bool) {
        next.cleanup = b;
    }
    if let Some(h) = patch.get("hotkey").and_then(Value::as_str) {
        let chord = ptt::parse(h)?;
        let clash = taken
            .iter()
            .any(|t| ptt::parse(t).is_ok_and(|o| o == chord));
        if clash {
            return Err(format!(
                "{} already opens AevalBuddy. Choose another.",
                ptt::label(h)
            ));
        }
        next.hotkey = ptt::accelerator(&chord);
    }
    if let Some(l) = patch.get("language").and_then(Value::as_str) {
        if !LANGUAGES.iter().any(|(id, _)| *id == l) {
            return Err(format!("{l} is not a language voice input knows."));
        }
        next.language = l.into();
    }
    if let Some(e) = patch.get("engine").and_then(Value::as_str) {
        if !ENGINES.contains(&e) {
            return Err(format!("{e} is not a transcription engine."));
        }
        next.engine = e.into();
    }
    if let Some(d) = patch.get("device").and_then(Value::as_str) {
        next.device = d.into();
    }
    // Turning dictation on with a chord an app shortcut took since.
    let turned_on = patch.get("dictation").and_then(Value::as_bool) == Some(true);
    if turned_on && patch.get("hotkey").is_none() {
        if let Ok(chord) = ptt::parse(&next.hotkey) {
            if taken
                .iter()
                .any(|t| ptt::parse(t).is_ok_and(|o| o == chord))
            {
                return Err(format!(
                    "{} already opens AevalBuddy. Choose another dictation shortcut.",
                    ptt::label(&next.hotkey)
                ));
            }
        }
    }
    Ok(next)
}

pub fn save(s: &VoiceSettings) {
    crate::store::get().set_raw(KEY, s.to_json());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_bad_fields_read_as_defaults() {
        assert_eq!(from_value(None), VoiceSettings::default());
        assert_eq!(from_value(Some(&json!("x"))), VoiceSettings::default());
        let s = from_value(Some(
            &json!({ "hotkey": "win+ctrl", "engine": "nope", "language": "fr", "cleanup": 0 }),
        ));
        assert_eq!(s.hotkey, "Ctrl+Super");
        assert_eq!(s.engine, "whisper-base");
        assert_eq!(s.language, "fr");
        assert!(s.cleanup, "a number is not a switch");
        assert_eq!(from_value(Some(&s.to_json())), s);
        assert_eq!(s.whisper_size(), Some("base"));
    }

    #[test]
    fn a_patch_is_checked_before_it_is_kept() {
        let cur = VoiceSettings::default();
        let taken = vec!["Ctrl+Alt+Space".to_string()];
        let next = merge(
            &cur,
            &json!({ "hotkey": "alt+shift+d", "cleanup": false }),
            &taken,
        )
        .unwrap();
        assert_eq!((next.hotkey.as_str(), next.cleanup), ("Alt+Shift+D", false));
        assert!(merge(&cur, &json!({ "hotkey": "Control+Alt+Space" }), &taken).is_err());
        assert!(merge(&cur, &json!({ "hotkey": "Ctrl" }), &taken).is_err());
        assert!(merge(&cur, &json!({ "language": "xx" }), &taken).is_err());
        assert!(merge(&cur, &json!({ "engine": "gemini" }), &taken).is_ok());
    }
}
