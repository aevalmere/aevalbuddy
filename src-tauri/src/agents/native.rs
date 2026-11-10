//! The Win32 calls agent watching needs: the process tree above a hook
//! call, the window that hosts a session, bringing that window to the
//! front, and how long the person has been away. Port of Aevalsistant's
//! `Windows.cs`.
//!
//! The declarations are this module's own, with plain integer handles, so
//! the hook client (which runs on every agent event and must start fast)
//! pulls in nothing else.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;

#[repr(C)]
struct ProcessEntry {
    size: u32,
    usage: u32,
    pid: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent: u32,
    priority: i32,
    flags: u32,
    exe: [u16; 260],
}

#[repr(C)]
struct LastInput {
    size: u32,
    time: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> isize;
    fn Process32FirstW(snapshot: isize, entry: *mut ProcessEntry) -> i32;
    fn Process32NextW(snapshot: isize, entry: *mut ProcessEntry) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
    fn CloseHandle(handle: isize) -> i32;
    fn GetProcessTimes(
        process: isize,
        created: *mut u64,
        exited: *mut u64,
        kernel: *mut u64,
        user: *mut u64,
    ) -> i32;
    fn GetExitCodeProcess(process: isize, code: *mut u32) -> i32;
    fn QueryFullProcessImageNameW(process: isize, flags: u32, name: *mut u16, size: *mut u32)
        -> i32;
    fn GetCurrentProcessId() -> u32;
    fn GetCurrentThreadId() -> u32;
    fn AttachConsole(pid: u32) -> i32;
    fn FreeConsole() -> i32;
    fn GetConsoleWindow() -> isize;
    fn GetShortPathNameW(long: *const u16, short: *mut u16, len: u32) -> u32;
    fn GetTickCount() -> u32;
}

#[link(name = "user32")]
extern "system" {
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn IsWindow(hwnd: isize) -> i32;
    fn IsIconic(hwnd: isize) -> i32;
    fn GetAncestor(hwnd: isize, flags: u32) -> isize;
    fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
    fn EnumWindows(callback: unsafe extern "system" fn(isize, isize) -> i32, param: isize) -> i32;
    fn GetWindow(hwnd: isize, command: u32) -> isize;
    fn GetWindowTextLengthW(hwnd: isize) -> i32;
    fn GetWindowTextW(hwnd: isize, text: *mut u16, max: i32) -> i32;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn ShowWindow(hwnd: isize, command: i32) -> i32;
    fn GetForegroundWindow() -> isize;
    fn AttachThreadInput(from: u32, to: u32, attach: i32) -> i32;
    fn BringWindowToTop(hwnd: isize) -> i32;
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn SwitchToThisWindow(hwnd: isize, alt_tab: i32);
    fn GetLastInputInfo(info: *mut LastInput) -> i32;
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmGetWindowAttribute(hwnd: isize, attribute: u32, value: *mut c_void, size: u32) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn SHQueryUserNotificationState(state: *mut i32) -> i32;
}

const TH32CS_SNAPPROCESS: u32 = 0x2;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const STILL_ACTIVE: u32 = 259;
const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
const GA_ROOTOWNER: u32 = 3;
const GW_OWNER: u32 = 4;
const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOOLWINDOW: isize = 0x80;
const DWMWA_CLOAKED: u32 = 14;
const SW_RESTORE: i32 = 9;

/// The long-lived process of each agent, used to notice when a session's
/// agent has exited. IDE agents (Cursor, Windsurf) run inside the editor's
/// helper processes, which can be short-lived, so they get none and fall
/// back to the inactivity timeout.
fn agent_processes(agent: &str) -> &'static [&'static str] {
    match agent {
        "claude" => &["claude.exe", "node.exe", "bun.exe"],
        "codex" => &["codex.exe"],
        "gemini" => &["gemini.exe", "node.exe"],
        "copilot" => &["copilot.exe", "node.exe"],
        "cursor" => &["cursor-agent.exe"],
        "opencode" => &["opencode.exe"],
        _ => &[],
    }
}

const ANY_AGENT: [&str; 8] = [
    "claude.exe",
    "node.exe",
    "bun.exe",
    "codex.exe",
    "gemini.exe",
    "copilot.exe",
    "cursor-agent.exe",
    "opencode.exe",
];

/// The walk up the tree stops at the shell and the session's own roots.
const STOP_AT: [&str; 6] = [
    "explorer.exe",
    "services.exe",
    "svchost.exe",
    "wininit.exe",
    "winlogon.exe",
    "userinit.exe",
];

const NAMES: [(&str, &str); 28] = [
    ("code.exe", "VS Code"),
    ("code - insiders.exe", "VS Code Insiders"),
    ("cursor.exe", "Cursor"),
    ("windsurf.exe", "Windsurf"),
    ("zed.exe", "Zed"),
    ("studio64.exe", "Android Studio"),
    ("studio.exe", "Android Studio"),
    ("idea64.exe", "IntelliJ IDEA"),
    ("pycharm64.exe", "PyCharm"),
    ("webstorm64.exe", "WebStorm"),
    ("rider64.exe", "Rider"),
    ("clion64.exe", "CLion"),
    ("goland64.exe", "GoLand"),
    ("phpstorm64.exe", "PhpStorm"),
    ("claude.exe", "Claude"),
    ("chatgpt.exe", "ChatGPT"),
    ("kiro.exe", "Kiro"),
    ("trae.exe", "Trae"),
    ("windowsterminal.exe", "Terminal"),
    ("conhost.exe", "Console"),
    ("openconsole.exe", "Terminal"),
    ("wezterm-gui.exe", "WezTerm"),
    ("alacritty.exe", "Alacritty"),
    ("hyper.exe", "Hyper"),
    ("tabby.exe", "Tabby"),
    ("warp.exe", "Warp"),
    ("mintty.exe", "Git Bash"),
    ("powershell.exe", "PowerShell"),
];

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proc {
    pub pid: u32,
    pub name: String,
}

/// What the hook client learns about where it was called from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub window: i64,
    pub agent_pid: u32,
    pub agent_start: u64,
    pub outer_pid: u32,
    pub outer_start: u64,
    pub host: String,
}

struct Handle(isize);

impl Handle {
    fn process(pid: u32) -> Option<Handle> {
        // SAFETY: a query-only handle, closed on drop.
        let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if h == 0 {
            None
        } else {
            Some(Handle(h))
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle is ours and still open.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// When the process started, as a FILETIME; 0 when it cannot be read.
pub fn start_time(pid: u32) -> u64 {
    let Some(h) = Handle::process(pid) else {
        return 0;
    };
    let (mut created, mut a, mut b, mut c) = (0u64, 0u64, 0u64, 0u64);
    // SAFETY: four valid out pointers of FILETIME's size.
    let ok = unsafe { GetProcessTimes(h.0, &mut created, &mut a, &mut b, &mut c) };
    if ok != 0 {
        created
    } else {
        0
    }
}

pub fn own_pid() -> u32 {
    // SAFETY: no arguments.
    unsafe { GetCurrentProcessId() }
}

/// Whether `pid` still runs and is the process that started at `start`
/// (a recycled pid belongs to some other program).
pub fn is_alive(pid: u32, start: u64) -> bool {
    let Some(h) = Handle::process(pid) else {
        return false;
    };
    let mut code = 0u32;
    // SAFETY: a valid out pointer.
    if unsafe { GetExitCodeProcess(h.0, &mut code) } == 0 || code != STILL_ACTIVE {
        return false;
    }
    let (mut created, mut a, mut b, mut c) = (0u64, 0u64, 0u64, 0u64);
    // SAFETY: four valid out pointers.
    let ok = unsafe { GetProcessTimes(h.0, &mut created, &mut a, &mut b, &mut c) };
    start == 0 || ok == 0 || created == start
}

pub fn image_path(pid: u32) -> String {
    let Some(h) = Handle::process(pid) else {
        return String::new();
    };
    let mut buf = [0u16; 1024];
    let mut size = buf.len() as u32;
    // SAFETY: the buffer is `size` units long.
    let ok = unsafe { QueryFullProcessImageNameW(h.0, 0, buf.as_mut_ptr(), &mut size) };
    if ok == 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..size as usize])
}

fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// The name people know an app by: "VS Code" for Code.exe, else the exe's
/// name without its extension.
pub fn label_for(pid: u32, exe_name: Option<&str>) -> String {
    let path = image_path(pid);
    let name = exe_name.unwrap_or_else(|| file_name(&path));
    label_of(name)
}

fn label_of(exe: &str) -> String {
    if let Some((_, known)) = NAMES.iter().find(|(n, _)| same(n, exe)) {
        return (*known).to_string();
    }
    let stem = exe.rsplit_once('.').map_or(exe, |(s, _)| s);
    stem.to_string()
}

/// Every process's parent and name.
fn snapshot() -> (HashMap<u32, u32>, HashMap<u32, String>) {
    let mut parent = HashMap::new();
    let mut name = HashMap::new();
    // SAFETY: a snapshot handle, closed below; the entry is sized as the
    // API asks.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == -1 || snap == 0 {
            return (parent, name);
        }
        let mut pe: ProcessEntry = std::mem::zeroed();
        pe.size = std::mem::size_of::<ProcessEntry>() as u32;
        let mut ok = Process32FirstW(snap, &mut pe);
        while ok != 0 {
            parent.insert(pe.pid, pe.parent);
            name.insert(pe.pid, from_wide(&pe.exe));
            ok = Process32NextW(snap, &mut pe);
        }
        CloseHandle(snap);
    }
    (parent, name)
}

/// The processes above `pid`, nearest first. A "parent" that started after
/// its child is some other program that reused the pid of a parent that has
/// exited, and ends the walk; a start time of 0 could not be read.
fn chain_from(
    pid: u32,
    parent: &HashMap<u32, u32>,
    name: &HashMap<u32, String>,
    start: impl Fn(u32) -> u64,
) -> Vec<Proc> {
    let mut chain = Vec::new();
    let mut seen = HashSet::from([pid]);
    let mut cur = pid;
    let mut cur_start = start(pid);
    while chain.len() < 40 {
        let Some(up) = parent.get(&cur).copied().filter(|up| *up != 0) else {
            break;
        };
        let Some(up_name) = name.get(&up) else { break };
        if !seen.insert(up) {
            break;
        }
        let up_start = start(up);
        if up_start != 0 && cur_start != 0 && up_start > cur_start {
            break;
        }
        chain.push(Proc {
            pid: up,
            name: up_name.clone(),
        });
        cur = up;
        cur_start = up_start;
    }
    chain
}

pub fn ancestors(pid: u32) -> Vec<Proc> {
    let (parent, name) = snapshot();
    chain_from(pid, &parent, &name, start_time)
}

/// This session's own agent process (the nearest one of `agent`'s) and the
/// agent process above it, if another agent started this one.
fn agents_in(chain: &[Proc], agent: &str) -> (Option<u32>, Option<u32>) {
    let names = agent_processes(agent);
    let Some(own) = chain
        .iter()
        .position(|p| names.iter().any(|n| same(n, &p.name)))
    else {
        return (None, None);
    };
    let outer = chain[own + 1..]
        .iter()
        .find(|p| ANY_AGENT.iter().any(|n| same(n, &p.name)));
    (Some(chain[own].pid), outer.map(|p| p.pid))
}

fn window_pid(hwnd: isize) -> u32 {
    let mut pid = 0u32;
    // SAFETY: a valid out pointer.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    pid
}

/// The process that owns `hwnd`.
pub fn pid_of_window(hwnd: i64) -> u32 {
    window_pid(hwnd as isize)
}

fn title(hwnd: isize) -> String {
    // SAFETY: the buffer is one longer than the length asked for.
    unsafe {
        let n = GetWindowTextLengthW(hwnd);
        if n <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; n as usize + 1];
        let got = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        String::from_utf16_lossy(&buf[..got.max(0) as usize])
    }
}

unsafe extern "system" fn collect(hwnd: isize, param: isize) -> i32 {
    // SAFETY: `param` is the map `visible_windows` passed, alive for the
    // whole enumeration; the rest are queries.
    unsafe {
        let map = &mut *(param as *mut HashMap<u32, Vec<isize>>);
        if IsWindowVisible(hwnd) == 0
            || GetWindow(hwnd, GW_OWNER) != 0
            || GetWindowTextLengthW(hwnd) == 0
        {
            return 1;
        }
        if GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & WS_EX_TOOLWINDOW != 0 {
            return 1;
        }
        let mut cloaked = 0i32;
        let hr = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut i32).cast(),
            4,
        );
        if hr == 0 && cloaked != 0 {
            return 1;
        }
        map.entry(window_pid(hwnd)).or_default().push(hwnd);
    }
    1
}

/// Each process's visible top-level windows, most recently used first.
fn visible_windows() -> HashMap<u32, Vec<isize>> {
    let mut map: HashMap<u32, Vec<isize>> = HashMap::new();
    // SAFETY: the callback only uses the map for the length of this call.
    unsafe {
        EnumWindows(collect, &mut map as *mut _ as isize);
    }
    map
}

/// A classic console window, or the Windows Terminal window that owns the
/// pseudo console. The bool is true for a classic console.
fn console_host_window() -> Option<(isize, bool)> {
    // SAFETY: attaches to the parent's console for two queries and lets go.
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return None;
        }
        let h = GetConsoleWindow();
        let found = if h == 0 {
            None
        } else if IsWindowVisible(h) != 0 {
            Some((h, true))
        } else {
            let owner = GetAncestor(h, GA_ROOTOWNER);
            (owner != 0 && owner != h && IsWindowVisible(owner) != 0).then_some((owner, false))
        };
        FreeConsole();
        found
    }
}

/// Runs inside the short-lived `--hook` process, whose ancestors are the
/// hook shell, the agent, and above that whatever app started the agent.
pub fn from_hook_process(project: &str, agent: &str) -> Found {
    let mut found = Found::default();
    let chain = ancestors(own_pid());
    let (own, outer) = agents_in(&chain, agent);
    if let Some(pid) = own {
        found.agent_pid = pid;
        found.agent_start = start_time(pid);
    }
    if let Some(pid) = outer {
        found.outer_pid = pid;
        found.outer_start = start_time(pid);
    }
    if let Some((window, classic)) = console_host_window() {
        found.window = window as i64;
        found.host = if classic {
            "Console".to_string()
        } else {
            label_for(window_pid(window), None)
        };
        return found;
    }
    // Otherwise the nearest ancestor that owns windows: Code.exe,
    // studio64.exe, Claude.exe and so on. With several windows (two VS Code
    // workspaces) the one whose title names the project is preferred.
    let windows = visible_windows();
    for p in &chain {
        if STOP_AT.iter().any(|n| same(n, &p.name)) {
            break;
        }
        let Some(list) = windows.get(&p.pid).filter(|l| !l.is_empty()) else {
            continue;
        };
        let needle = project.to_lowercase();
        let named = list
            .iter()
            .find(|h| !needle.is_empty() && title(**h).to_lowercase().contains(&needle));
        found.window = *named.unwrap_or(&list[0]) as i64;
        found.host = label_for(p.pid, Some(&p.name));
        break;
    }
    found
}

pub fn is_window(hwnd: i64) -> bool {
    // SAFETY: a query.
    hwnd != 0 && unsafe { IsWindow(hwnd as isize) } != 0
}

pub fn foreground() -> i64 {
    // SAFETY: a query.
    unsafe { GetForegroundWindow() as i64 }
}

/// Brings `hwnd` to the front. False when the window is gone.
pub fn focus(hwnd: i64) -> bool {
    if !is_window(hwnd) {
        return false;
    }
    let h = hwnd as isize;
    // SAFETY: plain window calls on a window that exists; the input queues
    // are detached again before returning.
    unsafe {
        if IsIconic(h) != 0 {
            ShowWindow(h, SW_RESTORE);
        }
        let fg = GetForegroundWindow();
        let fg_thread = if fg == 0 {
            0
        } else {
            GetWindowThreadProcessId(fg, std::ptr::null_mut())
        };
        let me = GetCurrentThreadId();
        let attached =
            fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, 1) != 0;
        BringWindowToTop(h);
        SetForegroundWindow(h);
        if attached {
            AttachThreadInput(me, fg_thread, 0);
        }
        if GetForegroundWindow() != h {
            SwitchToThisWindow(h, 1);
        }
    }
    true
}

/// Milliseconds since the last key or mouse input.
pub fn idle_ms() -> u64 {
    let mut info = LastInput {
        size: std::mem::size_of::<LastInput>() as u32,
        time: 0,
    };
    // SAFETY: a sized out struct.
    unsafe {
        if GetLastInputInfo(&mut info) == 0 {
            return 0;
        }
        u64::from(GetTickCount().wrapping_sub(info.time))
    }
}

/// Whether Windows says not to interrupt: a presentation, or a full-screen
/// app or game.
pub fn do_not_disturb() -> bool {
    let mut state = 0i32;
    // SAFETY: a valid out pointer.
    let hr = unsafe { SHQueryUserNotificationState(&mut state) };
    // QUNS_BUSY, QUNS_RUNNING_D3D_FULL_SCREEN, QUNS_PRESENTATION_MODE.
    hr == 0 && matches!(state, 2..=4)
}

/// The 8.3 short form of `path`, when the volume keeps short names.
pub fn short_path(path: &str) -> Option<String> {
    let long = wide(path);
    let mut buf = [0u16; 1024];
    // SAFETY: both buffers are NUL-terminated or sized as passed.
    let n = unsafe { GetShortPathNameW(long.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    (n > 0 && (n as usize) < buf.len()).then(|| from_wide(&buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> (HashMap<u32, u32>, HashMap<u32, String>) {
        // hook(1) <- bash(2) <- claude(3) <- bash(4) <- node(5) <- Code(6) <- explorer(7)
        let names = [
            "AevalBuddy.exe",
            "bash.exe",
            "claude.exe",
            "bash.exe",
            "node.exe",
            "Code.exe",
            "explorer.exe",
        ];
        let mut parent = HashMap::new();
        let mut name = HashMap::new();
        for (i, n) in names.iter().enumerate() {
            let pid = i as u32 + 1;
            parent.insert(pid, pid + 1);
            name.insert(pid, n.to_string());
        }
        parent.insert(7, 0);
        (parent, name)
    }

    #[test]
    fn the_chain_walks_up_and_finds_both_agents() {
        let (parent, name) = tree();
        let chain = chain_from(1, &parent, &name, |_| 0);
        let pids: Vec<u32> = chain.iter().map(|p| p.pid).collect();
        assert_eq!(pids, [2, 3, 4, 5, 6, 7]);
        // The nearest claude is this session's; node above it is the agent
        // that started it.
        assert_eq!(agents_in(&chain, "claude"), (Some(3), Some(5)));
        assert_eq!(agents_in(&chain, "codex"), (None, None));
        assert_eq!(agents_in(&chain, "windsurf"), (None, None));
    }

    #[test]
    fn a_recycled_parent_pid_ends_the_walk() {
        let (parent, name) = tree();
        // Process 4 started after its "child" 3: some other program.
        let start = |pid: u32| if pid == 4 { 2_000 } else { 1_000 - u64::from(pid) };
        let chain = chain_from(1, &parent, &name, start);
        let pids: Vec<u32> = chain.iter().map(|p| p.pid).collect();
        assert_eq!(pids, [2, 3]);
        // A loop in the table ends it too.
        let mut looped = parent.clone();
        looped.insert(3, 1);
        assert_eq!(chain_from(1, &looped, &name, |_| 0).len(), 2);
    }

    #[test]
    fn apps_get_the_names_people_use() {
        assert_eq!(label_of("Code.exe"), "VS Code");
        assert_eq!(label_of("WindowsTerminal.exe"), "Terminal");
        assert_eq!(label_of("Obsidian.exe"), "Obsidian");
        assert_eq!(label_of(""), "");
        assert_eq!(file_name("C:\\Program Files\\x\\Code.exe"), "Code.exe");
    }

    #[test]
    fn this_process_is_alive_and_has_a_start() {
        let me = own_pid();
        let start = start_time(me);
        assert!(start > 0);
        assert!(is_alive(me, start));
        assert!(!is_alive(me, start + 1), "another start time is another process");
        assert!(image_path(me).to_lowercase().ends_with(".exe"));
        assert!(!is_window(0));
    }
}
