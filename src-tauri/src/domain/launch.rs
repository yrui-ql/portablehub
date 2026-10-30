//! Build and execute a launch plan for an app entry.
//!
//! Normal launches use `Command::spawn`. Apps flagged `run_as_admin`, and normal
//! launches that fail with `ERROR_ELEVATION_REQUIRED` (os error 740), are
//! retried through `ShellExecuteW("runas")`, which shows the UAC prompt. An
//! elevated process has no usable pid, so `pid` is 0 and `elevated` is true.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::config::AppEntry;
use super::path::resolve;

/// `ERROR_ELEVATION_REQUIRED` (Win32).
const ERROR_ELEVATION_REQUIRED: i32 = 740;
/// `ERROR_CANCELLED` (UAC prompt dismissed, Win32).
#[cfg(windows)]
const ERROR_CANCELLED: u32 = 1223;
/// `SW_SHOWNORMAL`; avoids pulling in the WindowsAndMessaging feature.
#[cfg(windows)]
const SW_SHOWNORMAL: i32 = 1;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPlan {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub command_line: String,
    /// Launch through `ShellExecuteW("runas")` (UAC prompt).
    pub run_as_admin: bool,
}

/// Result of `spawn_with_receipt`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaunchOutcome {
    /// Process id, or 0 when the app was started elevated (ShellExecute gives none).
    pub pid: u32,
    pub elevated: bool,
}

pub fn plan(root: &Path, app: &AppEntry) -> Result<LaunchPlan, String> {
    if app.path.trim().is_empty() {
        return Err(format!("应用「{}」没有配置路径", app.name));
    }
    let program = resolve(root, &app.path);
    if !program.exists() {
        return Err(format!("找不到可执行文件：{}", program.display()));
    }

    let cwd = if app.work_dir.trim().is_empty() {
        program
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| root.to_path_buf())
    } else {
        resolve(root, &app.work_dir)
    };

    let command_line = compose(&program, &app.args);
    Ok(LaunchPlan {
        program: program.to_string_lossy().to_string(),
        args: app.args.clone(),
        cwd: cwd.to_string_lossy().to_string(),
        command_line,
        run_as_admin: app.run_as_admin,
    })
}

fn compose(program: &Path, args: &[String]) -> String {
    let mut s = quote(&program.to_string_lossy());
    for a in args {
        s.push(' ');
        s.push_str(&quote(a));
    }
    s
}

fn compose_args(args: &[String]) -> String {
    args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ")
}

fn quote(s: &str) -> String {
    if s.contains(' ') || s.contains('\t') {
        format!("\"{s}\"")
    } else {
        s.to_string()
    }
}

/// Spawn without elevation and return the child pid (kept for callers such as
/// autostart that only need "did it start").
pub fn spawn(plan: &LaunchPlan) -> Result<u32, String> {
    Ok(spawn_with_receipt(plan)?.pid)
}

/// Spawn, elevating through UAC when requested or required (os error 740).
pub fn spawn_with_receipt(plan: &LaunchPlan) -> Result<LaunchOutcome, String> {
    if plan.run_as_admin {
        run_as_admin(plan)?;
        return Ok(LaunchOutcome {
            pid: 0,
            elevated: true,
        });
    }
    match spawn_command(plan) {
        Ok(child) => Ok(LaunchOutcome {
            pid: child.id(),
            elevated: false,
        }),
        Err(e) if e.raw_os_error() == Some(ERROR_ELEVATION_REQUIRED) => {
            // The exe requires elevation: fall back to the UAC prompt.
            run_as_admin(plan)?;
            Ok(LaunchOutcome {
                pid: 0,
                elevated: true,
            })
        }
        Err(e) => Err(format!("启动失败：{e}（{}）", plan.program)),
    }
}

fn spawn_command(plan: &LaunchPlan) -> std::io::Result<std::process::Child> {
    let mut cmd = Command::new(&plan.program);
    cmd.args(&plan.args);
    if !plan.cwd.is_empty() {
        cmd.current_dir(PathBuf::from(&plan.cwd));
    }
    cmd.spawn()
}

#[cfg(windows)]
fn run_as_admin(plan: &LaunchPlan) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;

    fn wide(value: &str) -> Vec<u16> {
        std::ffi::OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let program = wide(&plan.program);
    let verb = wide("runas");
    let params = if plan.args.is_empty() {
        None
    } else {
        Some(wide(&compose_args(&plan.args)))
    };
    let cwd = if plan.cwd.is_empty() {
        None
    } else {
        Some(wide(&plan.cwd))
    };

    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            program.as_ptr(),
            params
                .as_ref()
                .map(|value| value.as_ptr())
                .unwrap_or(std::ptr::null()),
            cwd.as_ref()
                .map(|value| value.as_ptr())
                .unwrap_or(std::ptr::null()),
            SW_SHOWNORMAL,
        )
    } as isize;

    // ShellExecute returns a value <= 32 on failure.
    if result > 32 {
        return Ok(());
    }
    let code = unsafe { GetLastError() };
    if code == ERROR_CANCELLED {
        return Err("已取消管理员授权".to_string());
    }
    if code == 0 && result == 5 {
        // `SE_ERR_ACCESSDENIED` without a last-error code: the consent prompt
        // was dismissed/refused.
        return Err("已取消或拒绝了管理员授权".to_string());
    }
    if code != 0 {
        Err(format!("需要管理员权限，提权启动失败（错误码 {code}）"))
    } else {
        Err(format!(
            "需要管理员权限，提权启动失败（ShellExecute 返回 {result}）"
        ))
    }
}

#[cfg(not(windows))]
fn run_as_admin(_plan: &LaunchPlan) -> Result<(), String> {
    Err("当前平台不支持以管理员身份启动".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, args: Vec<&str>) -> AppEntry {
        let mut e = AppEntry::new("Demo", path);
        e.args = args.into_iter().map(|s| s.to_string()).collect();
        e
    }

    #[test]
    fn builds_command_line_with_spaces() {
        let dir = std::env::temp_dir().join(format!("ph-launch-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Apps/Demo")).unwrap();
        let exe = dir.join("Apps/Demo/Demo App.exe");
        std::fs::write(&exe, b"MZ").unwrap();

        let p = plan(&dir, &entry("Apps/Demo/Demo App.exe", vec!["--flag", "a b"])).unwrap();
        assert!(p.command_line.starts_with('"'), "program path must be quoted: {}", p.command_line);
        assert!(p.command_line.contains("Demo App.exe\""));
        assert!(p.command_line.contains("\"a b\""));
        assert_eq!(p.cwd, dir.join("Apps/Demo").to_string_lossy());
        assert!(!p.run_as_admin);
        assert!(!p.command_line.contains("run_as"));
    }

    #[test]
    fn plan_carries_run_as_admin() {
        let dir = std::env::temp_dir().join(format!("ph-launch-admin-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Apps/Demo")).unwrap();
        let exe = dir.join("Apps/Demo/Demo.exe");
        std::fs::write(&exe, b"MZ").unwrap();

        let mut app = entry("Apps/Demo/Demo.exe", vec![]);
        app.run_as_admin = true;
        let p = plan(&dir, &app).unwrap();
        assert!(p.run_as_admin);
        assert_eq!(p.program, exe.to_string_lossy());
    }

    #[test]
    fn spawns_a_real_process_without_elevation() {
        let dir = std::env::temp_dir();
        let shell = std::env::var("COMSPEC").expect("COMSPEC must be set on Windows");
        let mut app = AppEntry::new("cmd", shell);
        app.args = vec!["/C".into(), "exit 0".into()];
        let p = plan(&dir, &app).unwrap();
        let outcome = spawn_with_receipt(&p).expect("spawn should succeed");
        assert!(!outcome.elevated);
        assert!(outcome.pid > 0);
        // The plain helper stays compatible with autostart.
        assert!(spawn(&p).unwrap() > 0);
    }

    #[test]
    fn missing_exe_is_error() {
        let dir = std::env::temp_dir();
        let err = plan(&dir, &entry("nope/none.exe", vec![])).unwrap_err();
        assert!(err.contains("找不到"));
    }
}
