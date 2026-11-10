//! Keeping the PC on while agents work. A named power request stops sleep
//! (and, when asked, keeps the screen on); `powercfg /requests` shows it as
//! "AevalBuddy: AI agents are running". Power requests do not cover closing
//! the lid, which is a power-plan setting: while agents run (and only if the
//! person turned that on) the lid action is set to "Do nothing", and the
//! values it had are written back afterwards. Port of Aevalsistant's
//! `Power.cs`.

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

#[link(name = "kernel32")]
extern "system" {
    fn PowerCreateRequest(context: *const ReasonContext) -> isize;
    fn PowerSetRequest(request: isize, kind: i32) -> i32;
    fn PowerClearRequest(request: isize, kind: i32) -> i32;
    fn CloseHandle(handle: isize) -> i32;
    fn LocalFree(memory: isize) -> isize;
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
        let reason: Vec<u16> = "AevalBuddy: AI agents are running"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_lid_values_round_trip() {
        let text = saved_text(SUB_BUTTONS, 1, 2);
        assert_eq!(text, "4f971e89-eebd-4455-a8de-9e59040e7347|1|2");
        assert_eq!(parse_saved(&text), Some((SUB_BUTTONS, 1, 2)));
        assert_eq!(Guid::parse(&LID_CLOSE.text()), Some(LID_CLOSE));
        for bad in ["", "x|1|2", "4f971e89-eebd-4455-a8de-9e59040e7347|1", &format!("{text}|3")] {
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
