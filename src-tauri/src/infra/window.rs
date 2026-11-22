//! Hide the windows of an app that was just autostarted.
//!
//! `STARTUPINFO.wShowWindow` is only a hint for an app's first `ShowWindow`
//! call; Electron, Qt and Tauri apps call `show()` themselves. A quiet launch
//! therefore also watches the new process tree for a few seconds and hides any
//! top-level window it opens. The watcher runs on a detached thread and never
//! blocks the caller; if it finds nothing, it simply expires.

#[cfg(windows)]
use std::time::{Duration, Instant};

/// How long a freshly started app is watched for a window to hide.
#[cfg(windows)]
const WATCH_DURATION: Duration = Duration::from_millis(12_000);
/// Poll cadence while watching.
#[cfg(windows)]
const POLL_INTERVAL: Duration = Duration::from_millis(120);
/// Upper bound on tracked processes, so a misbehaving app cannot grow the list.
#[cfg(windows)]
const MAX_TRACKED_PIDS: usize = 256;

/// Watch `pid` and its children, hiding their top-level windows as they appear.
///
/// `pid` 0 means the app was started elevated (`ShellExecuteW` gives no pid);
/// there is nothing to watch in that case.
pub fn hide_windows_soon(pid: u32) {
    if pid == 0 {
        return;
    }
    #[cfg(windows)]
    std::thread::spawn(move || watch(pid));
    #[cfg(not(windows))]
    let _ = pid;
}

#[cfg(windows)]
fn watch(root: u32) {
    let deadline = Instant::now() + WATCH_DURATION;
    let mut tracked = vec![root];
    while Instant::now() < deadline {
        tracked = expand(tracked, &snapshot());
        hide_windows_of(&tracked);
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Add every process whose parent is already tracked. PID reuse is not a
/// concern while the parent is alive, which it is for the whole watch.
#[cfg(windows)]
fn expand(tracked: Vec<u32>, nodes: &[(u32, u32)]) -> Vec<u32> {
    let mut out = tracked;
    loop {
        let mut added = false;
        for &(pid, parent) in nodes {
            if out.len() >= MAX_TRACKED_PIDS {
                return out;
            }
            if out.contains(&parent) && !out.contains(&pid) {
                out.push(pid);
                added = true;
            }
        }
        if !added {
            return out;
        }
    }
}

/// `(process id, parent process id)` for every process on the machine.
#[cfg(windows)]
fn snapshot() -> Vec<(u32, u32)> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut out = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() {
            return out;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 {
            out.push((entry.th32ProcessID, entry.th32ParentProcessID));
            ok = Process32NextW(snapshot, &mut entry);
        }
        let _ = CloseHandle(snapshot);
    }
    out
}

#[cfg(windows)]
struct HideContext<'a> {
    pids: &'a [u32],
}

#[cfg(windows)]
unsafe extern "system" fn hide_if_tracked(
    hwnd: windows_sys::Win32::Foundation::HWND,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::BOOL {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, GetWindowThreadProcessId, IsWindowVisible, ShowWindow, GWL_EXSTYLE,
        SW_HIDE, WS_EX_TOOLWINDOW,
    };

    let context = &mut *(lparam as *mut HideContext);
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == 0 || !context.pids.contains(&pid) {
        return 1;
    }
    if IsWindowVisible(hwnd) == 0 {
        return 1;
    }
    // Tool windows (tray popups, IME bars) are not the app's main surface.
    let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    if style & WS_EX_TOOLWINDOW != 0 {
        return 1;
    }
    ShowWindow(hwnd, SW_HIDE);
    1
}

#[cfg(windows)]
fn hide_windows_of(pids: &[u32]) {
    use windows_sys::Win32::UI::WindowsAndMessaging::EnumWindows;

    if pids.is_empty() {
        return;
    }
    let mut context = HideContext { pids };
    unsafe {
        EnumWindows(
            Some(hide_if_tracked),
            &mut context as *mut HideContext as isize,
        );
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn expands_children_transitively() {
        let nodes = [(10, 1), (11, 10), (12, 11), (99, 1)];
        assert_eq!(expand(vec![10], &nodes), vec![10, 11, 12]);
    }

    #[test]
    fn ignores_processes_outside_the_tree() {
        let nodes = [(10, 1), (20, 2)];
        assert_eq!(expand(vec![10], &nodes), vec![10]);
    }
}
