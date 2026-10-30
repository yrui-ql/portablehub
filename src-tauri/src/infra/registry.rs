//! Read-only registry checks.
//!
//! The app never writes Run keys; these helpers exist so the UI and selftest can
//! prove that autostart did not fall back to the registry, and so discovery can
//! recognise programs that were installed through an MSI/installer.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const WEBVIEW_CLIENT: &str =
    r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

const UNINSTALL_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL_KEY_WOW: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL_KEY_HKCU: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
/// Epic Games Launcher keeps one JSON manifest per installed game here.
const EPIC_MANIFESTS: &str = r"Epic\EpicGamesLauncher\Data\Manifests";

pub fn run_key_has(name: &str) -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ) {
        Ok(key) => key.get_value::<String, _>(name).is_ok(),
        Err(_) => false,
    }
}

pub fn webview2_version() -> Option<String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm.open_subkey_with_flags(WEBVIEW_CLIENT, KEY_READ).ok()?;
    key.get_value::<String, _>("pv").ok()
}

/// Directories reported by installed software (Uninstall registry entries).
///
/// Reads HKLM 64-bit, HKLM 32-bit (`WOW6432Node`) and HKCU. For each entry the
/// installer's `InstallLocation`, the directory of its `DisplayIcon` exe and,
/// when the uninstall command is not `msiexec`, the directory of that exe are
/// collected. Results are canonicalized where possible, lower-cased otherwise,
/// deduplicated, and drive roots are dropped. Every failure is ignored: this is
/// best-effort metadata used to filter discovery, never a hard requirement.
///
/// Call this once per discovery scan: enumerating the registry is not free.
pub fn installed_locations() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    collect_uninstall(&hklm, UNINSTALL_KEY, &mut out, &mut seen);
    collect_uninstall(&hklm, UNINSTALL_KEY_WOW, &mut out, &mut seen);
    collect_uninstall(&hkcu, UNINSTALL_KEY_HKCU, &mut out, &mut seen);
    // Epic's launcher installs games without Uninstall registry entries, so its
    // own manifests are the only reliable source for them.
    if let Some(program_data) = std::env::var_os("ProgramData") {
        let manifests = PathBuf::from(program_data).join(EPIC_MANIFESTS);
        for location in epic_manifest_locations(&manifests) {
            push_location(&location, &mut out, &mut seen);
        }
    }
    out
}

/// `InstallLocation` values from the `*.item` manifests in `dir`.
///
/// Missing directories, malformed JSON and unreadable files are skipped: this
/// is best-effort metadata for discovery, never a hard requirement.
pub fn epic_manifest_locations(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_item = path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("item"))
            .unwrap_or(false);
        if !is_item || !path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Some(location) = parse_epic_manifest(&text) {
            out.push(location);
        }
    }
    out
}

fn parse_epic_manifest(text: &str) -> Option<PathBuf> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let location = value.get("InstallLocation")?.as_str()?.trim();
    let location = location.trim_matches('"').trim();
    if location.is_empty() {
        return None;
    }
    // Epic writes mixed separators (`F:\Library/Game`); normalize to `\`.
    Some(PathBuf::from(location.replace('/', "\\")))
}

fn push_location(candidate: &Path, out: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
    let Some(normalized) = normalize_location(candidate) else {
        return;
    };
    let key = normalized.to_string_lossy().to_lowercase();
    if seen.insert(key) {
        out.push(normalized);
    }
}

fn collect_uninstall(
    hive: &RegKey,
    key_path: &str,
    out: &mut Vec<PathBuf>,
    seen: &mut HashSet<String>,
) {
    let Ok(key) = hive.open_subkey_with_flags(key_path, KEY_READ) else {
        return;
    };
    for name in key.enum_keys().flatten() {
        let Ok(sub) = key.open_subkey_with_flags(&name, KEY_READ) else {
            continue;
        };
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(value) = sub.get_value::<String, _>("InstallLocation") {
            if let Some(dir) = installed_dir_from_install_location(&value) {
                candidates.push(dir);
            }
        }
        if let Ok(value) = sub.get_value::<String, _>("DisplayIcon") {
            if let Some(dir) = installed_dir_from_display_icon(&value) {
                candidates.push(dir);
            }
        }
        if let Ok(value) = sub.get_value::<String, _>("UninstallString") {
            if let Some(dir) = installed_dir_from_uninstall_string(&value) {
                candidates.push(dir);
            }
        }
        for candidate in candidates {
            push_location(&candidate, out, seen);
        }
    }
}

/// `InstallLocation` may be quoted or padded; anything non-empty is a hint.
fn installed_dir_from_install_location(value: &str) -> Option<PathBuf> {
    let dir = dequote(value);
    if dir.is_empty() {
        None
    } else {
        Some(PathBuf::from(dir))
    }
}

/// `DisplayIcon` is `"<path to exe>",<index>` or `<path to exe>,<index>`.
fn installed_dir_from_display_icon(value: &str) -> Option<PathBuf> {
    let raw = display_icon_path(value)?;
    let path = Path::new(&raw);
    if !has_exe_extension(path) {
        return None;
    }
    path.parent().map(Path::to_path_buf)
}

/// `UninstallString` may be quoted or unquoted; `msiexec` entries carry no app
/// directory and must be skipped.
fn installed_dir_from_uninstall_string(value: &str) -> Option<PathBuf> {
    let exe = executable_from_command(value)?;
    let path = Path::new(&exe);
    if !has_exe_extension(path) {
        return None;
    }
    let file_name = path.file_name()?.to_string_lossy().to_lowercase();
    if file_name.starts_with("msiexec") {
        return None;
    }
    path.parent().map(Path::to_path_buf)
}

fn display_icon_path(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let path = if let Some(rest) = value.strip_prefix('"') {
        let end = rest.find('"')?;
        rest[..end].trim().to_string()
    } else {
        match value.rfind(',') {
            Some(index) if value[index + 1..].trim().parse::<i32>().is_ok() => {
                value[..index].trim().to_string()
            }
            _ => value.to_string(),
        }
    };
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

fn executable_from_command(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(rest) = value.strip_prefix('"') {
        let end = rest.find('"')?;
        let exe = rest[..end].trim();
        return if exe.is_empty() {
            None
        } else {
            Some(exe.to_string())
        };
    }
    // Unquoted command lines keep their arguments after the executable; the
    // first `.exe` is the safest place to cut (paths rarely contain one).
    let bytes = value.as_bytes();
    for end in 4..=bytes.len() {
        if bytes[end - 4..end].eq_ignore_ascii_case(b".exe") {
            return Some(value[..end].trim().to_string());
        }
    }
    value
        .split_whitespace()
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn has_exe_extension(path: &Path) -> bool {
    path.extension()
        .map(|ext| ext.eq_ignore_ascii_case("exe"))
        .unwrap_or(false)
}

fn dequote(value: &str) -> &str {
    value.trim().trim_matches('"').trim()
}

/// Canonical form where possible, lowered and separator-normalized otherwise.
fn normalize_location(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        return None;
    }
    let normalized = dunce::canonicalize(path).unwrap_or_else(|_| {
        PathBuf::from(path.to_string_lossy().replace('/', "\\").to_lowercase())
    });
    // A drive root is not an install location we can meaningfully filter by.
    if normalized.file_name().is_none() {
        return None;
    }
    Some(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_display_icon() {
        let dir = installed_dir_from_display_icon(r#""Root/Tools/Sample/Sample.exe",0"#).unwrap();
        assert_eq!(dir, PathBuf::from("Root/Tools/Sample"));
    }

    #[test]
    fn parses_unquoted_display_icon_with_index() {
        let dir = installed_dir_from_display_icon("Root\\Tools\\Sample\\Sample.exe,1").unwrap();
        assert_eq!(dir, PathBuf::from("Root\\Tools\\Sample"));
    }

    #[test]
    fn ignores_non_exe_display_icon() {
        assert!(installed_dir_from_display_icon("Root\\Tools\\Sample\\logo.ico").is_none());
    }

    #[test]
    fn parses_quoted_uninstall_command() {
        let dir =
            installed_dir_from_uninstall_string(r#""Root/Tools/Sample/uninstall.exe" /S"#).unwrap();
        assert_eq!(dir, PathBuf::from("Root/Tools/Sample"));
    }

    #[test]
    fn parses_unquoted_uninstall_command_with_arguments() {
        let dir = installed_dir_from_uninstall_string("Root\\Tools\\Sample\\uninst.exe /S /L=2")
            .unwrap();
        assert_eq!(dir, PathBuf::from("Root\\Tools\\Sample"));
    }

    #[test]
    fn skips_msiexec_uninstall_command() {
        assert!(installed_dir_from_uninstall_string(
            r#""Root/System/msiexec.exe" /I{00000000-0000-0000-0000-000000000000}"#
        )
        .is_none());
    }

    #[test]
    fn install_location_is_dequoted() {
        let dir = installed_dir_from_install_location(r#" "Root/Tools/Sample" "#).unwrap();
        assert_eq!(dir, PathBuf::from("Root/Tools/Sample"));
        assert!(installed_dir_from_install_location("   ").is_none());
    }

    #[test]
    fn parses_epic_manifest_locations() {
        let base = std::env::temp_dir().join(format!("ph-registry-epic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(
            base.join("valid.item"),
            r#"{"DisplayName":"Sample Game","InstallLocation":"Root/Library/SampleGame"}"#,
        )
        .unwrap();
        std::fs::write(base.join("broken.item"), "{ not json").unwrap();
        std::fs::write(base.join("no-location.item"), r#"{"DisplayName":"No Location"}"#).unwrap();
        std::fs::write(base.join("ignored.txt"), r#"{"InstallLocation":"Root/Other"}"#).unwrap();

        let locations = epic_manifest_locations(&base);
        assert_eq!(locations.len(), 1, "{locations:?}");
        assert_eq!(
            locations[0].to_string_lossy().replace('\\', "/"),
            "Root/Library/SampleGame"
        );
    }

    #[test]
    fn missing_epic_manifest_dir_is_empty() {
        let base = std::env::temp_dir().join(format!("ph-registry-noepic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        assert!(epic_manifest_locations(&base).is_empty());
    }
}
