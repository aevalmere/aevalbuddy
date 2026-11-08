//! One running copy per user-data folder (PARITY LIFE-01, ARCHITECTURE.md D8).
//!
//! A named mutex keyed on the user-data path, so an `AEVAL_USERDATA` run gets
//! its own lock and never collides with a real session. A second launch
//! sends `open` over a named pipe of the same name and exits; the first copy
//! opens the bar (main.js `second-instance` -> triggerCapture).

use std::io::Write;
use std::path::Path;

use sha2::{Digest, Sha256};

/// `aevalbuddy-<first 16 hex of sha256(lowercased path)>`.
pub fn lock_name(user_data: &Path) -> String {
    let path = user_data.to_string_lossy().to_lowercase();
    let digest = Sha256::digest(path.as_bytes());
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("aevalbuddy-{hex}")
}

pub fn pipe_path(name: &str) -> String {
    format!(r"\\.\pipe\{name}")
}

/// Held for the life of the process.
pub struct Lock {
    _handle: isize,
    pub name: String,
}

/// Takes the lock, or tells the running copy to open its bar and returns
/// None (the caller exits).
pub fn acquire(user_data: &Path) -> Option<Lock> {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let name = lock_name(user_data);
    let wide: Vec<u16> = format!(r"Local\{name}")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: a NUL-terminated name; the handle is kept until exit.
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide.as_ptr()) };
    // SAFETY: read straight after the call that set it.
    let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if handle.is_null() || existed {
        signal(&name);
        return None;
    }
    Some(Lock {
        _handle: handle as isize,
        name,
    })
}

/// Tells the first copy to open. Retries briefly: its pipe may be between
/// two connections.
fn signal(name: &str) {
    for _ in 0..20 {
        let pipe = std::fs::OpenOptions::new()
            .write(true)
            .open(pipe_path(name));
        if let Ok(mut p) = pipe {
            let _ = p.write_all(b"open\n");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Serves the pipe; `on_open` runs once per second launch.
pub fn serve(name: String, on_open: impl Fn() + Send + Sync + 'static) {
    use tokio::io::AsyncReadExt;
    use tokio::net::windows::named_pipe::ServerOptions;
    tauri::async_runtime::spawn(async move {
        let path = pipe_path(&name);
        let mut first = true;
        loop {
            let server = ServerOptions::new()
                .first_pipe_instance(first)
                .access_outbound(false)
                .create(&path);
            first = false;
            let Ok(mut server) = server else {
                return;
            };
            if server.connect().await.is_err() {
                continue;
            }
            let mut buf = Vec::new();
            let _ = server.read_to_end(&mut buf).await;
            if buf.starts_with(b"open") {
                on_open();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_is_per_folder_and_case_blind() {
        let a = lock_name(Path::new(r"C:\Users\a\AppData\Roaming\aevalbuddy"));
        let b = lock_name(Path::new(r"C:\USERS\A\AppData\Roaming\aevalbuddy"));
        let c = lock_name(Path::new(r"D:\scratch"));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), "aevalbuddy-".len() + 16);
        assert_eq!(pipe_path("x"), r"\\.\pipe\x");
    }
}
