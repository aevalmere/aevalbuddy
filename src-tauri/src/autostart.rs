//! Launch at login (PARITY SYS-03, IPC-32).
//!
//! The HKCU Run value Electron's `setLoginItemSettings` wrote: named after the
//! AppUserModelID, holding the quoted exe path with no arguments, so an
//! upgrade from the Electron build replaces its entry instead of adding a
//! second one. The StartupApproved key (Task Manager's switch) is never
//! touched: a person who disabled startup there stays disabled.

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
use winreg::RegKey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const VALUE_NAME: &str = "com.aevalbuddy.app";

/// Whether this build writes the Run key at all. A dev build would register
/// a target\debug exe, so it only remembers the choice (`launchAtLoginLive`).
pub fn live() -> bool {
    !cfg!(debug_assertions)
}

/// The value Electron wrote: the exe path in quotes.
pub fn run_value(exe: &std::path::Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Writes or removes the Run value to match the stored choice; returns the
/// choice. A locked-down profile that refuses the key is not an error.
pub fn apply() -> bool {
    let on = crate::store::get().get_launch_at_login();
    if !live() {
        return on;
    }
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(run) = hkcu.open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE | KEY_READ) else {
        return on;
    };
    if on {
        if let Ok(exe) = std::env::current_exe() {
            let _ = run.set_value(VALUE_NAME, &run_value(&exe));
        }
    } else {
        let _ = run.delete_value(VALUE_NAME);
    }
    on
}

/// What Windows holds now, for the smoke run.
pub fn registered() -> Option<String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ).ok()?;
    run.get_value::<String, _>(VALUE_NAME).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_is_the_quoted_exe() {
        let v = run_value(std::path::Path::new(r"C:\Apps\AevalBuddy\AevalBuddy.exe"));
        assert_eq!(v, r#""C:\Apps\AevalBuddy\AevalBuddy.exe""#);
    }
}
