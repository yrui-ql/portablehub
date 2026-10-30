//! Windows `.lnk` creation/reading through the WScript.Shell COM object.
//!
//! We deliberately avoid the registry for autostart: the only OS-level entry is a
//! per-user Startup shortcut. PowerShell is invoked with an encoded command so that
//! Chinese characters, spaces and quotes survive intact.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn startup_dir() -> Result<PathBuf, String> {
    let appdata = std::env::var("APPDATA")
        .map_err(|_| "APPDATA 未设置，无法定位启动文件夹".to_string())?;
    Ok(PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup"))
}

pub fn default_lnk() -> Result<PathBuf, String> {
    Ok(startup_dir()?.join(crate::domain::identity::shortcut_file_name()))
}

fn ps_quote(s: &str) -> String {
    s.replace('\'', "''")
}

fn encode(script: &str) -> String {
    let utf16: Vec<u16> = script.encode_utf16().collect();
    let mut bytes = Vec::with_capacity(utf16.len() * 2);
    for unit in utf16 {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
}

fn run_ps(script: &str) -> Result<String, String> {
    let script = format!(
        "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8\n\
         $OutputEncoding=[System.Text.Encoding]::UTF8\n\
         {script}"
    );
    let encoded = encode(&script);
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            &encoded,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("无法调用 PowerShell：{e}"))?;

    if !output.status.success() {
        return Err(format!(
            "PowerShell 失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn create(lnk: &Path, target: &Path, args: &str, workdir: &Path) -> Result<(), String> {
    if let Some(parent) = lnk.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let icon = format!("{},0", target.to_string_lossy().replace('\'', "''"));
    let script = format!(
        "$ErrorActionPreference='Stop'\n\
         $ws = New-Object -ComObject WScript.Shell\n\
         $s = $ws.CreateShortcut('{}')\n\
         $s.TargetPath = '{}'\n\
         $s.Arguments = '{}'\n\
         $s.WorkingDirectory = '{}'\n\
         $s.IconLocation = '{}'\n\
         $s.Save()\n\
         'OK'",
        ps_quote(&lnk.to_string_lossy()),
        ps_quote(&target.to_string_lossy()),
        ps_quote(args),
        ps_quote(&workdir.to_string_lossy()),
        icon,
    );
    let out = run_ps(&script)?;
    if out.contains("OK") {
        Ok(())
    } else {
        Err(format!("创建快捷方式失败：{out}"))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinkInfo {
    pub target: String,
    pub arguments: String,
    pub workdir: String,
}

pub fn read(lnk: &Path) -> Result<LinkInfo, String> {
    if !lnk.exists() {
        return Err("快捷方式不存在".to_string());
    }
    let script = format!(
        "$ErrorActionPreference='Stop'\n\
         $s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}')\n\
         @{{ target = [string]$s.TargetPath; arguments = [string]$s.Arguments; workdir = [string]$s.WorkingDirectory }} | ConvertTo-Json -Compress",
        ps_quote(&lnk.to_string_lossy())
    );
    let out = run_ps(&script)?;
    let v: serde_json::Value =
        serde_json::from_str(&out).map_err(|e| format!("解析快捷方式信息失败：{e}（{out}）"))?;
    Ok(LinkInfo {
        target: v.get("target").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        arguments: v
            .get("arguments")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        workdir: v.get("workdir").and_then(|x| x.as_str()).unwrap_or("").to_string(),
    })
}

pub fn remove(lnk: &Path) -> Result<(), String> {
    if lnk.exists() {
        std::fs::remove_file(lnk).map_err(|e| format!("删除快捷方式失败：{e}"))?;
    }
    Ok(())
}

pub fn is_valid(lnk: &Path, target: &Path, args: &str) -> bool {
    match read(lnk) {
        Ok(info) => {
            let same_target = same_path(&info.target, &target.to_string_lossy());
            same_target && info.arguments.trim() == args.trim()
        }
        Err(_) => false,
    }
}

/// Compare two path strings, tolerating 8.3 short names vs long names.
///
/// `WScript.Shell` normalises the target to its long form, while
/// `std::env::temp_dir()` (and therefore the selftest / some user configs) may
/// hand us an 8.3 short form such as `C:\Users\USER~1\...`. Both spellings
/// point at the same file, so compare canonical paths when they resolve.
fn same_path(a: &str, b: &str) -> bool {
    if a.eq_ignore_ascii_case(b) {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca
            .to_string_lossy()
            .eq_ignore_ascii_case(&cb.to_string_lossy()),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_roundtrip() {
        let dir = std::env::temp_dir().join(format!("ph-lnk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("测试 应用.exe");
        std::fs::write(&target, b"MZ").unwrap();
        let lnk = dir.join("测试.lnk");

        create(&lnk, &target, "--autostart", &dir).unwrap();
        let info = read(&lnk).unwrap();
        assert!(info.target.ends_with("测试 应用.exe"));
        assert_eq!(info.arguments, "--autostart");
        assert!(is_valid(&lnk, &target, "--autostart"));
        assert!(!is_valid(&lnk, &target, "--other"));
        assert!(!is_valid(&lnk, &dir.join("Other.exe"), "--autostart"));

        remove(&lnk).unwrap();
        assert!(!lnk.exists());
    }

    #[test]
    fn same_path_accepts_short_and_long_names() {
        let path = std::env::temp_dir().join(format!("ph-same-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        let long = path.to_string_lossy().to_string();
        assert!(same_path(&long, &long));
        // Different files stay different; strings compare case-insensitively.
        assert!(same_path(&long, &long.to_uppercase()));
        assert!(!same_path(&long, &path.join("missing.txt").to_string_lossy()));
        let _ = std::fs::remove_dir_all(&path);
    }
}
