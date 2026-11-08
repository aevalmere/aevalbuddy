//! Shared app state: what main.js kept in module-level `let`s, behind one
//! lock, plus the seams other modules plug into (ARCHITECTURE.md section 3).
//!
//! Rule for every caller: take the lock, copy what you need, drop it, then
//! touch windows. A Tauri getter waits on the main thread, and the main
//! thread's window-event handler takes this lock; holding it across a window
//! call is a deadlock.

use std::collections::BTreeMap;

use parking_lot::{Mutex, MutexGuard, RwLock};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tokio::sync::broadcast;

/// A rectangle in DIPs (Electron's unit), the glass and never the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    /// Area shared with `other`, in square DIPs.
    pub fn overlap(&self, other: &Rect) -> f64 {
        let w = (self.x + self.width).min(other.x + other.width) - self.x.max(other.x);
        let h = (self.y + self.height).min(other.y + other.height) - self.y.max(other.y);
        if w <= 0.0 || h <= 0.0 {
            0.0
        } else {
            w * h
        }
    }
}

/// The overlay window's mode: the orb, the open bar, or the layout editor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Pill,
    Panel,
    Hud,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Pill => "pill",
            Mode::Panel => "panel",
            Mode::Hud => "hud",
        }
    }
}

/// One conversation: the messages main.js kept in `chat`, newest last, each
/// `{ role, text, kind? }` exactly as the renderer and history.json see them.
#[derive(Clone, Debug, Default)]
pub struct Chat {
    pub id: String,
    pub messages: Vec<Value>,
    /// The area this conversation is about, in DIPs, or None for the screen.
    pub region: Option<Rect>,
    /// The last question actually sent, for `query:retry`.
    pub last_question: String,
}

impl Chat {
    pub fn fresh() -> Self {
        Chat {
            id: new_chat_id(),
            ..Chat::default()
        }
    }
}

/// `c<base36 time><6 random base36 chars>`, as main.js's newChatId.
pub fn new_chat_id() -> String {
    use rand::Rng;
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let mut rng = rand::thread_rng();
    let tail: String = (0..6)
        .map(|_| std::char::from_digit(rng.gen_range(0..36), 36).unwrap_or('0'))
        .collect();
    format!("c{}{}", base36(ms), tail)
}

fn base36(mut n: u64) -> String {
    if n == 0 {
        return "0".into();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(std::char::from_digit((n % 36) as u32, 36).unwrap_or('0'));
        n /= 36;
    }
    out.iter().rev().collect()
}

/// What an output window is showing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputKind {
    #[default]
    Chat,
    Trail,
}

/// One floating output window (`output-<n>`, REQUESTS.md 11). Each keeps its
/// own conversation, position and size.
#[derive(Clone, Debug, Default)]
pub struct Output {
    pub label: String,
    pub chat: Chat,
    pub kind: OutputKind,
    /// Shown with content (main.js `outputActive`, per window).
    pub active: bool,
    /// The page finished loading and can take events.
    pub ready: bool,
    /// Placed at least once: later shows glide, the first one globs out.
    pub placed: bool,
    /// Height the transcript asked for (`output:resize`), glass DIPs.
    pub content_height: f64,
    /// Height the user dragged to; 0 = follow the transcript.
    pub user_height: f64,
    /// Where it last was, glass DIPs.
    pub rect: Option<Rect>,
    /// `chat` or `find` (`view:set`).
    pub view: String,
    /// When it last had focus or opened, for "most recently focused".
    pub touched: u64,
    /// The visible sheet inside the glass (`output:glass`): its top and
    /// height in glass DIPs, or None until the page reports it.
    pub sheet: Option<(f64, f64)>,
    /// The conversation "Start over" replaced, and when its Undo expires
    /// (ms since the epoch).
    pub undo: Option<(Chat, u64)>,
}

/// What the agent module reports for `overlay:state.agent`.
#[derive(Clone, Debug, Default)]
pub struct AgentStatus {
    pub running: bool,
    pub step: u64,
    pub state: String,
}

#[derive(Debug)]
pub struct AppState {
    pub mode: Mode,
    /// Hidden with the toggleVisibility chord.
    pub hidden: bool,
    pub input_height: f64,
    pub bar_mode: String,
    pub pill_error: bool,
    /// An answer is streaming somewhere (main.js `queryBusy`): "any entry in
    /// `queries`" once the Ask reports per window (K-03).
    pub busy: bool,
    pub query_gen: u64,
    /// Output label to the generation of the Ask streaming into it (K-03).
    pub queries: BTreeMap<String, u64>,
    /// The bar holds text the person typed (`bar:dirty`).
    pub bar_dirty: bool,
    pub capture_ok: bool,
    pub find_open: bool,
    pub agent: AgentStatus,
    /// The tour or the pointer is showing (cursorBuddy.isActive).
    pub buddy_active: bool,
    /// The area selector is open (region.isActive).
    pub region_active: bool,
    /// Settings is recording a chord: every global shortcut is off.
    pub capture_mode: bool,
    pub escape_suspended: bool,
    pub escape_registered: bool,
    /// Last registration result per action: Some(true), Some(false), None.
    pub hotkeys: BTreeMap<String, Option<bool>>,
    pub outputs: BTreeMap<String, Output>,
    /// The output the bar's next follow-up goes to.
    pub focused_output: Option<String>,
    pub next_output: u32,
    /// HUD editor: the bar anchor being placed.
    pub hud_pending: Option<Value>,
    pub reduced_motion: bool,
    pub boot_ms: u64,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            mode: Mode::Pill,
            hidden: false,
            input_height: crate::windows::layout::DEFAULT_INPUT_HEIGHT,
            bar_mode: "ask".into(),
            pill_error: false,
            busy: false,
            query_gen: 0,
            queries: BTreeMap::new(),
            bar_dirty: false,
            capture_ok: true,
            find_open: false,
            agent: AgentStatus::default(),
            buddy_active: false,
            region_active: false,
            capture_mode: false,
            escape_suspended: false,
            escape_registered: false,
            hotkeys: BTreeMap::new(),
            outputs: BTreeMap::new(),
            focused_output: None,
            next_output: 1,
            hud_pending: None,
            reduced_motion: false,
            boot_ms: 0,
        }
    }
}

impl AppState {
    /// The Ask with generation `gen` started streaming into `label` (K-03).
    pub fn query_started(&mut self, label: &str, gen: u64) {
        self.queries.insert(label.to_string(), gen);
        self.busy = true;
    }

    /// The Ask `gen` in `label` ended. A newer Ask in the same window keeps
    /// its entry.
    pub fn query_ended(&mut self, label: &str, gen: u64) {
        if self.queries.get(label) == Some(&gen) {
            self.queries.remove(label);
        }
        self.busy = !self.queries.is_empty();
    }

    /// Whether an Ask is streaming into `label`.
    pub fn query_running(&self, label: &str) -> bool {
        self.queries.contains_key(label)
    }

    /// The window went away: its entry goes, whatever the generation.
    pub fn query_forget(&mut self, label: &str) {
        if self.queries.remove(label).is_some() {
            self.busy = !self.queries.is_empty();
        }
    }
}

/// A hook another module registers at setup. ARCHITECTURE.md section 4.5.
pub type Hook = Box<dyn Fn(&AppHandle) + Send + Sync>;
/// A hook that takes one string (an event name, a mode, a message).
pub type StrHook = Box<dyn Fn(&AppHandle, &str) + Send + Sync>;

/// A hook that returns JSON (merged into a reply by the caller).
pub type ValueHook = Box<dyn Fn(&AppHandle) -> Value + Send + Sync>;

/// The calls core makes into modules other agents own. Each is optional:
/// unset, the core does what main.js did with that module absent.
#[derive(Default)]
pub struct Hooks {
    /// agent.stop() (Escape, tray Stop agent).
    pub stop_agent: Option<Hook>,
    /// closeFind().
    pub close_find: Option<Hook>,
    /// region.cancel().
    pub cancel_region: Option<Hook>,
    /// cursorBuddy.cancel().
    pub cancel_tour: Option<Hook>,
    /// triggerRegionCapture() (captureRegion chord, tray item).
    pub start_region: Option<Hook>,
    /// startWalkthrough() (`tour:replay`, first run).
    pub start_tour: Option<Hook>,
    /// abortCurrentQuery().
    pub abort_query: Option<Hook>,
    /// Stops the Ask streaming into one output window, and only that one
    /// (K-03; llm registers it). Arg: the output label.
    pub abort_query_for: Option<StrHook>,
    /// prefetchBarContext() after the bar opens.
    pub prefetch_bar_context: Option<Hook>,
    /// Warm the model catalogue at boot (liveModels()).
    pub warm_catalogue: Option<Hook>,
    /// The agent's trail is wanted (showTrail(false)) while a run is going.
    pub show_trail: Option<Hook>,
    /// cursorBuddy.notify(event): settings-opened, settings-closed, key-saved.
    pub tour_notify: Option<StrHook>,
    /// setBarMode(mode): opens or closes Find as the mode changes.
    pub set_bar_mode: Option<StrHook>,
    /// A key was saved (arg: provider). Unset: the tour hears `key-saved`.
    /// Agent b/c replace it with main.js's test-then-advance logic.
    pub key_saved: Option<StrHook>,
    /// Extra `settings:get` fields (models resolved from the catalogue,
    /// skills, glass tier), merged over the core's.
    pub settings_extra: Option<ValueHook>,
    /// Extra `overlay:state` fields (K-07; llm registers `recipients`),
    /// merged over the core's. Called before the state lock is taken, so
    /// it may read the state itself.
    pub overlay_extra: Option<ValueHook>,
    /// Copies the live buddy, region, Find and agent state into `AppState`
    /// (`find_open`, `buddy_active`, `region_active`, `agent`). The core
    /// calls it through `refresh` before it reads those flags.
    pub refresh_flags: Option<Hook>,
}

/// One event as the windows saw it, mirrored for the smoke run (main.js
/// `bus`).
#[derive(Clone, Debug)]
pub struct BusEvent {
    pub channel: String,
    pub target: String,
    pub payload: Value,
}

pub struct Shared {
    state: Mutex<AppState>,
    hooks: RwLock<Hooks>,
    pub bus: broadcast::Sender<BusEvent>,
}

impl Default for Shared {
    fn default() -> Self {
        let (bus, _) = broadcast::channel(256);
        Shared {
            state: Mutex::new(AppState::default()),
            hooks: RwLock::new(Hooks::default()),
            bus,
        }
    }
}

impl Shared {
    pub fn lock(&self) -> MutexGuard<'_, AppState> {
        self.state.lock()
    }

    /// Registers hooks; call during setup.
    pub fn set_hooks(&self, f: impl FnOnce(&mut Hooks)) {
        f(&mut self.hooks.write());
    }

    /// Runs `pick(hooks)` if set. Returns whether a hook ran.
    pub fn call(&self, app: &AppHandle, pick: impl Fn(&Hooks) -> Option<&Hook>) -> bool {
        let hooks = self.hooks.read_recursive();
        match pick(&hooks) {
            Some(h) => {
                h(app);
                true
            }
            None => false,
        }
    }

    pub fn call_value(
        &self,
        app: &AppHandle,
        pick: impl Fn(&Hooks) -> Option<&ValueHook>,
    ) -> Option<Value> {
        let hooks = self.hooks.read_recursive();
        pick(&hooks).map(|h| h(app))
    }

    pub fn call_str(
        &self,
        app: &AppHandle,
        arg: &str,
        pick: impl Fn(&Hooks) -> Option<&StrHook>,
    ) -> bool {
        let hooks = self.hooks.read_recursive();
        match pick(&hooks) {
            Some(h) => {
                h(app, arg);
                true
            }
            None => false,
        }
    }
}

/// The shared state of a running app.
pub fn shared(app: &AppHandle) -> tauri::State<'_, Shared> {
    app.state::<Shared>()
}

/// Brings the flags other modules own up to date. Never call it with the
/// state lock held.
pub fn refresh(app: &AppHandle) {
    shared(app).call(app, |k| k.refresh_flags.as_ref());
}

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_ids_look_like_electrons() {
        let id = new_chat_id();
        assert!(id.starts_with('c'));
        assert!(id.len() >= 10);
        assert!(id[1..].chars().all(|c| c.is_ascii_alphanumeric()));
        assert_ne!(new_chat_id(), new_chat_id());
    }

    #[test]
    fn base36_matches_javascript() {
        // (1700000000000).toString(36) in node
        assert_eq!(base36(1_700_000_000_000), "loyw3v28");
        assert_eq!(base36(35), "z");
    }

    #[test]
    fn queries_are_per_window_and_busy_is_any() {
        let mut s = AppState::default();
        s.query_started("output-1", 1);
        s.query_started("output-2", 2);
        assert!(s.busy);
        assert!(s.query_running("output-1") && s.query_running("output-2"));
        // A stale end for an older generation leaves the newer Ask alone.
        s.query_started("output-1", 3);
        s.query_ended("output-1", 1);
        assert!(s.query_running("output-1"));
        s.query_ended("output-1", 3);
        assert!(!s.query_running("output-1"));
        assert!(s.busy);
        s.query_forget("output-2");
        assert!(!s.busy);
        assert!(s.queries.is_empty());
    }

    #[test]
    fn rect_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.overlap(&b), 25.0);
        assert_eq!(a.overlap(&Rect::new(20.0, 0.0, 5.0, 5.0)), 0.0);
        assert!(a.contains(10.0, 0.0));
    }
}
