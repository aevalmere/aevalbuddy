//! Settings store compatible with electron-store (PARITY STORE-01..44).
//!
//! The file is `%APPDATA%\aevalbuddy\aevalbuddy.json` (or under
//! `AEVAL_USERDATA`), the same keys, defaults and sanitisers as lib/store.js.
//! Defaults are merged on open and written back, as electron-store does;
//! unknown keys are kept; every write is atomic (temp + rename) and uses
//! electron-store's tab-indented JSON so either build can read the other's.
//!
//! Additions for the Tauri build (REQUESTS.md 2, 3, 5):
//! - `apiKeys`: one safeStorage blob per provider (anthropic, openai, gemini,
//!   openrouter). The OpenRouter key is migrated from `apiKeyEncrypted` /
//!   `apiKeyPlain` and kept mirrored there, so a rollback still finds it.
//! - `glassIntensity`: 0 (no glass) to 1 (most liquid), default 0.75, on the
//!   v2.2 scale named by `glassScale` (2); a v2.1 value is moved on open.
//! - `hasSentFirstRequest`: set once the first request is sent; suggestions
//!   are shown only before it.

pub mod crypto;
pub mod durable;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use parking_lot::Mutex;
use serde_json::{json, Map, Value};

use crypto::OsCrypt;

pub const DEFAULT_MODEL: &str = "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free";
pub const DEFAULT_BRAIN: &str = "nvidia/nemotron-3.5-super-vl:free";
pub const DEFAULT_FIND: &str = "nvidia/nemotron-3.5-lightning:free";
pub const DEFAULT_AGENT: &str = "nvidia/nemotron-3.5-super-vl:free";

/// Providers a key can be saved for (REQUESTS.md 2).
pub const PROVIDERS: [&str; 4] = ["anthropic", "openai", "gemini", "openrouter"];

/// The shipping glass intensity on the v2.2 scale: "Liquid", well past
/// v2.1's top stop.
pub const DEFAULT_GLASS_INTENSITY: f64 = 0.75;
/// The glass scale `glassIntensity` is written in. 2 is the widened v2.2
/// scale; a profile without `glassScale` holds a v2.1 value (see
/// `glass_v1_to_v2`).
pub const GLASS_SCALE: i64 = 2;

/// A v2.1 `glassIntensity` on the v2.2 scale. The untouched v2.1 default
/// (0.5) becomes the new default; any other level keeps about the look it
/// had (v2.1 0.25 / 0.75 / 1 match v2.2 0.31 / 0.68 / 0.86), and 0, no
/// glass, stays 0.
pub fn glass_v1_to_v2(v: f64) -> f64 {
    if !v.is_finite() {
        return DEFAULT_GLASS_INTENSITY;
    }
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0 {
        0.0
    } else if (v - 0.5).abs() < 0.005 {
        DEFAULT_GLASS_INTENSITY
    } else {
        ((0.12 + 0.74 * v) * 100.0).round() / 100.0
    }
}

pub const MIN_W: i64 = 320;
pub const MAX_W: i64 = 900;
/// The bar's width on a new profile (D-16); a stored width is kept.
pub const DEFAULT_INPUT_WIDTH: i64 = 600;
const SHORTCUTS_VERSION: i64 = 2;
const API_KEYS_VERSION: i64 = 1;
const INPUT_HISTORY_MAX: usize = 50;

/// Action ids in the order Settings lists them.
pub const ACTIONS: [&str; 5] = [
    "captureQuery",
    "captureRegion",
    "findOnScreen",
    "agentTask",
    "toggleVisibility",
];

pub fn default_shortcuts() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("captureQuery".into(), json!("Ctrl+Alt+Space"));
    for k in &ACTIONS[1..] {
        m.insert((*k).into(), json!(""));
    }
    m
}

fn old_default(action: &str) -> &'static str {
    match action {
        "captureQuery" => "Ctrl+Shift+Space",
        "captureRegion" => "Ctrl+Shift+A",
        "findOnScreen" => "Ctrl+Shift+G",
        "agentTask" => "Ctrl+Shift+U",
        _ => "Ctrl+Shift+H",
    }
}

/// electron-store's defaults, in lib/store.js order (the file keeps it).
pub fn defaults() -> Map<String, Value> {
    let v = json!({
        "apiKeyEncrypted": "",
        "apiKeyPlain": "",
        "model": DEFAULT_MODEL,
        "modelChosen": false,
        "brainEnabled": false,
        "brainEnabledChosen": false,
        "brain": DEFAULT_BRAIN,
        "brainChosen": false,
        "showPaidModels": false,
        "skill": "auto",
        "shortcuts": default_shortcuts(),
        "overlayPos": { "h": "right", "v": "top", "offX": 16, "offY": 16 },
        "barPos": { "h": "center", "v": "center", "offX": 16, "offY": 16 },
        "showButton": false,
        "outputPos": { "docked": true, "h": "right", "v": "top", "offX": 16, "offY": 240 },
        "inputWidth": DEFAULT_INPUT_WIDTH,
        "outputWidth": 420,
        "outputHeight": 0,
        "settingsSize": { "width": 660, "height": 560 },
        "historySize": { "width": 460, "height": 560 },
        "saveHistory": true,
        "launchAtLogin": false,
        "surface": "auto",
        "presenceDecided": false,
        "presenceNoticePending": false,
        "walkthroughAutoShown": false,
        "shortcutsVersion": 0,
        "inputHistory": [],
        "findModel": DEFAULT_FIND,
        "findModelChosen": false,
        "agentModel": DEFAULT_AGENT,
        "agentModelChosen": false,
        "findScope": "window",
        "findRelatedMode": "ask",
        "findOcr": true,
        "agentPermission": "risky",
        "agentMaxSteps": 25,
        "agentScreenshots": true,
        "agentReturnCursor": true,
        "agentConsent": false,
        "apiKeys": {},
        "apiKeysVersion": 0,
        "glassIntensity": DEFAULT_GLASS_INTENSITY,
        "glassScale": GLASS_SCALE,
        "hasSentFirstRequest": false,
    });
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// JavaScript's `Number(v)`.
pub fn js_number(v: Option<&Value>) -> f64 {
    match v {
        None => f64::NAN,
        Some(Value::Null) => 0.0,
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                0.0
            } else {
                t.parse::<f64>().unwrap_or(f64::NAN)
            }
        }
        Some(_) => f64::NAN,
    }
}

/// JavaScript's `Math.round`: halves go up.
pub fn js_round(n: f64) -> f64 {
    (n + 0.5).floor()
}

/// `Math.round(Number(v)) || fallback`, as an integer.
fn round_or(v: Option<&Value>, fallback: i64) -> i64 {
    let n = js_round(js_number(v));
    if n.is_nan() || n == 0.0 {
        fallback
    } else {
        n as i64
    }
}

/// JavaScript truthiness (`!!v`).
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// `String(v || '')`.
fn js_string(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(v) if truthy(Some(v)) => match v {
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

fn clamp_i(n: i64, lo: i64, hi: i64) -> i64 {
    n.max(lo).min(hi)
}

/// When a task asks first: "risky" (the gate's flagged steps), "every"
/// (each action) or "auto" (a second check decides the flagged ones).
fn permission_of(stored: &str) -> &'static str {
    match stored {
        "every" => "every",
        "auto" => "auto",
        _ => "risky",
    }
}

/// sanitizeOverlayPos.
pub fn sanitize_pos(p: Option<&Value>) -> Value {
    let obj = p.and_then(Value::as_object);
    let get = |k: &str| obj.and_then(|o| o.get(k));
    let h = get("h")
        .and_then(Value::as_str)
        .filter(|s| matches!(*s, "left" | "center" | "right"))
        .unwrap_or("right");
    let v = get("v")
        .and_then(Value::as_str)
        .filter(|s| matches!(*s, "top" | "center" | "bottom"))
        .unwrap_or("top");
    let clamp = |k: &str| match get(k) {
        Some(Value::Number(n)) if n.as_f64().is_some_and(f64::is_finite) => {
            clamp_i(js_round(n.as_f64().unwrap_or(16.0)) as i64, 0, 8000)
        }
        _ => 16,
    };
    json!({ "h": h, "v": v, "offX": clamp("offX"), "offY": clamp("offY") })
}

fn clamp_w(v: Option<&Value>, fallback: i64) -> i64 {
    let n = js_round(js_number(v));
    if !n.is_finite() {
        return fallback;
    }
    clamp_i(n as i64, MIN_W, MAX_W)
}

fn size_from(v: Option<&Value>, w_fallback: i64, h_fallback: i64) -> Value {
    let o = v.and_then(Value::as_object);
    let w = round_or(o.and_then(|o| o.get("width")), w_fallback);
    let h = round_or(o.and_then(|o| o.get("height")), h_fallback);
    json!({ "width": clamp_i(w, 380, 900), "height": clamp_i(h, 360, 1400) })
}

/// The settings file.
pub struct Store {
    path: PathBuf,
    data: Mutex<Map<String, Value>>,
    crypt: OsCrypt,
    /// B-002: the file exists but could not be read, so nothing is written
    /// over it this session; settings live in memory only.
    saving_disabled: bool,
    /// Whether the file on disk is one this session read or wrote whole, so
    /// it may become `aevalbuddy.json.bak` before the next replace.
    disk_good: AtomicBool,
}

static GLOBAL: OnceLock<Store> = OnceLock::new();

/// B-001: one save at a time. The snapshot, the write and the replace all
/// happen under it, so the last save always writes the newest state and two
/// saves never interleave.
static SAVE: Mutex<()> = parking_lot::const_mutex(());

/// A failed save is logged once per session, not on every window move.
static SAVE_FAILURE_LOGGED: AtomicBool = AtomicBool::new(false);

type Reader<'a> = &'a dyn Fn(&Path) -> io::Result<Vec<u8>>;

fn read_file(path: &Path) -> io::Result<Vec<u8>> {
    fs::read(path)
}

/// `<path><suffix>`, beside the settings file.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// The settings object in `bytes`, decoded as Node did (invalid UTF-8
/// becomes U+FFFD, a BOM is skipped). None when it is not a JSON object.
fn parse_object(bytes: &[u8]) -> Option<Map<String, Value>> {
    let text = String::from_utf8_lossy(bytes);
    match serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) {
        Ok(Value::Object(m)) => Some(m),
        _ => None,
    }
}

/// Reads the file, retrying an error that may pass (a scanner holding it):
/// three more tries, 100 ms apart. A missing file is not retried.
fn read_retrying(path: &Path, read: Reader<'_>) -> io::Result<Vec<u8>> {
    let mut result = read(path);
    for _ in 0..3 {
        match &result {
            Err(e) if e.kind() != io::ErrorKind::NotFound => {
                std::thread::sleep(std::time::Duration::from_millis(100));
                result = read(path);
            }
            _ => break,
        }
    }
    result
}

/// What `open` found on disk.
struct Loaded {
    file: Map<String, Value>,
    saving_disabled: bool,
    disk_good: bool,
}

/// B-001, B-002: only a missing file starts fresh. A file that cannot be
/// read is never written over; a file that does not parse is kept as
/// `.corrupt` and the last good copy, `.bak`, is used in its place.
fn load(path: &Path, read: Reader<'_>) -> Loaded {
    let corrupt = sibling(path, ".corrupt");
    let bytes = match read_retrying(path, read) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Loaded {
                file: Map::new(),
                saving_disabled: false,
                disk_good: false,
            };
        }
        Err(e) => {
            let _ = fs::copy(path, &corrupt);
            let kind = e.kind();
            crate::crash::write("store", &format!("read failed: {kind:?}; saving disabled"));
            return Loaded {
                file: Map::new(),
                saving_disabled: true,
                disk_good: false,
            };
        }
    };
    if let Some(file) = parse_object(&bytes) {
        return Loaded {
            file,
            saving_disabled: false,
            disk_good: true,
        };
    }
    // electron-store would refuse to start. Keep the file for a person to
    // recover, and carry on from the last good copy, else from defaults.
    let _ = fs::copy(path, &corrupt);
    let backup = read(&sibling(path, ".bak"))
        .ok()
        .and_then(|b| parse_object(&b));
    let note = if backup.is_some() {
        "settings file unreadable; restored aevalbuddy.json.bak"
    } else {
        "settings file unreadable; starting from defaults"
    };
    crate::crash::write("store", note);
    Loaded {
        file: backup.unwrap_or_default(),
        saving_disabled: false,
        disk_good: false,
    }
}

/// electron-store's serialisation: tab-indented JSON.
fn serialize(data: &Map<String, Value>) -> serde_json::Result<Vec<u8>> {
    use serde::Serialize;
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b"\t");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    data.serialize(&mut ser)?;
    Ok(buf)
}

/// B-084: a key saved without encryption is logged once per session.
static PLAIN_KEY_LOGGED: AtomicBool = AtomicBool::new(false);

/// Opens the store in the user-data folder once; every module shares it.
pub fn init(user_data: &Path) -> &'static Store {
    GLOBAL.get_or_init(|| Store::open(user_data))
}

/// The shared store. Panics if `init` has not run (it runs first in lib.rs).
pub fn get() -> &'static Store {
    GLOBAL
        .get()
        .expect("store::init runs before anything reads settings")
}

impl Store {
    /// Reads `aevalbuddy.json`, merges defaults over it, runs the migrations
    /// and writes the result back.
    pub fn open(user_data: &Path) -> Store {
        let crypt = OsCrypt::load_or_create(user_data);
        Self::open_with(user_data, crypt)
    }

    pub fn open_with(user_data: &Path, crypt: OsCrypt) -> Store {
        Self::open_reading(user_data, crypt, &read_file)
    }

    fn open_reading(user_data: &Path, crypt: OsCrypt, read: Reader<'_>) -> Store {
        let path = user_data.join("aevalbuddy.json");
        durable::sweep_temps(&path);
        let Loaded {
            file,
            saving_disabled,
            disk_good,
        } = load(&path, read);
        // A profile that chose the solid surface before the slider existed
        // keeps its look: no glass (ARCHITECTURE.md 10.1).
        let was_solid = !file.contains_key("glassIntensity")
            && file.get("surface").and_then(Value::as_str) == Some("solid");
        // A level stored on the v2.1 glass scale moves to the v2.2 one.
        let glass_v1 = file.contains_key("glassIntensity") && !file.contains_key("glassScale");
        // Object.assign({}, defaults, file): defaults' order, file values.
        let mut data = defaults();
        if was_solid {
            data.insert("glassIntensity".into(), json!(0.0));
        }
        for (k, v) in file {
            data.insert(k, v);
        }
        if glass_v1 {
            let v = glass_v1_to_v2(js_number(data.get("glassIntensity")));
            data.insert("glassIntensity".into(), json!(v));
            data.insert("glassScale".into(), json!(GLASS_SCALE));
        }
        let store = Store {
            path,
            data: Mutex::new(data),
            crypt,
            saving_disabled,
            disk_good: AtomicBool::new(disk_good),
        };
        store.migrate();
        store.save();
        store
    }

    /// B-001: under `SAVE`, snapshot the settings, copy the good file on disk
    /// to `.bak`, then write the snapshot durably (unique temp, fsync,
    /// write-through replace).
    fn save(&self) {
        if self.saving_disabled {
            return;
        }
        let _one = SAVE.lock();
        let snapshot = serialize(&self.data.lock());
        let bytes = match snapshot {
            Ok(bytes) => bytes,
            Err(e) => {
                self.save_failed(&e.to_string());
                return;
            }
        };
        if self.disk_good.load(Ordering::SeqCst) {
            let _ = fs::copy(&self.path, sibling(&self.path, ".bak"));
        }
        match durable::write(&self.path, &bytes) {
            Ok(()) => self.disk_good.store(true, Ordering::SeqCst),
            Err(e) => self.save_failed(&e.to_string()),
        }
    }

    fn save_failed(&self, why: &str) {
        // Always on stderr, so a smoke log shows it; errors.log once.
        eprintln!("[aevalbuddy] store: settings not saved: {why}");
        if !SAVE_FAILURE_LOGGED.swap(true, Ordering::SeqCst) {
            crate::crash::write("store", &format!("settings not saved: {why}"));
        }
    }

    pub fn raw(&self, key: &str) -> Option<Value> {
        self.data.lock().get(key).cloned()
    }

    /// K-08: stores `value` under `key` as given, for settings that have no
    /// typed setter here (textScale, providerOrder). Saved like every other
    /// setting.
    pub fn set_raw(&self, key: &str, value: Value) {
        self.set(key, value);
    }

    /// K-09: how API keys are kept on this machine: "encrypted" with the
    /// Windows data protection key, or "plain" when that key could not be
    /// read or made (Settings says so, C-58).
    pub fn key_storage(&self) -> &'static str {
        if self.crypt.available() {
            "encrypted"
        } else {
            "plain"
        }
    }

    fn set(&self, key: &str, value: Value) {
        self.data.lock().insert(key.to_string(), value);
        self.save();
    }

    fn flag(&self, key: &str) -> bool {
        truthy(self.raw(key).as_ref())
    }

    fn str_of(&self, key: &str) -> String {
        js_string(self.raw(key).as_ref())
    }

    /// STORE-39 and the provider-key migration.
    fn migrate(&self) {
        if js_number(self.raw("shortcutsVersion").as_ref()) < SHORTCUTS_VERSION as f64 {
            let mut saved = self
                .raw("shortcuts")
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default();
            let defaults = default_shortcuts();
            for k in ACTIONS {
                let keep = match saved.get(k) {
                    None => false,
                    Some(v) => v.as_str() != Some(old_default(k)),
                };
                if !keep {
                    saved.insert(k.into(), defaults[k].clone());
                }
            }
            let mut d = self.data.lock();
            d.insert("shortcuts".into(), Value::Object(saved));
            if d.get("findRelated") == Some(&Value::Bool(false)) {
                d.insert("findRelatedMode".into(), json!("off"));
            }
            d.shift_remove("findRelated");
            d.shift_remove("lastMode");
            d.insert("shortcutsVersion".into(), json!(SHORTCUTS_VERSION));
        }
        if js_number(self.raw("apiKeysVersion").as_ref()) < API_KEYS_VERSION as f64 {
            let enc = self.str_of("apiKeyEncrypted");
            let plain = self.str_of("apiKeyPlain");
            let mut d = self.data.lock();
            let keys = d.entry("apiKeys").or_insert_with(|| json!({}));
            if !keys.is_object() {
                *keys = json!({});
            }
            if let Some(m) = keys.as_object_mut() {
                let have = m.get("openrouter").is_some_and(|e| {
                    e.get("encrypted").is_some_and(truthy_ref)
                        || e.get("plain").is_some_and(truthy_ref)
                });
                if !have && (!enc.is_empty() || !plain.is_empty()) {
                    m.insert(
                        "openrouter".into(),
                        json!({ "encrypted": enc, "plain": plain }),
                    );
                }
            }
            d.insert("apiKeysVersion".into(), json!(API_KEYS_VERSION));
        }
        self.mirror_v1_key();
    }

    /// B-082: the v1 fields are the OpenRouter key's source of truth (the
    /// Electron build reads and writes only them), so the copy in `apiKeys`
    /// follows them: a key changed or cleared after a rollback stays changed
    /// or cleared.
    fn mirror_v1_key(&self) {
        let enc = self.str_of("apiKeyEncrypted");
        let plain = self.str_of("apiKeyPlain");
        let mut d = self.data.lock();
        let keys = d.entry("apiKeys").or_insert_with(|| json!({}));
        if !keys.is_object() {
            *keys = json!({});
        }
        let Some(m) = keys.as_object_mut() else {
            return;
        };
        let entry = m.get("openrouter");
        let same = js_string(entry.and_then(|e| e.get("encrypted"))) == enc
            && js_string(entry.and_then(|e| e.get("plain"))) == plain;
        let nothing = entry.is_none() && enc.is_empty() && plain.is_empty();
        if !same && !nothing {
            m.insert(
                "openrouter".into(),
                json!({ "encrypted": enc, "plain": plain }),
            );
        }
    }

    // ---------------------------------------------------------------- keys

    pub fn encryption_ready(&self) -> bool {
        self.crypt.available()
    }

    fn decrypt_b64(&self, blob: &str) -> String {
        if !self.crypt.available() && !blob.is_empty() {
            // safeStorage unavailable: Electron fell through to the plain key.
            return String::new();
        }
        B64.decode(blob.trim())
            .ok()
            .and_then(|b| self.crypt.decrypt(&b))
            .unwrap_or_default()
    }

    /// The saved key for one provider, decrypted. '' when none or when it
    /// cannot be decrypted (the person re-enters it; never a crash).
    pub fn get_provider_key(&self, provider: &str) -> String {
        if provider == "openrouter" {
            // B-082: the v1 fields win (see `mirror_v1_key`).
            return self.legacy_api_key();
        }
        let entry = self
            .raw("apiKeys")
            .and_then(|k| k.get(provider).cloned())
            .unwrap_or(Value::Null);
        let enc = js_string(entry.get("encrypted"));
        let plain = js_string(entry.get("plain"));
        if !enc.is_empty() && self.crypt.available() {
            return self.decrypt_b64(&enc);
        }
        plain
    }

    /// lib/store.js getApiKey: the OpenRouter key in the v1 fields.
    fn legacy_api_key(&self) -> String {
        let blob = self.str_of("apiKeyEncrypted");
        if !blob.is_empty() && self.crypt.available() {
            return self.decrypt_b64(&blob);
        }
        self.str_of("apiKeyPlain")
    }

    pub fn set_provider_key(&self, provider: &str, key: &str) {
        if !PROVIDERS.contains(&provider) {
            return;
        }
        let value = key.trim();
        let (enc, plain) = match self.crypt.encrypt(value) {
            Some(blob) if !value.is_empty() => (B64.encode(blob), String::new()),
            _ => (String::new(), value.to_string()),
        };
        if !plain.is_empty() && !PLAIN_KEY_LOGGED.swap(true, Ordering::SeqCst) {
            let why = "Windows data protection is not available";
            crate::crash::write(
                "store",
                &format!("the {provider} key was saved in plain text: {why}"),
            );
        }
        {
            let mut d = self.data.lock();
            let keys = d.entry("apiKeys").or_insert_with(|| json!({}));
            if !keys.is_object() {
                *keys = json!({});
            }
            if let Some(m) = keys.as_object_mut() {
                m.insert(
                    provider.into(),
                    json!({ "encrypted": enc.clone(), "plain": plain.clone() }),
                );
            }
            if provider == "openrouter" {
                // Mirrored into the v1 fields so the Electron build reads it.
                d.insert("apiKeyEncrypted".into(), json!(enc));
                d.insert("apiKeyPlain".into(), json!(plain));
            }
        }
        self.save();
    }

    pub fn clear_provider_key(&self, provider: &str) {
        self.set_provider_key(provider, "");
    }

    pub fn has_provider_key(&self, provider: &str) -> bool {
        !self.get_provider_key(provider).is_empty()
    }

    /// `{ anthropic: bool, openai: bool, gemini: bool, openrouter: bool }`.
    pub fn provider_keys_present(&self) -> Value {
        let mut m = Map::new();
        for p in PROVIDERS {
            m.insert(p.into(), json!(self.has_provider_key(p)));
        }
        Value::Object(m)
    }

    /// lib/store.js getApiKey (OpenRouter).
    pub fn get_api_key(&self) -> String {
        self.get_provider_key("openrouter")
    }

    pub fn set_api_key(&self, key: &str) {
        self.set_provider_key("openrouter", key);
    }

    pub fn clear_api_key(&self) {
        self.clear_provider_key("openrouter");
    }

    /// STORE-41, widened for REQUESTS.md 2: true when any provider has a key,
    /// since any one of them can answer.
    pub fn has_api_key(&self) -> bool {
        PROVIDERS.iter().any(|p| self.has_provider_key(p))
    }

    // -------------------------------------------------------------- models

    fn chosen(&self, key: &str, fallback: &str) -> String {
        if !self.flag(&format!("{key}Chosen")) {
            return fallback.into();
        }
        let v = self.str_of(key).trim().to_string();
        if v.is_empty() {
            fallback.into()
        } else {
            v
        }
    }

    fn choose(&self, key: &str, model: &str, fallback: &str) -> String {
        let v = model.trim();
        {
            let mut d = self.data.lock();
            d.insert(key.into(), json!(if v.is_empty() { fallback } else { v }));
            d.insert(format!("{key}Chosen"), json!(true));
        }
        self.save();
        self.chosen(key, fallback)
    }

    fn heal(&self, key: &str, id: &str, fallback: &str) -> String {
        if self.flag(&format!("{key}Chosen")) && !id.is_empty() && id != self.chosen(key, fallback)
        {
            self.set(key, json!(id));
        }
        self.chosen(key, fallback)
    }

    pub fn get_model(&self) -> String {
        self.chosen("model", DEFAULT_MODEL)
    }
    pub fn set_model(&self, m: &str) -> String {
        self.choose("model", m, DEFAULT_MODEL)
    }
    pub fn heal_model(&self, id: &str) -> String {
        self.heal("model", id, DEFAULT_MODEL)
    }
    pub fn get_brain(&self) -> String {
        self.chosen("brain", DEFAULT_BRAIN)
    }
    pub fn set_brain(&self, m: &str) -> String {
        self.choose("brain", m, DEFAULT_BRAIN)
    }
    pub fn heal_brain(&self, id: &str) -> String {
        self.heal("brain", id, DEFAULT_BRAIN)
    }
    pub fn get_find_model(&self) -> String {
        self.chosen("findModel", DEFAULT_FIND)
    }
    pub fn set_find_model(&self, m: &str) -> String {
        self.choose("findModel", m, DEFAULT_FIND)
    }
    pub fn heal_find_model(&self, id: &str) {
        self.heal("findModel", id, DEFAULT_FIND);
    }
    pub fn get_agent_model(&self) -> String {
        self.chosen("agentModel", DEFAULT_AGENT)
    }
    pub fn set_agent_model(&self, m: &str) -> String {
        self.choose("agentModel", m, DEFAULT_AGENT)
    }
    pub fn heal_agent_model(&self, id: &str) {
        self.heal("agentModel", id, DEFAULT_AGENT);
    }

    pub fn get_brain_enabled(&self) -> bool {
        self.flag("brainEnabledChosen") && self.flag("brainEnabled")
    }

    pub fn set_brain_enabled(&self, on: bool) -> bool {
        {
            let mut d = self.data.lock();
            d.insert("brainEnabled".into(), json!(on));
            d.insert("brainEnabledChosen".into(), json!(true));
        }
        self.save();
        self.get_brain_enabled()
    }

    pub fn get_show_paid_models(&self) -> bool {
        self.flag("showPaidModels")
    }

    pub fn set_show_paid_models(&self, on: bool) -> bool {
        self.set("showPaidModels", json!(on));
        self.get_show_paid_models()
    }

    /// 'auto' or a skill id `exists` accepts, else 'auto'.
    pub fn get_skill_id(&self, exists: impl Fn(&str) -> bool) -> String {
        let v = self.str_of("skill").trim().to_string();
        if v == "auto" || exists(&v) {
            v
        } else {
            "auto".into()
        }
    }

    pub fn set_skill_id(&self, id: &str, exists: impl Fn(&str) -> bool) -> String {
        let v = id.trim();
        let keep = v == "auto" || exists(v);
        self.set("skill", json!(if keep { v } else { "auto" }));
        self.get_skill_id(exists)
    }

    // ----------------------------------------------------------- shortcuts

    /// Stored chords merged over the defaults, never rewritten (KEY-14).
    pub fn get_shortcuts(&self) -> Map<String, Value> {
        let mut m = default_shortcuts();
        if let Some(Value::Object(saved)) = self.raw("shortcuts") {
            for (k, v) in saved {
                m.insert(k, v);
            }
        }
        m
    }

    pub fn shortcut(&self, action: &str) -> String {
        js_string(self.get_shortcuts().get(action))
    }

    pub fn set_shortcut(&self, action: &str, accelerator: &str) {
        let mut m = self.get_shortcuts();
        m.insert(action.into(), json!(accelerator));
        self.set("shortcuts", Value::Object(m));
    }

    pub fn reset_shortcuts(&self) -> Map<String, Value> {
        self.set("shortcuts", Value::Object(default_shortcuts()));
        self.get_shortcuts()
    }

    // ------------------------------------------------------------- layout

    /// Whether the round button that opens the bar floats on screen. Off
    /// is the default: the bar comes with its shortcut or from the tray.
    pub fn get_show_button(&self) -> bool {
        self.flag("showButton")
    }

    pub fn set_show_button(&self, on: bool) -> bool {
        self.set("showButton", json!(on));
        self.get_show_button()
    }

    /// The bar has two places, one for each way of opening it: with the
    /// button it grows out of the button's corner (`overlayPos`), without
    /// it the bar comes up on its own (`barPos`, the middle of the screen
    /// unless moved).
    fn pos_key(&self) -> &'static str {
        if self.get_show_button() {
            "overlayPos"
        } else {
            "barPos"
        }
    }

    pub fn get_overlay_pos(&self) -> Value {
        let key = self.pos_key();
        match self.raw(key) {
            None if key == "barPos" => {
                json!({ "h": "center", "v": "center", "offX": 16, "offY": 16 })
            }
            p => sanitize_pos(p.as_ref()),
        }
    }

    pub fn set_overlay_pos(&self, pos: &Value) -> Value {
        let v = sanitize_pos(Some(pos));
        self.set(self.pos_key(), v.clone());
        v
    }

    /// Read for migration and `overlay:state.outputPos` only; floating
    /// output windows ignore it (PARITY section 26).
    pub fn get_output_pos(&self) -> Value {
        let p = self.raw("outputPos");
        let mut base = sanitize_pos(p.as_ref());
        let docked = match p.as_ref().and_then(Value::as_object) {
            None => true,
            Some(o) => o.get("docked") != Some(&Value::Bool(false)),
        };
        base["docked"] = json!(docked);
        base
    }

    /// Where an output window was last dropped (a hint for the next one).
    pub fn set_output_pos(&self, pos: &Value) -> Value {
        let mut v = sanitize_pos(Some(pos));
        v["docked"] = json!(false);
        self.set("outputPos", v.clone());
        v
    }

    pub fn get_input_width(&self) -> i64 {
        clamp_w(self.raw("inputWidth").as_ref(), DEFAULT_INPUT_WIDTH)
    }

    pub fn set_input_width(&self, w: f64) -> i64 {
        let v = clamp_w(Some(&json!(w)), DEFAULT_INPUT_WIDTH);
        self.set("inputWidth", json!(v));
        self.get_input_width()
    }

    pub fn get_output_width(&self) -> i64 {
        clamp_w(self.raw("outputWidth").as_ref(), 420)
    }

    pub fn set_output_width(&self, w: f64) -> i64 {
        self.set("outputWidth", json!(clamp_w(Some(&json!(w)), 420)));
        self.get_output_width()
    }

    pub fn get_output_height(&self) -> i64 {
        let v = js_round(js_number(self.raw("outputHeight").as_ref()));
        if v.is_finite() && v > 0.0 {
            v as i64
        } else {
            0
        }
    }

    pub fn set_output_height(&self, h: f64) -> i64 {
        let v = js_round(h);
        let v = if v.is_finite() && v > 0.0 {
            v as i64
        } else {
            0
        };
        self.set("outputHeight", json!(v));
        self.get_output_height()
    }

    pub fn get_settings_size(&self) -> (i64, i64) {
        wh(&size_from(self.raw("settingsSize").as_ref(), 660, 560))
    }

    /// Falls back to 460x470 on bad input, as lib/store.js does.
    pub fn set_settings_size(&self, width: f64, height: f64) -> (i64, i64) {
        let v = size_from(Some(&json!({ "width": width, "height": height })), 460, 470);
        self.set("settingsSize", v);
        self.get_settings_size()
    }

    pub fn get_history_size(&self) -> (i64, i64) {
        wh(&size_from(self.raw("historySize").as_ref(), 460, 560))
    }

    pub fn set_history_size(&self, width: f64, height: f64) -> (i64, i64) {
        let v = size_from(Some(&json!({ "width": width, "height": height })), 460, 560);
        self.set("historySize", v);
        self.get_history_size()
    }

    // ------------------------------------------------------------- toggles

    pub fn get_save_history(&self) -> bool {
        self.flag("saveHistory")
    }

    pub fn set_save_history(&self, on: bool) -> bool {
        self.set("saveHistory", json!(on));
        self.get_save_history()
    }

    pub fn get_launch_at_login(&self) -> bool {
        self.flag("launchAtLogin")
    }

    pub fn set_launch_at_login(&self, on: bool) -> bool {
        self.set("launchAtLogin", json!(on));
        self.get_launch_at_login()
    }

    pub fn get_surface(&self) -> &'static str {
        if self.str_of("surface") == "solid" {
            "solid"
        } else {
            "auto"
        }
    }

    pub fn set_surface(&self, v: &str) -> &'static str {
        self.set(
            "surface",
            json!(if v == "solid" { "solid" } else { "auto" }),
        );
        self.get_surface()
    }

    /// 0..1; anything unreadable is the shipping default.
    pub fn get_glass_intensity(&self) -> f64 {
        let n = js_number(self.raw("glassIntensity").as_ref());
        if n.is_finite() {
            n.clamp(0.0, 1.0)
        } else {
            DEFAULT_GLASS_INTENSITY
        }
    }

    /// Clamped to 0..1, two decimals.
    pub fn set_glass_intensity(&self, v: f64) -> f64 {
        let n = if v.is_finite() {
            (v.clamp(0.0, 1.0) * 100.0).round() / 100.0
        } else {
            DEFAULT_GLASS_INTENSITY
        };
        self.set("glassIntensity", json!(n));
        self.get_glass_intensity()
    }

    pub fn get_has_sent_first_request(&self) -> bool {
        self.flag("hasSentFirstRequest")
    }

    /// Called when the first request goes out; suggestions never return.
    pub fn mark_first_request_sent(&self) {
        if !self.get_has_sent_first_request() {
            self.set("hasSentFirstRequest", json!(true));
        }
    }

    /// SYS-02: a profile with no key and no tour shown is a fresh install.
    pub fn decide_presence(&self) {
        if self.flag("presenceDecided") {
            return;
        }
        self.set("presenceDecided", json!(true));
        if self.has_api_key() || self.flag("walkthroughAutoShown") {
            return;
        }
        self.set("launchAtLogin", json!(true));
        self.set("presenceNoticePending", json!(true));
    }

    pub fn take_presence_notice(&self) -> bool {
        let due = self.flag("presenceNoticePending");
        if due {
            self.set("presenceNoticePending", json!(false));
        }
        due
    }

    pub fn get_walkthrough_auto_shown(&self) -> bool {
        self.flag("walkthroughAutoShown")
    }

    pub fn set_walkthrough_auto_shown(&self) {
        self.set("walkthroughAutoShown", json!(true));
    }

    // ------------------------------------------------------- input history

    pub fn get_input_history(&self) -> Vec<String> {
        let list: Vec<String> = match self.raw("inputHistory") {
            Some(Value::Array(a)) => a
                .into_iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => Vec::new(),
        };
        let skip = list.len().saturating_sub(INPUT_HISTORY_MAX);
        list[skip..].to_vec()
    }

    pub fn push_input_history(&self, text: &str) -> Vec<String> {
        let t = utf16_prefix(text.trim(), 500);
        if t.is_empty() || !self.get_save_history() {
            return self.get_input_history();
        }
        let mut h: Vec<String> = self
            .get_input_history()
            .into_iter()
            .filter(|x| *x != t)
            .collect();
        h.push(t);
        let skip = h.len().saturating_sub(INPUT_HISTORY_MAX);
        self.set("inputHistory", json!(h[skip..].to_vec()));
        self.get_input_history()
    }

    /// Kept for parity; unused, as in lib/store.js (IPC-31).
    #[allow(dead_code)]
    pub fn clear_input_history(&self) {
        self.set("inputHistory", json!([]));
    }

    // --------------------------------------------------------- find, agent

    pub fn get_find_settings(&self) -> Value {
        let related = match self.str_of("findRelatedMode").as_str() {
            m @ ("ask" | "auto" | "off") => m.to_string(),
            _ => "ask".into(),
        };
        json!({
            "scope": if self.str_of("findScope") == "screen" { "screen" } else { "window" },
            "related": related,
            "ocr": self.raw("findOcr") != Some(Value::Bool(false)),
        })
    }

    pub fn set_find_settings(&self, p: &Value) -> Value {
        {
            let mut d = self.data.lock();
            if let Some(s) = p.get("scope") {
                let v = if s.as_str() == Some("screen") {
                    "screen"
                } else {
                    "window"
                };
                d.insert("findScope".into(), json!(v));
            }
            if let Some(r) = p.get("related") {
                let m = match r {
                    Value::Bool(true) => "auto".to_string(),
                    Value::Bool(false) => "off".to_string(),
                    other => js_string(Some(other)),
                };
                let ok = matches!(m.as_str(), "ask" | "auto" | "off");
                d.insert(
                    "findRelatedMode".into(),
                    json!(if ok { m } else { "ask".into() }),
                );
            }
            if let Some(o) = p.get("ocr") {
                d.insert("findOcr".into(), json!(truthy(Some(o))));
            }
        }
        self.save();
        self.get_find_settings()
    }

    pub fn get_agent_settings(&self) -> Value {
        let steps = js_round(js_number(self.raw("agentMaxSteps").as_ref()));
        let steps = if steps.is_finite() {
            clamp_i(steps as i64, 10, 50)
        } else {
            25
        };
        json!({
            "permission": permission_of(&self.str_of("agentPermission")),
            "maxSteps": steps,
            "screenshots": self.raw("agentScreenshots") != Some(Value::Bool(false)),
            "returnCursor": self.raw("agentReturnCursor") != Some(Value::Bool(false)),
            "consent": self.flag("agentConsent"),
        })
    }

    pub fn set_agent_settings(&self, p: &Value) -> Value {
        {
            let mut d = self.data.lock();
            if let Some(v) = p.get("permission") {
                let permission = permission_of(v.as_str().unwrap_or_default());
                d.insert("agentPermission".into(), json!(permission));
            }
            if let Some(v) = p.get("maxSteps") {
                d.insert(
                    "agentMaxSteps".into(),
                    json!(clamp_i(round_or(Some(v), 25), 10, 50)),
                );
            }
            for (field, key) in [
                ("screenshots", "agentScreenshots"),
                ("returnCursor", "agentReturnCursor"),
                ("consent", "agentConsent"),
            ] {
                if let Some(v) = p.get(field) {
                    d.insert(key.into(), json!(truthy(Some(v))));
                }
            }
        }
        self.save();
        self.get_agent_settings()
    }
}

fn truthy_ref(v: &Value) -> bool {
    truthy(Some(v))
}

/// JavaScript's `s.slice(0, units)`, counted in UTF-16 units, except that a
/// surrogate pair is kept whole or dropped, never split.
fn utf16_prefix(s: &str, units: usize) -> String {
    let mut used = 0;
    s.chars()
        .take_while(|c| {
            used += c.len_utf16();
            used <= units
        })
        .collect()
}

fn wh(v: &Value) -> (i64, i64) {
    (
        v["width"].as_i64().unwrap_or(0),
        v["height"].as_i64().unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aeval-store-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn open(dir: &Path) -> Store {
        Store::open_with(dir, OsCrypt::with_key([3u8; 32]))
    }

    fn text(dir: &Path, name: &str) -> String {
        fs::read_to_string(dir.join(name)).unwrap()
    }

    fn file(dir: &Path) -> Map<String, Value> {
        let text = fs::read_to_string(dir.join("aevalbuddy.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn fresh_file_gets_every_default_in_order() {
        let dir = temp("fresh");
        let s = open(&dir);
        let f = file(&dir);
        let keys: Vec<&String> = f.keys().collect();
        assert_eq!(keys[0], "apiKeyEncrypted");
        assert_eq!(keys[2], "model");
        assert_eq!(f["shortcutsVersion"], 2);
        assert_eq!(s.get_model(), DEFAULT_MODEL);
        assert_eq!(s.shortcut("captureQuery"), "Ctrl+Alt+Space");
        assert_eq!(s.get_glass_intensity(), DEFAULT_GLASS_INTENSITY);
        assert!(!s.get_has_sent_first_request());
        // D-16: a new profile's bar is 600 px wide.
        assert_eq!(f["inputWidth"], 600);
        assert_eq!(s.get_input_width(), 600);
        assert_eq!(f["outputWidth"], 420);
        // electron-store writes tabs.
        let text = fs::read_to_string(dir.join("aevalbuddy.json")).unwrap();
        assert!(text.contains("\n\t\"apiKeyEncrypted\": \"\""));
    }

    /// A fresh profile saves, and a set lands on disk at once even while a
    /// reader holds the file open (bar's suggestionsOnce polls it).
    #[test]
    fn fresh_store_set_lands_on_disk_while_file_is_open() {
        let dir = temp("fresh-set");
        let s = open(&dir);
        assert!(!s.saving_disabled);
        let held = fs::File::open(dir.join("aevalbuddy.json")).unwrap();
        s.mark_first_request_sent();
        drop(held);
        assert_eq!(file(&dir)["hasSentFirstRequest"], true);
    }

    /// The bar keeps one place for each way of opening it: the middle of
    /// the screen without the button (the default), the button's corner
    /// with it.
    #[test]
    fn the_bar_has_a_place_for_each_way_of_opening_it() {
        let dir = temp("bar-pos");
        let s = open(&dir);
        assert!(!s.get_show_button());
        let mid = s.get_overlay_pos();
        assert_eq!(
            (mid["h"].as_str(), mid["v"].as_str()),
            (Some("center"), Some("center"))
        );
        s.set_overlay_pos(&json!({ "h": "left", "v": "bottom", "offX": 16, "offY": 16 }));
        assert!(s.set_show_button(true));
        let corner = s.get_overlay_pos();
        assert_eq!(
            (corner["h"].as_str(), corner["v"].as_str()),
            (Some("right"), Some("top"))
        );
        assert!(!s.set_show_button(false));
        assert_eq!(s.get_overlay_pos()["v"], "bottom");
        assert_eq!(file(&dir)["barPos"]["h"], "left");
    }

    #[test]
    fn unknown_keys_survive_and_values_win_over_defaults() {
        let dir = temp("unknown");
        fs::write(
            dir.join("aevalbuddy.json"),
            r#"{"future":{"a":1},"inputWidth":700,"shortcutsVersion":2}"#,
        )
        .unwrap();
        let s = open(&dir);
        assert_eq!(s.get_input_width(), 700);
        s.set_save_history(false);
        let f = file(&dir);
        assert_eq!(f["future"]["a"], 1);
        assert_eq!(f["saveHistory"], false);
    }

    #[test]
    fn v2_shortcut_migration() {
        let dir = temp("migrate");
        fs::write(
            dir.join("aevalbuddy.json"),
            r#"{"shortcuts":{"captureQuery":"Ctrl+Shift+Space","captureRegion":"Ctrl+Alt+R","findOnScreen":"Ctrl+Shift+G"},"findRelated":false,"lastMode":"ask"}"#,
        )
        .unwrap();
        let s = open(&dir);
        assert_eq!(s.shortcut("captureQuery"), "Ctrl+Alt+Space");
        assert_eq!(s.shortcut("captureRegion"), "Ctrl+Alt+R");
        assert_eq!(s.shortcut("findOnScreen"), "");
        assert_eq!(s.shortcut("agentTask"), "");
        let f = file(&dir);
        assert_eq!(f["findRelatedMode"], "off");
        assert!(!f.contains_key("findRelated"));
        assert!(!f.contains_key("lastMode"));
        assert_eq!(f["shortcutsVersion"], 2);
    }

    #[test]
    fn chosen_flag_rules() {
        let dir = temp("chosen");
        let s = open(&dir);
        assert_eq!(s.heal_model("x/retired"), DEFAULT_MODEL);
        assert_eq!(s.set_model("  "), DEFAULT_MODEL);
        assert_eq!(s.set_model("a/b"), "a/b");
        assert_eq!(s.heal_model("c/d"), "c/d");
        assert!(!s.get_brain_enabled());
        assert!(s.set_brain_enabled(true));
        assert_eq!(s.set_find_model("f/m"), "f/m");
        assert_eq!(s.get_agent_model(), DEFAULT_AGENT);
    }

    #[test]
    fn sanitisers_match_lib_store() {
        let p = sanitize_pos(Some(
            &json!({"h":"middle","v":"bottom","offX":12.5,"offY":"9"}),
        ));
        assert_eq!(p, json!({"h":"right","v":"bottom","offX":13,"offY":16}));
        assert_eq!(sanitize_pos(None)["offX"], 16);
        assert_eq!(sanitize_pos(Some(&json!({"offX": 9999})))["offX"], 8000);
        let dir = temp("sizes");
        let s = open(&dir);
        assert_eq!(s.set_input_width(100.0), 320);
        assert_eq!(s.set_output_width(2000.0), 900);
        assert_eq!(s.set_output_height(-5.0), 0);
        assert_eq!(s.set_output_height(300.4), 300);
        assert_eq!(s.set_settings_size(f64::NAN, 2000.0), (460, 1400));
        assert_eq!(s.get_history_size(), (460, 560));
        assert_eq!(s.get_output_pos()["docked"], true);
    }

    #[test]
    fn find_and_agent_settings() {
        let dir = temp("findagent");
        let s = open(&dir);
        assert_eq!(
            s.set_find_settings(&json!({"related": true}))["related"],
            "auto"
        );
        assert_eq!(
            s.set_find_settings(&json!({"related": false}))["related"],
            "off"
        );
        assert_eq!(
            s.set_find_settings(&json!({"related": "x"}))["related"],
            "ask"
        );
        assert_eq!(
            s.set_find_settings(&json!({"scope": "screen"}))["scope"],
            "screen"
        );
        assert_eq!(
            s.set_agent_settings(&json!({"maxSteps": 99}))["maxSteps"],
            50
        );
        assert_eq!(
            s.set_agent_settings(&json!({"maxSteps": "x"}))["maxSteps"],
            25
        );
        assert_eq!(
            s.set_agent_settings(&json!({"permission": "auto"}))["permission"],
            "auto"
        );
        assert_eq!(
            s.set_agent_settings(&json!({"permission": "always"}))["permission"],
            "risky"
        );
        assert_eq!(
            s.set_agent_settings(&json!({"permission": "every"}))["permission"],
            "every"
        );
    }

    #[test]
    fn input_history() {
        let dir = temp("input");
        let s = open(&dir);
        s.push_input_history(" a ");
        s.push_input_history("b");
        assert_eq!(s.push_input_history("a"), vec!["b", "a"]);
        for i in 0..60 {
            s.push_input_history(&i.to_string());
        }
        assert_eq!(s.get_input_history().len(), 50);
        s.set_save_history(false);
        assert_eq!(s.push_input_history("zzz").last().unwrap(), "59");
        // 500 UTF-16 units, as JavaScript's slice(0, 500): 250 emoji.
        s.set_save_history(true);
        let kept = s.push_input_history(&"😀".repeat(300));
        assert_eq!(kept.last().unwrap(), &"😀".repeat(250));
        let odd = s.push_input_history(&format!("a{}", "😀".repeat(300)));
        assert_eq!(odd.last().unwrap(), &format!("a{}", "😀".repeat(249)));
    }

    #[test]
    fn provider_keys_encrypt_and_mirror_openrouter() {
        let dir = temp("keys");
        let s = open(&dir);
        assert!(!s.has_api_key());
        s.set_provider_key("anthropic", " sk-ant-1 ");
        assert_eq!(s.get_provider_key("anthropic"), "sk-ant-1");
        assert!(s.has_api_key());
        assert!(!s.has_provider_key("openrouter"));
        s.set_api_key("sk-or-xyz");
        let f = file(&dir);
        let enc = f["apiKeyEncrypted"].as_str().unwrap();
        let blob = B64.decode(enc).unwrap();
        assert!(blob.starts_with(b"v10"));
        assert_eq!(
            crypto::open(&[3u8; 32], &blob).as_deref(),
            Some("sk-or-xyz")
        );
        assert_eq!(f["apiKeyPlain"], "");
        assert_eq!(f["apiKeys"]["openrouter"]["encrypted"], enc);
        s.clear_api_key();
        assert_eq!(s.get_api_key(), "");
        assert_eq!(s.provider_keys_present()["anthropic"], true);
    }

    #[test]
    fn electron_key_is_migrated() {
        let dir = temp("legacy");
        let blob = crypto::seal(&[3u8; 32], &[5u8; 12], "sk-or-old").unwrap();
        let text = json!({ "apiKeyEncrypted": B64.encode(blob), "shortcutsVersion": 2 });
        fs::write(dir.join("aevalbuddy.json"), text.to_string()).unwrap();
        let s = open(&dir);
        assert_eq!(s.get_api_key(), "sk-or-old");
        let f = file(&dir);
        assert!(
            f["apiKeys"]["openrouter"]["encrypted"]
                .as_str()
                .unwrap()
                .len()
                > 10
        );
        assert_eq!(f["apiKeysVersion"], 1);
    }

    #[test]
    fn v1_fields_win_after_rollback() {
        let dir = temp("rollback");
        let a = crypto::seal(&[3u8; 32], &[5u8; 12], "sk-or-a").unwrap();
        let b = crypto::seal(&[3u8; 32], &[6u8; 12], "sk-or-b").unwrap();
        let (a, b) = (B64.encode(a), B64.encode(b));
        // This build saved A in both places; the Electron build, after a
        // rollback, changed the v1 field to B.
        let text = json!({
            "apiKeyEncrypted": b,
            "apiKeys": { "openrouter": { "encrypted": a, "plain": "" } },
            "apiKeysVersion": 1
        });
        fs::write(dir.join("aevalbuddy.json"), text.to_string()).unwrap();
        let s = open(&dir);
        assert_eq!(s.get_api_key(), "sk-or-b");
        assert_eq!(file(&dir)["apiKeys"]["openrouter"]["encrypted"], b);
        // Then it cleared the key: both v1 fields empty.
        let text = json!({
            "apiKeyEncrypted": "",
            "apiKeyPlain": "",
            "apiKeys": { "openrouter": { "encrypted": a, "plain": "" } },
            "apiKeysVersion": 1
        });
        fs::write(dir.join("aevalbuddy.json"), text.to_string()).unwrap();
        let s = open(&dir);
        assert_eq!(s.get_api_key(), "");
        assert!(!s.has_provider_key("openrouter"));
        assert_eq!(file(&dir)["apiKeys"]["openrouter"]["encrypted"], "");
    }

    #[test]
    fn undecryptable_key_reads_as_empty() {
        let dir = temp("badkey");
        let blob = crypto::seal(&[8u8; 32], &[5u8; 12], "sk-or-old").unwrap();
        let text = json!({ "apiKeyEncrypted": B64.encode(blob) });
        fs::write(dir.join("aevalbuddy.json"), text.to_string()).unwrap();
        let s = open(&dir);
        assert_eq!(s.get_api_key(), "");
        assert!(!s.has_api_key());
    }

    #[test]
    fn presence_and_glass() {
        let dir = temp("presence");
        let s = open(&dir);
        s.decide_presence();
        assert!(s.get_launch_at_login());
        assert!(s.take_presence_notice());
        assert!(!s.take_presence_notice());
        assert_eq!(s.set_glass_intensity(4.0), 1.0);
        assert_eq!(s.set_glass_intensity(f64::NAN), DEFAULT_GLASS_INTENSITY);
        assert_eq!(s.set_glass_intensity(0.333), 0.33);
        s.mark_first_request_sent();
        assert!(s.get_has_sent_first_request());
        assert_eq!(s.set_skill_id("debug", |id| id == "debug"), "debug");
        assert_eq!(s.set_skill_id("gone", |_| false), "auto");
    }

    #[test]
    fn solid_profiles_start_with_no_glass() {
        let dir = temp("solid");
        fs::write(dir.join("aevalbuddy.json"), r#"{"surface":"solid"}"#).unwrap();
        assert_eq!(open(&dir).get_glass_intensity(), 0.0);
        let dir = temp("solid-set");
        let text = r#"{"surface":"solid","glassIntensity":0.7,"glassScale":2}"#;
        fs::write(dir.join("aevalbuddy.json"), text).unwrap();
        assert_eq!(open(&dir).get_glass_intensity(), 0.7);
    }

    #[test]
    fn v1_glass_levels_move_to_the_wider_scale() {
        for (stored, want) in [("0.5", 0.75), ("0", 0.0), ("0.25", 0.31), ("1", 0.86)] {
            let dir = temp(&format!("glass-v1-{stored}"));
            let text = format!(r#"{{"glassIntensity":{stored}}}"#);
            fs::write(dir.join("aevalbuddy.json"), text).unwrap();
            let s = open(&dir);
            assert_eq!(s.get_glass_intensity(), want, "v2.1 {stored}");
            assert_eq!(file(&dir)["glassScale"], GLASS_SCALE);
            // Opened again, the value is not moved a second time.
            assert_eq!(open(&dir).get_glass_intensity(), want);
        }
    }

    #[test]
    fn corrupt_file_is_kept_aside() {
        let dir = temp("corrupt");
        fs::write(dir.join("aevalbuddy.json"), "{not json").unwrap();
        let s = open(&dir);
        assert_eq!(s.get_input_width(), DEFAULT_INPUT_WIDTH);
        assert!(dir.join("aevalbuddy.json.corrupt").exists());
    }

    #[test]
    fn concurrent_sets_never_lose_or_corrupt() {
        let dir = temp("concurrent");
        let s = open(&dir);
        std::thread::scope(|scope| {
            for t in 0..8 {
                let s = &s;
                scope.spawn(move || {
                    for i in 0..100 {
                        s.set_raw(&format!("k{t}-{i}"), json!(i));
                    }
                });
            }
        });
        let f = file(&dir);
        for t in 0..8 {
            for i in 0..100 {
                assert_eq!(f[&format!("k{t}-{i}")], i, "k{t}-{i}");
            }
        }
        // The last good file before the final save is kept beside it.
        let bak = text(&dir, "aevalbuddy.json.bak");
        assert!(serde_json::from_str::<Map<String, Value>>(&bak).is_ok());
        let left: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn parse_failure_recovers_from_bak() {
        let dir = temp("bak");
        let s = open(&dir);
        s.set_input_width(700.0);
        s.set_save_history(false);
        // .bak holds the file as it was before the latest save.
        s.set_output_width(500.0);
        drop(s);
        assert!(dir.join("aevalbuddy.json.bak").exists());
        fs::write(dir.join("aevalbuddy.json"), "{\"inputWidth\": 7").unwrap();
        let s = open(&dir);
        assert_eq!(s.get_input_width(), 700);
        assert!(!s.get_save_history());
        assert_eq!(text(&dir, "aevalbuddy.json.corrupt"), "{\"inputWidth\": 7");
        // The torn file never becomes the backup.
        s.set_input_width(500.0);
        let bak = text(&dir, "aevalbuddy.json.bak");
        let bak: Map<String, Value> = serde_json::from_str(&bak).unwrap();
        assert_eq!(bak["inputWidth"], 700);
        assert_eq!(file(&dir)["inputWidth"], 500);
    }

    #[test]
    fn invalid_utf8_is_read_lossy_not_reset() {
        let dir = temp("lossy");
        let mut bytes = br#"{"inputWidth":700,"shortcutsVersion":2,"note":"a"#.to_vec();
        bytes.push(0xff);
        bytes.extend_from_slice(br#"b"}"#);
        fs::write(dir.join("aevalbuddy.json"), &bytes).unwrap();
        let s = open(&dir);
        assert_eq!(s.get_input_width(), 700);
        assert_eq!(s.raw("note"), Some(json!("a\u{fffd}b")));
        assert!(!dir.join("aevalbuddy.json.corrupt").exists());
    }

    fn denied(_: &Path) -> io::Result<Vec<u8>> {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }

    #[test]
    fn unreadable_file_disables_saving() {
        let dir = temp("denied");
        let original = br#"{"inputWidth":700,"apiKeyPlain":"sk-keep"}"#;
        fs::write(dir.join("aevalbuddy.json"), original).unwrap();
        let s = Store::open_reading(&dir, OsCrypt::with_key([3u8; 32]), &denied);
        assert_eq!(s.get_input_width(), DEFAULT_INPUT_WIDTH);
        s.set_save_history(false);
        s.set_api_key("sk-new");
        assert!(!s.get_save_history(), "settings still work in memory");
        assert_eq!(fs::read(dir.join("aevalbuddy.json")).unwrap(), original);
        assert!(!dir.join("aevalbuddy.json.bak").exists());
    }

    #[test]
    fn stored_input_width_is_kept() {
        let dir = temp("width");
        fs::write(dir.join("aevalbuddy.json"), r#"{"inputWidth":420}"#).unwrap();
        assert_eq!(open(&dir).get_input_width(), 420);
    }

    #[test]
    fn set_raw_stores_and_saves() {
        let dir = temp("raw");
        let s = open(&dir);
        s.set_raw("providerOrder", json!(["gemini", "openrouter"]));
        s.set_raw("textScale", json!(1.3));
        assert_eq!(s.raw("textScale"), Some(json!(1.3)));
        let f = file(&dir);
        assert_eq!(f["providerOrder"], json!(["gemini", "openrouter"]));
        assert_eq!(open(&dir).raw("textScale"), Some(json!(1.3)));
    }

    #[test]
    fn key_storage_reports_plain_without_os_key() {
        let dir = temp("plainkey");
        assert_eq!(open(&dir).key_storage(), "encrypted");
        let dir = temp("plainkey-none");
        let s = Store::open_with(&dir, OsCrypt::without_key());
        assert_eq!(s.key_storage(), "plain");
        s.set_provider_key("openai", " sk-plain ");
        assert_eq!(s.get_provider_key("openai"), "sk-plain");
        let f = file(&dir);
        assert_eq!(f["apiKeys"]["openai"]["plain"], "sk-plain");
        assert_eq!(f["apiKeys"]["openai"]["encrypted"], "");
    }

    #[test]
    fn js_helpers() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_number(Some(&json!(" 12 "))), 12.0);
        assert!(js_number(None).is_nan());
        assert_eq!(js_number(Some(&Value::Null)), 0.0);
    }
}
