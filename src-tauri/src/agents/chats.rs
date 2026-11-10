//! Watching the Claude and ChatGPT desktop apps. Their chats have no hooks,
//! but while a reply is being written they show a Stop button, which
//! Windows accessibility (UI Automation) can see. A window showing one is
//! treated as a working session, and as done when the button goes. Port of
//! Aevalsistant's `Chats.cs`.
//!
//! A window is read only when you switch away from one of these apps, and
//! then every few seconds until its reply is done, so nothing is read while
//! you are not waiting on a reply. All of it runs on one thread of its own,
//! which is the only one that ever touches the UI Automation objects.
//!
//! UI Automation is COM. Its three interfaces used here are declared by
//! hand, as the rest of this module's Windows calls are: each method table
//! names the methods that are called and counts the ones in between.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::time::Duration;

use parking_lot::Mutex;

use super::native;
use super::sessions::HookEvent;

/// How often the window in front is looked up (a cheap call, no reading).
const POLL_MS: u64 = 500;
/// Windows being watched are read again every this many polls: 2.5 s.
const READ_EVERY: u64 = 5;
/// After you switch away, before the first read: Chromium builds its
/// accessibility tree on the first request.
const SETTLE_MS: u64 = 500;
/// Before the second look at a window whose first read found no button.
const RETRY_MS: u64 = 700;

/// What the Stop button is called, in either app.
const STOP_NAMES: [&str; 4] = ["Stop response", "Stop streaming", "Stop generating", "Stop"];

/// The chat app an exe is, by the name people know it by.
pub fn app_of(exe: &str) -> Option<&'static str> {
    if exe.eq_ignore_ascii_case("claude.exe") {
        Some("Claude")
    } else if exe.eq_ignore_ascii_case("chatgpt.exe") {
        Some("ChatGPT")
    } else {
        None
    }
}

/// The window just left, when the one in front changed. `last` follows it.
fn switched(last: &mut i64, front: i64) -> Option<i64> {
    if front == *last {
        return None;
    }
    let left = std::mem::replace(last, front);
    (left != 0).then_some(left)
}

/// Turns "is this chat window writing a reply?" samples into session
/// events. No Windows code, so the rules can be tested anywhere.
#[derive(Default)]
pub struct Tracker {
    /// Each window with a reply under way, and how many reads in a row
    /// found no Stop button.
    watching: HashMap<i64, u8>,
}

impl Tracker {
    pub fn is_watching(&self, hwnd: i64) -> bool {
        self.watching.contains_key(&hwnd)
    }

    pub fn watched(&self) -> Vec<i64> {
        self.watching.keys().copied().collect()
    }

    pub fn session_id(hwnd: i64) -> String {
        format!("chat:{hwnd}")
    }

    /// One sample of a window. `generating` is whether the Stop button
    /// shows, or None when the window could not be read.
    pub fn observe(
        &mut self,
        hwnd: i64,
        pid: u32,
        app: &str,
        exists: bool,
        generating: Option<bool>,
        foreground: bool,
    ) -> Option<HookEvent> {
        let event = |name: &str| HookEvent {
            name: name.to_string(),
            session_id: Tracker::session_id(hwnd),
            agent: "chat".to_string(),
            label: "Chat".to_string(),
            host: app.to_string(),
            hwnd,
            pid,
            ..HookEvent::default()
        };
        if !exists {
            return self.watching.remove(&hwnd).map(|_| event("SessionEnd"));
        }
        if generating? {
            if let Some(misses) = self.watching.get_mut(&hwnd) {
                *misses = 0;
                return None;
            }
            self.watching.insert(hwnd, 0);
            return Some(event("UserPromptSubmit"));
        }
        let misses = self.watching.get_mut(&hwnd)?;
        // Two misses in a row, so a redraw that drops the button for a
        // moment does not count.
        *misses += 1;
        if *misses < 2 {
            return None;
        }
        self.watching.remove(&hwnd);
        // Finished while you were looking at it: nothing to tell you.
        Some(event(if foreground { "Interrupt" } else { "Stop" }))
    }
}

// ----------------------------------------------------------------- watcher

/// Chat watching is wanted.
static ON: AtomicBool = AtomicBool::new(false);
/// The watcher thread is running.
static ALIVE: AtomicBool = AtomicBool::new(false);
/// Why chat watching stopped by itself; empty when it has not.
static PROBLEM: Mutex<String> = Mutex::new(String::new());

/// Starts or stops watching. The thread ends by itself within a poll of
/// being switched off.
pub fn set(on: bool) {
    if !on {
        ON.store(false, SeqCst);
        return;
    }
    PROBLEM.lock().clear();
    ON.store(true, SeqCst);
    if ALIVE.swap(true, SeqCst) {
        return;
    }
    let started = std::thread::Builder::new()
        .name("agents-chats".into())
        .spawn(run);
    if started.is_err() {
        ALIVE.store(false, SeqCst);
        fail("Windows would not start another thread.");
    }
}

/// Why chat watching stopped by itself, for Settings.
pub fn problem() -> Option<String> {
    let why = PROBLEM.lock().clone();
    (!why.is_empty()).then_some(why)
}

/// Chat watching switches itself off and Settings says why.
fn fail(why: &str) {
    ON.store(false, SeqCst);
    *PROBLEM.lock() = format!("Chat watching stopped: {why}");
    super::chats_stopped();
}

fn run() {
    loop {
        if crate::crash::guard("agents-chats", watch).is_none() {
            fail("it ran into an error. Turn it off and on to try again.");
        }
        ALIVE.store(false, SeqCst);
        // Switched back on in the moment this thread was leaving.
        if !ON.load(SeqCst) || ALIVE.swap(true, SeqCst) {
            return;
        }
    }
}

fn pause(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn watch() {
    let Some(uia) = Uia::new() else {
        fail("Windows UI Automation did not load.");
        return;
    };
    let mut tracker = Tracker::default();
    let mut last = native::foreground();
    let mut polls: u64 = 0;
    while ON.load(SeqCst) {
        pause(POLL_MS);
        polls += 1;
        let left = switched(&mut last, native::foreground())
            .filter(|hwnd| app_of(&native::window_exe(*hwnd).1).is_some());
        if let Some(hwnd) = left {
            pause(SETTLE_MS);
            sample(&uia, &mut tracker, hwnd, true);
        }
        if polls % READ_EVERY == 0 {
            for hwnd in tracker.watched() {
                sample(&uia, &mut tracker, hwnd, false);
            }
        }
    }
}

/// Reads one window and passes on what changed.
fn sample(uia: &Uia, tracker: &mut Tracker, hwnd: i64, retry_if_empty: bool) {
    let exists = native::is_window(hwnd);
    let (pid, exe) = native::window_exe(hwnd);
    let app = if exists { app_of(&exe) } else { None };
    if exists && app.is_none() {
        return;
    }
    let watching = tracker.is_watching(hwnd);
    if exists && !watching && super::hook_busy(hwnd) {
        return;
    }
    let mut generating = if exists { uia.generating(hwnd) } else { None };
    if generating == Some(false) && retry_if_empty && !watching {
        pause(RETRY_MS);
        generating = uia.generating(hwnd);
    }
    let front = native::foreground() == hwnd;
    let app = app.unwrap_or_default();
    if let Some(e) = tracker.observe(hwnd, pid, app, exists, generating, front) {
        super::on_event(&e);
    }
}

// --------------------------------------------------------------------- COM

type Raw = *mut c_void;

#[repr(C)]
struct Guid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}

/// A VARIANT: its type, then the value in a slot two pointers wide.
#[repr(C)]
#[derive(Clone, Copy)]
struct Variant {
    kind: u16,
    reserved: [u16; 3],
    value: [usize; 2],
}

/// Every COM interface starts with these three.
#[repr(C)]
struct UnknownVtbl {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(Raw) -> u32,
}

type TwoConditions = unsafe extern "system" fn(Raw, Raw, Raw, *mut Raw) -> i32;

/// IUIAutomation's method table, as far as CreateOrCondition (slot 28).
#[repr(C)]
struct AutomationVtbl {
    unknown: [usize; 3],
    /// CompareElements, CompareRuntimeIds, GetRootElement.
    slots_3_to_5: [usize; 3],
    element_from_handle: unsafe extern "system" fn(Raw, isize, *mut Raw) -> i32,
    /// ElementFromPoint to CreateFalseCondition.
    slots_7_to_22: [usize; 16],
    create_property_condition: unsafe extern "system" fn(Raw, i32, Variant, *mut Raw) -> i32,
    /// CreatePropertyConditionEx.
    slot_24: usize,
    create_and_condition: TwoConditions,
    /// CreateAndConditionFromArray, CreateAndConditionFromNativeArray.
    slots_26_to_27: [usize; 2],
    create_or_condition: TwoConditions,
}

/// IUIAutomationElement's method table, as far as get_CurrentIsOffscreen
/// (slot 38).
#[repr(C)]
struct ElementVtbl {
    unknown: [usize; 3],
    /// SetFocus, GetRuntimeId.
    slots_3_to_4: [usize; 2],
    find_first: unsafe extern "system" fn(Raw, i32, Raw, *mut Raw) -> i32,
    /// FindAll to get_CurrentItemType.
    slots_6_to_37: [usize; 32],
    is_offscreen: unsafe extern "system" fn(Raw, *mut i32) -> i32,
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, model: u32) -> i32;
    fn CoUninitialize();
    fn CoCreateInstance(
        class: *const Guid,
        outer: Raw,
        context: u32,
        interface: *const Guid,
        out: *mut Raw,
    ) -> i32;
}

#[link(name = "oleaut32")]
extern "system" {
    fn SysAllocString(text: *const u16) -> *mut u16;
    fn SysFreeString(text: *mut u16);
}

/// CUIAutomation, ff48dba4-60ef-4201-aa87-54103eef594e.
const CLSID_AUTOMATION: Guid = Guid {
    a: 0xff48_dba4,
    b: 0x60ef,
    c: 0x4201,
    d: [0xaa, 0x87, 0x54, 0x10, 0x3e, 0xef, 0x59, 0x4e],
};
/// IUIAutomation, 30cbe57d-d9d0-452a-ab13-7ac5ac4825ee.
const IID_AUTOMATION: Guid = Guid {
    a: 0x30cb_e57d,
    b: 0xd9d0,
    c: 0x452a,
    d: [0xab, 0x13, 0x7a, 0xc5, 0xac, 0x48, 0x25, 0xee],
};
const COINIT_MULTITHREADED: u32 = 0;
const CLSCTX_INPROC_SERVER: u32 = 1;
const VT_I4: u16 = 3;
const VT_BSTR: u16 = 8;
const UIA_CONTROL_TYPE_PROPERTY: i32 = 30003;
const UIA_NAME_PROPERTY: i32 = 30005;
const UIA_BUTTON_CONTROL_TYPE: usize = 50000;
const TREE_SCOPE_DESCENDANTS: i32 = 4;

/// Lets go of a COM object.
///
/// # Safety
/// `object` is null or a live COM interface pointer this code owns.
unsafe fn release(object: Raw) {
    if object.is_null() {
        return;
    }
    // SAFETY: a COM object starts with a pointer to its method table, whose
    // third entry is Release.
    unsafe {
        let table = *(object as *const *const UnknownVtbl);
        ((*table).release)(object);
    }
}

/// A COM object this code owns, released when dropped.
struct Com(Raw);

impl Com {
    /// What a COM call handed back: the object, when the call succeeded
    /// and gave one.
    fn got(hr: i32, object: Raw) -> Option<Com> {
        (hr >= 0 && !object.is_null()).then_some(Com(object))
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        // SAFETY: the pointer came from a COM call that succeeded.
        unsafe { release(self.0) }
    }
}

/// A BSTR, freed when dropped.
struct Bstr(*mut u16);

impl Bstr {
    fn new(text: &str) -> Option<Bstr> {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: a NUL-terminated string, which the call copies.
        let made = unsafe { SysAllocString(wide.as_ptr()) };
        (!made.is_null()).then_some(Bstr(made))
    }
}

impl Drop for Bstr {
    fn drop(&mut self) {
        // SAFETY: allocated by SysAllocString and not freed since.
        unsafe { SysFreeString(self.0) }
    }
}

/// UI Automation, on the thread that made it. It holds a raw pointer, so it
/// cannot be sent to another thread.
struct Uia {
    automation: Raw,
}

impl Uia {
    fn new() -> Option<Uia> {
        // SAFETY: COM is started on this thread and, when the object cannot
        // be made, stopped again; the out pointer is valid.
        unsafe {
            if CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED) < 0 {
                return None;
            }
            let mut automation: Raw = std::ptr::null_mut();
            let hr = CoCreateInstance(
                &CLSID_AUTOMATION,
                std::ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_AUTOMATION,
                &mut automation,
            );
            if hr < 0 || automation.is_null() {
                CoUninitialize();
                return None;
            }
            Some(Uia { automation })
        }
    }

    fn table(&self) -> &AutomationVtbl {
        // SAFETY: `automation` is a live IUIAutomation, which starts with a
        // pointer to its method table.
        unsafe { &**(self.automation as *const *const AutomationVtbl) }
    }

    /// "This property has this value."
    fn property(&self, id: i32, value: Variant) -> Option<Com> {
        let mut out: Raw = std::ptr::null_mut();
        // SAFETY: slot 23 of a live IUIAutomation; the call copies the value.
        let hr = unsafe {
            (self.table().create_property_condition)(self.automation, id, value, &mut out)
        };
        Com::got(hr, out)
    }

    /// Both conditions (`all`), or either of them.
    fn pair(&self, all: bool, a: &Com, b: &Com) -> Option<Com> {
        let table = self.table();
        let make = if all {
            table.create_and_condition
        } else {
            table.create_or_condition
        };
        let mut out: Raw = std::ptr::null_mut();
        // SAFETY: slot 25 or 28 of a live IUIAutomation, with two live
        // conditions.
        let hr = unsafe { make(self.automation, a.0, b.0, &mut out) };
        Com::got(hr, out)
    }

    /// A button with one of the Stop button's names.
    fn stop_button(&self) -> Option<Com> {
        let mut names: Option<Com> = None;
        for name in STOP_NAMES {
            let text = Bstr::new(name)?;
            let value = Variant {
                kind: VT_BSTR,
                reserved: [0; 3],
                value: [text.0 as usize, 0],
            };
            let one = self.property(UIA_NAME_PROPERTY, value)?;
            names = Some(match names {
                Some(so_far) => self.pair(false, &so_far, &one)?,
                None => one,
            });
        }
        let button = Variant {
            kind: VT_I4,
            reserved: [0; 3],
            value: [UIA_BUTTON_CONTROL_TYPE, 0],
        };
        let is_button = self.property(UIA_CONTROL_TYPE_PROPERTY, button)?;
        self.pair(true, &is_button, &names?)
    }

    /// Whether the window shows a Stop button; None when it could not be
    /// read (the window closed mid-read, or the app is not answering).
    fn generating(&self, hwnd: i64) -> Option<bool> {
        let mut root: Raw = std::ptr::null_mut();
        // SAFETY: slot 6 of a live IUIAutomation.
        let hr = unsafe {
            (self.table().element_from_handle)(self.automation, hwnd as isize, &mut root)
        };
        let root = Com::got(hr, root)?;
        let button = self.stop_button()?;
        let mut found: Raw = std::ptr::null_mut();
        // SAFETY: `root` is a live IUIAutomationElement, which starts with a
        // pointer to its method table; slot 5 is FindFirst.
        let hr = unsafe {
            let table = &**(root.0 as *const *const ElementVtbl);
            (table.find_first)(root.0, TREE_SCOPE_DESCENDANTS, button.0, &mut found)
        };
        if hr < 0 {
            return None;
        }
        if found.is_null() {
            return Some(false);
        }
        let found = Com(found);
        let mut offscreen = 0i32;
        // SAFETY: as above; slot 38 is get_CurrentIsOffscreen.
        let hr = unsafe {
            let table = &**(found.0 as *const *const ElementVtbl);
            (table.is_offscreen)(found.0, &mut offscreen)
        };
        (hr >= 0).then_some(offscreen == 0)
    }
}

impl Drop for Uia {
    fn drop(&mut self) {
        // SAFETY: the object is released before COM is stopped on the thread
        // that started it.
        unsafe {
            release(self.automation);
            CoUninitialize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::sessions::{CardKind, SessionBook};
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn a_stop_button_starts_a_session_and_two_misses_end_it() {
        let mut t = Tracker::default();
        let idle = t.observe(5, 100, "Claude", true, Some(false), false);
        assert_eq!(idle, None, "an idle window is ignored");
        let start = t.observe(5, 100, "Claude", true, Some(true), false);
        let start = start.expect("the Stop button starts a session");
        assert_eq!(start.name, "UserPromptSubmit");
        assert_eq!(start.session_id, "chat:5");
        assert_eq!(
            (start.agent.as_str(), start.label.as_str()),
            ("chat", "Chat")
        );
        assert_eq!(
            (start.host.as_str(), start.hwnd, start.pid),
            ("Claude", 5, 100)
        );
        assert!(t.is_watching(5));
        assert_eq!(t.watched(), [5]);
        // Still writing: nothing new to say.
        assert_eq!(t.observe(5, 100, "Claude", true, Some(true), false), None);
        // A read that failed is not a miss, and one miss is not the end.
        assert_eq!(t.observe(5, 100, "Claude", true, None, false), None);
        assert_eq!(t.observe(5, 100, "Claude", true, Some(false), false), None);
        // The button coming back starts the count again.
        assert_eq!(t.observe(5, 100, "Claude", true, Some(true), false), None);
        assert_eq!(t.observe(5, 100, "Claude", true, Some(false), false), None);
        let done = t.observe(5, 100, "Claude", true, Some(false), false);
        assert_eq!(done.map(|e| e.name), Some("Stop".to_string()));
        assert!(!t.is_watching(5));
    }

    #[test]
    fn a_reply_you_watched_finish_or_a_closed_window_shows_no_card() {
        let mut t = Tracker::default();
        t.observe(6, 200, "ChatGPT", true, Some(true), false);
        t.observe(6, 200, "ChatGPT", true, Some(false), true);
        let seen = t.observe(6, 200, "ChatGPT", true, Some(false), true);
        assert_eq!(seen.map(|e| e.name), Some("Interrupt".to_string()));
        t.observe(7, 300, "ChatGPT", true, Some(true), false);
        let closed = t.observe(7, 300, "", false, None, false);
        assert_eq!(closed.map(|e| e.name), Some("SessionEnd".to_string()));
        // A window that was never watched closing says nothing.
        assert_eq!(t.observe(8, 400, "", false, None, false), None);
    }

    #[test]
    fn the_card_reads_chat_finished_and_names_the_app() {
        let mut book = SessionBook::default();
        let mut t = Tracker::default();
        let start = t.observe(9, 1, "Claude", true, Some(true), false);
        assert_eq!(book.apply(&start.expect("a start"), 1_000), None);
        assert_eq!(book.busy_count(), 1);
        // The chat watcher leaves a window alone only for a hook session.
        assert!(!book.hook_busy_in(9));
        t.observe(9, 1, "Claude", true, Some(false), false);
        let stop = t.observe(9, 1, "Claude", true, Some(false), false);
        let card = book.apply(&stop.expect("a stop"), 2_000).expect("a card");
        assert_eq!(card.kind, CardKind::Done);
        assert_eq!(card.title, "Chat finished");
        assert_eq!(card.host, "Claude");
        assert_eq!(card.detail, "Ready for your next prompt.");
        assert!(!card.background && !card.wants_snippet);
        assert_eq!(book.busy_count(), 0);
        assert!(book.forget_agent("chat"));
        assert!(book.is_empty() && !book.forget_agent("chat"));
    }

    #[test]
    fn only_the_two_chat_apps_count() {
        assert_eq!(app_of("claude.exe"), Some("Claude"));
        assert_eq!(app_of("Claude.exe"), Some("Claude"));
        assert_eq!(app_of("ChatGPT.exe"), Some("ChatGPT"));
        assert_eq!(app_of("Code.exe"), None);
        assert_eq!(app_of(""), None);
    }

    #[test]
    fn leaving_a_window_is_noticed_once() {
        let mut last = 0;
        assert_eq!(switched(&mut last, 10), None, "nothing was in front");
        assert_eq!(switched(&mut last, 10), None, "the same window still is");
        assert_eq!(switched(&mut last, 20), Some(10));
        assert_eq!(switched(&mut last, 0), Some(20));
        assert_eq!(last, 0);
    }

    #[test]
    fn the_method_tables_put_each_call_in_its_slot() {
        let slot = size_of::<usize>();
        assert_eq!(offset_of!(UnknownVtbl, release), 2 * slot);
        assert_eq!(offset_of!(AutomationVtbl, element_from_handle), 6 * slot);
        assert_eq!(
            offset_of!(AutomationVtbl, create_property_condition),
            23 * slot
        );
        assert_eq!(offset_of!(AutomationVtbl, create_and_condition), 25 * slot);
        assert_eq!(offset_of!(AutomationVtbl, create_or_condition), 28 * slot);
        assert_eq!(offset_of!(ElementVtbl, find_first), 5 * slot);
        assert_eq!(offset_of!(ElementVtbl, is_offscreen), 38 * slot);
        // A VARIANT is 24 bytes on 64-bit Windows, 16 on 32-bit.
        assert_eq!(size_of::<Variant>(), 8 + 2 * slot);
        assert_eq!(size_of::<Guid>(), 16);
    }
}
