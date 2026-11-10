//! Launch at login (PARITY SYS-03, IPC-32).
//!
//! The HKCU Run value Electron's `setLoginItemSettings` wrote: named after the
//! AppUserModelID, holding the quoted exe path with no arguments, so an
//! upgrade from the Electron build replaces its entry instead of adding a
//! second one. The StartupApproved value (Task Manager's switch) is written
//! only when the person turns the setting on in Settings (B-081, D-11): at
//! boot it is left alone, so a person who disabled startup in Task Manager
//! stays disabled.

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_BINARY};
use winreg::reg_value::RegValue;
use winreg::RegKey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
pub const VALUE_NAME: &str = "com.aevaleron.app";
/// The value's name while the app was AevalBuddy. Its exe is gone after the
/// upgrade, so the entry is removed whatever the setting says.
const OLD_VALUE_NAME: &str = "com.aevalbuddy.app";
/// Task Manager's "Enabled": 02 then eleven zero bytes.
pub const APPROVED_ON: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

/// Whether this build writes the Run key at all. A dev build would register
/// a target\debug exe, so it only remembers the choice (`launchAtLoginLive`).
pub fn live() -> bool {
    !cfg!(debug_assertions)
}

/// The value Electron wrote: the exe path in quotes.
pub fn run_value(exe: &std::path::Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Boot: writes or removes the Run value to match the stored choice and
/// returns the choice. StartupApproved is not touched.
pub fn sync() -> bool {
    write(false)
}

/// The Settings switch (`settings:set-launch-at-login`): as `sync`, and
/// turning it on also marks the entry enabled in StartupApproved, as
/// Electron's `setLoginItemSettings` did, so an entry disabled earlier in
/// Task Manager really launches again.
pub fn apply() -> bool {
    write(true)
}

/// A locked-down profile that refuses either key is not an error.
fn write(explicit: bool) -> bool {
    let on = crate::store::get().get_launch_at_login();
    if !live() {
        return on;
    }
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(run) = hkcu.open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE | KEY_READ) else {
        return on;
    };
    let _ = run.delete_value(OLD_VALUE_NAME);
    if let Ok(approved) = hkcu.open_subkey_with_flags(APPROVED_KEY, KEY_SET_VALUE) {
        let _ = approved.delete_value(OLD_VALUE_NAME);
    }
    if on {
        if let Ok(exe) = std::env::current_exe() {
            let _ = run.set_value(VALUE_NAME, &run_value(&exe));
        }
        if explicit {
            approve(&hkcu);
        }
    } else {
        let _ = run.delete_value(VALUE_NAME);
    }
    on
}

fn approve(hkcu: &RegKey) {
    let Ok((key, _)) = hkcu.create_subkey(APPROVED_KEY) else {
        return;
    };
    let value = RegValue {
        bytes: APPROVED_ON.to_vec(),
        vtype: REG_BINARY,
    };
    let _ = key.set_raw_value(VALUE_NAME, &value);
}

/// What Windows holds now, for the smoke run.
pub fn registered() -> Option<String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ).ok()?;
    run.get_value::<String, _>(VALUE_NAME).ok()
}

/// The StartupApproved bytes, for the smoke run.
pub fn approved() -> Option<Vec<u8>> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey_with_flags(APPROVED_KEY, KEY_READ).ok()?;
    key.get_raw_value(VALUE_NAME).ok().map(|v| v.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_is_the_quoted_exe() {
        let v = run_value(std::path::Path::new(r"C:\Apps\Aevaleron\Aevaleron.exe"));
        assert_eq!(v, r#""C:\Apps\Aevaleron\Aevaleron.exe""#);
    }
}
