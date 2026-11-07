//! Where AevalBuddy keeps its files. The folder is Electron's userData,
//! `%APPDATA%\aevalbuddy`, so an upgraded install finds its settings, key and
//! history (PARITY FILE-01). Tauri's own `app_data_dir()` would be
//! `%APPDATA%\com.aevalbuddy.app` and must not be used for user data.

use std::path::PathBuf;

/// `AEVAL_USERDATA` when set (automated runs), else `%APPDATA%\aevalbuddy`.
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
        .join("aevalbuddy")
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
        .join("aevalbuddy")
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

    #[test]
    fn user_data_is_electrons_folder() {
        let dir = user_data_dir_from(env(&[("APPDATA", r"C:\Users\a\AppData\Roaming")]));
        assert_eq!(
            dir,
            PathBuf::from(r"C:\Users\a\AppData\Roaming").join("aevalbuddy")
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
        assert_eq!(dir, PathBuf::from(r"C:\r").join("aevalbuddy"));
    }

    #[test]
    fn webview_profile_is_not_in_the_userdata_root() {
        let get = env(&[("APPDATA", r"C:\r"), ("LOCALAPPDATA", r"C:\l")]);
        let web = webview_data_dir_from(&get);
        assert_eq!(
            web,
            PathBuf::from(r"C:\l").join("aevalbuddy").join("WebView2")
        );
        assert_ne!(web.parent(), Some(user_data_dir_from(&get).as_path()));
    }
}
