//! Silent toasts with click handlers (PARITY SYS-01, UPD-08, UPD-11,
//! ARCHITECTURE.md D9). They carry the app's AUMID, `com.aevaleron.app`,
//! which the installer puts on the Start Menu shortcut; without it Windows
//! shows them under a generic name or drops them.

use tauri_winrt_notification::Toast;

pub const AUMID: &str = "com.aevaleron.app";

pub type OnClick = Box<dyn FnMut() + Send + 'static>;

/// Shows a silent toast. A toast that cannot be shown is not worth an error:
/// the tray and Settings carry the same message.
pub fn show(title: &str, body: &str, on_click: Option<OnClick>) {
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        let mut toast = Toast::new(AUMID).title(&title).text1(&body).sound(None);
        if let Some(mut click) = on_click {
            toast = toast.on_activated(move |_action| {
                click();
                Ok(())
            });
        }
        if let Err(e) = toast.show() {
            eprintln!("[aevaleron] toast not shown: {e}");
        }
    });
}

/// Shows a silent toast and waits up to `timeout` for Windows to take it:
/// for a process about to end, where a thread left running would die first.
pub fn show_and_wait(title: &str, body: &str, timeout: std::time::Duration) {
    let (done, wait) = std::sync::mpsc::channel();
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        let toast = Toast::new(AUMID).title(&title).text1(&body).sound(None);
        if let Err(e) = toast.show() {
            eprintln!("[aevaleron] toast not shown: {e}");
        }
        let _ = done.send(());
    });
    let _ = wait.recv_timeout(timeout);
}

/// LIFE-04: the process carries the same AUMID, so a click reaches us.
pub fn set_app_user_model_id() {
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    let wide: Vec<u16> = AUMID.encode_utf16().chain(Some(0)).collect();
    // SAFETY: a NUL-terminated UTF-16 string that outlives the call.
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(wide.as_ptr());
    }
}
