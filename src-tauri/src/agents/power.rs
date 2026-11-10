//! Keeping the PC on while agents work. A named power request stops sleep
//! (and, when asked, keeps the screen on); `powercfg /requests` shows it as
//! "Aevaleron: AI agents are running". Power requests do not cover closing
//! the lid, which is a power-plan setting: while agents run (and only if the
//! person turned that on) the lid action is set to "Do nothing", and the
//! values it had are written back afterwards. Port of Aevalsistant's
//! `Power.cs`.
//!
//! The plan's lid action only runs when the lid moves. So when the last
//! agent stops with the lid already shut, this app follows the lid
//! (`watch_lid`) and, a minute later, does what closing it would have done.

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering::SeqCst};

#[repr(C)]
struct ReasonContext {
    version: u32,
    flags: u32,
    /// The union's simple-string member; the detailed member is three
    /// pointers long.
    reason: *const u16,
    rest: [usize; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Guid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}

/// What `PowerSettingRegisterNotification` takes in place of a window: the
/// function to call, and a value handed back to it.
#[repr(C)]
struct Subscribe {
    callback: unsafe extern "system" fn(usize, u32, *const SettingChange) -> u32,
    context: usize,
}

/// The head of a POWERBROADCAST_SETTING; the value's bytes follow it.
#[repr(C)]
struct SettingChange {
    setting: Guid,
    length: u32,
}

/// TOKEN_PRIVILEGES with room for one privilege.
#[repr(C)]
struct TokenPrivilege {
    count: u32,
    luid: [u32; 2],
    attributes: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn PowerCreateRequest(context: *const ReasonContext) -> isize;
    fn PowerSetRequest(request: isize, kind: i32) -> i32;
    fn PowerClearRequest(request: isize, kind: i32) -> i32;
    fn CloseHandle(handle: isize) -> i32;
    fn LocalFree(memory: isize) -> isize;
    fn GetSystemPowerStatus(status: *mut u8) -> i32;
    fn GetCurrentProcess() -> isize;
}

#[link(name = "powrprof")]
extern "system" {
    fn PowerGetActiveScheme(root: isize, scheme: *mut *mut Guid) -> u32;
    fn PowerReadACValueIndex(
        root: isize,
        scheme: *const Guid,
        group: *const Guid,
        setting: *const Guid,
        value: *mut u32,
    ) -> u32;
    fn PowerReadDCValueIndex(
        root: isize,
        scheme: *const Guid,
        group: *const Guid,
        setting: *const Guid,
        value: *mut u32,
    ) -> u32;
    fn PowerWriteACValueIndex(
        root: isize,
        scheme: *const Guid,
        group: *const Guid,
        setting: *const Guid,
        value: u32,
    ) -> u32;
    fn PowerWriteDCValueIndex(
        root: isize,
        scheme: *const Guid,
        group: *const Guid,
        setting: *const Guid,
        value: u32,
    ) -> u32;
    fn PowerSetActiveScheme(root: isize, scheme: *const Guid) -> u32;
    fn GetPwrCapabilities(capabilities: *mut u8) -> u8;
    fn PowerSettingRegisterNotification(
        setting: *const Guid,
        flags: u32,
        recipient: *const Subscribe,
        registration: *mut isize,
    ) -> u32;
    // Its three arguments and its result are one-byte BOOLEANs.
    fn SetSuspendState(hibernate: u8, force: u8, wake_events_off: u8) -> u8;
}

#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(process: isize, access: u32, token: *mut isize) -> i32;
    fn LookupPrivilegeValueW(system: *const u16, name: *const u16, luid: *mut [u32; 2]) -> i32;
    fn AdjustTokenPrivileges(
        token: isize,
        disable_all: i32,
        state: *const TokenPrivilege,
        length: u32,
        previous: *mut TokenPrivilege,
        returned: *mut u32,
    ) -> i32;
}

const DISPLAY_REQUIRED: i32 = 0;
const SYSTEM_REQUIRED: i32 = 1;
const SIMPLE_STRING: u32 = 1;

/// "Power buttons and lid" in a power plan.
const SUB_BUTTONS: Guid = Guid {
    a: 0x4f97_1e89,
    b: 0xeebd,
    c: 0x4455,
    d: [0xa8, 0xde, 0x9e, 0x59, 0x04, 0x0e, 0x73, 0x47],
};
/// "Lid close action": 0 do nothing, 1 sleep, 2 hibernate, 3 shut down.
const LID_CLOSE: Guid = Guid {
    a: 0x5ca8_3367,
    b: 0x6e45,
    c: 0x459f,
    d: [0xa2, 0x7b, 0x47, 0x6b, 0x1d, 0x01, 0xc9, 0x36],
};
/// GUID_LIDSWITCH_STATE_CHANGE: the value is 0 with the lid shut, 1 open.
const LID_SWITCH: Guid = Guid {
    a: 0xba3e_0f4d,
    b: 0xb817,
    c: 0x4094,
    d: [0xa2, 0xd1, 0xd5, 0x63, 0x79, 0xe6, 0xa0, 0xf3],
};
const PBT_POWERSETTINGCHANGE: u32 = 0x8013;
const DEVICE_NOTIFY_CALLBACK: u32 = 2;
const TOKEN_ADJUST_PRIVILEGES: u32 = 0x20;
const TOKEN_QUERY: u32 = 0x8;
const SE_PRIVILEGE_ENABLED: u32 = 0x2;

impl Guid {
    fn text(&self) -> String {
        let d = self.d;
        format!(
            "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.a, self.b, self.c, d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
        )
    }

    fn parse(text: &str) -> Option<Guid> {
        let hex: String = text.chars().filter(|c| *c != '-').collect();
        if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let mut d = [0u8; 8];
        for (i, byte) in d.iter_mut().enumerate() {
            let at = 16 + i * 2;
            *byte = u8::from_str_radix(&hex[at..at + 2], 16).ok()?;
        }
        Some(Guid {
            a: u32::from_str_radix(&hex[..8], 16).ok()?,
            b: u16::from_str_radix(&hex[8..12], 16).ok()?,
            c: u16::from_str_radix(&hex[12..16], 16).ok()?,
            d,
        })
    }
}

/// The power request. One for the life of the app; `set` turns its two
/// halves on and off.
pub struct Awake {
    handle: isize,
    system: bool,
    display: bool,
}

impl Default for Awake {
    fn default() -> Self {
        Awake::new()
    }
}

impl Awake {
    pub fn new() -> Awake {
        let reason: Vec<u16> = "Aevaleron: AI agents are running"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let context = ReasonContext {
            version: 0,
            flags: SIMPLE_STRING,
            reason: reason.as_ptr(),
            rest: [0; 2],
        };
        // SAFETY: the context and its string live through the call, which
        // copies them.
        let handle = unsafe { PowerCreateRequest(&context) };
        Awake {
            handle,
            system: false,
            display: false,
        }
    }

    fn ok(&self) -> bool {
        self.handle != 0 && self.handle != -1
    }

    /// Keeps the PC from sleeping (`system`) and the screen from turning
    /// off (`display`), or lets go. True when something changed.
    pub fn set(&mut self, system: bool, display: bool) -> bool {
        if !self.ok() || (system == self.system && display == self.display) {
            return false;
        }
        let flip = |on: bool, kind: i32| {
            // SAFETY: a request handle this struct owns.
            unsafe {
                if on {
                    PowerSetRequest(self.handle, kind);
                } else {
                    PowerClearRequest(self.handle, kind);
                }
            }
        };
        if system != self.system {
            flip(system, SYSTEM_REQUIRED);
        }
        if display != self.display {
            flip(display, DISPLAY_REQUIRED);
        }
        self.system = system;
        self.display = display;
        true
    }

    pub fn holding(&self) -> (bool, bool) {
        (self.system, self.display)
    }
}

impl Drop for Awake {
    fn drop(&mut self) {
        self.set(false, false);
        if self.ok() {
            // SAFETY: the handle is ours and still open.
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

fn active_scheme() -> Option<Guid> {
    let mut p: *mut Guid = std::ptr::null_mut();
    // SAFETY: on success `p` points at a GUID the API allocated, freed here.
    unsafe {
        if PowerGetActiveScheme(0, &mut p) != 0 || p.is_null() {
            return None;
        }
        let scheme = *p;
        LocalFree(p as isize);
        Some(scheme)
    }
}

fn write_lid(scheme: Guid, ac: u32, dc: u32) -> bool {
    // SAFETY: plain value writes into the plan `scheme` names.
    unsafe {
        if PowerWriteACValueIndex(0, &scheme, &SUB_BUTTONS, &LID_CLOSE, ac) != 0 {
            return false;
        }
        if PowerWriteDCValueIndex(0, &scheme, &SUB_BUTTONS, &LID_CLOSE, dc) != 0 {
            return false;
        }
        // Applying the plan again is what makes the new value take effect.
        if active_scheme() == Some(scheme) {
            PowerSetActiveScheme(0, &scheme);
        }
    }
    true
}

/// Sets the lid action to "Do nothing". Returns what to keep so it can be
/// put back: `scheme|ac|dc`.
pub fn override_lid() -> Result<String, String> {
    let scheme = active_scheme().ok_or("Could not read the active power plan.")?;
    let (mut ac, mut dc) = (0u32, 0u32);
    // SAFETY: valid out pointers.
    let read = unsafe {
        PowerReadACValueIndex(0, &scheme, &SUB_BUTTONS, &LID_CLOSE, &mut ac) == 0
            && PowerReadDCValueIndex(0, &scheme, &SUB_BUTTONS, &LID_CLOSE, &mut dc) == 0
    };
    if !read {
        return Err("Could not read the lid setting.".into());
    }
    if !write_lid(scheme, 0, 0) {
        // The plugged-in half may have been written before the battery half
        // failed; put it back so the plan is not left changed with nothing
        // saved to restore it from.
        write_lid(scheme, ac, dc);
        return Err("Windows refused the lid setting change.".into());
    }
    Ok(saved_text(scheme, ac, dc))
}

fn saved_text(scheme: Guid, ac: u32, dc: u32) -> String {
    format!("{}|{ac}|{dc}", scheme.text())
}

fn parse_saved(saved: &str) -> Option<(Guid, u32, u32)> {
    let mut parts = saved.split('|');
    let scheme = Guid::parse(parts.next()?)?;
    let ac = parts.next()?.parse().ok()?;
    let dc = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((scheme, ac, dc))
}

/// Writes the lid action back as `override_lid` found it. True when there
/// is nothing left to restore.
pub fn restore_lid(saved: &str) -> bool {
    match parse_saved(saved) {
        Some((scheme, ac, dc)) => write_lid(scheme, ac, dc),
        None => true,
    }
}

/// Whether this PC has a lid at all. SYSTEM_POWER_CAPABILITIES starts with
/// three BOOLEANs: power button, sleep button, lid.
pub fn has_lid() -> bool {
    let mut caps = [0u8; 256];
    // SAFETY: the buffer is larger than the struct.
    unsafe { GetPwrCapabilities(caps.as_mut_ptr()) != 0 && caps[2] != 0 }
}

// --------------------------------------------------------------------- lid

const LID_UNKNOWN: u8 = 0;
const LID_OPEN: u8 = 1;
const LID_SHUT: u8 = 2;

/// Where the lid is, as Windows last reported it.
static LID: AtomicU8 = AtomicU8::new(LID_UNKNOWN);
/// Moves on whenever a sleep that was waiting is no longer wanted: the lid
/// opened, an agent started working, or a newer sleep was planned.
static LID_EPOCH: AtomicU64 = AtomicU64::new(0);

static LID_SUBSCRIBE: Subscribe = Subscribe {
    callback: on_power_setting,
    context: 0,
};

/// What a power-setting report says about the lid: `Some(true)` shut,
/// `Some(false)` open, `None` when it is about something else. `first` is
/// the first byte of the value (a DWORD: 0 shut, 1 open).
fn lid_report(setting: Guid, length: u32, first: u8) -> Option<bool> {
    (setting == LID_SWITCH && length >= 1).then_some(first == 0)
}

fn note_lid(shut: bool) {
    LID.store(if shut { LID_SHUT } else { LID_OPEN }, SeqCst);
    if !shut {
        // Opening the lid calls off a sleep that was waiting.
        LID_EPOCH.fetch_add(1, SeqCst);
    }
}

/// Windows calls this, on a thread of its own, with the lid's state right
/// after `watch_lid` and then on every change.
unsafe extern "system" fn on_power_setting(
    _context: usize,
    kind: u32,
    change: *const SettingChange,
) -> u32 {
    if kind != PBT_POWERSETTINGCHANGE || change.is_null() {
        return 0;
    }
    // SAFETY: for this kind of report Windows passes a POWERBROADCAST_SETTING,
    // whose value starts right after its head and is `length` bytes long.
    let (setting, length, first) = unsafe {
        let head = &*change;
        let first = if head.length >= 1 {
            *change.add(1).cast::<u8>()
        } else {
            0
        };
        (head.setting, head.length, first)
    };
    if let Some(shut) = lid_report(setting, length, first) {
        note_lid(shut);
    }
    0
}

/// Starts following the lid, for the life of the app. No window is needed:
/// Windows calls `on_power_setting` directly.
pub fn watch_lid() {
    let mut registration = 0isize;
    // SAFETY: the subscription is a static, so the function it names is there
    // for as long as the registration, which is never taken back.
    unsafe {
        PowerSettingRegisterNotification(
            &LID_SWITCH,
            DEVICE_NOTIFY_CALLBACK,
            &LID_SUBSCRIBE,
            &mut registration,
        );
    }
}

/// Whether the lid is shut. False until Windows has said.
pub fn lid_shut() -> bool {
    LID.load(SeqCst) == LID_SHUT
}

/// Plans a sleep; the token stays good until the lid opens, an agent works
/// again or another sleep is planned.
pub fn plan_lid_sleep() -> u64 {
    LID_EPOCH.fetch_add(1, SeqCst) + 1
}

/// Calls off a sleep that is waiting.
pub fn cancel_lid_sleep() {
    LID_EPOCH.fetch_add(1, SeqCst);
}

pub fn lid_sleep_planned(token: u64) -> bool {
    LID_EPOCH.load(SeqCst) == token
}

/// What closing the lid would have done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LidSleep {
    Sleep,
    Hibernate,
}

/// The person's own lid action from before `override_lid`, for the power
/// source in use: 0 do nothing, 1 sleep, 2 hibernate, 3 shut down. None
/// when `saved` is not a string `override_lid` made.
pub fn saved_action(saved: &str, on_battery: bool) -> Option<u32> {
    let (_, ac, dc) = parse_saved(saved)?;
    Some(if on_battery { dc } else { ac })
}

/// What to do when the lid override has just been lifted. Nothing while an
/// agent still works or with the lid open, and nothing the person's own lid
/// setting would not have done: only sleep and hibernate are carried out.
pub fn lid_plan(action: Option<u32>, busy: bool, shut: bool) -> Option<LidSleep> {
    if busy || !shut {
        return None;
    }
    match action {
        Some(1) => Some(LidSleep::Sleep),
        Some(2) => Some(LidSleep::Hibernate),
        _ => None,
    }
}

/// What a planned sleep is checked against when its minute is up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LidCheck {
    /// Keep-awake with the lid closed is still switched on.
    pub on: bool,
    /// An agent is working again.
    pub busy: bool,
    pub shut: bool,
    /// Nothing called the sleep off meanwhile.
    pub planned: bool,
    /// Someone used the keyboard or mouse during the wait, so the PC is in
    /// use with its lid shut (docked to a screen).
    pub in_use: bool,
}

/// Whether the PC goes to sleep now. Never with the lid open, an agent
/// working, the setting off, or someone at the keyboard.
pub fn lid_sleep_due(c: LidCheck) -> bool {
    c.on && !c.busy && c.shut && c.planned && !c.in_use
}

/// ACLineStatus is 0 on battery, 1 plugged in, 255 unknown; unknown counts
/// as plugged in.
pub fn on_battery() -> bool {
    // SYSTEM_POWER_STATUS is 12 bytes and starts with ACLineStatus.
    let mut status = [0u8; 12];
    // SAFETY: the buffer is the size of the struct.
    unsafe { GetSystemPowerStatus(status.as_mut_ptr()) != 0 && status[0] == 0 }
}

/// SetSuspendState needs SeShutdownPrivilege enabled, not just held. People
/// hold it on Windows client editions, but it starts out disabled in every
/// process. If this fails, SetSuspendState fails too.
fn enable_shutdown_privilege() {
    let name: Vec<u16> = "SeShutdownPrivilege"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut token = 0isize;
    // SAFETY: valid out pointers, a NUL-terminated name, and a token handle
    // that is closed before returning.
    unsafe {
        let access = TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY;
        if OpenProcessToken(GetCurrentProcess(), access, &mut token) == 0 {
            return;
        }
        let mut state = TokenPrivilege {
            count: 1,
            luid: [0; 2],
            attributes: SE_PRIVILEGE_ENABLED,
        };
        if LookupPrivilegeValueW(std::ptr::null(), name.as_ptr(), &mut state.luid) != 0 {
            AdjustTokenPrivileges(
                token,
                0,
                &state,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
        }
        CloseHandle(token);
    }
}

/// Sleeps or hibernates now, as closing the lid would have, with wake
/// timers left working. False when Windows refused.
pub fn suspend(how: LidSleep) -> bool {
    enable_shutdown_privilege();
    let hibernate = u8::from(how == LidSleep::Hibernate);
    // SAFETY: three plain values.
    unsafe { SetSuspendState(hibernate, 0, 0) != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lid_report_is_read_from_its_first_byte() {
        assert_eq!(lid_report(LID_SWITCH, 4, 0), Some(true));
        assert_eq!(lid_report(LID_SWITCH, 4, 1), Some(false));
        // Another setting, or a report with no value in it.
        assert_eq!(lid_report(LID_CLOSE, 4, 0), None);
        assert_eq!(lid_report(LID_SWITCH, 0, 0), None);
        assert_eq!(LID_SWITCH.text(), "ba3e0f4d-b817-4094-a2d1-d56379e6a0f3");
        assert_eq!(std::mem::size_of::<SettingChange>(), 20);
        assert_eq!(std::mem::size_of::<TokenPrivilege>(), 16);
    }

    #[test]
    fn the_saved_lid_action_is_read_per_power_source() {
        let saved = saved_text(SUB_BUTTONS, 1, 2);
        assert_eq!(saved_action(&saved, false), Some(1));
        assert_eq!(saved_action(&saved, true), Some(2));
        for bad in ["", "junk", "x|1|2"] {
            assert_eq!(saved_action(bad, true), None, "{bad}");
        }
    }

    #[test]
    fn sleep_is_planned_only_for_a_shut_lid_and_nothing_working() {
        assert_eq!(lid_plan(Some(1), false, true), Some(LidSleep::Sleep));
        assert_eq!(lid_plan(Some(2), false, true), Some(LidSleep::Hibernate));
        // Their own setting was "do nothing" or "shut down", or unknown.
        for action in [Some(0), Some(3), Some(9), None] {
            assert_eq!(lid_plan(action, false, true), None);
        }
        assert_eq!(lid_plan(Some(1), true, true), None, "an agent works");
        assert_eq!(lid_plan(Some(1), false, false), None, "the lid is open");
    }

    #[test]
    fn the_sleep_is_checked_again_when_its_minute_is_up() {
        let due = LidCheck {
            on: true,
            busy: false,
            shut: true,
            planned: true,
            in_use: false,
        };
        assert!(lid_sleep_due(due));
        assert!(!lid_sleep_due(LidCheck { on: false, ..due }));
        assert!(!lid_sleep_due(LidCheck { busy: true, ..due }));
        assert!(!lid_sleep_due(LidCheck { shut: false, ..due }));
        assert!(!lid_sleep_due(LidCheck {
            planned: false,
            ..due
        }));
        assert!(!lid_sleep_due(LidCheck {
            in_use: true,
            ..due
        }));
    }

    #[test]
    fn opening_the_lid_or_new_work_calls_a_planned_sleep_off() {
        let first = plan_lid_sleep();
        assert!(lid_sleep_planned(first));
        note_lid(true);
        assert!(
            lid_shut() && lid_sleep_planned(first),
            "shutting it keeps the plan"
        );
        note_lid(false);
        assert!(!lid_shut() && !lid_sleep_planned(first));
        let second = plan_lid_sleep();
        cancel_lid_sleep();
        assert!(!lid_sleep_planned(second));
        // A newer plan replaces an older one.
        let third = plan_lid_sleep();
        let fourth = plan_lid_sleep();
        assert!(!lid_sleep_planned(third) && lid_sleep_planned(fourth));
        // Asking about the power source never fails the caller.
        let _ = on_battery();
    }

    #[test]
    fn saved_lid_values_round_trip() {
        let text = saved_text(SUB_BUTTONS, 1, 2);
        assert_eq!(text, "4f971e89-eebd-4455-a8de-9e59040e7347|1|2");
        assert_eq!(parse_saved(&text), Some((SUB_BUTTONS, 1, 2)));
        assert_eq!(Guid::parse(&LID_CLOSE.text()), Some(LID_CLOSE));
        for bad in [
            "",
            "x|1|2",
            "4f971e89-eebd-4455-a8de-9e59040e7347|1",
            &format!("{text}|3"),
        ] {
            assert_eq!(parse_saved(bad), None, "{bad}");
        }
        // Nothing saved: nothing to restore, and nothing is written.
        assert!(restore_lid(""));
    }

    #[test]
    fn the_request_follows_what_is_asked() {
        let mut awake = Awake::new();
        assert_eq!(awake.holding(), (false, false));
        if awake.ok() {
            assert!(awake.set(true, false));
            assert!(!awake.set(true, false), "the same again changes nothing");
            assert!(awake.set(true, true));
            assert_eq!(awake.holding(), (true, true));
            assert!(awake.set(false, false));
        }
        // Asking about the lid never fails the caller.
        let _ = has_lid();
    }
}
