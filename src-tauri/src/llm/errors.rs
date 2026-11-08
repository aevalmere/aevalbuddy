//! Request failures and the sentences the user reads about them (PARITY
//! FAIL-01, ERR-01..13). Port of `ApiError`, `shouldFailover` and
//! `friendlyError` in lib/openrouter.js, with the codes and actions of
//! design-ux U14 and the copy deck (C-61, C-70..C-74).

use std::fmt;
use std::sync::OnceLock;

use regex::Regex;

use super::catalogue::{self, Kind};
use super::payloads::{Action, Tone};
use super::provider::Provider;

/// What went wrong, in the terms the Electron code branched on: an HTTP
/// status (`ApiError`), an abort (`AbortError`), a dropped connection
/// (`TypeError` from fetch), or anything else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Http,
    Abort,
    Network,
    Other,
}

/// The codes a `query:error` carries, so the page can offer the action that
/// fixes each one (design-ux U14, contract K-17).
pub mod codes {
    pub const BAD_KEY: &str = "bad-key";
    pub const NO_CREDIT: &str = "no-credit";
    pub const DAILY_CAP: &str = "daily-cap";
    pub const FREE_BUSY: &str = "free-busy";
    pub const RATE_LIMITED: &str = "rate-limited";
    pub const MODEL_GONE: &str = "model-gone";
    pub const NO_VISION: &str = "no-vision";
    pub const DATA_POLICY: &str = "data-policy";
    pub const REFUSED: &str = "refused";
    pub const OFFLINE: &str = "offline";
    pub const SERVER: &str = "server";
    pub const UNKNOWN: &str = "unknown";
    pub const PAID_HOP: &str = "paid-hop";
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApiError {
    pub kind: ErrorKind,
    /// The HTTP status, or the numeric `error.code` of a stream error.
    pub status: Option<u16>,
    pub message: String,
    /// `Some("reader")` when the reader stage failed, so the error names the
    /// reader model rather than the brain that never ran.
    pub stage: Option<&'static str>,
    /// How long the provider asked for before the next request
    /// (`Retry-After`), in ms.
    pub retry_after_ms: Option<u64>,
    /// How many models were tried before this error came back; 1 means no
    /// stand-in was tried, so the copy must not claim one.
    pub attempts: u32,
    /// One of [`codes`], when the error was classified where it happened
    /// (a refusal in the stream); otherwise [`code_of`] works it out.
    pub code: Option<&'static str>,
}

impl ApiError {
    pub fn http(status: u16, message: impl Into<String>) -> Self {
        Self::coded(Some(status), message)
    }

    /// A stream or body error; `status` is `None` when its code is not a
    /// number.
    pub fn coded(status: Option<u16>, message: impl Into<String>) -> Self {
        ApiError {
            kind: ErrorKind::Http,
            status,
            message: redact_keys(&message.into()),
            stage: None,
            retry_after_ms: None,
            attempts: 1,
            code: None,
        }
    }

    pub fn abort() -> Self {
        Self::of(ErrorKind::Abort, "aborted")
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self::of(ErrorKind::Network, message)
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self::of(ErrorKind::Other, message)
    }

    fn of(kind: ErrorKind, message: impl Into<String>) -> Self {
        ApiError {
            kind,
            status: None,
            message: redact_keys(&message.into()),
            stage: None,
            retry_after_ms: None,
            attempts: 1,
            code: None,
        }
    }

    pub fn with_stage(mut self, stage: &'static str) -> Self {
        self.stage = Some(stage);
        self
    }

    pub fn with_retry_after(mut self, ms: Option<u64>) -> Self {
        self.retry_after_ms = ms;
        self
    }

    pub fn with_attempts(mut self, attempts: u32) -> Self {
        self.attempts = attempts.max(1);
        self
    }

    pub fn with_code(mut self, code: &'static str) -> Self {
        self.code = Some(code);
        self
    }

    pub fn is_abort(&self) -> bool {
        self.kind == ErrorKind::Abort
    }

    pub fn is_network(&self) -> bool {
        self.kind == ErrorKind::Network
    }
}

/// A provider's message with anything shaped like an API key replaced:
/// OpenAI quotes part of a rejected key back, and the message travels to
/// the page (Copy details) and the logs, where no key may appear.
pub fn redact_keys(text: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(KEY_PATTERN).expect("key pattern"));
    re.replace_all(text, "<key hidden>").into_owned()
}

/// OpenAI, OpenRouter and Anthropic keys start `sk-`, Gemini's `AIza`.
const KEY_PATTERN: &str = r"\b(?:sk-[A-Za-z0-9_*-]{4,}|AIza[0-9A-Za-z_-]{10,})";

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

/// Failure a different model can answer. Free models are queued and rate
/// limited by design, and a retired id 404s until someone edits the list, so
/// these are the everyday states rather than the exceptional ones. 529 is
/// Anthropic's "overloaded", which is busy by another name.
pub const FAILOVER_STATUS: [u16; 8] = [404, 408, 429, 500, 502, 503, 504, 529];

/// Busy: the same request may succeed after a wait (B-120).
pub const BUSY_STATUS: [u16; 2] = [429, 529];

pub fn should_failover(err: &ApiError) -> bool {
    if err.is_abort() || err.code.is_some() {
        return false;
    }
    // The daily free cap and an empty quota hold for every model of the
    // account, so another free model is one more refused request (B-043).
    if quota_code(err.status.unwrap_or(0), &err.message).is_some() {
        return false;
    }
    // A rejected key, no credit, or a malformed request fails the same way on
    // every model; retrying is just a second wait before the same message.
    err.status.is_some_and(|s| FAILOVER_STATUS.contains(&s))
}

fn mentions(detail: &str, words: &[&str]) -> bool {
    let lower = detail.to_lowercase();
    words.iter().any(|w| lower.contains(w))
}

/// An account-wide limit in a provider's own words: the daily free cap
/// (OpenRouter's `free-models-per-day`, Gemini's per-day quota) or an
/// exhausted balance (OpenAI's `insufficient_quota`).
fn quota_code(status: u16, detail: &str) -> Option<&'static str> {
    if !matches!(status, 402 | 403 | 429) {
        return None;
    }
    if mentions(detail, &NO_CREDIT_WORDS) {
        return Some(codes::NO_CREDIT);
    }
    if mentions(detail, &DAILY_WORDS) {
        return Some(codes::DAILY_CAP);
    }
    None
}

const NO_CREDIT_WORDS: [&str; 2] = ["insufficient_quota", "exceeded your current quota"];

const DAILY_WORDS: [&str; 6] = [
    "per-day",
    "per day",
    "perday",
    "daily",
    "resource_exhausted",
    "quota",
];

fn data_policy(provider: Provider, detail: &str) -> bool {
    provider == Provider::OpenRouter && mentions(detail, &["data policy", "free model publication"])
}

/// What kind of failure this is (design-ux U14's table).
pub fn code_of(provider: Provider, err: &ApiError, model: &str) -> &'static str {
    if let Some(code) = err.code {
        return code;
    }
    let detail = err.message.as_str();
    if data_policy(provider, detail) {
        return codes::DATA_POLICY;
    }
    if err.is_network() {
        return codes::OFFLINE;
    }
    let status = err.status.unwrap_or(0);
    if let Some(code) = quota_code(status, detail) {
        return code;
    }
    let bad_gemini_key = provider == Provider::Gemini
        && status == 400
        && mentions(detail, &["api key not valid", "api_key_invalid"]);
    let busy = BUSY_STATUS.contains(&status);
    let free = provider == Provider::OpenRouter && model.ends_with(":free");
    match status {
        401 => codes::BAD_KEY,
        _ if bad_gemini_key => codes::BAD_KEY,
        402 => codes::NO_CREDIT,
        403 => codes::REFUSED,
        404 => codes::MODEL_GONE,
        _ if busy && free => codes::FREE_BUSY,
        _ if busy => codes::RATE_LIMITED,
        400 if mentions(detail, &["image", "vision", "multimodal", "modality"]) => {
            codes::NO_VISION
        }
        408 | 500..=599 => codes::SERVER,
        _ => codes::UNKNOWN,
    }
}

/// What an error means for the person: its code, the sentence, the row's
/// tone and the actions that fix it, in the order shown (contract K-19).
#[derive(Clone, Debug, PartialEq)]
pub struct Explained {
    pub code: &'static str,
    pub text: String,
    pub tone: Tone,
    pub actions: Vec<Action>,
}

/// What [`explain`] knows besides the error.
#[derive(Clone, Copy, Debug)]
pub struct About<'a> {
    pub provider: Provider,
    /// The model id; people read `model_name`.
    pub model: &'a str,
    pub model_name: &'a str,
    /// The job, so `Choose model` opens the right picker.
    pub kind: Kind,
    /// Another provider ready to answer, for `Ask <Provider>`.
    pub next: Option<Provider>,
    /// The OS reports a connection, so a dropped stream is not "offline".
    pub online: bool,
}

impl<'a> About<'a> {
    pub fn new(provider: Provider, model: &'a str, model_name: &'a str, kind: Kind) -> Self {
        About {
            provider,
            model,
            model_name,
            kind,
            next: None,
            online: false,
        }
    }
}

const PRIVACY_URL: &str = "https://openrouter.ai/settings/privacy";

/// Where each provider takes payment.
pub fn credit_url(provider: Provider) -> &'static str {
    match provider {
        Provider::OpenRouter => "https://openrouter.ai/credits",
        Provider::Anthropic => "https://console.anthropic.com/settings/billing",
        Provider::OpenAi => "https://platform.openai.com/settings/organization/billing",
        Provider::Gemini => "https://aistudio.google.com/apikey",
    }
}

fn link(id: &str, label: &str, url: &str) -> Action {
    Action {
        url: Some(url.to_string()),
        ..Action::new(id, label)
    }
}

fn section(id: &str, label: &str, at: String) -> Action {
    Action {
        section: Some(at),
        ..Action::new(id, label)
    }
}

/// `Ask <Provider>`: the same question on another provider.
pub fn hop_action(label: String, to: Provider, model: Option<&str>) -> Action {
    Action {
        provider: Some(to.id().to_string()),
        model: model.map(str::to_string),
        ..Action::new("hop", label)
    }
}

fn ask_next(a: &About<'_>) -> Option<Action> {
    a.next.map(|p| hop_action(format!("Ask {}", p.name()), p, None))
}

fn retry() -> Action {
    Action::new("retry", "Retry")
}

fn copy_details() -> Action {
    Action::new("copy-details", "Copy details")
}

fn choose_model(kind: Kind) -> Action {
    let at = format!("models-{}", kind.as_str());
    section("choose-model", "Choose model", at)
}

const DATA_POLICY_TEXT: &str = "Your OpenRouter privacy setting blocks free models. They're \
offered on the condition that their providers may keep and train on what they receive, which \
includes your screen. Turn on \"Free model publication\" to use them, or add credit and pick a \
paid model.";

const DAILY_CAP_OPENROUTER: &str = "You've used today's free questions on OpenRouter. They \
come back tomorrow, and adding credit raises the daily limit.";

const FREE_STILL_BUSY: &str = "Free models are still busy. Try again in a minute, or add \
another provider for busy times.";

const SERVER_BACKUP: &str = "OpenRouter returned a server error, on this model and on the \
backup. Retry in a moment.";

const SERVER_ONCE: &str = "OpenRouter returned a server error on this model. Retry in a moment.";

const OFFLINE_TEXT: &str =
    "No connection. Finding words on your screen still works; answers need the internet.";

/// The sentence and the actions for an error, or `None` for Stop.
pub fn explain(err: &ApiError, a: &About<'_>) -> Option<Explained> {
    if err.is_abort() {
        return None;
    }
    let p = a.provider.name();
    let or = a.provider == Provider::OpenRouter;
    let name = if a.model_name.is_empty() {
        "This model"
    } else {
        a.model_name
    };
    let detail = err.message.trim();
    let code = code_of(a.provider, err, a.model);
    let backup = err.attempts > 1;
    let mut tone = Tone::Error;
    let mut actions: Vec<Action> = Vec::new();
    let text = match code {
        codes::DATA_POLICY => {
            let privacy = link("privacy", "Open OpenRouter privacy settings", PRIVACY_URL);
            actions.push(privacy);
            actions.push(Action {
                provider: Some(a.provider.id().to_string()),
                ..Action::new("test-again", "Test again")
            });
            DATA_POLICY_TEXT.to_string()
        }
        codes::BAD_KEY => {
            let at = format!("prov-{}", a.provider.id());
            actions.push(section("fix-key", "Fix key", at));
            actions.extend(ask_next(a));
            format!(
                "{p} rejected your key. It may have been revoked or mistyped."
            )
        }
        codes::NO_CREDIT if or => {
            actions.push(link("add-credit", "Add credit", credit_url(a.provider)));
            actions.push(Action {
                model: Some(catalogue::default_for(a.kind).to_string()),
                ..Action::new("free-model", "Use a free model")
            });
            format!(
                "{name} bills per token and this OpenRouter account has no credit. A dollar \
lasts thousands of questions at this rate."
            )
        }
        codes::NO_CREDIT => {
            actions.push(link("add-credit", "Add credit", credit_url(a.provider)));
            actions.extend(ask_next(a));
            format!(
                "The {p} account has no credit left, so {name} can't answer."
            )
        }
        codes::DAILY_CAP => {
            actions.push(link("add-credit", "Add credit", credit_url(a.provider)));
            actions.extend(ask_next(a));
            if or {
                DAILY_CAP_OPENROUTER.to_string()
            } else {
                format!(
                    "{p} says this account has used its quota for today. It resets tomorrow, \
or add credit to keep asking now."
                )
            }
        }
        codes::FREE_BUSY => {
            tone = Tone::Note;
            actions.push(retry());
            actions.extend(ask_next(a));
            let at = "providers".to_string();
            actions.push(section("add-provider", "Add a provider", at));
            FREE_STILL_BUSY.to_string()
        }
        codes::RATE_LIMITED => {
            tone = Tone::Note;
            actions.push(retry());
            actions.extend(ask_next(a));
            format!("{p} is still limiting requests. Try again in a minute.")
        }
        codes::MODEL_GONE => {
            actions.push(choose_model(a.kind));
            if or && backup {
                format!(
                    "{name} isn't available on OpenRouter right now, and neither was the backup."
                )
            } else {
                format!("{name} isn't available on {p} right now.")
            }
        }
        codes::NO_VISION => {
            actions.push(choose_model(Kind::Vision));
            format!("{name} can't read images. Choose a screen model that can.")
        }
        codes::REFUSED if err.status == Some(403) => {
            actions.push(copy_details());
            format!("{p} refused the request. It said: {detail}")
        }
        codes::REFUSED => {
            actions.push(retry());
            actions.push(choose_model(a.kind));
            format!(
                "{name} declined to answer. Try asking another way, or choose another model."
            )
        }
        codes::OFFLINE if a.online => {
            actions.push(retry());
            actions.push(copy_details());
            format!(
                "The connection to {p} dropped before the answer finished. Retry in a moment."
            )
        }
        codes::OFFLINE => OFFLINE_TEXT.to_string(),
        codes::SERVER => {
            actions.push(retry());
            actions.push(copy_details());
            if err.status == Some(408) {
                format!("{p} stopped sending the answer. Retry in a moment.")
            } else if !or {
                format!("{p} returned a server error. Retry in a moment.")
            } else if backup {
                SERVER_BACKUP.to_string()
            } else {
                SERVER_ONCE.to_string()
            }
        }
        _ => {
            actions.push(retry());
            match err.status {
                Some(204) => {
                    actions.push(choose_model(a.kind));
                    format!("{name} sent back no answer. Choose a different model.")
                }
                Some(400) if !detail.is_empty() => {
                    actions.push(copy_details());
                    format!("{p} rejected the request: {detail}")
                }
                _ => {
                    actions.push(copy_details());
                    unknown_text(a.provider)
                }
            }
        }
    };
    Some(Explained {
        code,
        text,
        tone,
        actions,
    })
}

/// C-61.
fn unknown_text(provider: Provider) -> String {
    let p = provider.name();
    format!(
        "The request failed and {p} didn't say why. Retry, or copy the details if it keeps \
happening."
    )
}

/// The countdown note while a busy model waits for its automatic retry
/// (C-71, C-72): `Try now`, and `Ask <Provider> instead` when another
/// provider is ready.
// B-120's automatic retry in host::ask shows this; tested here until then.
#[allow(dead_code)]
pub fn busy_note(code: &str, a: &About<'_>, wait_ms: u64) -> Explained {
    let secs = wait_ms.div_ceil(1000).max(1);
    let free = code == codes::FREE_BUSY;
    let text = if free {
        format!("Free models are busy right now. Trying again in {secs} s.")
    } else {
        let p = a.provider.name();
        format!(
            "{p} is limiting requests right now. Trying again in {secs} s."
        )
    };
    let mut actions = vec![Action::new("try-now", "Try now")];
    if let Some(p) = a.next {
        let label = format!("Ask {} instead", p.name());
        actions.push(hop_action(label, p, None));
    }
    let code = if free {
        codes::FREE_BUSY
    } else {
        codes::RATE_LIMITED
    };
    Explained {
        code,
        text,
        tone: Tone::Note,
        actions,
    }
}

/// C-63: free models are busy and the next ready provider bills per
/// question, so the person decides (design-ux 3.4).
// B-046's provider hop in host::ask asks this; tested here until then.
#[allow(dead_code)]
pub fn paid_hop(to: Provider, model: &str, model_name: &str) -> Explained {
    let p = to.name();
    let text = format!(
        "OpenRouter's free models are busy. Ask {p} ({model_name}) instead? {p} bills your \
account for it."
    );
    Explained {
        code: codes::PAID_HOP,
        text,
        tone: Tone::Note,
        actions: vec![
            hop_action(format!("Ask {p}"), to, Some(model)),
            Action::new("wait", "Keep waiting"),
            Action::new("never-ask", "Don't ask again"),
        ],
    }
}

/// `Copy details`: the raw facts behind the sentence, ids included.
pub fn details(err: &ApiError, a: &About<'_>) -> String {
    let mut parts = vec![a.provider.name().to_string()];
    if !a.model.is_empty() {
        parts.push(a.model.to_string());
    }
    match err.status {
        Some(s) => parts.push(format!("HTTP {s}")),
        None => parts.push(format!("{:?}", err.kind).to_lowercase()),
    }
    if err.attempts > 1 {
        parts.push(format!("{} models tried", err.attempts));
    }
    let detail = err.message.trim();
    if !detail.is_empty() {
        parts.push(detail.to_string());
    }
    parts.join(" \u{b7} ")
}

/// The sentence alone, for callers that show no actions (the agent). The
/// model's display name comes from its id.
pub fn friendly_error_for(
    provider: Provider,
    err: Option<&ApiError>,
    model: &str,
) -> Option<String> {
    let Some(err) = err else {
        return Some(unknown_text(provider));
    };
    let name = catalogue::model_name(model, None);
    let a = About::new(provider, model, &name, Kind::Vision);
    explain(err, &a).map(|e| e.text)
}

/// `Retry-After` as milliseconds: a number of seconds, or an HTTP date
/// (`Wed, 21 Oct 2015 07:28:00 GMT`) measured from `now_ms`.
pub fn parse_retry_after(value: &str, now_ms: u64) -> Option<u64> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    if let Ok(secs) = v.parse::<f64>() {
        if secs.is_finite() && secs >= 0.0 {
            return Some((secs * 1000.0).round() as u64);
        }
        return None;
    }
    http_date_ms(v).map(|at| at.saturating_sub(now_ms))
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// An IMF-fixdate as ms since the epoch.
fn http_date_ms(v: &str) -> Option<u64> {
    let parts: Vec<&str> = v.split_whitespace().collect();
    let p = match parts.first() {
        Some(day) if day.ends_with(',') => &parts[1..],
        _ => &parts[..],
    };
    if p.len() < 4 {
        return None;
    }
    let day: i64 = p[0].parse().ok()?;
    let lower = p[1].to_lowercase();
    let month = MONTHS.iter().position(|m| lower.starts_with(m))? as i64 + 1;
    let year: i64 = p[2].parse().ok()?;
    let hms: Vec<i64> = p[3].split(':').filter_map(|x| x.parse().ok()).collect();
    if hms.len() != 3 || !(1..=31).contains(&day) {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + hms[0] * 3600 + hms[1] * 60 + hms[2];
    u64::try_from(secs).ok().map(|s| s * 1000)
}

/// Days since 1970-01-01 (Howard Hinnant's `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const TABLE: &str = include_str!("../../../tools/fixtures/llm/errors.json");

    fn ids(e: &Explained) -> Vec<&str> {
        e.actions.iter().map(|a| a.id.as_str()).collect()
    }

    fn about(provider: Provider, model: &'static str, name: &'static str) -> About<'static> {
        About::new(provider, model, name, Kind::Vision)
    }

    #[test]
    fn failover_statuses_are_the_everyday_ones() {
        for s in [404, 408, 429, 500, 502, 503, 504, 529] {
            assert!(should_failover(&ApiError::http(s, "x")), "{s}");
        }
        for s in [400, 401, 402, 403, 204] {
            assert!(!should_failover(&ApiError::http(s, "x")), "{s}");
        }
        assert!(!should_failover(&ApiError::abort()));
        assert!(!should_failover(&ApiError::network("fetch failed")));
        let refused = ApiError::coded(None, "refusal").with_code(codes::REFUSED);
        assert!(!should_failover(&refused));
    }

    fn row_error(row: &Value) -> Option<ApiError> {
        let detail = row["detail"].as_str().unwrap_or_default();
        let err = match row["kind"].as_str() {
            Some("network") => ApiError::network(detail),
            Some("other") => ApiError::other(detail),
            Some("abort") => ApiError::abort(),
            Some("refusal") => ApiError::coded(None, detail).with_code(codes::REFUSED),
            Some(_) => return None,
            None => {
                let status = row["status"].as_u64().unwrap() as u16;
                ApiError::http(status, detail)
            }
        };
        let attempts = row["attempts"].as_u64().unwrap_or(1) as u32;
        Some(err.with_attempts(attempts))
    }

    #[test]
    fn error_table_is_verbatim() {
        let rows: Vec<Value> = serde_json::from_str(TABLE).unwrap();
        assert!(rows.len() >= 25);
        for row in rows {
            let s = |k: &str| row[k].as_str().unwrap_or_default().to_string();
            let provider = Provider::from_id(&s("provider")).unwrap();
            let (model, name) = (s("model"), s("modelName"));
            let Some(err) = row_error(&row) else {
                let got = friendly_error_for(provider, None, &model);
                assert_eq!(got.as_deref(), row["text"].as_str(), "{row}");
                continue;
            };
            let mut a = About::new(provider, &model, &name, Kind::Vision);
            a.online = row["online"].as_bool().unwrap_or(false);
            a.next = row["next"].as_str().and_then(Provider::from_id);
            let got = explain(&err, &a);
            let text = got.as_ref().map(|e| e.text.as_str());
            assert_eq!(text, row["text"].as_str(), "{row}");
            let Some(got) = got else {
                continue;
            };
            assert_eq!(Some(got.code), row["code"].as_str(), "{row}");
            let want: Vec<&str> = row["actions"]
                .as_array()
                .map(|l| l.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            assert_eq!(ids(&got), want, "{row}");
        }
    }

    #[test]
    fn codes_and_actions() {
        let sonnet = "Claude Sonnet 5.5";
        let mut a = about(Provider::Anthropic, "claude-sonnet-5-5", sonnet);
        a.next = Some(Provider::OpenRouter);
        let e = explain(&ApiError::http(401, "invalid x-api-key"), &a).unwrap();
        assert_eq!(e.code, codes::BAD_KEY);
        assert_eq!(e.actions[0].section.as_deref(), Some("prov-anthropic"));
        assert_eq!(e.actions[1].label, "Ask OpenRouter");
        assert_eq!(e.actions[1].provider.as_deref(), Some("openrouter"));
        let or = about(Provider::OpenRouter, "qwen/qwen3.7-flash", "Qwen3.7 Flash");
        let e = explain(&ApiError::http(402, "Insufficient credits"), &or).unwrap();
        assert_eq!(ids(&e), ["add-credit", "free-model"]);
        let url = e.actions[0].url.as_deref();
        assert_eq!(url, Some("https://openrouter.ai/credits"));
        let model = e.actions[1].model.as_deref();
        assert_eq!(model, Some(catalogue::DEFAULT_MODEL));
        let free = about(Provider::OpenRouter, "a/b:free", "B");
        let e = explain(&ApiError::http(429, "busy"), &free).unwrap();
        assert_eq!((e.code, e.tone), (codes::FREE_BUSY, Tone::Note));
        let e = explain(&ApiError::http(529, "Overloaded"), &a).unwrap();
        assert_eq!(e.code, codes::RATE_LIMITED);
        let refusal = ApiError::coded(None, "refusal").with_code(codes::REFUSED);
        let e = explain(&refusal, &a).unwrap();
        let want = "Claude Sonnet 5.5 declined to answer. Try asking another way, or choose \
another model.";
        assert_eq!(e.text, want);
        assert!(explain(&ApiError::abort(), &a).is_none());
        let note = busy_note(codes::FREE_BUSY, &a, 19_200);
        let want = "Free models are busy right now. Trying again in 20 s.";
        assert_eq!(note.text, want);
        assert_eq!(ids(&note), ["try-now", "hop"]);
        assert_eq!(note.actions[1].label, "Ask OpenRouter instead");
        let hop = paid_hop(Provider::Anthropic, "claude-sonnet-5-5", sonnet);
        let want = "OpenRouter's free models are busy. Ask Anthropic (Claude Sonnet 5.5) instead? \
Anthropic bills your account for it.";
        assert_eq!(hop.text, want);
        assert_eq!(ids(&hop), ["hop", "wait", "never-ask"]);
        assert_eq!(hop.actions[0].model.as_deref(), Some("claude-sonnet-5-5"));
        let err = ApiError::http(404, "gone").with_attempts(2);
        let want = "OpenRouter \u{b7} a/b:free \u{b7} HTTP 404 \u{b7} 2 models tried \u{b7} gone";
        assert_eq!(details(&err, &free), want);
    }

    #[test]
    fn gemini_invalid_key_is_bad_key() {
        let a = about(Provider::Gemini, "gemini-3.8-flash", "Gemini 3.8 Flash");
        let err = ApiError::http(400, "API key not valid. Please pass a valid API key.");
        let e = explain(&err, &a).unwrap();
        assert_eq!(e.code, codes::BAD_KEY);
        let want = "Google Gemini rejected your key. It may have been revoked or mistyped.";
        assert_eq!(e.text, want);
    }

    #[test]
    fn single_attempt_copy_claims_no_stand_in() {
        let a = about(Provider::OpenRouter, "qwen/qwen3.7-flash", "Qwen3.7 Flash");
        let gone = ApiError::http(404, "No endpoints found");
        let once = explain(&gone, &a).unwrap().text;
        let want = "Qwen3.7 Flash isn't available on OpenRouter right now.";
        assert_eq!(once, want);
        let twice = explain(&gone.clone().with_attempts(2), &a).unwrap().text;
        assert!(twice.ends_with("and neither was the backup."));
        let down = ApiError::http(502, "bad gateway");
        let once = explain(&down, &a).unwrap().text;
        assert_eq!(once, SERVER_ONCE);
        let twice = explain(&down.with_attempts(3), &a).unwrap().text;
        assert!(twice.contains("on this model and on the backup"));
    }

    #[test]
    fn daily_cap_is_not_transient() {
        let cap = ApiError::http(429, "Rate limit exceeded: free-models-per-day");
        assert!(!should_failover(&cap));
        let a = about(Provider::OpenRouter, "a/b:free", "B");
        let code = code_of(Provider::OpenRouter, &cap, "a/b:free");
        assert_eq!(code, codes::DAILY_CAP);
        let e = explain(&cap, &a).unwrap();
        assert_eq!(ids(&e), ["add-credit"]);
        let quota = ApiError::http(429, "You exceeded your current quota");
        assert!(!should_failover(&quota));
        let code = code_of(Provider::OpenAi, &quota, "gpt-6.1-terra");
        assert_eq!(code, codes::NO_CREDIT);
        let gemini = ApiError::http(429, "RESOURCE_EXHAUSTED");
        assert_eq!(code_of(Provider::Gemini, &gemini, "g"), codes::DAILY_CAP);
        // The per-minute free limit is ordinary busy, and moves on.
        let minute = ApiError::http(429, "Rate limit exceeded: free-models-per-min");
        assert!(should_failover(&minute));
    }

    #[test]
    fn keys_never_reach_the_message() {
        let quoted = "Incorrect API key provided: sk-proj-abc123****wxyz. See the docs.";
        let e = ApiError::http(401, quoted);
        assert!(!e.message.contains("abc123"), "{}", e.message);
        let want = "Incorrect API key provided: <key hidden>.";
        assert!(e.message.starts_with(want));
        let e = ApiError::network("bad key AIzaSyA1234567890abcdefghij sent");
        assert_eq!(e.message, "bad key <key hidden> sent");
        let task = ApiError::other("task-runner failed on desk-01");
        assert_eq!(task.message, "task-runner failed on desk-01");
    }

    #[test]
    fn retry_after_parsed() {
        assert_eq!(parse_retry_after("7", 0), Some(7_000));
        assert_eq!(parse_retry_after(" 1.5 ", 0), Some(1_500));
        assert_eq!(parse_retry_after("", 0), None);
        assert_eq!(parse_retry_after("-3", 0), None);
        assert_eq!(parse_retry_after("soon", 0), None);
        let at = 1_445_412_480_000;
        let date = "Wed, 21 Oct 2015 07:28:00 GMT";
        assert_eq!(parse_retry_after(date, at - 30_000), Some(30_000));
        assert_eq!(parse_retry_after(date, at + 5_000), Some(0));
    }
}
