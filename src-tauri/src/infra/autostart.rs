//! Two-layer autostart.
//!
//! 1. The hub itself: one `.lnk` in the per-user Startup folder, never a Run key.
//!    The file name follows the executable name (see `domain::identity`).
//! 2. Per-app: entries flagged `autostart` are ensured to be running whenever
//!    the hub starts (login or manual launch); an already-running process is
//!    never spawned twice.

use serde::Serialize;
use std::path::Path;
use std::time::Duration;

#[cfg(windows)]
use windows_sys::Win32::Foundation::CloseHandle;
#[cfg(windows)]
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};

use super::shortcut;
use crate::domain::config::Config;
use crate::domain::launch;

pub const AUTOSTART_ARG: &str = "--autostart";
const INITIAL_DELAY_MS: u64 = 2500;

/// True when a process with the same executable name is already running.
/// Comparing names (not PIDs) is the right granularity for "do not start it
/// again": the user may have launched it outside the hub.
#[cfg(windows)]
fn process_is_running(program: &Path) -> bool {
    let Some(expected) = program.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 {
            let name = String::from_utf16_lossy(&entry.szExeFile);
            let name = name.trim_end_matches('\0');
            if name.eq_ignore_ascii_case(expected) {
                let _ = CloseHandle(snapshot);
                return true;
            }
            ok = Process32NextW(snapshot, &mut entry);
        }
        let _ = CloseHandle(snapshot);
        false
    }
}

#[cfg(not(windows))]
fn process_is_running(_program: &Path) -> bool {
    false
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn detects_the_current_test_process() {
        let exe = std::env::current_exe().unwrap();
        assert!(process_is_running(&exe));
    }

    #[test]
    fn does_not_match_an_unknown_process_name() {
        assert!(!process_is_running(Path::new(
            "PortableHub-definitely-not-running-xyz.exe"
        )));
    }

    #[test]
    fn skips_an_autostart_entry_that_is_already_running() {
        let exe = std::env::current_exe().unwrap();
        let mut cfg = Config::default();
        let mut app = crate::domain::config::AppEntry::new("self", exe.to_string_lossy());
        app.autostart = true;
        app.skip_if_running = true;
        cfg.apps.push(app);

        let reports = launch_flagged(Path::new("."), &cfg, false);
        assert_eq!(reports.len(), 1);
        assert!(reports[0].ok);
        assert_eq!(reports[0].message, "already running");
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupStatus {
    pub enabled: bool,
    pub link_exists: bool,
    pub target_ok: bool,
    pub link_path: String,
    pub detail: String,
}

pub fn status() -> StartupStatus {
    let lnk = match shortcut::default_lnk() {
        Ok(p) => p,
        Err(e) => {
            return StartupStatus {
                enabled: false,
                link_exists: false,
                target_ok: false,
                link_path: String::new(),
                detail: e,
            }
        }
    };
    let exists = lnk.exists();
    let exe = std::env::current_exe().unwrap_or_default();
    let target_ok = exists && shortcut::is_valid(&lnk, &exe, AUTOSTART_ARG);
    let detail = if !exists {
        "未创建启动快捷方式".to_string()
    } else if !target_ok {
        "快捷方式存在但不是当前程序，请重建".to_string()
    } else {
        "已启用".to_string()
    };
    StartupStatus {
        enabled: target_ok,
        link_exists: exists,
        target_ok,
        link_path: lnk.to_string_lossy().to_string(),
        detail,
    }
}

pub fn enable() -> Result<StartupStatus, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let workdir = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let lnk = shortcut::default_lnk()?;
    if !shortcut::is_valid(&lnk, &exe, AUTOSTART_ARG) {
        shortcut::create(&lnk, &exe, AUTOSTART_ARG, &workdir)?;
    }
    let st = status();
    if !st.enabled {
        return Err(format!("快捷方式已写入但校验失败：{}", st.detail));
    }
    Ok(st)
}

pub fn disable() -> Result<StartupStatus, String> {
    let lnk = shortcut::default_lnk()?;
    shortcut::remove(&lnk)?;
    Ok(status())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchReport {
    pub name: String,
    pub ok: bool,
    pub message: String,
}

/// Launch every entry flagged `autostart`. With `dry_run` no process is spawned
/// and no delay is applied; the report carries the command that would run.
pub fn launch_flagged(root: &Path, cfg: &Config, dry_run: bool) -> Vec<LaunchReport> {
    let mut out = Vec::new();
    let mut first = true;

    for app in cfg.apps.iter().filter(|a| a.autostart && !a.hidden) {
        let plan = match launch::plan(root, app) {
            Ok(plan) => plan,
            Err(e) => {
                out.push(LaunchReport {
                    name: app.name.clone(),
                    ok: false,
                    message: e,
                });
                continue;
            }
        };

        // "Auto start" means "make sure it is running", not "spawn another copy".
        // The same rule applies whether the hub was started at login or by the user.
        if app.skip_if_running
            && !dry_run
            && process_is_running(Path::new(&plan.program))
        {
            out.push(LaunchReport {
                name: app.name.clone(),
                ok: true,
                message: "already running".to_string(),
            });
            continue;
        }

        if !dry_run {
            let delay = if first {
                INITIAL_DELAY_MS
            } else {
                cfg.autostart_stagger_ms + app.autostart_delay_ms
            };
            std::thread::sleep(Duration::from_millis(delay));
        }
        first = false;

        if dry_run {
            out.push(LaunchReport {
                name: app.name.clone(),
                ok: true,
                message: plan.command_line,
            });
            continue;
        }

        match launch::spawn(&plan) {
            Ok(pid) => out.push(LaunchReport {
                name: app.name.clone(),
                ok: true,
                message: format!("pid {pid}"),
            }),
            Err(e) => out.push(LaunchReport {
                name: app.name.clone(),
                ok: false,
                message: e,
            }),
        }
    }
    out
}
