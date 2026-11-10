//! The Ask flow (PARITY ASK-*): main.js handleQuerySubmit, abortCurrentQuery,
//! prefetchBarContext, freshBarText, readExactText, showModelPoints,
//! resolveUiaPoint, and the models:list handler, on the
//! floating output windows: a question goes to the output `outputs::target`
//! gives, and every event of that turn goes to that window only.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::ask::{
    is_visual, point_fraction, AUTO_SKILL, CHAT_HISTORY_LIMIT, DEFAULT_PROMPT, TEXT_ONLY_CHARS,
};
use crate::capture::{self, Shot};
use crate::llm::catalogue::{self, Kind};
use crate::llm::client::{ChatTurn, QueryOpts, ReaderOpts};
use crate::llm::elements::{match_element, parse_elements, View};
use crate::llm::errors::{busy_note, code_of, codes, details, explain, paid_hop, About, ApiError};
use crate::llm::failover::{short_model, with_failover};
use crate::llm::payloads::{FailoverPrefs, QueryError, Usage};
use crate::llm::points::{Point, PointExtractor};
use crate::llm::prompt::{extract_task, ExactText, DOC_TEXT_LIMIT};
use crate::llm::provider::Provider;
use crate::llm::settings::{
    self as llm_settings, active, candidates_for, model_for, Active, LlmSettings,
};
use crate::llm::sse::Meter;
use crate::state::{shared, Mode};
use crate::windows::{self, outputs};
use crate::{buddy, store};

use super::{services, BarContext};

/// `query:submit` options.
#[derive(Clone, Debug, Default)]
pub struct SubmitOpts {
    /// Re-run the turn already at the end of the transcript.
    pub retry: bool,
    /// The bar guessed Ask; the answer offers Find instead.
    pub inferred: bool,
    /// Always send the picture (Find's "Ask about this").
    pub with_image: bool,
    /// The bar's `@` pick for this one question: that provider and model
    /// answer, with no reading step and no failover (B-122).
    pub pin: Option<(Provider, String)>,
}

/// `{provider, model}` in a `query:submit` or `query:retry` payload, when
/// that provider has a key to send it with.
fn pin_of(p: &Value) -> Option<(Provider, String)> {
    let model = p.get("model").and_then(Value::as_str).map(str::trim)?;
    let provider = p
        .get("provider")
        .and_then(Value::as_str)
        .and_then(Provider::from_id)?;
    if model.is_empty() {
        return None;
    }
    llm_settings::usable_key(&*services().settings, provider)?;
    Some((provider, model.to_string()))
}

struct Current {
    gen: u64,
    cancel: CancellationToken,
}

/// The Ask running in each output window, by label (B-040).
fn current() -> &'static Mutex<HashMap<String, Current>> {
    static C: OnceLock<Mutex<HashMap<String, Current>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Puts `label`'s new Ask in place, cancelling only the one it replaces in
/// that window, in one step so two quick questions cannot both run there.
fn install(map: &Mutex<HashMap<String, Current>>, label: &str, gen: u64) -> CancellationToken {
    let cancel = CancellationToken::new();
    let new = Current {
        gen,
        cancel: cancel.clone(),
    };
    if let Some(old) = map.lock().insert(label.to_string(), new) {
        old.cancel.cancel();
    }
    cancel
}

/// Removes `label`'s entry when it is still the Ask `gen`.
fn uninstall(map: &Mutex<HashMap<String, Current>>, label: &str, gen: u64) {
    let mut m = map.lock();
    if m.get(label).is_some_and(|c| c.gen == gen) {
        m.remove(label);
    }
}

/// How many answers may stream at once (B-040, D-15). A third question
/// waits for one of them to finish.
const MAX_ANSWERS: usize = 2;

/// C-24: what a waiting question's window says.
const WAITING_FOR_SLOT: &str = "Waiting for your other answer to finish";

fn answer_slots() -> &'static Arc<Semaphore> {
    static S: OnceLock<Arc<Semaphore>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Semaphore::new(MAX_ANSWERS)))
}

/// A place among the answers streaming now. When none is free, `waiting`
/// runs once and this waits for one; `None` when the question was stopped
/// while it waited.
async fn wait_for_slot(
    slots: &Arc<Semaphore>,
    cancel: &CancellationToken,
    waiting: impl FnOnce(),
) -> Option<OwnedSemaphorePermit> {
    if let Ok(p) = slots.clone().try_acquire_owned() {
        return Some(p);
    }
    waiting();
    tokio::select! {
        p = slots.clone().acquire_owned() => p.ok(),
        _ = cancel.cancelled() => None,
    }
}

/// A busy model is asked again by itself this many times (B-120).
const MAX_BUSY_RETRIES: u32 = 2;

/// The waits before those retries when the provider names none.
const BUSY_WAITS_MS: [u64; 2] = [20_000, 40_000];

/// A provider that asks for a longer wait than this is not waited for.
const LONGEST_BUSY_WAIT_MS: u64 = 120_000;

/// How long to wait before busy retry number `waits` (0 or 1): the
/// provider's Retry-After when it sent one, else 20 s then 40 s.
fn busy_wait(waits: u32, retry_after_ms: Option<u64>) -> Option<u64> {
    let default = *BUSY_WAITS_MS.get(waits as usize)?;
    match retry_after_ms {
        Some(ms) if ms > LONGEST_BUSY_WAIT_MS => None,
        Some(ms) => Some(ms.max(1_000)),
        None => Some(default),
    }
}

/// What a failed attempt knows when it decides what happens next.
#[derive(Clone, Copy, Debug)]
struct Failed<'a> {
    /// One of `llm::errors::codes`.
    code: &'a str,
    /// The OS reports a connection.
    online: bool,
    /// Some of the answer has been shown already.
    shown: bool,
    /// The person picked this model for this question (B-122).
    pinned: bool,
    /// The failed attempt was on one of OpenRouter's free models.
    from_free: bool,
    /// Another provider is ready; true when it bills per question.
    next_bills: Option<bool>,
    prefs: FailoverPrefs,
    /// Busy retries made so far.
    waits: u32,
    retry_after_ms: Option<u64>,
}

/// What happens after a failed attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Next {
    /// The error is shown.
    Stop,
    /// The next provider answers.
    Hop,
    /// The same provider is asked again after this many ms.
    Wait(u64),
    /// As `Wait`, and the note offers the next provider, which bills.
    AskPaid(u64),
}

/// B-046 and B-120. A question moves to the next provider only before any
/// of the answer was shown, only for a failure another provider could get
/// past (busy, a server error, a missing model, a rejected key, no credit, a
/// dropped connection while online), and never with a pinned model. From a
/// free model to a provider that bills it asks first, unless the person
/// turned that off. A busy provider with nowhere to go is asked again by
/// itself, twice at most.
fn after_failure(f: &Failed<'_>) -> Next {
    if f.shown || f.pinned {
        return Next::Stop;
    }
    let busy = matches!(f.code, codes::FREE_BUSY | codes::RATE_LIMITED);
    let elsewhere = matches!(
        f.code,
        codes::SERVER | codes::MODEL_GONE | codes::BAD_KEY | codes::NO_CREDIT | codes::DAILY_CAP
    );
    let dropped = f.code == codes::OFFLINE && f.online;
    let mut ask = false;
    if (busy || elsewhere || dropped) && f.prefs.across_providers {
        if let Some(bills) = f.next_bills {
            if !(f.from_free && bills && f.prefs.ask_before_paid) {
                return Next::Hop;
            }
            ask = true;
        }
    }
    if !busy || f.waits >= MAX_BUSY_RETRIES {
        return Next::Stop;
    }
    match busy_wait(f.waits, f.retry_after_ms) {
        Some(ms) if ask => Next::AskPaid(ms),
        Some(ms) => Next::Wait(ms),
        None => Next::Stop,
    }
}

/// The stage shown while a question moves to another provider (C-25).
fn hop_stage(code: &str, from: Provider, to: Provider) -> String {
    let (from, to) = (from.name(), to.name());
    if matches!(code, codes::FREE_BUSY | codes::RATE_LIMITED) {
        format!("{from} is busy. Asking {to}\u{2026}")
    } else {
        format!("{from} didn't answer. Asking {to}\u{2026}")
    }
}

/// `usage.hopReason`: why the answer came from another provider.
fn hop_reason_text(code: &str, from: Provider) -> String {
    let from = from.name();
    match code {
        codes::FREE_BUSY | codes::RATE_LIMITED => format!("{from} was busy"),
        codes::BAD_KEY => format!("{from} rejected its key"),
        codes::NO_CREDIT => format!("{from} had no credit left"),
        codes::DAILY_CAP => format!("{from} reached its daily limit"),
        _ => format!("{from} didn't answer"),
    }
}

/// The screen model and the brain a question uses on `provider`.
async fn models_on(provider: Provider) -> (String, String) {
    if !provider.is_native() {
        return super::live_models(true).await;
    }
    let sv = services();
    let settings = &*sv.settings;
    let pick = |kind| {
        sv.llm
            .known_native(provider, kind, &model_for(settings, provider, kind))
    };
    (pick(Kind::Vision), pick(Kind::Text))
}

/// How many sent pictures each window keeps for "What was sent" (D-38).
const SENT_KEPT: usize = 3;

/// The JPEGs (base64) the last answers in each window sent, by `msgId`,
/// newest last. Memory only, never on disk (B-121).
type SentImages = HashMap<String, VecDeque<(u64, String)>>;

fn sent_images() -> &'static Mutex<SentImages> {
    static S: OnceLock<Mutex<SentImages>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Keeps `label`'s picture for answer `msg_id`, dropping the oldest past
/// [`SENT_KEPT`] and every window `alive` no longer knows.
fn keep_sent(
    map: &Mutex<SentImages>,
    label: &str,
    msg_id: u64,
    base64: String,
    alive: impl Fn(&str) -> bool,
) {
    let mut m = map.lock();
    m.retain(|l, _| l == label || alive(l));
    let q = m.entry(label.to_string()).or_default();
    q.push_back((msg_id, base64));
    while q.len() > SENT_KEPT {
        q.pop_front();
    }
}

/// `output:sent-image`: the picture answer `msg_id` sent, as a data URL,
/// or None once it has been dropped.
fn sent_image(map: &Mutex<SentImages>, label: &str, msg_id: u64) -> Option<String> {
    let m = map.lock();
    let (_, b64) = m.get(label)?.iter().find(|(id, _)| *id == msg_id)?;
    Some(format!("data:image/jpeg;base64,{b64}"))
}

/// The shot taken when an area was chosen, and when, for the next question.
fn pending_shot() -> &'static Mutex<Option<(Shot, Instant)>> {
    static P: OnceLock<Mutex<Option<(Shot, Instant)>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(None))
}

pub fn set_pending_shot(shot: Option<Shot>) {
    *pending_shot().lock() = shot.map(|s| (s, Instant::now()));
}

/// A pick-time shot stands in for a capture only this soon after the pick;
/// later, the screen under the area may have changed (B-124, Decision D-08).
const PICK_SHOT_FRESH: Duration = Duration::from_secs(1);

/// What `slot` holds if it was stored less than [`PICK_SHOT_FRESH`] before
/// `now`. The slot is emptied either way.
fn take_fresh<T>(slot: &Mutex<Option<(T, Instant)>>, now: Instant) -> Option<T> {
    let (value, at) = slot.lock().take()?;
    (now.saturating_duration_since(at) < PICK_SHOT_FRESH).then_some(value)
}

/// The name people read for a model id: the catalogue's display name when
/// the catalogue is cached, else one made from the id (B-119).
fn display_name(id: &str) -> String {
    let all = services().llm.known_catalogue();
    catalogue::model_name(id, all.as_deref().map(Vec::as_slice))
}

/// Stage labels C-21..C-23: a busy model handed the question on.
fn busy_stage(doing: &str, model: &str) -> Value {
    let name = display_name(model);
    let label = format!("That model is busy. {doing} {name}\u{2026}");
    json!({ "label": label })
}

/// The `query:error` row for a failure (contract K-19), or `None` for
/// Stop.
fn query_error(err: &ApiError, a: &About<'_>, question: &str) -> Option<QueryError> {
    let e = explain(err, a)?;
    Some(QueryError {
        message: e.text,
        question: question.to_string(),
        code: e.code.to_string(),
        provider: a.provider.id().to_string(),
        model: a.model.to_string(),
        model_name: a.model_name.to_string(),
        retry_in_ms: None,
        tone: e.tone,
        actions: e.actions,
        detail: details(err, a),
    })
}

/// [`query_error`] with what the app knows now: the model's display name,
/// the next ready provider for `Ask <Provider>`, and whether the OS has a
/// connection.
fn error_row(
    provider: Provider,
    err: &ApiError,
    model: &str,
    kind: Kind,
    question: &str,
) -> Option<QueryError> {
    let name = display_name(model);
    let mut a = About::new(provider, model, &name, kind);
    let ready = candidates_for(&*services().settings, kind);
    a.next = ready.iter().map(|c| c.provider).find(|p| *p != provider);
    a.online = super::online();
    query_error(err, &a, question)
}

/// The window an Ask read its exact text from, for pointing by name.
fn last_exact() -> &'static Mutex<Option<ExactText>> {
    static L: OnceLock<Mutex<Option<ExactText>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(None))
}

/// main.js abortCurrentQuery: every window's Ask stops (Stop on the bar,
/// New chat, hiding the overlay).
pub fn abort(_app: &AppHandle) {
    for (_, c) in current().lock().drain() {
        c.cancel.cancel();
    }
}

/// Stops the Ask in `label` and no other (contract K-03).
pub fn abort_for(_app: &AppHandle, label: &str) {
    if let Some(c) = current().lock().remove(label) {
        c.cancel.cancel();
    }
}

fn no_uia() -> bool {
    std::env::var_os("AEVAL_NO_UIA").is_some()
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// How much of a document the helper is asked for. It is more than is sent,
/// so that the part in view can be found in a document longer than the
/// limit.
const DOC_READ_CHARS: u64 = 240_000;

/// The document is not asked for with less time than this left.
const DOC_MIN_MS: u64 = 200;

/// A line shorter than this is not used to find the part in view: a short
/// line such as a menu name can occur anywhere in a document.
const ANCHOR_MIN_CHARS: usize = 12;

fn unix_lines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// The longest document in a `docText` reply: its text, the start of what
/// the application reports as in view, and whether the helper stopped
/// before the document's end. A window's other editors (an address bar, a
/// search field) are short and already in the visible text.
fn longest_doc(r: &Value) -> Option<(String, String, bool)> {
    let docs = r["docs"].as_array()?;
    let best = docs
        .iter()
        .max_by_key(|d| d["text"].as_str().map(str::len).unwrap_or(0))?;
    let text = unix_lines(best["text"].as_str()?);
    if text.trim().is_empty() {
        return None;
    }
    let seen = unix_lines(best["seen"].as_str().unwrap_or(""));
    let more = best["truncated"].as_bool().unwrap_or(false);
    Some((text, seen, more))
}

/// Cuts a document to `limit` characters and says whether it was cut. A
/// document that fits is returned whole. A longer one keeps the part
/// around what is in view, found by the first long enough line of `seen`
/// (what the application reports as visible) or of `visible` (the window's
/// visible text) that occurs in it; when none does, it keeps the start.
/// Each end that was cut gets a `[cut: ...]` line, so the model knows the
/// text goes on. `more` means the helper itself stopped before the end.
fn fit_document(doc: &str, seen: &str, visible: &str, more: bool, limit: usize) -> (String, bool) {
    const GOES_ON: &str = "[cut: the document goes on after this, and the rest is not included]";
    let chars: Vec<char> = doc.chars().collect();
    let n = chars.len();
    if n <= limit {
        if more {
            return (format!("{doc}\n{GOES_ON}"), true);
        }
        return (doc.to_string(), false);
    }
    let at = seen
        .lines()
        .chain(visible.lines())
        .map(str::trim)
        .filter(|l| l.chars().count() >= ANCHOR_MIN_CHARS)
        .find_map(|l| doc.find(l))
        .map(|bytes| doc[..bytes].chars().count())
        .unwrap_or(0);
    // A quarter of the room goes to what is above the part in view.
    let mut start = at.saturating_sub(limit / 4);
    if start + limit > n {
        start = n - limit;
    }
    let mut end = start + limit;
    // Begin and end on a line break when one is near.
    let near = (limit / 50).max(1);
    if start > 0 {
        let to = (start + near).min(end);
        if let Some(k) = chars[start..to].iter().position(|c| *c == '\n') {
            start += k + 1;
        }
    }
    if end < n {
        let from = end.saturating_sub(near).max(start);
        if let Some(k) = chars[from..end].iter().rposition(|c| *c == '\n') {
            end = from + k;
        }
    }
    let mut parts: Vec<String> = Vec::new();
    if start > 0 {
        parts.push(format!(
            "[cut: {start} characters before this are not included]"
        ));
    }
    parts.push(chars[start..end].iter().collect());
    let after = n - end;
    if after > 0 {
        let least = if more { "at least " } else { "" };
        parts.push(format!(
            "[cut: {least}{after} characters after this are not included]"
        ));
    } else if more {
        parts.push(GOES_ON.to_string());
    }
    (parts.join("\n"), true)
}

/// The document to send with `visible`, from a `docText` reply: the text
/// and whether it was cut. None when the window has no document, or when
/// the document holds no more than what is already in view.
fn document_for(r: &Value, visible: &str, limit: usize) -> Option<(String, bool)> {
    let (doc, seen, more) = longest_doc(r)?;
    if doc.chars().count() <= visible.chars().count() {
        return None;
    }
    Some(fit_document(&doc, &seen, visible, more, limit))
}

/// `query:done.sent.windowText`: how much of the window's text went with
/// the question. `document` is "whole" or "part" when the document's text
/// went too, and absent when only the visible text did.
fn sent_window_text(e: &ExactText) -> Option<Value> {
    let chars = e.text.chars().count() + e.document.chars().count();
    if chars == 0 {
        return None;
    }
    let app: String = e.title.chars().take(60).collect();
    let mut v = json!({ "app": app, "chars": chars });
    if !e.document.is_empty() {
        v["document"] = json!(if e.document_cut { "part" } else { "whole" });
    }
    Some(v)
}

/// The window in front's own text, through UI Automation. Never waits
/// long: the first use starts the desktop helper, and an answer is worth more
/// than a perfect context.
///
/// Without a picked area it then asks for the whole text of the window's
/// document, in whatever is left of the same time limit. When that read
/// fails, runs out of time or finds nothing, the visible text goes alone.
/// With a picked area only that area's text is read: the person chose it.
pub async fn read_exact_text(region: Option<crate::state::Rect>) -> Option<ExactText> {
    if no_uia() {
        return None;
    }
    let d = services().desktop.inner().clone();
    let limit: u64 = if d.is_ready() { 1500 } else { 4000 };
    let began = Instant::now();
    let (mut exact, handle) = read_visible_text(d.clone(), region, limit).await?;
    if region.is_some() {
        return Some(exact);
    }
    let left = limit.saturating_sub(began.elapsed().as_millis() as u64);
    if left < DOC_MIN_MS {
        return Some(exact);
    }
    // The helper stops a little early so its reply arrives inside the limit.
    let read = d.doc_text(Some(&handle), DOC_READ_CHARS, left - 100);
    if let Ok(Ok(r)) = tokio::time::timeout(Duration::from_millis(left), read).await {
        if let Some((document, cut)) = document_for(&r, &exact.text, DOC_TEXT_LIMIT) {
            exact.document = document;
            exact.document_cut = cut;
        }
    }
    Some(exact)
}

/// What is in view in the window in front (or in `region` of it), and the
/// window's handle as the helper gave it.
async fn read_visible_text(
    d: Arc<crate::desktop::client::Desktop>,
    region: Option<crate::state::Rect>,
    limit: u64,
) -> Option<(ExactText, Value)> {
    let work = async move {
        let fg = d.foreground_window().await.ok()?;
        if !fg.is_object() {
            return None;
        }
        let rect = region
            .map(|r| {
                let p = windows::screen::dip_to_px(r);
                json!({ "x": p.x.round(), "y": p.y.round(), "width": p.width.round(), "height": p.height.round() })
            })
            .unwrap_or(Value::Null);
        let handle = fg["handle"].clone();
        let title = text_of(&fg["title"]);
        let g = d.get_text(Some(&handle), &rect, 16000, false).await.ok()?;
        let text = text_of(&g["text"]);
        let text = if text.trim().is_empty() {
            String::new()
        } else {
            text
        };
        let exact = ExactText {
            handle: text_of(&handle),
            title,
            text,
            ..ExactText::default()
        };
        Some((exact, handle))
    };
    tokio::time::timeout(Duration::from_millis(limit), work)
        .await
        .ok()
        .flatten()
}

/// main.js prefetchBarContext: starts the desktop engine and the catalogue,
/// reads the foreground window's text and sends `bar:context`.
pub fn prefetch_bar_context(app: &AppHandle) {
    if no_uia() {
        return;
    }
    let sv = services();
    let d = sv.desktop.inner().clone();
    let llm = sv.llm.clone();
    let ctx = BarContext {
        at: Instant::now(),
        region: super::active_region(),
        read: Arc::new(tokio::sync::OnceCell::new()),
    };
    *super::bar_context().lock() = Some(ctx.clone());
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let d2 = d.clone();
        tauri::async_runtime::spawn(async move {
            let _ = d2.start().await;
        });
        if store::get().has_api_key() {
            tauri::async_runtime::spawn(async move {
                llm.fetch_catalogue().await;
            });
        }
        let fg = d.foreground_window().await.ok();
        let region = ctx.region;
        let exact = ctx
            .read
            .get_or_init(|| read_exact_text(region))
            .await
            .clone();
        let still = super::bar_context()
            .lock()
            .as_ref()
            .is_some_and(|c| Arc::ptr_eq(&c.read, &ctx.read));
        if !still {
            return;
        }
        let process = fg
            .as_ref()
            .map(|f| text_of(&f["process"]))
            .unwrap_or_default();
        let title = exact
            .as_ref()
            .map(|e| e.title.clone())
            .filter(|t| !t.is_empty())
            .or_else(|| fg.as_ref().map(|f| text_of(&f["title"])))
            .unwrap_or_default();
        let text = exact.as_ref().map(|e| e.text.clone()).unwrap_or_default();
        let screen: String = text.chars().take(16000).collect();
        windows::emit(
            &app,
            windows::OVERLAY,
            "bar:context",
            json!({ "screen": screen, "title": title, "process": process, "chars": text.encode_utf16().count() }),
        );
    });
}

/// The text read when the bar opened, while it is recent (8 s) and for the
/// same area.
async fn fresh_bar_text(region: Option<crate::state::Rect>) -> Option<Option<ExactText>> {
    let ctx = super::bar_context().lock().clone()?;
    if ctx.at.elapsed() > Duration::from_secs(8) || ctx.region != region {
        return None;
    }
    Some(
        ctx.read
            .get_or_init(|| read_exact_text(region))
            .await
            .clone(),
    )
}

fn norm_name(s: &str) -> String {
    let mapped: String = s
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' {
                c
            } else {
                ' '
            }
        })
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_role_words(label: &str) -> String {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"(?i)\b(button|link|field|tab|menu|checkbox)\b").expect("role words")
    });
    re.replace_all(label, "").into_owned()
}

/// `[[point Save]]` against the real control: only a clear winner counts.
/// Returns a DIP screen point and the control's name.
async fn resolve_uia_point(label: &str, handle: &str) -> Option<(f64, f64, String)> {
    if label.is_empty() || handle.is_empty() {
        return None;
    }
    let d = services().desktop.inner().clone();
    let h = json!(handle);
    let f = d.find(label, Some(&h), 4, 55).await.ok()?;
    let nodes = f["nodes"].as_array()?;
    let a = nodes.first()?;
    let b = nodes.get(1);
    let score = |n: &Value| n["score"].as_f64().unwrap_or(0.0);
    let rect = &a["rect"];
    if !rect.is_object() || score(a) < 70.0 {
        return None;
    }
    let name_a = norm_name(&text_of(&a["name"]));
    let exact = name_a == norm_name(&strip_role_words(label));
    let tie = b.is_some_and(|b| norm_name(&text_of(&b["name"])) == name_a);
    if !(exact && !tie) {
        if let Some(b) = b {
            if score(a) - score(b) < 8.0 {
                return None;
            }
        }
    }
    let n = |k: &str| rect[k].as_f64().unwrap_or(0.0);
    let (x, y) =
        windows::screen::px_to_dip_point(n("x") + n("width") / 2.0, n("y") + n("height") / 2.0);
    Some((x.round(), y.round(), text_of(&a["name"])))
}

/// main.js showModelPoints: directives to screen points, on the display the
/// shot came from.
async fn show_model_points(
    app: &AppHandle,
    points: &[Point],
    shot: Option<&Shot>,
    report: Option<&str>,
) {
    let Some(shot) = shot.filter(|s| s.width > 0 && s.height > 0) else {
        return;
    };
    if points.is_empty() || shared(app).lock().mode == Mode::Hud {
        return;
    }
    let b = shot.region.unwrap_or(shot.display);
    let view = View {
        width: shot.width as f64,
        height: shot.height as f64,
    };
    let elements = parse_elements(report, Some(view));
    let to_screen = |fx: f64, fy: f64, label: &str| buddy::Point {
        x: (b.x + fx * b.width).round(),
        y: (b.y + fy * b.height).round(),
        label: if label.is_empty() {
            "Here".into()
        } else {
            label.to_string()
        },
    };
    let exact_handle = last_exact().lock().as_ref().map(|e| e.handle.clone());
    let mut mapped = Vec::new();
    for p in points {
        match (p.x, p.y) {
            (Some(x), Some(y)) => {
                if let Some((fx, fy)) = point_fraction(x, y, view.width, view.height) {
                    mapped.push(to_screen(fx, fy, &p.label));
                }
            }
            _ => {
                if let Some(h) = &exact_handle {
                    if let Some((x, y, name)) = resolve_uia_point(&p.label, h).await {
                        let label = if p.label.is_empty() {
                            name
                        } else {
                            p.label.clone()
                        };
                        mapped.push(buddy::Point { x, y, label });
                        continue;
                    }
                }
                if let Some(el) = match_element(&p.label, &elements) {
                    let label = if p.label.is_empty() {
                        el.label.clone()
                    } else {
                        p.label.clone()
                    };
                    mapped.push(to_screen(el.x / 1000.0, el.y / 1000.0, &label));
                }
            }
        }
    }
    if mapped.is_empty() {
        return;
    }
    mapped.truncate(3);
    let display = capture::screen::matching(&capture::screen::all_displays(), &b);
    buddy::show_points(mapped, Some(&display));
    super::apply_registrations(app);
}

/// The provider, the vision model and the brain for this question.
async fn models_now() -> (Option<Active>, String, String) {
    let settings = &*services().settings;
    let act = active(settings);
    match act.as_ref().map(|a| a.provider) {
        Some(p) if p.is_native() => {
            let llm = &services().llm;
            // The provider's list loads in the background; until it is
            // cached the stored pick goes out as it is.
            if let Some(a) = act.clone() {
                let l = llm.clone();
                tauri::async_runtime::spawn(async move {
                    l.native_catalogue(p, &a.key).await;
                });
            }
            let pick = |kind| llm.known_native(p, kind, &model_for(settings, p, kind));
            (act.clone(), pick(Kind::Vision), pick(Kind::Text))
        }
        _ => {
            let (m, b) = super::live_models(true).await;
            (act, m, b)
        }
    }
}

/// One conversation turn (main.js handleQuerySubmit). `label` is the output
/// the question came from (a retry), or None for the bar's target.
pub async fn submit_to(
    app: &AppHandle,
    label: Option<String>,
    question: &str,
    o: SubmitOpts,
) -> Value {
    let q = question.trim().to_string();
    let help = {
        static RE: OnceLock<regex::Regex> = OnceLock::new();
        RE.get_or_init(|| regex::Regex::new(r"(?i)^help me[.!]?$").expect("help"))
    };
    if !o.retry && help.is_match(&q) {
        super::start_walkthrough(app);
        return json!({ "status": "walkthrough" });
    }
    if !store::get().has_api_key() {
        super::send_overlay_state(app);
        return json!({ "status": "needs-key" });
    }
    let gen = {
        let st = shared(app);
        let mut s = st.lock();
        s.query_gen += 1;
        s.query_gen
    };
    let label = match label.filter(|l| outputs::with(app, l, |_| ()).is_some()) {
        Some(l) => l,
        None => match outputs::target(app) {
            Some(l) => l,
            None => return json!({ "status": "error" }),
        },
    };
    shared(app).lock().focused_output = Some(label.clone());
    let cancel = install(current(), &label, gen);
    let text = if q.is_empty() {
        DEFAULT_PROMPT.to_string()
    } else {
        q
    };
    let region = super::active_region();
    let (resume, prior) = outputs::with(app, &label, |out| {
        out.chat.last_question = text.clone();
        out.chat.region = region;
        let last = out.chat.messages.last();
        let resume = o.retry
            && last
                .is_some_and(|m| m["role"] == "user" && m["text"].as_str() == Some(text.as_str()));
        let upto = if resume {
            out.chat.messages.len() - 1
        } else {
            out.chat.messages.len()
        };
        let from = upto.saturating_sub(CHAT_HISTORY_LIMIT);
        let prior: Vec<ChatTurn> = out.chat.messages[from..upto]
            .iter()
            .map(|m| ChatTurn {
                role: text_of(&m["role"]),
                text: text_of(&m["text"]),
                kind: m.get("kind").and_then(Value::as_str).map(str::to_string),
            })
            .collect();
        if !resume {
            out.chat
                .messages
                .push(json!({ "role": "user", "text": text }));
        }
        (resume, prior)
    })
    .unwrap_or((false, Vec::new()));
    // A pinned model answers by itself: no reading step by another model.
    let use_brain = store::get().get_brain_enabled() && o.pin.is_none();
    let send = {
        let app = app.clone();
        let label = label.clone();
        move |channel: &str, payload: Value| {
            let mut p = payload;
            if let Value::Object(m) = &mut p {
                m.insert("gen".into(), json!(gen));
            }
            windows::emit(&app, &label, channel, p);
        }
    };
    if !resume {
        send("chat:user", json!({ "text": text }));
    }
    send("query:start", json!({}));
    shared(app).lock().query_started(&label, gen);
    super::send_overlay_state(app);
    outputs::show(app, &label);

    let early = fresh_bar_text(region);
    let exact_f = async {
        match early.await {
            Some(Some(x)) => Some(x),
            _ => read_exact_text(region).await,
        }
    };
    let cap_region = region.map(|r| capture::Rect::new(r.x, r.y, r.width, r.height));
    let pending = if region.is_some() {
        take_fresh(pending_shot(), Instant::now())
    } else {
        None
    };
    let shot_f = async move {
        match pending {
            Some(s) => Some(s),
            None => capture::capture_screen_async(cap_region).await,
        }
    };
    let (shot_taken, (act, model, brain), exact) = tokio::join!(shot_f, models_now(), exact_f);
    let pinned = o.pin.is_some();
    let (act, model) = match &o.pin {
        Some((provider, m)) => {
            let key = llm_settings::usable_key(&*services().settings, *provider);
            let act = key.map(|key| Active {
                provider: *provider,
                key,
            });
            (act, m.clone())
        }
        None => (act, model),
    };
    *last_exact().lock() = exact.clone();
    shared(app).lock().capture_ok = shot_taken.is_some();
    super::send_overlay_state(app);

    let exact_len = exact
        .as_ref()
        .map(|e| e.text.encode_utf16().count())
        .unwrap_or(0);
    let text_only = shot_taken.is_some()
        && !use_brain
        && !o.with_image
        && exact_len >= TEXT_ONLY_CHARS
        && !is_visual(&text);
    let shot = if text_only { None } else { shot_taken };
    let via_region = shot.as_ref().is_some_and(|s| s.region.is_some());
    let mut provider = act
        .as_ref()
        .map(|a| a.provider)
        .unwrap_or(Provider::OpenRouter);
    let mut act = act.unwrap_or(Active {
        provider: Provider::OpenRouter,
        key: String::new(),
    });
    // The picks for the provider being asked; a hop replaces them.
    let mut cur_model = model;
    let mut cur_brain = brain;

    let mut ex = Arc::new(Mutex::new(PointExtractor::new()));
    let meter = Arc::new(Mutex::new(Meter::default()));
    let t0 = meter.lock().t0;
    let llm = services().llm.clone();
    let prefs = services().settings.failover();
    // A pinned model never hands the question on (B-122).
    let next_free = |kind: Kind, tried: &[String]| {
        if pinned {
            None
        } else {
            llm.next_free_model(kind, tried)
        }
    };
    let mut answering = cur_model.clone();
    let mut answer_kind = Kind::Vision;
    // The screen model whose reading the answer used (B-121).
    let mut reader_used: Option<String> = None;
    // The screen model the provider was asked with, for a reader's error.
    let mut asked = cur_model.clone();
    // The providers that failed this question, and why it last moved on.
    let mut failed: Vec<Provider> = Vec::new();
    let mut hop_reason: Option<String> = None;
    let mut waits = 0u32;

    // At most two answers stream at once; a third waits its turn (B-040).
    let slot = wait_for_slot(answer_slots(), &cancel, || {
        send("query:stage", json!({ "label": WAITING_FOR_SLOT }))
    })
    .await;

    let result: Result<Option<String>, ApiError> = loop {
        if slot.is_none() {
            break Err(ApiError::abort());
        }
        // A free model that answered 404 earlier this session is not asked
        // again (B-042). A pinned one is asked as picked.
        let model = if pinned {
            cur_model.clone()
        } else {
            llm.first_model(Kind::Vision, &cur_model)
        };
        let brain = llm.first_model(Kind::Text, &cur_brain);
        asked = model.clone();
        answering = model.clone();
        answer_kind = Kind::Vision;
        reader_used = None;

        let attempted: Result<Option<String>, ApiError> = async {
            let mut report: Option<String> = None;
            // The screen model that last answered, so "Answering directly"
            // reuses a stand-in that worked rather than the busy first pick.
            let mut seeing = model.clone();
            if use_brain {
                if let Some(s) = &shot {
                    send("query:stage", json!({ "label": "Reading your screen" }));
                    let read = with_failover(
                        &model,
                        |tried| next_free(Kind::Vision, tried),
                        || true,
                        |m| send("query:stage", busy_stage("Reading with", m)),
                        |m| {
                            let llm = llm.clone();
                            let act = act.clone();
                            let img = s.base64.clone();
                            let q = text.clone();
                            let exact = exact.clone();
                            let cancel = cancel.clone();
                            async move {
                                let opts = ReaderOpts {
                                    model: &m,
                                    image_base64: &img,
                                    question: &q,
                                    region: via_region,
                                    exact_text: exact.as_ref(),
                                };
                                llm.read_screen(&act, opts, &cancel).await
                            }
                        },
                    )
                    .await;
                    match read {
                        Ok((m, r)) => {
                            reader_used = Some(m.clone());
                            seeing = m;
                            report = Some(r);
                        }
                        Err(e) if e.is_abort() => return Err(e),
                        Err(_) => report = None,
                    }
                }
            }

            let skills = &services().skills;
            let pinned = store::get().get_skill_id(super::skill_exists);
            let chosen = if pinned == AUTO_SKILL {
                let task = extract_task(report.as_deref());
                skills
                    .route(&text, Some(task.as_str()).filter(|t| !t.is_empty()))
                    .0
            } else {
                pinned
            };
            let skill = skills.get(&chosen).map(|s| s.text());

            // One streamed attempt on `m`, with or without the picture or the
            // reading.
            let attempt = |m: String, image: Option<String>, rep: Option<String>| {
                let llm = llm.clone();
                let act = act.clone();
                let q = text.clone();
                let exact = exact.clone();
                let skill = skill.clone();
                let prior = prior.clone();
                let cancel = cancel.clone();
                let ex = ex.clone();
                let meter = meter.clone();
                let send = send.clone();
                let (w, h) = shot.as_ref().map(|s| (s.width, s.height)).unwrap_or((0, 0));
                async move {
                    let mut on_delta = move |t: &str| {
                        let pieces = ex.lock().push(t);
                        for p in pieces {
                            send("query:delta", json!({ "text": p }));
                        }
                    };
                    let opts = QueryOpts {
                        model: &m,
                        question: &q,
                        image_base64: image.as_deref(),
                        image_width: w,
                        image_height: h,
                        history: &prior,
                        skill: skill.as_ref(),
                        screen_report: rep.as_deref(),
                        via_region,
                        exact_text: exact.as_ref(),
                    };
                    let mut mtr = Meter::default();
                    let r = llm
                        .stream_query(&act, opts, &cancel, &mut on_delta, &mut mtr)
                        .await;
                    let mut total = meter.lock();
                    if total.ttft_ms == 0 && mtr.ttft_ms > 0 {
                        total.ttft_ms = (mtr.t0 - total.t0).as_millis() as u64 + mtr.ttft_ms;
                    }
                    total.prompt += mtr.prompt;
                    total.completion += mtr.completion;
                    if let Some(c) = mtr.cost {
                        total.cost = Some(total.cost.unwrap_or(0.0) + c);
                    }
                    r
                }
            };
            let nothing_shown = || ex.lock().text.is_empty();

            if let Some(rep) = report.clone() {
                answering = brain.clone();
                answer_kind = Kind::Text;
                send("query:stage", json!({ "label": "Thinking" }));
                let spoke = with_failover(
                    &brain,
                    |tried| next_free(Kind::Text, tried),
                    nothing_shown,
                    |m| send("query:stage", busy_stage("Thinking with", m)),
                    |m| attempt(m, None, Some(rep.clone())),
                )
                .await;
                match spoke {
                    Ok((m, _)) => answering = m,
                    Err(e) => {
                        if e.is_abort() || !nothing_shown() {
                            return Err(e);
                        }
                        answering = seeing.clone();
                        answer_kind = Kind::Vision;
                        send("query:stage", json!({ "label": "Answering directly" }));
                        let img = shot.as_ref().map(|s| s.base64.clone());
                        attempt(seeing.clone(), img, None).await?;
                    }
                }
            } else {
                answering = model.clone();
                if use_brain && shot.is_some() {
                    send("query:stage", json!({ "label": "Answering directly" }));
                }
                let img = shot.as_ref().map(|s| s.base64.clone());
                let (m, _) = with_failover(
                    &model,
                    |tried| next_free(Kind::Vision, tried),
                    nothing_shown,
                    |m| send("query:stage", busy_stage("Trying", m)),
                    |m| attempt(m, img.clone(), None),
                )
                .await?;
                answering = m;
            }
            Ok(report)
        }
        .await;

        let err = match attempted {
            Ok(report) => break Ok(report),
            Err(e) => e,
        };
        if err.is_abort() {
            break Err(err);
        }
        let (named, kind) = if err.stage == Some("reader") {
            (asked.clone(), Kind::Vision)
        } else {
            (answering.clone(), answer_kind)
        };
        super::settings::note_answer(app, provider, &act.key, Err(&err), &named);
        if !failed.contains(&provider) {
            failed.push(provider);
        }
        let code = code_of(provider, &err, &named);
        let shown = !ex.lock().text.is_empty();
        let online = super::online();
        // The next provider ready for this question, and its models.
        let mut next = None;
        if !shown && !pinned {
            let ready = candidates_for(&*services().settings, Kind::Vision);
            if let Some(c) = ready.into_iter().find(|c| !failed.contains(&c.provider)) {
                let (m, b) = models_on(c.provider).await;
                next = Some((c, m, b));
            }
        }
        let from_free = provider == Provider::OpenRouter && named.ends_with(":free");
        let f = Failed {
            code,
            online,
            shown,
            pinned,
            from_free,
            next_bills: next
                .as_ref()
                .map(|(c, m, _)| c.provider.is_native() || !m.ends_with(":free")),
            prefs,
            waits,
            retry_after_ms: err.retry_after_ms,
        };
        match (after_failure(&f), next) {
            (Next::Hop, Some((c, m, b))) => {
                let label = hop_stage(code, provider, c.provider);
                send("query:stage", json!({ "label": label }));
                hop_reason = Some(hop_reason_text(code, provider));
                provider = c.provider;
                act = c.active();
                cur_model = m;
                cur_brain = b;
                waits = 0;
            }
            (step @ (Next::Wait(ms) | Next::AskPaid(ms)), next) => {
                // The countdown note (C-71, C-72), or the question whether a
                // provider that bills should answer (C-63); either way this
                // one is asked again when the wait is over.
                let name = display_name(&named);
                let mut about = About::new(provider, &named, &name, kind);
                about.next = next.as_ref().map(|(c, _, _)| c.provider);
                about.online = online;
                let note = match (&next, step) {
                    (Some((c, m, _)), Next::AskPaid(_)) => {
                        paid_hop(c.provider, m, &display_name(m))
                    }
                    _ => busy_note(code, &about, ms),
                };
                let row = QueryError {
                    message: note.text,
                    question: text.clone(),
                    code: note.code.to_string(),
                    provider: provider.id().to_string(),
                    model: named.clone(),
                    model_name: name.clone(),
                    retry_in_ms: Some(ms),
                    tone: note.tone,
                    actions: note.actions,
                    detail: details(&err, &about),
                };
                send("query:error", json!(row));
                let stopped = tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(ms)) => false,
                    _ = cancel.cancelled() => true,
                };
                if stopped {
                    break Err(ApiError::abort());
                }
                waits += 1;
                send("query:start", json!({}));
            }
            _ => break Err(err),
        }
        // Nothing of the failed attempt was shown; the next starts clean.
        ex = Arc::new(Mutex::new(PointExtractor::new()));
    };
    drop(slot);

    for p in ex.lock().flush() {
        send("query:delta", json!({ "text": p }));
    }
    let (answer, points) = {
        let e = ex.lock();
        (e.text.clone(), e.points.clone())
    };
    let status = match result {
        Ok(report) => {
            outputs::with(app, &label, |out| {
                out.chat
                    .messages
                    .push(json!({ "role": "assistant", "text": answer }));
            });
            let m = meter.lock().clone();
            let usage = Usage {
                model: short_model(&answering),
                ttft_ms: m.ttft_ms,
                ms: t0.elapsed().as_millis() as u64,
                prompt: m.prompt,
                completion: m.completion,
                cost: m.cost,
                text_only,
                model_id: answering.clone(),
                model_name: display_name(&answering),
                provider: provider.id().to_string(),
                provider_name: provider.name().to_string(),
                via: hop_reason.as_ref().map(|_| provider.name().to_string()),
                hop_reason: hop_reason.clone(),
            };
            // What left this PC for this answer (B-121, K-19).
            // The count covers the visible text and the document, since both
            // went to the model that answered.
            let window_text = exact.as_ref().and_then(sent_window_text);
            let image = shot.as_ref().map(|s| {
                if s.region.is_some() {
                    "region"
                } else {
                    "screen"
                }
            });
            let reader = reader_used
                .as_ref()
                .filter(|_| report.is_some())
                .map(|m| json!({ "modelName": display_name(m) }));
            let sent = json!({
                "provider": provider.id(),
                "providerName": provider.name(),
                "model": answering,
                "modelName": display_name(&answering),
                "free": answering.ends_with(":free")
                    || reader_used.as_ref().is_some_and(|m| m.ends_with(":free")),
                "image": image,
                "imageSize": shot.as_ref().map(|s| json!([s.width, s.height])),
                "windowText": window_text,
                "history": prior.len(),
                "reader": reader,
                "msgId": gen,
            });
            if let Some(s) = &shot {
                let alive = |l: &str| outputs::with(app, l, |_| ()).is_some();
                keep_sent(sent_images(), &label, gen, s.base64.clone(), alive);
            }
            send(
                "query:done",
                json!({
                    "usage": usage,
                    "inferred": o.inferred,
                    "question": text,
                    "sent": sent,
                    "msgId": gen,
                }),
            );
            super::remember_chat(app, &label, &answering);
            super::settings::note_answer(app, provider, &act.key, Ok(()), &answering);
            shared(app).lock().pill_error = false;
            buddy::notify("first-answer");
            show_model_points(app, &points, shot.as_ref(), report.as_deref()).await;
            "done"
        }
        Err(err) => {
            if !answer.is_empty() {
                outputs::with(app, &label, |out| {
                    out.chat
                        .messages
                        .push(json!({ "role": "assistant", "text": answer }));
                });
                super::remember_chat(app, &label, &answering);
            }
            // The loop above told the status table about the failure.
            let (named, kind) = if err.stage == Some("reader") {
                (&asked, Kind::Vision)
            } else {
                (&answering, answer_kind)
            };
            match error_row(provider, &err, named, kind, &text) {
                Some(row) => send("query:error", json!(row)),
                None => send("query:done", json!({})),
            }
            "error"
        }
    };
    uninstall(current(), &label, gen);
    let idle = {
        let st = shared(app);
        let mut s = st.lock();
        s.query_ended(&label, gen);
        !s.busy
    };
    super::send_overlay_state(app);
    if idle {
        crate::updater::idle(app);
    }
    json!({ "status": status })
}

/// The bar's question, to its target output.
pub async fn submit(app: &AppHandle, question: &str, o: SubmitOpts) -> Value {
    submit_to(app, None, question, o).await
}

/// `models:list`: the picker for a job, with the model in use now.
async fn models_list(p: &Value) -> Value {
    let sv = services();
    let k = p.get("kind").and_then(Value::as_str);
    let kind = Kind::from_list_kind(k);
    let settings = &*sv.settings;
    let provider = p
        .get("provider")
        .and_then(Value::as_str)
        .and_then(Provider::from_id)
        .or_else(|| active(settings).map(|a| a.provider))
        .unwrap_or(Provider::OpenRouter);
    if let Some(key) = llm_settings::usable_key(settings, provider) {
        sv.llm.native_catalogue(provider, &key).await;
    }
    let mut result = sv
        .llm
        .list_models(provider, kind, settings.show_paid())
        .await;
    let current = if provider.is_native() {
        sv.llm
            .known_native(provider, kind, &model_for(settings, provider, kind))
    } else {
        let s = store::get();
        match kind {
            Kind::Vision => super::live_models(true).await.0,
            Kind::Text => super::live_models(true).await.1,
            Kind::Find => sv.llm.resolve_live(Kind::Find, &s.get_find_model()).await,
            Kind::Agent => sv.llm.resolve_live(Kind::Agent, &s.get_agent_model()).await,
        }
    };
    if let Value::Object(m) = &mut result {
        m.insert("kind".into(), json!(kind.as_str()));
        m.insert("current".into(), json!(current));
    }
    // The bar's `@` menu and an answer's "Retry with" ask with no kind and
    // no provider: they list every provider with a key (K-19, B-122).
    if p.get("kind").is_none() && p.get("provider").is_none() {
        result["providers"] = json!(ready_providers().await);
    }
    result
}

/// Every provider with a usable key, in the person's order, with its
/// screen-question models and the one in use now.
async fn ready_providers() -> Vec<Value> {
    let sv = services();
    let settings = &*sv.settings;
    let mut out = Vec::new();
    for c in llm_settings::keyed(settings) {
        let provider = c.provider;
        if provider.is_native() {
            sv.llm.native_catalogue(provider, &c.key).await;
        }
        let list = sv
            .llm
            .list_models(provider, Kind::Vision, settings.show_paid())
            .await;
        let current = if provider.is_native() {
            let pick = model_for(settings, provider, Kind::Vision);
            sv.llm.known_native(provider, Kind::Vision, &pick)
        } else {
            super::live_models(true).await.0
        };
        out.push(json!({
            "id": provider.id(),
            "name": provider.name(),
            "groups": list["groups"].clone(),
            "current": current,
            "ready": true,
        }));
    }
    out
}

/// IPC rows query:submit, query:cancel, query:retry and models:list, after
/// the Settings rows `host::settings::route` owns
/// (contract K-15); None for any other channel.
pub async fn route(
    app: &AppHandle,
    label: &str,
    channel: &str,
    p: &Value,
) -> Option<Result<Value, String>> {
    if let Some(r) = super::settings::route(app, label, channel, p).await {
        return Some(r);
    }
    let out = match channel {
        "query:submit" => {
            let q = text_of(&p["question"]);
            let o = SubmitOpts {
                inferred: crate::store::truthy(p.get("inferred")),
                pin: pin_of(p),
                ..SubmitOpts::default()
            };
            submit(app, &q, o).await
        }
        "query:cancel" => {
            // The caller's own window, else the focused output (B-040).
            let named = p.get("label").and_then(Value::as_str).map(str::to_string);
            let own = outputs::is_output(label).then(|| label.to_string());
            match named.or(own).or_else(|| outputs::focused(app)) {
                Some(l) => abort_for(app, &l),
                None => abort(app),
            }
            Value::Null
        }
        "query:retry" => {
            let from = if outputs::is_output(label) {
                Some(label.to_string())
            } else {
                outputs::focused(app)
            };
            let last = from
                .as_ref()
                .and_then(|l| outputs::with(app, l, |o| o.chat.last_question.clone()))
                .unwrap_or_default();
            if last.is_empty() {
                json!({ "status": "nothing-to-retry" })
            } else {
                // "Retry with" another model sends it; a plain Retry has none.
                let o = SubmitOpts {
                    retry: true,
                    pin: pin_of(p),
                    ..SubmitOpts::default()
                };
                submit_to(app, from, &last, o).await
            }
        }
        "models:list" => models_list(p).await,
        "output:sent-image" => {
            let id = p.get("msgId").and_then(Value::as_u64).unwrap_or(0);
            json!(sent_image(sent_images(), label, id))
        }
        _ => return None,
    };
    Some(Ok(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_compare_without_punctuation_or_role_words() {
        assert_eq!(norm_name("  Save   As… "), "save as");
        assert_eq!(norm_name(&strip_role_words("Save button")), "save");
    }

    #[test]
    fn second_window_does_not_abort_first() {
        let map = Mutex::new(HashMap::new());
        let first = install(&map, "output-1", 1);
        let second = install(&map, "output-2", 2);
        assert!(
            !first.is_cancelled(),
            "another window's Ask leaves it running"
        );
        let again = install(&map, "output-1", 3);
        assert!(
            first.is_cancelled(),
            "a new question in the same window replaces it"
        );
        assert!(!again.is_cancelled() && !second.is_cancelled());
        uninstall(&map, "output-1", 1);
        assert!(
            map.lock().contains_key("output-1"),
            "a stale end keeps the newer Ask"
        );
        uninstall(&map, "output-1", 3);
        assert!(!map.lock().contains_key("output-1"));
    }

    #[test]
    fn third_query_waits_for_a_slot() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        rt.block_on(async {
            let slots = Arc::new(Semaphore::new(MAX_ANSWERS));
            let cancel = CancellationToken::new();
            let told = std::cell::Cell::new(0);
            let tell = || told.set(told.get() + 1);
            let first = wait_for_slot(&slots, &cancel, tell).await;
            let second = wait_for_slot(&slots, &cancel, tell).await;
            assert!(first.is_some() && second.is_some());
            assert_eq!(told.get(), 0, "two answers start at once");
            // The third is told to wait, and starts when one finishes.
            let third = wait_for_slot(&slots, &cancel, tell);
            tokio::pin!(third);
            let early = tokio::time::timeout(Duration::from_millis(30), &mut third).await;
            assert!(early.is_err(), "the third did not wait");
            assert_eq!(told.get(), 1);
            drop(first);
            let third = third.await;
            assert!(third.is_some());
            // A question stopped while it waits never starts.
            let stopped = CancellationToken::new();
            stopped.cancel();
            assert!(wait_for_slot(&slots, &stopped, || ()).await.is_none());
            drop((second, third));
        });
    }

    fn failed(code: &'static str) -> Failed<'static> {
        Failed {
            code,
            online: true,
            shown: false,
            pinned: false,
            from_free: true,
            next_bills: None,
            prefs: FailoverPrefs::default(),
            waits: 0,
            retry_after_ms: None,
        }
    }

    #[test]
    fn hop_only_before_text() {
        let ready = Failed {
            from_free: false,
            next_bills: Some(true),
            ..failed(codes::RATE_LIMITED)
        };
        assert_eq!(after_failure(&ready), Next::Hop);
        for code in [
            codes::SERVER,
            codes::MODEL_GONE,
            codes::BAD_KEY,
            codes::NO_CREDIT,
            codes::DAILY_CAP,
            codes::OFFLINE,
        ] {
            assert_eq!(
                after_failure(&Failed { code, ..ready }),
                Next::Hop,
                "{code}"
            );
        }
        // Not after text started, not with a pinned model, not when the
        // person turned hopping off, and not for a failure that would be
        // the same anywhere.
        let shown = Failed {
            shown: true,
            ..ready
        };
        assert_eq!(after_failure(&shown), Next::Stop);
        let pinned = Failed {
            pinned: true,
            ..ready
        };
        assert_eq!(after_failure(&pinned), Next::Stop);
        let mut off = ready;
        off.prefs.across_providers = false;
        assert_eq!(after_failure(&off), Next::Wait(20_000));
        for code in [
            codes::REFUSED,
            codes::NO_VISION,
            codes::DATA_POLICY,
            codes::UNKNOWN,
        ] {
            assert_eq!(
                after_failure(&Failed { code, ..ready }),
                Next::Stop,
                "{code}"
            );
        }
        let offline = Failed {
            code: codes::OFFLINE,
            online: false,
            ..ready
        };
        assert_eq!(after_failure(&offline), Next::Stop);
    }

    #[test]
    fn free_to_paid_asks_first() {
        let busy = Failed {
            next_bills: Some(true),
            ..failed(codes::FREE_BUSY)
        };
        assert_eq!(after_failure(&busy), Next::AskPaid(20_000));
        // A free model's other failures show their own row, which offers
        // the provider; nothing moves to a bill by itself.
        let down = Failed {
            code: codes::SERVER,
            ..busy
        };
        assert_eq!(after_failure(&down), Next::Stop);
        // Free to free, and paid to paid, move without asking.
        let free_next = Failed {
            next_bills: Some(false),
            ..busy
        };
        assert_eq!(after_failure(&free_next), Next::Hop);
        let from_paid = Failed {
            from_free: false,
            ..busy
        };
        assert_eq!(after_failure(&from_paid), Next::Hop);
        // "Don't ask again".
        let mut never = busy;
        never.prefs.ask_before_paid = false;
        assert_eq!(after_failure(&never), Next::Hop);
    }

    #[test]
    fn busy_retries_twice_then_stops() {
        let busy = failed(codes::FREE_BUSY);
        assert_eq!(after_failure(&busy), Next::Wait(20_000));
        let again = Failed { waits: 1, ..busy };
        assert_eq!(after_failure(&again), Next::Wait(40_000));
        let done = Failed { waits: 2, ..busy };
        assert_eq!(after_failure(&done), Next::Stop);
        // Retry-After is honoured, within reason.
        let told = Failed {
            retry_after_ms: Some(7_000),
            ..busy
        };
        assert_eq!(after_failure(&told), Next::Wait(7_000));
        let at_once = Failed {
            retry_after_ms: Some(0),
            ..busy
        };
        assert_eq!(after_failure(&at_once), Next::Wait(1_000));
        let hour = Failed {
            retry_after_ms: Some(3_600_000),
            ..busy
        };
        assert_eq!(after_failure(&hour), Next::Stop);
        // Only a busy provider is asked again.
        assert_eq!(after_failure(&failed(codes::SERVER)), Next::Stop);
    }

    #[test]
    fn hops_say_where_and_why() {
        let (or, an) = (Provider::OpenRouter, Provider::Anthropic);
        let stage = hop_stage(codes::FREE_BUSY, or, an);
        assert_eq!(stage, "OpenRouter is busy. Asking Anthropic\u{2026}");
        assert_eq!(hop_reason_text(codes::FREE_BUSY, or), "OpenRouter was busy");
        assert!(hop_stage(codes::SERVER, an, or).starts_with("Anthropic didn't answer."));
    }

    fn numbered(lines: usize) -> String {
        (1..=lines)
            .map(|i| format!("this is line number {i:04}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_document_that_fits_goes_whole() {
        let doc = numbered(10);
        let (text, cut) = fit_document(&doc, "", "", false, 1000);
        assert_eq!(text, doc);
        assert!(!cut);
        let (text, cut) = fit_document(&doc, "", "", true, 1000);
        assert!(cut, "the helper stopped before the end");
        assert!(text.starts_with(&doc));
        assert!(text.ends_with("the rest is not included]"));
    }

    #[test]
    fn a_long_document_keeps_the_part_in_view() {
        // 400 lines of 24 characters and a line break: 9999 characters.
        let doc = numbered(400);
        let (text, cut) = fit_document(&doc, "this is line number 0200\n", "", false, 1000);
        assert!(cut);
        assert!(text.starts_with("[cut: "));
        assert!(text.contains("characters before this are not included]\n"));
        assert!(text.contains("this is line number 0200"));
        assert!(
            text.contains("this is line number 0195"),
            "some of what is above"
        );
        assert!(
            text.contains("this is line number 0220"),
            "more of what is below"
        );
        assert!(!text.contains("this is line number 0001"));
        assert!(text.ends_with("characters after this are not included]"));
        // Whole lines only: every kept line is a full one.
        for l in text.lines().filter(|l| !l.starts_with("[cut: ")) {
            assert_eq!(l.len(), 24, "{l}");
        }
        let kept = text.lines().filter(|l| !l.starts_with("[cut: ")).count();
        assert!((36..=40).contains(&kept), "{kept}");
    }

    #[test]
    fn a_long_document_without_a_known_place_keeps_its_start() {
        let doc = numbered(400);
        // "File" is too short to say where the window is scrolled to.
        let (text, cut) = fit_document(&doc, "", "File\nno such line here", false, 1000);
        assert!(cut);
        assert!(text.starts_with("this is line number 0001\n"));
        assert!(text.ends_with("characters after this are not included]"));
        // The window's visible text is the fallback when the helper gave no
        // visible range.
        let (text, _) = fit_document(&doc, "", "File\n  this is line number 0390", true, 1000);
        assert!(text.starts_with("[cut: "));
        assert!(text.contains("this is line number 0400"));
        assert!(text.ends_with("the rest is not included]"), "{text}");
    }

    #[test]
    fn the_document_goes_only_when_it_adds_to_what_is_in_view() {
        let doc = numbered(30);
        let reply = json!({ "docs": [
            { "ref": "e1", "text": "example.com", "seen": "example.com", "truncated": false },
            { "ref": "e2", "text": doc.replace('\n', "\r\n"), "seen": "", "truncated": false },
        ] });
        let (text, cut) = document_for(&reply, "File\nthis is line number 0001", 60000).unwrap();
        assert_eq!(text, doc, "the longest document, with plain line breaks");
        assert!(!cut);
        let all = format!("File\n{doc}");
        assert_eq!(document_for(&reply, &all, 60000), None);
        assert_eq!(document_for(&json!({ "docs": [] }), "x", 60000), None);
        assert_eq!(document_for(&json!({}), "x", 60000), None);
    }

    #[test]
    fn what_was_sent_counts_the_document() {
        let mut e = ExactText {
            title: "Notes".into(),
            text: "abc".into(),
            ..ExactText::default()
        };
        let seen = json!({ "app": "Notes", "chars": 3 });
        assert_eq!(sent_window_text(&e), Some(seen));
        e.document = "abcdefg".into();
        let whole = json!({ "app": "Notes", "chars": 10, "document": "whole" });
        assert_eq!(sent_window_text(&e), Some(whole));
        e.document_cut = true;
        let part = sent_window_text(&e).unwrap();
        assert_eq!(part["document"], json!("part"));
        assert_eq!(sent_window_text(&ExactText::default()), None);
    }

    #[test]
    fn sent_images_keep_last_three() {
        let map = Mutex::new(HashMap::new());
        for id in 1..=4 {
            keep_sent(&map, "output-1", id, format!("img{id}"), |_| true);
        }
        assert_eq!(
            sent_image(&map, "output-1", 1),
            None,
            "the oldest is dropped"
        );
        let url = sent_image(&map, "output-1", 4).unwrap();
        assert_eq!(url, "data:image/jpeg;base64,img4");
        assert_eq!(sent_image(&map, "output-2", 4), None, "per window");
        keep_sent(&map, "output-2", 5, "x".into(), |_| false);
        assert!(!map.lock().contains_key("output-1"), "a closed window's go");
    }

    #[test]
    fn stale_pick_shot_is_not_reused() {
        let slot = Mutex::new(None);
        let t0 = Instant::now();
        *slot.lock() = Some((7, t0));
        let soon = t0 + Duration::from_millis(400);
        assert_eq!(take_fresh(&slot, soon), Some(7));
        assert!(slot.lock().is_none());
        *slot.lock() = Some((8, t0));
        let late = t0 + Duration::from_millis(1_500);
        assert_eq!(take_fresh(&slot, late), None);
        assert!(slot.lock().is_none(), "a stale shot is dropped, not kept");
    }

    #[test]
    fn error_rows_name_the_model_and_the_fix() {
        let name = "Gemma 4 26B A4B";
        let id = "google/gemma-4-26b-a4b-it:free";
        let a = About::new(Provider::OpenRouter, id, name, Kind::Vision);
        let gone = ApiError::http(404, "No endpoints found").with_attempts(2);
        let row = query_error(&gone, &a, "what now?").unwrap();
        let v = json!(row);
        assert_eq!(v["code"], "model-gone");
        assert_eq!(v["modelName"], name);
        assert_eq!(v["model"], id);
        assert_eq!(v["provider"], "openrouter");
        assert_eq!(v["tone"], "error");
        assert_eq!(v["question"], "what now?");
        assert_eq!(v["actions"][0]["id"], "choose-model");
        assert!(v["message"].as_str().unwrap().starts_with(name));
        assert!(v["detail"].as_str().unwrap().contains(id));
        assert!(v.get("retryInMs").is_none());
        assert!(query_error(&ApiError::abort(), &a, "q").is_none());
    }
}
