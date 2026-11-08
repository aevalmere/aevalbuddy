//! One running copy per user-data folder (PARITY LIFE-01, ARCHITECTURE.md D8).
//!
//! A named mutex keyed on the user-data path, so an `AEVAL_USERDATA` run gets
//! its own lock and never collides with a real session. A second launch
//! sends `open` over a named pipe of the same name and exits; the first copy
//! opens the bar (main.js `second-instance` -> triggerCapture).
//!
//! The pipe is served from before the app is built, so a second launch
//! during a slow boot is heard; its `open` waits on a `Doorbell` until setup
//! is done. The second launch hands the first copy the right to take the
//! foreground, so the bar comes up with keyboard focus.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use tokio::io::AsyncBufReadExt;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

/// How long a second launch keeps trying: the first copy may still be
/// booting (the 15 s bootMs gate).
const KNOCK_FOR: Duration = Duration::from_secs(15);
/// How long the first copy waits for a client to say what it wants.
const READ_FOR: Duration = Duration::from_secs(2);

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

/// Tells the first copy to open. This process got the user's launch, so it
/// may pass the foreground on; the first copy needs it to focus the bar.
fn signal(name: &str) {
    // The feature is in the windows lane's windows-sys block of Cargo.toml.
    use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};
    // SAFETY: takes a process id only.
    unsafe { AllowSetForegroundWindow(ASFW_ANY) };
    if !knock(name, KNOCK_FOR) {
        eprintln!("[aevalbuddy] the running copy did not answer");
    }
}

/// Writes `open` to the pipe, retrying every 100 ms until `patience` runs
/// out (the server may be between two connections, or not up yet).
fn knock(name: &str, patience: Duration) -> bool {
    let start = Instant::now();
    loop {
        let pipe = std::fs::OpenOptions::new()
            .write(true)
            .open(pipe_path(name));
        if let Ok(mut p) = pipe {
            if p.write_all(b"open\n").is_ok() {
                return true;
            }
        }
        if start.elapsed() >= patience {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A second launch's `open`, kept until the app can act on it.
#[derive(Clone, Default)]
pub struct Doorbell(Arc<Mutex<Bell>>);

type Handler = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct Bell {
    handler: Option<Handler>,
    rung: bool,
}

impl Doorbell {
    fn ring(&self) {
        let handler = {
            let mut bell = self.0.lock();
            bell.rung = bell.handler.is_none();
            bell.handler.clone()
        };
        if let Some(h) = handler {
            h();
        }
    }

    /// Sets what an `open` does, and does it now for one that came while
    /// the app was still starting (once, however many came).
    pub fn answer(&self, handler: impl Fn() + Send + Sync + 'static) {
        let handler: Handler = Arc::new(handler);
        let rung = {
            let mut bell = self.0.lock();
            bell.handler = Some(handler.clone());
            std::mem::take(&mut bell.rung)
        };
        if rung {
            handler();
        }
    }
}

/// Serves the pipe; each `open` rings the returned bell. Call before the
/// app is built.
pub fn serve(name: String) -> Doorbell {
    let bell = Doorbell::default();
    tauri::async_runtime::spawn(listen(pipe_path(&name), bell.clone()));
    bell
}

async fn listen(path: String, bell: Doorbell) {
    let mut server = create(&path, true).await;
    loop {
        if server.connect().await.is_err() {
            server = create(&path, false).await;
            continue;
        }
        // The next instance first, so the next client never finds the pipe
        // busy; this client is read on its own task, so one that connects
        // and says nothing holds up nobody.
        let next = create(&path, false).await;
        let client = std::mem::replace(&mut server, next);
        let b = bell.clone();
        tauri::async_runtime::spawn(async move {
            if reads_open(client).await {
                b.ring();
            }
        });
    }
}

/// A pipe instance. A failure is retried, 100 ms doubling to 5 s, instead
/// of leaving later launches unanswered.
async fn create(path: &str, first: bool) -> NamedPipeServer {
    let mut wait = Duration::from_millis(100);
    loop {
        let made = ServerOptions::new()
            .first_pipe_instance(first)
            .access_outbound(false)
            .create(path);
        match made {
            Ok(server) => return server,
            Err(e) => {
                eprintln!("[aevalbuddy] instance pipe not created: {e}");
                tokio::time::sleep(wait).await;
                wait = (wait * 2).min(Duration::from_secs(5));
            }
        }
    }
}

/// Whether the client's first line is `open`, waiting at most 2 s for it.
async fn reads_open(pipe: NamedPipeServer) -> bool {
    let mut reader = tokio::io::BufReader::new(pipe);
    let mut line = String::new();
    let read = tokio::time::timeout(READ_FOR, reader.read_line(&mut line)).await;
    matches!(read, Ok(Ok(_))) && line.starts_with("open")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

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

    #[test]
    fn an_open_during_boot_is_answered_once_ready() {
        let bell = Doorbell::default();
        bell.ring();
        bell.ring();
        let n = Arc::new(AtomicUsize::new(0));
        let seen = n.clone();
        bell.answer(move || {
            seen.fetch_add(1, SeqCst);
        });
        assert_eq!(n.load(SeqCst), 1);
        bell.ring();
        assert_eq!(n.load(SeqCst), 2);
    }

    #[test]
    fn stalled_client_does_not_block_server() {
        let name = format!("aevalbuddy-test-{}-stall", std::process::id());
        let bell = serve(name.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        bell.answer(move || {
            let _ = tx.send(());
        });
        // A client that connects and never writes.
        let start = Instant::now();
        let stalled = loop {
            let pipe = std::fs::OpenOptions::new()
                .write(true)
                .open(pipe_path(&name));
            match pipe {
                Ok(p) => break p,
                Err(e) if start.elapsed() > Duration::from_secs(5) => panic!("no pipe: {e}"),
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        };
        assert!(knock(&name, Duration::from_secs(2)));
        let heard = rx.recv_timeout(Duration::from_millis(1000));
        assert!(heard.is_ok(), "the second client waited on the first");
        drop(stalled);
    }
}
