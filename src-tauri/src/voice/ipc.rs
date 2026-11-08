//! The `voice:*` channels. The bar starts, stops and cancels its mic;
//! Settings reads and changes the Voice section and fetches the model.
//! `None` for any other channel, which falls through to the next router.

use serde_json::{json, Value};
use tauri::AppHandle;

use super::{capture, cleanup, engine, gemini, model, ptt, settings, whisper, Origin};

pub async fn route(
    app: &AppHandle,
    _label: &str,
    channel: &str,
    p: &Value,
) -> Option<Result<Value, String>> {
    let origin = Origin::Bar;
    let out = match channel {
        "voice:start" => match super::start(app, origin) {
            Ok(()) => json!({ "ok": true }),
            Err(e) => json!({ "ok": false, "error": e }),
        },
        "voice:stop" => {
            super::finish(app, origin);
            json!({ "ok": true })
        }
        "voice:cancel" => {
            // The bar's own utterance; a dictation in progress is not its.
            if !super::recording(Some(Origin::Dictation)) {
                super::cancel(app, true);
            }
            json!({ "ok": true })
        }
        "voice:get" => snapshot().await,
        "voice:set" => set(p),
        "voice:download" => download(app).await,
        "voice:capture-hotkey" => {
            ptt::set_paused(p.get("on").and_then(Value::as_bool).unwrap_or(false));
            json!({ "ok": true })
        }
        _ => return None,
    };
    Some(Ok(out))
}

/// The other shortcuts, which the dictation chord may not equal.
fn taken() -> Vec<String> {
    crate::store::ACTIONS
        .iter()
        .map(|a| crate::store::get().shortcut(a))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Everything the Voice section shows.
async fn snapshot() -> Value {
    let s = settings::get();
    let devices = tauri::async_runtime::spawn_blocking(capture::devices)
        .await
        .unwrap_or_default();
    let engines: Vec<Value> = settings::ENGINES
        .iter()
        .map(|id| {
            let probe = settings::VoiceSettings {
                engine: (*id).into(),
                ..s.clone()
            };
            let spec = engine::spec(&probe);
            json!({
                "id": id,
                "local": spec.is_some(),
                "bytes": spec.map(|m| m.bytes),
                "ready": engine::ready(&probe),
                "available": spec.is_some() || gemini::available(),
            })
        })
        .collect();
    let languages: Vec<Value> = settings::LANGUAGES
        .iter()
        .map(|(id, name)| json!({ "id": id, "name": name }))
        .collect();
    json!({
        "settings": s.to_json(),
        "hotkeyLabel": ptt::label(&s.hotkey),
        "devices": devices,
        "engines": engines,
        "languages": languages,
        "download": model::progress().map(|(id, got, total)| json!({ "id": id, "got": got, "total": total })),
        "cleanupModel": cleanup::model_now(),
        "gemini": gemini::available(),
        "cpuOk": whisper::cpu_ok(),
    })
}

fn set(p: &Value) -> Value {
    let cur = settings::get();
    match settings::merge(&cur, p, &taken()) {
        Ok(next) => {
            settings::save(&next);
            ptt::set_chord(super::chord_of(&next));
            json!({
                "ok": true,
                "settings": next.to_json(),
                "hotkeyLabel": ptt::label(&next.hotkey),
            })
        }
        Err(e) => json!({ "ok": false, "error": e }),
    }
}

/// Fetches the chosen engine's model, reporting to Settings as it goes.
async fn download(app: &AppHandle) -> Value {
    let s = settings::get();
    let Some(m) = engine::spec(&s) else {
        return json!({ "ok": true });
    };
    let a = app.clone();
    let on_progress = move |got: u64, total: u64| {
        let v = json!({ "origin": "settings", "id": m.id, "got": got, "total": total });
        crate::windows::emit(&a, "settings", "voice:progress", v);
    };
    match model::ensure(m, &on_progress).await {
        Ok(_) => json!({ "ok": true }),
        Err(e) => json!({ "ok": false, "error": e }),
    }
}
