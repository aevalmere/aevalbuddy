//! Auto-update on the existing GitHub feed (PARITY UPD-01..15,
//! ARCHITECTURE.md D6). A port of lib/updater.js speaking electron-builder's
//! feed: `releases/latest` for the tag (not the rate-limited API), the
//! releases atom feed for the notes, `latest.yml` for the file and sha512.
//! The installer downloads in the background into `pending\`, is verified,
//! and runs silently on restart or on quit.
//!
//! Schedule: 20 s after launch (2 s with a test feed), every 4 h, 15 s after
//! a resume if stale or failing, when the network returns; never while an
//! answer streams (deferred 60 s); no background check under smoke or shots.
//! Failures retry on 2, 5, 15, 30, 60, then every 120 min, +0..10 % jitter,
//! and are told in one plain sentence; the raw error goes to updater.log.
//! Every failed attempt counts and schedules the next one, even when its
//! message matches the last (B-017). Nothing is installed without a sha512
//! that matches (B-079).

use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use parking_lot::Mutex;
use regex::Regex;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha512};
use tauri::{AppHandle, Emitter};

const FIRST_CHECK_MS: u64 = 20_000;
const RECHECK_MS: u64 = 4 * 60 * 60 * 1000;
const RESUME_DELAY_MS: u64 = 15_000;
const RESUME_STALE_MS: u64 = 30 * 60 * 1000;
const BUSY_DEFER_MS: u64 = 60_000;
const ONLINE_POLL_MS: u64 = 30_000;
const BACKOFF_MIN: [u64; 6] = [2, 5, 15, 30, 60, 120];
const LOG_MAX_BYTES: u64 = 256 * 1024;

/// FILE-10: the feed compiled in, replacing electron-builder's app-update.yml.
pub const FEED_PROVIDER: &str = "github";
pub const FEED_OWNER: &str = "aevalmere";
pub const FEED_REPO: &str = "aevalbuddy";

pub fn feed_config() -> Value {
    json!({ "provider": FEED_PROVIDER, "owner": FEED_OWNER, "repo": FEED_REPO, "private": false })
}

fn release_base() -> String {
    format!("https://github.com/{FEED_OWNER}/{FEED_REPO}/releases/tag/v")
}

// ------------------------------------------------------------- pure parts

/// Strips tags and decodes the entities GitHub's notes use.
pub fn plain(html: &str) -> String {
    static TAG: OnceLock<Regex> = OnceLock::new();
    let tag = TAG.get_or_init(|| Regex::new(r"<[^>]+>").expect("tag regex"));
    let s = tag.replace_all(html, " ");
    let s = s
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&");
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// UPD-06: up to three list items joined ` · `, else the first paragraph;
/// 180 characters at most, cut at a word with an ellipsis.
pub fn notes_snippet(html: &str) -> String {
    static LI: OnceLock<Regex> = OnceLock::new();
    static P_END: OnceLock<Regex> = OnceLock::new();
    let li = LI.get_or_init(|| Regex::new(r"(?is)<li[^>]*>(.*?)</li>").expect("li regex"));
    let p_end = P_END.get_or_init(|| Regex::new(r"(?i)</p>").expect("p regex"));
    let items: Vec<String> = li
        .captures_iter(html)
        .map(|c| plain(&c[1]))
        .filter(|s| !s.is_empty())
        .collect();
    let mut text = if items.is_empty() {
        plain(p_end.split(html).next().unwrap_or(""))
    } else {
        items[..items.len().min(3)].join(" · ")
    };
    if text.chars().count() > 180 {
        let head: String = text.chars().take(177).collect();
        // JS: slice(0, 177).replace(/\s+\S*$/, '')
        let cut = match head.rfind(char::is_whitespace) {
            Some(i) => head[..i].trim_end().to_string(),
            None => head,
        };
        text = format!("{cut}…");
    }
    text
}

/// UPD-05. `status` is an HTTP status when there was one.
pub fn classify(message: &str, status: Option<u16>) -> &'static str {
    static OFFLINE: OnceLock<Regex> = OnceLock::new();
    static LIMITED: OnceLock<Regex> = OnceLock::new();
    static CORRUPT: OnceLock<Regex> = OnceLock::new();
    let offline = OFFLINE.get_or_init(|| {
        Regex::new(
            r"(?i)ERR_INTERNET_DISCONNECTED|ERR_NAME_NOT_RESOLVED|ERR_NAME_RESOLUTION_FAILED|ERR_NETWORK_CHANGED|ERR_NETWORK_IO_SUSPENDED|ERR_ADDRESS_UNREACHABLE|ERR_CONNECTION_(REFUSED|RESET|TIMED_OUT|CLOSED|ABORTED|FAILED)|ERR_TIMED_OUT|ERR_PROXY_CONNECTION_FAILED|ERR_NETWORK_ACCESS_DENIED|ENOTFOUND|EAI_AGAIN|ECONNRESET|ECONNREFUSED|ETIMEDOUT|ENETUNREACH|socket hang up|dns error|failed to lookup address|connection refused|connection reset|timed out|error trying to connect",
        )
        .expect("offline regex")
    });
    let limited = LIMITED
        .get_or_init(|| Regex::new(r"(?i)\b429\b|rate.?limit|too many requests").expect("re"));
    let corrupt = CORRUPT.get_or_init(|| {
        Regex::new(r"(?i)sha512 checksum mismatch|ERR_CHECKSUM_MISMATCH|checksum").expect("re")
    });
    if offline.is_match(message) {
        return "offline";
    }
    if status == Some(429) || limited.is_match(message) {
        return "limited";
    }
    if corrupt.is_match(message) {
        return "corrupt";
    }
    "server"
}

/// Local `H:MM AM` as toLocaleTimeString([], {hour:'numeric', minute:'2-digit'}).
fn clock(ms: u64) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_millis_opt(ms as i64).single() {
        Some(t) => t.format("%-I:%M %p").to_string(),
        None => String::new(),
    }
}

/// UPD-10. Never says error, exception, stack or http.
pub fn problem_text(kind: &str, next_check: u64) -> String {
    let at = if next_check > 0 {
        format!(" Trying again at {}.", clock(next_check))
    } else {
        String::new()
    };
    match kind {
        "offline" => "You're offline. AevalBuddy checks again once you're connected.".into(),
        "limited" => format!("GitHub asked for a pause.{at}"),
        "corrupt" => format!("The download did not verify, so it was thrown away.{at}"),
        _ => format!("The update server did not answer.{at}"),
    }
}

/// UPD-04, before jitter.
pub fn backoff_base(failures: u32) -> u64 {
    if failures == 0 {
        return RECHECK_MS;
    }
    let i = (failures as usize).min(BACKOFF_MIN.len()) - 1;
    BACKOFF_MIN[i] * 60 * 1000
}

fn backoff(failures: u32) -> u64 {
    use rand::Rng;
    let base = backoff_base(failures) as f64;
    (base * (1.0 + rand::thread_rng().gen::<f64>() * 0.1)).round() as u64
}

/// UPD-12: a test feed, honoured in a packaged build only under smoke or
/// `AEVAL_UPDATE_TEST=1`; https, or http to loopback; no credentials.
pub fn feed_override_from(get: impl Fn(&str) -> Option<String>, packaged: bool) -> String {
    let url = get("AEVAL_UPDATE_FEED")
        .unwrap_or_default()
        .trim()
        .to_string();
    if url.is_empty() {
        return String::new();
    }
    let test = get("AEVAL_SMOKE").is_some_and(|v| !v.is_empty())
        || get("AEVAL_UPDATE_TEST").as_deref() == Some("1");
    if packaged && !test {
        return String::new();
    }
    let Ok(u) = tauri::Url::parse(&url) else {
        return String::new();
    };
    if !u.username().is_empty() || u.password().is_some() {
        return String::new();
    }
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    let loopback = matches!(host.as_str(), "127.0.0.1" | "localhost" | "[::1]" | "::1");
    if u.scheme() != "https" && !(u.scheme() == "http" && loopback) {
        return String::new();
    }
    if url.ends_with('/') {
        url
    } else {
        format!("{url}/")
    }
}

pub fn feed_override() -> String {
    feed_override_from(|k| std::env::var(k).ok(), packaged())
}

/// What `latest.yml` says.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Latest {
    pub version: String,
    pub path: String,
    pub sha512: String,
    pub notes: String,
}

fn yaml_scalar(v: &str) -> String {
    let v = v.trim();
    let v = v
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .or_else(|| v.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
        .unwrap_or(v);
    v.to_string()
}

/// electron-builder's latest.yml: `version`, `files: [{url, sha512, size}]`,
/// `path`, `sha512`, optional `releaseNotes`. The first file wins, as in
/// electron-updater.
pub fn parse_latest_yml(text: &str) -> Option<Latest> {
    let mut out = Latest::default();
    let mut file_url = String::new();
    let mut file_sha = String::new();
    let mut in_files = false;
    let mut first_file_done = false;
    let mut notes_block = false;
    for raw in text.lines() {
        let line = raw.trim_end();
        if notes_block {
            if line.starts_with("  ") || line.is_empty() {
                if !out.notes.is_empty() {
                    out.notes.push('\n');
                }
                out.notes.push_str(line.trim());
                continue;
            }
            notes_block = false;
        }
        if !line.starts_with(' ') && !line.starts_with('-') {
            in_files = false;
            let Some((k, v)) = line.split_once(':') else {
                continue;
            };
            match k.trim() {
                "version" => out.version = yaml_scalar(v),
                "path" => out.path = yaml_scalar(v),
                "sha512" => out.sha512 = yaml_scalar(v),
                "files" => in_files = true,
                "releaseNotes" => {
                    let v = v.trim();
                    if v == "|" || v == "|-" || v == ">" || v.is_empty() {
                        notes_block = true;
                    } else {
                        out.notes = yaml_scalar(v);
                    }
                }
                _ => {}
            }
            continue;
        }
        if in_files && !first_file_done {
            let item = line.trim_start();
            let starts_item = item.starts_with("- ");
            if starts_item && !file_url.is_empty() {
                first_file_done = true;
                continue;
            }
            let item = item.trim_start_matches("- ");
            if let Some((k, v)) = item.split_once(':') {
                match k.trim() {
                    "url" => file_url = yaml_scalar(v),
                    "sha512" => file_sha = yaml_scalar(v),
                    _ => {}
                }
            }
        }
    }
    if !file_url.is_empty() {
        out.path = file_url;
    }
    if !file_sha.is_empty() {
        out.sha512 = file_sha;
    }
    if out.version.is_empty() || out.path.is_empty() {
        return None;
    }
    Some(out)
}

fn xml_decode(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// The notes HTML of the atom entry for `tag`.
pub fn atom_notes(atom: &str, tag: &str) -> Option<String> {
    let want = format!("/releases/tag/{tag}\"");
    for entry in atom.split("<entry>").skip(1) {
        if !entry.contains(&want) {
            continue;
        }
        let start = entry.find("<content")?;
        let body = &entry[start..];
        let open_end = body.find('>')? + 1;
        let close = body.find("</content>")?;
        return Some(xml_decode(&body[open_end..close]));
    }
    None
}

/// The tag `releases/latest` answers with (`{"tag_name": "v1.2.0", ...}`).
pub fn latest_tag(json_text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json_text).ok()?;
    v.get("tag_name")?.as_str().map(String::from)
}

pub fn is_newer(remote: &str, current: &str) -> bool {
    let r = semver::Version::parse(remote.trim_start_matches('v'));
    let c = semver::Version::parse(current.trim_start_matches('v'));
    match (r, c) {
        (Ok(r), Ok(c)) => r > c,
        _ => false,
    }
}

pub fn sha512_b64(bytes: &[u8]) -> String {
    B64.encode(Sha512::digest(bytes))
}

// ------------------------------------------------------------ runtime

fn packaged() -> bool {
    !cfg!(debug_assertions)
}

fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

struct Inner {
    state: Map<String, Value>,
    failures: u32,
    /// The id the next check or download takes.
    next_attempt: u64,
    /// The attempt that failed last: one attempt is counted once.
    last_failed_attempt: u64,
    announce_pending: bool,
    announced_version: String,
    in_flight: bool,
    timer_gen: u64,
    waiting_network: bool,
    last_progress_emit: u64,
    installer: Option<PathBuf>,
    installer_started: bool,
    last_run_version: String,
}

fn initial_state() -> Map<String, Value> {
    let v = json!({
        "status": "idle",
        "version": "",
        "notes": "",
        "releaseUrl": "",
        "progress": 0,
        "transferred": 0,
        "total": 0,
        "bytesPerSecond": 0,
        "lastChecked": 0,
        "nextCheck": 0,
        "problem": "",
        "restartPending": false,
        "updatedFrom": "",
    });
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

fn inner() -> &'static Mutex<Inner> {
    static I: OnceLock<Mutex<Inner>> = OnceLock::new();
    I.get_or_init(|| {
        Mutex::new(Inner {
            state: initial_state(),
            failures: 0,
            next_attempt: 1,
            last_failed_attempt: 0,
            announce_pending: false,
            announced_version: String::new(),
            in_flight: false,
            timer_gen: 0,
            waiting_network: false,
            last_progress_emit: 0,
            installer: None,
            installer_started: false,
            last_run_version: String::new(),
        })
    })
}

static APP: OnceLock<AppHandle> = OnceLock::new();
static DIR: OnceLock<PathBuf> = OnceLock::new();

fn user_file(name: &str) -> PathBuf {
    DIR.get()
        .cloned()
        .unwrap_or_else(crate::paths::user_data_dir)
        .join(name)
}

/// `[ISO] level message`, keeping the newest 128 KB past 256 KB (FILE-05).
pub fn log(level: &str, message: &str) {
    let p = user_file("updater.log");
    if let Ok(meta) = fs::metadata(&p) {
        if meta.len() > LOG_MAX_BYTES {
            if let Ok(bytes) = fs::read(&p) {
                let keep = (LOG_MAX_BYTES / 2) as usize;
                let tail = &bytes[bytes.len().saturating_sub(keep)..];
                let _ = fs::write(&p, String::from_utf8_lossy(tail).as_bytes());
            }
        }
    }
    let line = format!("[{}] {level} {message}\n", crate::crash::iso_now());
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = f.write_all(line.as_bytes());
    }
    if level != "info" {
        eprintln!("[aevalbuddy] update {level}: {message}");
    }
}

/// A copy of the update state with `current`, for tray, Settings and smoke.
pub fn get() -> Value {
    let mut m = inner().lock().state.clone();
    m.insert("current".into(), json!(version()));
    Value::Object(m)
}

fn set(patch: Value) {
    {
        let mut i = inner().lock();
        if let Value::Object(p) = patch {
            for (k, v) in p {
                i.state.insert(k, v);
            }
        }
    }
    if let Some(app) = APP.get() {
        crate::tray::refresh(app);
        let _ = app.emit_to("settings", "update:state", get());
    }
}

fn status() -> String {
    inner().lock().state["status"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

fn is_busy() -> bool {
    APP.get()
        .map(|a| crate::state::shared(a).lock().busy)
        .unwrap_or(false)
}

/// LIFE-13, LIFE-17: a non-empty `AEVAL_OFFLINE` forces offline (an empty
/// one does not, as in Electron); otherwise the OS says.
pub fn online() -> bool {
    online_from(std::env::var_os("AEVAL_OFFLINE").as_deref(), connected)
}

fn online_from(offline_env: Option<&OsStr>, connected: impl FnOnce() -> bool) -> bool {
    if offline_env.is_some_and(|v| !v.is_empty()) {
        return false;
    }
    connected()
}

fn connected() -> bool {
    use windows_sys::Win32::Networking::WinInet::InternetGetConnectedState;
    let mut flags = 0u32;
    // SAFETY: an out-parameter and a reserved zero.
    unsafe { InternetGetConnectedState(&mut flags, 0) != 0 }
}

/// A fresh id for one check or one download.
fn next_attempt() -> u64 {
    let mut i = inner().lock();
    let id = i.next_attempt;
    i.next_attempt += 1;
    id
}

/// `in_flight` for as long as one check runs, cleared however it ends.
struct InFlight;

impl InFlight {
    /// None while another check is in flight.
    fn take() -> Option<InFlight> {
        let already = std::mem::replace(&mut inner().lock().in_flight, true);
        (!already).then_some(InFlight)
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        inner().lock().in_flight = false;
    }
}

fn pending_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("AEVAL_USERDATA").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join("pending");
    }
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("aevalbuddy")
        .join("pending")
}

fn load_persisted() {
    let Ok(text) = fs::read_to_string(user_file("updater.json")) else {
        return;
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return;
    };
    let mut i = inner().lock();
    if let Some(n) = v["lastChecked"].as_f64() {
        i.state.insert("lastChecked".into(), json!(n as u64));
    }
    if let Some(s) = v["lastRunVersion"].as_str() {
        i.last_run_version = s.to_string();
    }
}

fn persist() {
    let last = inner().lock().state["lastChecked"].clone();
    let body = json!({ "lastChecked": last, "lastRunVersion": version() });
    let _ = fs::write(user_file("updater.json"), body.to_string());
}

fn schedule(ms: u64) {
    let gen = {
        let mut i = inner().lock();
        i.timer_gen += 1;
        i.state
            .insert("nextCheck".into(), json!(crate::state::now_ms() + ms));
        i.timer_gen
    };
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        if inner().lock().timer_gen != gen {
            return;
        }
        if is_busy() {
            schedule(BUSY_DEFER_MS);
            return;
        }
        if let Some(app) = APP.get() {
            check(app).await;
        }
    });
}

fn clear_timer() {
    inner().lock().timer_gen += 1;
}

fn wait_for_network() {
    {
        let mut i = inner().lock();
        if i.waiting_network {
            return;
        }
        i.waiting_network = true;
    }
    tauri::async_runtime::spawn(async {
        loop {
            tokio::time::sleep(Duration::from_millis(ONLINE_POLL_MS)).await;
            if online() {
                break;
            }
        }
        inner().lock().waiting_network = false;
        schedule(if is_busy() { BUSY_DEFER_MS } else { 1000 });
    });
}

/// Counts attempt `attempt` as failed, once: a download that fails after
/// its check did is still one attempt, while two attempts that fail with
/// the same message are two. None when this attempt was already counted.
fn count_failure(i: &mut Inner, attempt: u64) -> Option<u32> {
    if attempt == i.last_failed_attempt {
        return None;
    }
    i.last_failed_attempt = attempt;
    i.failures += 1;
    Some(i.failures)
}

/// Every failed attempt is told and schedules the next, so a manual check
/// that fails ends in `retrying` or `offline`, never `checking`.
fn fail(attempt: u64, message: &str, http: Option<u16>) {
    let Some(failures) = count_failure(&mut inner().lock(), attempt) else {
        return;
    };
    let kind = classify(message, http);
    log("warn", &format!("{kind} (attempt {failures}): {message}"));
    if kind == "offline" && !online() {
        clear_timer();
        set(json!({
            "status": "offline",
            "problem": problem_text("offline", 0),
            "progress": 0,
            "nextCheck": 0,
        }));
        wait_for_network();
        return;
    }
    let wait = backoff(failures);
    schedule(wait);
    let next = crate::state::now_ms() + wait;
    let shown = if kind == "offline" { "server" } else { kind };
    set(json!({ "status": "retrying", "problem": problem_text(shown, next), "progress": 0 }));
}

/// Called once at boot (UPD-02, UPD-11, UPD-14).
pub fn init(app: &AppHandle, user_data: &Path) {
    let _ = APP.set(app.clone());
    let _ = DIR.set(user_data.to_path_buf());
    if !packaged() {
        set(json!({ "status": "dev" }));
        return;
    }
    load_persisted();
    let last_run = inner().lock().last_run_version.clone();
    let current = version();
    let updated_from = if !last_run.is_empty() && last_run != current {
        last_run
    } else {
        String::new()
    };
    if !updated_from.is_empty() {
        log(
            "info",
            &format!("running {current}, updated from {updated_from}"),
        );
        let url = format!("{}{current}", release_base());
        set(json!({ "updatedFrom": updated_from, "releaseUrl": url }));
    }
    persist();
    if std::env::var_os("AEVAL_SMOKE").is_some() || std::env::var_os("AEVAL_SHOT").is_some() {
        return;
    }
    if !updated_from.is_empty() {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            announce_updated(&app);
        });
    }
    let first = if feed_override().is_empty() {
        FIRST_CHECK_MS
    } else {
        2000
    };
    schedule(first);
}

/// After a resume (WM_POWERBROADCAST, from the window manager).
pub fn on_resume() {
    let (s, in_flight, last) = {
        let i = inner().lock();
        let s = i.state["status"].as_str().unwrap_or("").to_string();
        let last = i.state["lastChecked"].as_u64().unwrap_or(0);
        (s, i.in_flight, last)
    };
    if s == "ready" || s == "downloading" || in_flight || s == "dev" {
        return;
    }
    let stale = crate::state::now_ms().saturating_sub(last) > RESUME_STALE_MS;
    if s == "offline" || s == "retrying" || stale {
        schedule(RESUME_DELAY_MS);
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(format!("AevalBuddy/{}", version()))
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(45))
        .build()
        .unwrap_or_default()
}

struct NetError {
    message: String,
    status: Option<u16>,
}

impl From<reqwest::Error> for NetError {
    fn from(e: reqwest::Error) -> Self {
        let mut message = e.to_string();
        let mut src: Option<&dyn std::error::Error> = std::error::Error::source(&e);
        while let Some(s) = src {
            message.push_str(": ");
            message.push_str(&s.to_string());
            src = s.source();
        }
        NetError {
            status: e.status().map(|s| s.as_u16()),
            message,
        }
    }
}

async fn fetch_text(c: &reqwest::Client, url: &str, accept: &str) -> Result<String, NetError> {
    let res = c.get(url).header("Accept", accept).send().await?;
    let status = res.status();
    if !status.is_success() {
        return Err(NetError {
            message: format!("{} {} for {url}", status.as_u16(), status),
            status: Some(status.as_u16()),
        });
    }
    Ok(res.text().await?)
}

/// What the feed offers: (latest, notes html, download url).
async fn read_feed(c: &reqwest::Client) -> Result<(Latest, String, String), NetError> {
    let feed = feed_override();
    if !feed.is_empty() {
        let yml = fetch_text(c, &format!("{feed}latest.yml"), "*/*").await?;
        let latest = parse_latest_yml(&yml).ok_or_else(|| NetError {
            message: "latest.yml could not be read".into(),
            status: None,
        })?;
        let url = format!("{feed}{}", latest.path);
        let notes = latest.notes.clone();
        return Ok((latest, notes, url));
    }
    let base = format!("https://github.com/{FEED_OWNER}/{FEED_REPO}");
    let tag_json = fetch_text(c, &format!("{base}/releases/latest"), "application/json").await?;
    let tag = latest_tag(&tag_json).ok_or_else(|| NetError {
        message: "no tag_name in releases/latest".into(),
        status: None,
    })?;
    let yml_url = format!("{base}/releases/download/{tag}/latest.yml");
    let yml = fetch_text(c, &yml_url, "*/*").await?;
    let latest = parse_latest_yml(&yml).ok_or_else(|| NetError {
        message: "latest.yml could not be read".into(),
        status: None,
    })?;
    let notes = match fetch_text(c, &format!("{base}/releases.atom"), "*/*").await {
        Ok(atom) => atom_notes(&atom, &tag).unwrap_or_default(),
        Err(_) => latest.notes.clone(),
    };
    let url = format!("{base}/releases/download/{tag}/{}", latest.path);
    Ok((latest, notes, url))
}

/// One check, now; resolves to the state once settled (UPD-03, IPC-34).
pub async fn check(app: &AppHandle) -> Value {
    let _ = APP.set(app.clone());
    if !packaged() {
        return get();
    }
    let s = status();
    if s == "ready" || s == "downloading" {
        return get();
    }
    let Some(_flight) = InFlight::take() else {
        // Wait for the check in flight to settle.
        while inner().lock().in_flight {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        return get();
    };
    let attempt = next_attempt();
    clear_timer();
    if !online() {
        set(json!({ "status": "offline", "problem": problem_text("offline", 0), "nextCheck": 0 }));
        wait_for_network();
        return get();
    }
    set(json!({ "status": "checking", "problem": "" }));
    let c = client();
    match read_feed(&c).await {
        Ok((latest, notes, url)) => {
            {
                let mut i = inner().lock();
                i.failures = 0;
                i.state
                    .insert("lastChecked".into(), json!(crate::state::now_ms()));
            }
            persist();
            if is_newer(&latest.version, &version()) {
                let release_url = if feed_override().is_empty() {
                    format!("{}{}", release_base(), latest.version)
                } else {
                    String::new()
                };
                set(json!({
                    "status": "downloading",
                    "version": latest.version,
                    "notes": notes_snippet(&notes),
                    "releaseUrl": release_url,
                    "progress": 0,
                    "transferred": 0,
                    "total": 0,
                    "problem": "",
                }));
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    download(&app, c, latest, url).await;
                });
                return get();
            }
            schedule(RECHECK_MS);
            set(json!({ "status": "current", "problem": "" }));
        }
        Err(e) => fail(attempt, &e.message, e.status),
    }
    get()
}

async fn download(app: &AppHandle, c: reqwest::Client, latest: Latest, url: String) {
    let attempt = next_attempt();
    match download_inner(&c, &latest, &url).await {
        Ok(file) => {
            {
                let mut i = inner().lock();
                i.failures = 0;
                i.installer = Some(file.clone());
                i.announce_pending = true;
            }
            clear_timer();
            set(json!({
                "status": "ready",
                "version": latest.version,
                "progress": 1,
                "problem": "",
                "nextCheck": 0,
            }));
            // Re-hash for the log: what the cloud test compares (UPD-07).
            if let Ok(bytes) = fs::read(&file) {
                let got = sha512_b64(&bytes);
                let ok = got == latest.sha512;
                log(
                    "info",
                    &format!(
                        "downloaded {} sha512={got} expected={} match={ok}",
                        latest.version, latest.sha512
                    ),
                );
            }
            if !is_busy() {
                idle(app);
            }
            if std::env::var("AEVAL_UPDATE_QUIT").as_deref() == Ok("1") {
                log(
                    "info",
                    "AEVAL_UPDATE_QUIT: quitting so the installer runs on quit",
                );
                app.exit(0);
            }
        }
        Err(e) => {
            set(json!({ "status": "idle" }));
            fail(attempt, &e.message, e.status);
        }
    }
}

async fn download_inner(
    c: &reqwest::Client,
    latest: &Latest,
    url: &str,
) -> Result<PathBuf, NetError> {
    let dir = pending_dir();
    let _ = fs::create_dir_all(&dir);
    let name = Path::new(&latest.path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "AevalBuddy-Setup.exe".into());
    let target = dir.join(&name);
    let part = dir.join(format!("{name}.part"));
    // Nothing to check it against: not worth the download.
    if latest.sha512.is_empty() {
        verify("", "")?;
    }
    let mut res = c.get(url).send().await?;
    let status = res.status();
    if !status.is_success() {
        return Err(NetError {
            message: format!("{} {} for {url}", status.as_u16(), status),
            status: Some(status.as_u16()),
        });
    }
    let total = res.content_length().unwrap_or(0);
    let started = std::time::Instant::now();
    let mut hasher = Sha512::new();
    let mut out = fs::File::create(&part).map_err(|e| NetError {
        message: e.to_string(),
        status: None,
    })?;
    let mut done: u64 = 0;
    while let Some(chunk) = res.chunk().await? {
        hasher.update(&chunk);
        out.write_all(&chunk).map_err(|e| NetError {
            message: e.to_string(),
            status: None,
        })?;
        done += chunk.len() as u64;
        progress(done, total, started.elapsed().as_secs_f64());
    }
    drop(out);
    let got = B64.encode(hasher.finalize());
    if let Err(e) = verify(&latest.sha512, &got) {
        let _ = fs::remove_file(&part);
        return Err(e);
    }
    let _ = fs::remove_file(&target);
    fs::rename(&part, &target).map_err(|e| NetError {
        message: e.to_string(),
        status: None,
    })?;
    Ok(target)
}

/// The download matches latest.yml's sha512. A feed without one is refused:
/// nothing unverified is installed.
fn verify(expected: &str, got: &str) -> Result<(), NetError> {
    let message = if expected.is_empty() {
        "no sha512 checksum in latest.yml; the update is not installed".to_string()
    } else if got != expected {
        format!("sha512 checksum mismatch, expected {expected}, got {got}")
    } else {
        return Ok(());
    };
    Err(NetError {
        message,
        status: None,
    })
}

/// One menu rebuild per visible change, not one per chunk.
fn progress(done: u64, total: u64, secs: f64) {
    let p = if total > 0 {
        (done as f64 / total as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let now = crate::state::now_ms();
    {
        let mut i = inner().lock();
        let prev = i.state["progress"].as_f64().unwrap_or(0.0);
        let same = (p * 100.0).floor() == (prev * 100.0).floor();
        if same && now - i.last_progress_emit < 1000 {
            return;
        }
        i.last_progress_emit = now;
    }
    let bps = if secs > 0.0 { done as f64 / secs } else { 0.0 };
    set(json!({
        "status": "downloading",
        "progress": p,
        "transferred": done,
        "total": total,
        "bytesPerSecond": bps.round() as u64,
    }));
}

fn announce(app: &AppHandle, version: &str) {
    {
        let mut i = inner().lock();
        if i.announced_version == version {
            return;
        }
        i.announced_version = version.to_string();
    }
    let app = app.clone();
    crate::notify::show(
        &format!("AevalBuddy {version} is ready"),
        "It installs the next time you quit. Click to restart now.",
        Some(Box::new(move || {
            restart(&app);
        })),
    );
}

fn announce_updated(app: &AppHandle) {
    let v = version();
    let url = format!("{}{v}", release_base());
    let app = app.clone();
    crate::notify::show(
        &format!("AevalBuddy updated to {v}"),
        "Click to see what's new.",
        Some(Box::new(move || {
            crate::windows::open_url(&app, &url);
        })),
    );
}

fn launch_installer(relaunch: bool) -> bool {
    use std::os::windows::process::CommandExt;
    let file = {
        let mut i = inner().lock();
        if i.installer_started {
            return true;
        }
        let Some(f) = i.installer.clone() else {
            return false;
        };
        i.installer_started = true;
        f
    };
    let mut cmd = std::process::Command::new(&file);
    cmd.arg("/S").arg("--updated");
    if relaunch {
        cmd.arg("--force-run");
    }
    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP: it outlives us.
    cmd.creation_flags(0x0000_0008 | 0x0000_0200);
    match cmd.spawn() {
        Ok(_) => {
            log("info", &format!("installer started: {}", file.display()));
            true
        }
        Err(e) => {
            log("error", &format!("installer did not start: {e}"));
            inner().lock().installer_started = false;
            false
        }
    }
}

/// UPD-09: quit and run the installer; waits for a streaming answer.
pub fn restart(app: &AppHandle) -> bool {
    if status() != "ready" {
        return false;
    }
    if is_busy() {
        set(json!({ "restartPending": true }));
        return true;
    }
    if launch_installer(true) {
        app.exit(0);
        return true;
    }
    false
}

/// Called whenever an answer or run finishes: held-back work runs here.
pub fn idle(app: &AppHandle) {
    let (ready, pending, announce_due, version) = {
        let i = inner().lock();
        (
            i.state["status"] == "ready",
            i.state["restartPending"] == true,
            i.announce_pending,
            i.state["version"].as_str().unwrap_or("").to_string(),
        )
    };
    if !ready {
        return;
    }
    if pending {
        restart(app);
        return;
    }
    if announce_due {
        inner().lock().announce_pending = false;
        announce(app, &version);
    }
}

pub fn is_ready() -> bool {
    status() == "ready"
}

/// On exit: a downloaded update installs now (install on quit).
pub fn install_on_quit() {
    if is_ready() {
        launch_installer(false);
    }
}

/// will-quit: timers stop.
pub fn destroy() {
    clear_timer();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_prefer_list_items() {
        let html = "<h2>What's new</h2><ul><li>Faster <b>Find</b></li><li>Fix &amp; polish</li>\
                    <li>Third</li><li>Fourth</li></ul>";
        assert_eq!(notes_snippet(html), "Faster Find · Fix & polish · Third");
        assert_eq!(notes_snippet("<p>One para.</p><p>Two.</p>"), "One para.");
        let long = format!("<p>{}</p>", "word ".repeat(60));
        let s = notes_snippet(&long);
        assert!(s.ends_with("word…"), "{s}");
        assert!(s.chars().count() <= 178);
    }

    #[test]
    fn classify_matches_lib_updater() {
        assert_eq!(classify("net::ERR_INTERNET_DISCONNECTED", None), "offline");
        assert_eq!(
            classify("getaddrinfo ENOTFOUND github.com", None),
            "offline"
        );
        assert_eq!(classify("whatever", Some(429)), "limited");
        assert_eq!(
            classify("HttpError: 429 Too Many Requests", None),
            "limited"
        );
        assert_eq!(classify("sha512 checksum mismatch", None), "corrupt");
        assert_eq!(classify("500 Internal Server Error", Some(500)), "server");
    }

    #[test]
    fn problem_texts_are_plain() {
        let off = problem_text("offline", 0);
        assert_eq!(
            off,
            "You're offline. AevalBuddy checks again once you're connected."
        );
        assert_eq!(problem_text("limited", 0), "GitHub asked for a pause.");
        let t = problem_text("server", 1_700_000_000_000);
        assert!(t.starts_with("The update server did not answer. Trying again at "));
        for k in ["offline", "limited", "corrupt", "server"] {
            let s = problem_text(k, 1).to_lowercase();
            for bad in ["error", "exception", "stack", "http"] {
                assert!(!s.contains(bad), "{k}: {s}");
            }
        }
    }

    #[test]
    fn backoff_table() {
        let mins: Vec<u64> = (1..=8).map(|n| backoff_base(n) / 60_000).collect();
        assert_eq!(mins, vec![2, 5, 15, 30, 60, 120, 120, 120]);
        let b = backoff(1);
        assert!((120_000..=132_000).contains(&b));
    }

    #[test]
    fn feed_override_rules() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        let https = &[("AEVAL_UPDATE_FEED", "https://example.com/feed")];
        assert_eq!(
            feed_override_from(env(https), false),
            "https://example.com/feed/"
        );
        assert_eq!(feed_override_from(env(https), true), "");
        let smoke = &[
            ("AEVAL_UPDATE_FEED", "http://127.0.0.1:8080/"),
            ("AEVAL_SMOKE", "1"),
        ];
        assert_eq!(
            feed_override_from(env(smoke), true),
            "http://127.0.0.1:8080/"
        );
        let remote_http = &[
            ("AEVAL_UPDATE_FEED", "http://example.com/"),
            ("AEVAL_SMOKE", "1"),
        ];
        assert_eq!(feed_override_from(env(remote_http), true), "");
        let creds = &[
            ("AEVAL_UPDATE_FEED", "https://u:p@example.com/"),
            ("AEVAL_UPDATE_TEST", "1"),
        ];
        assert_eq!(feed_override_from(env(creds), true), "");
        let junk = &[("AEVAL_UPDATE_FEED", "not a url")];
        assert_eq!(feed_override_from(env(junk), false), "");
    }

    const YML: &str = "version: 1.2.0\nfiles:\n  - url: AevalBuddy-Setup.exe\n    sha512: \
                       abc123==\n    size: 1234\n  - url: other.exe\n    sha512: zzz\npath: \
                       AevalBuddy-Setup.exe\nsha512: abc123==\nreleaseDate: \
                       '2026-11-01T10:00:00.000Z'\n";

    #[test]
    fn latest_yml() {
        let l = parse_latest_yml(YML).unwrap();
        assert_eq!(l.version, "1.2.0");
        assert_eq!(l.path, "AevalBuddy-Setup.exe");
        assert_eq!(l.sha512, "abc123==");
        let notes = "version: '2.0.0'\npath: a.exe\nsha512: x\nreleaseNotes: |\n  line one\n  \
                     line two\nreleaseDate: now\n";
        let l = parse_latest_yml(notes).unwrap();
        assert_eq!(l.version, "2.0.0");
        assert_eq!(l.notes, "line one\nline two");
        assert!(parse_latest_yml("nothing: here").is_none());
    }

    #[test]
    fn atom_and_tag() {
        let atom = r#"<feed><entry><id>1</id><link rel="alternate" type="text/html" href="https://github.com/aevalmere/aevalbuddy/releases/tag/v1.2.0"/><content type="html">&lt;ul&gt;&lt;li&gt;New&lt;/li&gt;&lt;/ul&gt;</content></entry><entry><link href="https://github.com/aevalmere/aevalbuddy/releases/tag/v1.1.1"/><content type="html">old</content></entry></feed>"#;
        assert_eq!(atom_notes(atom, "v1.2.0").unwrap(), "<ul><li>New</li></ul>");
        assert_eq!(atom_notes(atom, "v1.1.1").unwrap(), "old");
        assert!(atom_notes(atom, "v9").is_none());
        assert_eq!(
            latest_tag(r#"{"id":1,"tag_name":"v1.2.0"}"#).unwrap(),
            "v1.2.0"
        );
    }

    #[test]
    fn versions_and_hashes() {
        assert!(is_newer("1.2.0", "1.1.1"));
        assert!(is_newer("v1.10.0", "1.9.9"));
        assert!(!is_newer("1.1.1", "1.1.1"));
        assert!(!is_newer("junk", "1.1.1"));
        // sha512("abc"), base64, as electron-builder writes it.
        assert_eq!(
            sha512_b64(b"abc"),
            "3a81oZNherrMQXNJriBBMRLm+k6JqX6iCp7u5ktV05ohkpkqJ0/BqDa6PCOj/uu9RU1EI2Q86A4qmslPpUyknw=="
        );
        assert_eq!(feed_config()["owner"], "aevalmere");
    }

    #[test]
    fn identical_failures_both_reschedule() {
        let dir = std::env::temp_dir().join(format!("aeval-upd-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let _ = DIR.set(dir);
        let message = "503 Service Unavailable for https://example.invalid/latest.yml";
        let (before, gen) = {
            let i = inner().lock();
            (i.failures, i.timer_gen)
        };
        let first = next_attempt();
        fail(first, message, Some(503));
        let second = next_attempt();
        fail(second, message, Some(503));
        // The same attempt reporting again is not a new failure.
        fail(second, message, Some(503));
        let i = inner().lock();
        assert_eq!(i.failures - before, 2);
        assert!(i.timer_gen >= gen + 2, "each failure schedules a retry");
        assert_eq!(i.state["status"], "retrying");
        let next = i.state["nextCheck"].as_u64().unwrap_or(0);
        assert!(next > crate::state::now_ms());
    }

    #[test]
    fn in_flight_clears_on_panic() {
        let crashed = std::panic::catch_unwind(|| {
            let _flight = InFlight::take().expect("nothing in flight");
            assert!(InFlight::take().is_none(), "a second check waits");
            panic!("the check failed");
        });
        assert!(crashed.is_err());
        assert!(!inner().lock().in_flight);
        assert!(InFlight::take().is_some());
        assert!(!inner().lock().in_flight);
    }

    #[test]
    fn missing_sha512_refuses() {
        let message = |r: Result<(), NetError>| r.err().map(|e| e.message).unwrap_or_default();
        let none = message(verify("", "abc"));
        assert!(none.contains("no sha512"), "{none}");
        assert_eq!(classify(&none, None), "corrupt");
        let bad = message(verify("abc==", "xyz=="));
        assert!(bad.starts_with("sha512 checksum mismatch"), "{bad}");
        assert!(verify("abc==", "abc==").is_ok());
    }

    #[test]
    fn empty_offline_env_is_online() {
        assert!(online_from(None, || true));
        assert!(online_from(Some(OsStr::new("")), || true));
        assert!(!online_from(Some(OsStr::new("1")), || true));
        assert!(!online_from(None, || false));
        // Forced offline never asks the OS.
        assert!(!online_from(Some(OsStr::new("1")), || panic!("asked")));
    }
}
