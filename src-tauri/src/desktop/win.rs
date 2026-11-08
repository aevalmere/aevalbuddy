//! Win32 details for the full-display sheets this split owns (region
//! selector, lens, pointer) and for telling the helper which processes are
//! ours. Agent (a)'s `windows/flags.rs` does the same for the app windows;
//! these stay local so the sheets do not wait on it.

use std::collections::HashSet;

use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_TOPMOST,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_SHOWNOACTIVATE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use crate::capture::{self, PxRect};

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut _)
}

/// Off the taskbar and Alt-Tab; with `no_activate`, never takes focus.
pub fn tool_window(raw: isize, no_activate: bool) {
    if raw == 0 {
        return;
    }
    unsafe {
        let ex = GetWindowLongPtrW(hwnd(raw), GWL_EXSTYLE);
        let mut add = WS_EX_TOOLWINDOW.0 as isize;
        if no_activate {
            add |= WS_EX_NOACTIVATE.0 as isize;
        }
        SetWindowLongPtrW(hwnd(raw), GWL_EXSTYLE, ex | add);
    }
}

pub fn set_no_activate(raw: isize, on: bool) {
    if raw == 0 {
        return;
    }
    unsafe {
        let ex = GetWindowLongPtrW(hwnd(raw), GWL_EXSTYLE);
        let bit = WS_EX_NOACTIVATE.0 as isize;
        let next = if on { ex | bit } else { ex & !bit };
        SetWindowLongPtrW(hwnd(raw), GWL_EXSTYLE, next);
    }
}

/// Shown without taking focus, above the always-on-top band, and (again)
/// excluded from capture: Windows may drop affinity while hidden.
pub fn show_inactive(raw: isize) {
    if raw == 0 {
        return;
    }
    capture::exclude_window(raw);
    unsafe {
        let _ = ShowWindow(hwnd(raw), SW_SHOWNOACTIVATE);
    }
    raise(raw);
}

pub fn raise(raw: isize) {
    if raw == 0 {
        return;
    }
    unsafe {
        let _ = SetWindowPos(
            hwnd(raw),
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// Places a window on exact physical pixels (logical positions would be
/// converted with whichever monitor's scale the window started on).
pub fn place<R: tauri::Runtime>(win: &tauri::WebviewWindow<R>, r: PxRect) {
    let _ = win.set_position(tauri::PhysicalPosition::new(r.x, r.y));
    let _ = win.set_size(tauri::PhysicalSize::new(
        r.width.max(1) as u32,
        r.height.max(1) as u32,
    ));
}

/// Every process descended from ours: the WebView2 browser, renderer and
/// GPU processes stand where Electron's child pids stood (DESK-06).
pub fn own_pids() -> Vec<u32> {
    let me = std::process::id();
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return vec![me];
        };
        let mut e = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut e).is_ok() {
            loop {
                pairs.push((e.th32ProcessID, e.th32ParentProcessID));
                if Process32NextW(snap, &mut e).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    descendants(me, &pairs)
}

/// `root` and everything under it, from (pid, parent) pairs.
pub fn descendants(root: u32, pairs: &[(u32, u32)]) -> Vec<u32> {
    let mut seen: HashSet<u32> = HashSet::from([root]);
    let mut out = vec![root];
    let mut i = 0;
    while i < out.len() {
        let parent = out[i];
        for &(pid, ppid) in pairs {
            if ppid == parent && pid != 0 && seen.insert(pid) {
                out.push(pid);
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descendants_walks_the_tree_once() {
        let pairs = [(10, 1), (11, 10), (12, 11), (13, 2), (10, 12)];
        let mut d = descendants(1, &pairs);
        d.sort();
        assert_eq!(d, vec![1, 10, 11, 12]);
    }

    #[test]
    fn our_own_pid_is_always_listed() {
        assert!(own_pids().contains(&std::process::id()));
    }
}
