//! The other half of K-02 (B-090, WIN-16). When WebView2's browser process
//! exits, every page goes with it and no reload brings one back, so each
//! window is destroyed and built again under its own label: the bar as the
//! orb, an output where it was with its conversation (sent again when the
//! new page says hello), Settings and History opened again.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::Mutex;
use tauri::AppHandle;

use super::{outputs, window, HISTORY, OVERLAY, SETTINGS};

/// At most this many rebuilds of one window a minute, so a browser that
/// dies at every start does not spin.
const MAX_PER_MINUTE: usize = 3;

/// Windows destroyed to be built again.
fn marked() -> &'static Mutex<HashSet<String>> {
    static M: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Per label, when it was last rebuilt (ms).
fn recent() -> &'static Mutex<HashMap<String, Vec<u64>>> {
    static R: OnceLock<Mutex<HashMap<String, Vec<u64>>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Whether one more rebuild fits in the last minute; records it when it does.
pub fn may_rebuild(times: &mut Vec<u64>, now: u64) -> bool {
    times.retain(|t| now.saturating_sub(*t) < 60_000);
    if times.len() >= MAX_PER_MINUTE {
        return false;
    }
    times.push(now);
    true
}

/// `crash::guard_webview`'s callback, on the main thread: the window goes,
/// and `on_destroyed` builds it again.
pub fn on_browser_exit(app: &AppHandle, label: &str) {
    let now = crate::state::now_ms();
    let fits = may_rebuild(recent().lock().entry(label.to_string()).or_default(), now);
    if !fits {
        let why = format!("{label} lost its browser process again; not rebuilt");
        crate::crash::write("renderer", &why);
        return;
    }
    let Some(w) = window(app, label) else {
        return;
    };
    marked().lock().insert(label.to_string());
    // Not from inside WebView2's own event handler.
    tauri::async_runtime::spawn(async move {
        let _ = w.destroy();
    });
}

/// Whether `label` was destroyed to be built again (answers once).
pub fn take(label: &str) -> bool {
    marked().lock().remove(label)
}

/// The rebuilt bar shows itself once its page has loaded, as at boot.
fn show_on_load() -> &'static Mutex<bool> {
    static S: OnceLock<Mutex<bool>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(false))
}

/// Whether the overlay page that just loaded is a rebuilt one (answers once).
pub fn overlay_loaded() -> bool {
    std::mem::take(&mut *show_on_load().lock())
}

/// Builds `label` again once Tauri has let go of the old window.
pub async fn rebuild(app: AppHandle, label: String) {
    for _ in 0..40 {
        if window(&app, &label).is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    if label == OVERLAY {
        *show_on_load().lock() = true;
        if let Err(e) = super::rebuild_overlay(&app) {
            *show_on_load().lock() = false;
            crate::crash::write("renderer", &format!("overlay not rebuilt: {e}"));
        }
    } else if outputs::is_output(&label) {
        outputs::rebuild(&app, &label);
    } else if label == SETTINGS {
        super::open_settings(&app, None);
    } else if label == HISTORY {
        super::open_history(&app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuild_is_limited() {
        let mut times = Vec::new();
        assert!(may_rebuild(&mut times, 1_000));
        assert!(may_rebuild(&mut times, 2_000));
        assert!(may_rebuild(&mut times, 3_000));
        assert!(!may_rebuild(&mut times, 4_000));
        assert_eq!(times.len(), 3);
        // A minute after the first, one more fits.
        assert!(may_rebuild(&mut times, 61_000));
        assert_eq!(times, vec![2_000, 3_000, 61_000]);
    }
}
