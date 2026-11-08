//! Writes that survive a crash or a power cut halfway through (B-001, B-003,
//! B-004).
//!
//! The bytes go to a temp file with a name no other write uses, are flushed
//! to the disk, and only then replace the target with
//! `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)`. A reader sees the old
//! file or the new one, never a torn or empty one.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static SEQ: AtomicU64 = AtomicU64::new(0);

/// `<target>.<pid>.<n>.tmp`: unique per write, so two writers never share
/// a temp file.
pub fn temp_name(target: &Path) -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let mut name = target.as_os_str().to_owned();
    name.push(format!(".{}.{n}.tmp", std::process::id()));
    PathBuf::from(name)
}

/// Writes `bytes` to `target` durably; on failure the target is untouched
/// and the temp file is removed.
pub fn write(target: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = temp_name(target);
    let result = write_synced(&tmp, bytes).and_then(|()| replace(&tmp, target));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut f = fs::File::create(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

fn wide(p: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    p.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// Moves `from` over `to` and flushes the move to the disk.
///
/// `MoveFileExW` refuses (access denied) while any other handle has the
/// target open, even one that shares delete: a reader polling the file, a
/// virus scanner or the search indexer. `std::fs::rename` uses POSIX rename
/// semantics, which replace a file that is open elsewhere, so it is the
/// fallback; the bytes were already flushed by `write_synced`. Both are
/// retried with a growing pause (about 1.5 s in all) before giving up.
pub fn replace(from: &Path, to: &Path) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let (src, dst) = (wide(from), wide(to));
    let mut last = io::Error::other("not attempted");
    for attempt in 0..7u32 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(25 << attempt.min(4)));
        }
        let flags = MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH;
        // SAFETY: two NUL-terminated UTF-16 paths that outlive the call.
        if unsafe { MoveFileExW(src.as_ptr(), dst.as_ptr(), flags) } != 0 {
            return Ok(());
        }
        let moved = io::Error::last_os_error();
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => last = io::Error::new(moved.kind(), format!("{moved}; rename: {e}")),
        }
    }
    Err(last)
}

/// Removes temp files a crash left behind next to `target` (from any
/// process: only one copy runs per folder).
pub fn sweep_temps(target: &Path) {
    let (Some(dir), Some(name)) = (target.parent(), target.file_name()) else {
        return;
    };
    let prefix = format!("{}.", name.to_string_lossy());
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        let Some(rest) = file.strip_prefix(&prefix) else {
            continue;
        };
        // `<pid>.<n>.tmp`, exactly what temp_name makes.
        let parts: Vec<&str> = rest.split('.').collect();
        let ours = parts.len() == 3 && digits(parts[0]) && digits(parts[1]) && parts[2] == "tmp";
        if ours {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aeval-dur-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn replaces_and_leaves_no_temp() {
        let dir = temp("replace");
        let target = dir.join("a.json");
        write(&target, b"one").unwrap();
        write(&target, b"two").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"two");
        let names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["a.json".to_string()]);
        assert_ne!(temp_name(&target), temp_name(&target));
    }

    /// A reader holding the target open (the probe polling it, a scanner)
    /// must not make the save fail.
    #[test]
    fn replaces_a_target_another_handle_has_open() {
        let dir = temp("open");
        let target = dir.join("a.json");
        write(&target, b"one").unwrap();
        let held = fs::File::open(&target).unwrap();
        write(&target, b"two").unwrap();
        drop(held);
        assert_eq!(fs::read(&target).unwrap(), b"two");
    }

    #[test]
    fn sweep_removes_only_our_temps() {
        let dir = temp("sweep");
        let target = dir.join("s.json");
        for name in [
            "s.json",
            "s.json.12.3.tmp",
            "s.json.bak",
            "s.json.x.1.tmp",
            "t.json.1.2.tmp",
        ] {
            fs::write(dir.join(name), "x").unwrap();
        }
        sweep_temps(&target);
        assert!(!dir.join("s.json.12.3.tmp").exists());
        assert!(target.exists());
        for kept in ["s.json.bak", "s.json.x.1.tmp", "t.json.1.2.tmp"] {
            assert!(dir.join(kept).exists(), "{kept}");
        }
    }
}
