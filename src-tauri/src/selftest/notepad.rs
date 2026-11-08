//! The smoke's own Notepad, for the probes that need a real app to work in
//! (lib/smoke-v2-desktop.js openNotepad). It opens a temp file holding
//! [`TEXT`], is told apart from any other Notepad by the file's unique name
//! in its title, and closes without saving through the desktop helper's
//! patterns, so it never needs the keyboard focus to go away.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::desktop::{self, selfcheck, DesktopError};

/// The file's text, as Electron's FIND_TEXT.
pub const TEXT: &str = "Invoice 2291 for March\r\nPlease settle by Friday\r\ntotal due 42.00";

/// An open Notepad of the smoke's own.
// The lanes' probes read the fields the harness does not.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Notepad {
    /// The desktop helper's record of the window (`handle`, `pid`, `title`,
    /// `process`, `className`, `rect`).
    pub win: Value,
    /// The handle as the helper spells it (`w1A2B3C`).
    pub handle: String,
    /// The unique part of the file name, which is in the window title.
    pub tag: String,
    pub file: PathBuf,
    had_notepad: bool,
}

fn err_text(e: &DesktopError) -> String {
    format!("{}:{}", e.code, e.message)
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

/// A window that was not there before and carries the file's name.
fn is_ours(w: &Value, before: &HashSet<String>, tag: &str) -> bool {
    !before.contains(text(&w["handle"])) && text(&w["title"]).contains(tag)
}

async fn windows(include_minimized: bool) -> Vec<Value> {
    let listed = desktop::client().list_windows(include_minimized).await;
    listed
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
}

/// Starts the helper if need be, opens Notepad on a fresh temp file and
/// waits up to 15 s for its window.
pub async fn open() -> Result<Notepad, String> {
    desktop::client().start().await.map_err(|e| err_text(&e))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let tag = format!("aeval-v2-{}-{stamp:x}", std::process::id());
    let file = std::env::temp_dir().join(format!("{tag}.txt"));
    std::fs::write(&file, format!("{TEXT}\r\n")).map_err(|e| format!("temp file: {e}"))?;
    let before: HashSet<String> = windows(true)
        .await
        .iter()
        .map(|w| text(&w["handle"]).to_string())
        .collect();
    let had_notepad = windows(false)
        .await
        .iter()
        .any(|w| text(&w["process"]).eq_ignore_ascii_case("notepad"));
    if let Err(e) = Command::new("notepad.exe").arg(&file).spawn() {
        let _ = std::fs::remove_file(&file);
        return Err(format!("notepad.exe: {e}"));
    }
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        let all = windows(false).await;
        if let Some(win) = all.into_iter().find(|w| is_ours(w, &before, &tag)) {
            let handle = text(&win["handle"]).to_string();
            return Ok(Notepad {
                win,
                handle,
                tag,
                file,
                had_notepad,
            });
        }
        if Instant::now() > end {
            let _ = std::fs::remove_file(&file);
            return Err("no Notepad window with the fixture file in 15 s".to_string());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

impl Notepad {
    /// Closes the tab or window without saving and deletes the temp file;
    /// true once no window carries the file's name.
    pub async fn close(&self) -> bool {
        // close_without_saving logs closeError and dontSave as it goes.
        let rep = selfcheck::Report::new("SMOKE");
        let client = desktop::client();
        let (win, tag) = (&self.win, &self.tag);
        let closed = selfcheck::close_without_saving(client, win, tag, self.had_notepad, &rep);
        let closed = closed.await;
        let _ = std::fs::remove_file(&self.file);
        closed
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn only_a_new_window_with_the_file_name_is_ours() {
        let before: HashSet<String> = ["w1".to_string()].into();
        let tag = "aeval-v2-1-abc";
        let new = json!({ "handle": "w2", "title": "aeval-v2-1-abc.txt - Notepad" });
        let old = json!({ "handle": "w1", "title": "aeval-v2-1-abc.txt - Notepad" });
        let other = json!({ "handle": "w3", "title": "Untitled - Notepad" });
        assert!(is_ours(&new, &before, tag));
        assert!(!is_ours(&old, &before, tag));
        assert!(!is_ours(&other, &before, tag));
    }
}
