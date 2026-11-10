//! Where Aevaleron keeps its files: `%APPDATA%\aevaleron`. Tauri's own
//! `app_data_dir()` would be `%APPDATA%\com.aevaleron.app` and must not be
//! used for user data.
//!
//! The app was AevalBuddy before 2.0 and kept the same files in
//! `%APPDATA%\aevalbuddy` (Electron's userData, PARITY FILE-01). The first
//! start under the new name moves that folder, so an upgraded install finds
//! its settings, key and history (`adopt_old_name`).

use std::fs;
use std::path::{Path, PathBuf};

/// The folder's name from before the rename.
const OLD_NAME: &str = "aevalbuddy";
/// The settings file and its backup: the name from before, the name now.
const RENAMED: [(&str, &str); 2] = [
    ("aevalbuddy.json", "aevaleron.json"),
    ("aevalbuddy.json.bak", "aevaleron.json.bak"),
];

/// Boot, before anything reads the folder: an AevalBuddy profile becomes
/// this app's. Nothing happens once `%APPDATA%\aevaleron` exists, or in an
/// automated run with its own folder.
pub fn adopt_old_name() {
    let get = |k: &str| std::env::var(k).ok();
    if get("AEVAL_USERDATA").is_some_and(|d| !d.trim().is_empty()) {
        return;
    }
    let Some(roaming) = get("APPDATA").map(PathBuf::from) else {
        return;
    };
    adopt(&roaming.join(OLD_NAME), &user_data_dir());
}

/// Moves `old` to `new` when only `old` exists, then gives the settings
/// file its new name. A folder that cannot be moved (a file in it is open
/// elsewhere) is copied, so the old one is left as it was.
fn adopt(old: &Path, new: &Path) {
    if new.exists() || !old.is_dir() {
        return;
    }
    if fs::rename(old, new).is_err() && copy_dir(old, new).is_err() {
        // Half a copy would read as a profile next time: start clean.
        let _ = fs::remove_dir_all(new);
        return;
    }
    for (was, now) in RENAMED {
        let (was, now) = (new.join(was), new.join(now));
        if was.exists() && !now.exists() {
            let _ = fs::rename(was, now);
        }
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// `AEVAL_USERDATA` when set (automated runs), else `%APPDATA%\aevaleron`.
pub fn user_data_dir() -> PathBuf {
    user_data_dir_from(|k| std::env::var(k).ok())
}

pub fn user_data_dir_from(get: impl Fn(&str) -> Option<String>) -> PathBuf {
    if let Some(dir) = get("AEVAL_USERDATA").filter(|d| !d.trim().is_empty()) {
        return PathBuf::from(dir);
    }
    get("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("aevaleron")
}

/// WebView2's profile. Kept out of the userData root so it never rewrites the
/// `Local State` file that holds Electron's key for the saved API key
/// (PARITY FILE-07, FILE-13).
pub fn webview_data_dir() -> PathBuf {
    webview_data_dir_from(|k| std::env::var(k).ok())
}

pub fn webview_data_dir_from(get: impl Fn(&str) -> Option<String>) -> PathBuf {
    if let Some(dir) = get("AEVAL_USERDATA").filter(|d| !d.trim().is_empty()) {
        return PathBuf::from(dir).join("WebView2");
    }
    get("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("aevaleron")
        .join("WebView2")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| owned.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())
    }

    fn scratch(name: &str) -> PathBuf {
        let leaf = format!("aevaleron-paths-{}-{name}", std::process::id());
        let dir = std::env::temp_dir().join(leaf);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn text(file: PathBuf) -> String {
        fs::read_to_string(file).unwrap()
    }

    #[test]
    fn an_old_profile_is_adopted_once() {
        let root = scratch("adopt");
        let (old, new) = (root.join("aevalbuddy"), root.join("aevaleron"));
        fs::create_dir_all(old.join("voice")).unwrap();
        fs::write(old.join("aevalbuddy.json"), "{\"a\":1}").unwrap();
        fs::write(old.join("aevalbuddy.json.bak"), "{}").unwrap();
        fs::write(old.join("history.json"), "[]").unwrap();
        fs::write(old.join("voice").join("m.bin"), "m").unwrap();
        adopt(&old, &new);
        assert!(!old.exists());
        assert_eq!(text(new.join("aevaleron.json")), "{\"a\":1}");
        assert!(new.join("aevaleron.json.bak").exists());
        assert!(!new.join("aevalbuddy.json").exists());
        assert!(new.join("history.json").exists());
        assert!(new.join("voice").join("m.bin").exists());
        // A later AevalBuddy folder never replaces the profile in use.
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("aevalbuddy.json"), "{\"a\":2}").unwrap();
        adopt(&old, &new);
        assert_eq!(text(new.join("aevaleron.json")), "{\"a\":1}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_copied_profile_keeps_its_folders() {
        let root = scratch("copy");
        let (old, new) = (root.join("aevalbuddy"), root.join("aevaleron"));
        fs::create_dir_all(old.join("voice")).unwrap();
        fs::write(old.join("voice").join("m.bin"), "m").unwrap();
        copy_dir(&old, &new).unwrap();
        assert_eq!(text(new.join("voice").join("m.bin")), "m");
        assert!(old.join("voice").join("m.bin").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn user_data_is_under_roaming() {
        let dir = user_data_dir_from(env(&[("APPDATA", r"C:\Users\a\AppData\Roaming")]));
        assert_eq!(
            dir,
            PathBuf::from(r"C:\Users\a\AppData\Roaming").join("aevaleron")
        );
    }

    #[test]
    fn aeval_userdata_overrides() {
        let get = env(&[("APPDATA", r"C:\x"), ("AEVAL_USERDATA", r"D:\scratch")]);
        assert_eq!(user_data_dir_from(&get), PathBuf::from(r"D:\scratch"));
        assert_eq!(
            webview_data_dir_from(&get),
            PathBuf::from(r"D:\scratch").join("WebView2")
        );
    }

    #[test]
    fn blank_override_is_ignored() {
        let dir = user_data_dir_from(env(&[("APPDATA", r"C:\r"), ("AEVAL_USERDATA", " ")]));
        assert_eq!(dir, PathBuf::from(r"C:\r").join("aevaleron"));
    }

    #[test]
    fn webview_profile_is_not_in_the_userdata_root() {
        let get = env(&[("APPDATA", r"C:\r"), ("LOCALAPPDATA", r"C:\l")]);
        let web = webview_data_dir_from(&get);
        assert_eq!(
            web,
            PathBuf::from(r"C:\l").join("aevaleron").join("WebView2")
        );
        assert_ne!(web.parent(), Some(user_data_dir_from(&get).as_path()));
    }
}
