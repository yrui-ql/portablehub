//! Global discovery of portable apps across fixed drives.
//!
//! Heuristics are deliberately conservative:
//!   · skip system areas (`Program Files`, `Windows`, `steamapps`, `XboxGames`, …)
//!     at any depth
//!   · skip directories reported by installed software (Uninstall registry and
//!     Epic Games Launcher manifests)
//!   · skip loose installer/updater packages (filename markers) and loose exes
//!     sitting in download/staging folders
//!   · a folder is a candidate when it has a main exe and no uninstaller
//!   · never descend into a folder already classified as an app
//!   · bounded depth (6) and a visit budget, so a full-drive scan stays quick

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::path::resolve;
use super::scan;
use crate::infra::registry;

const MAX_DEPTH: usize = 6;
const VISIT_BUDGET: usize = 20_000;
const SIZE_ENTRY_BUDGET: usize = 50_000;

/// Build/dependency folders never contain portable apps; skip at any depth so
/// raising [`MAX_DEPTH`] does not surface compiler artifacts (BUG-0017).
const BUILD_COMPONENTS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".git",
    ".svn",
    ".hg",
    ".next",
    ".nuxt",
    ".gradle",
    "__pycache__",
    ".venv",
    "venv",
];

/// Directory names that only ever hold installed/system software, skipped at
/// any depth (a second drive's `Program Files` must not be scanned either).
const SYSTEM_COMPONENTS: &[&str] = &[
    "program files",
    "program files (x86)",
    "programdata",
    "windows",
    "windowsapps",
    "common files",
    "msocache",
    "config.msi",
    "$recycle.bin",
    "system volume information",
    "recovery",
    "$winreagent",
    "onedrivetemp",
    // Launcher libraries hold installed games. `steamapps` covers
    // `...\steamapps\common\<Game>`; `xboxgames` is the Xbox app's library root.
    "steamapps",
    "xboxgames",
];

const SKIP_TOP: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "users",
    "$recycle.bin",
    "system volume information",
    "recovery",
    "perf logs",
    "msocache",
    "config.msi",
    "$winreagent",
    "onedrivetemp",
    "intel",
    "amd",
    "nvidia",
    "drivers",
    "temp",
    "tmp",
    "cache",
    "node_modules",
    ".git",
    "target",
    "dist",
    "found.000",
];

/// Normalize a scan root. Windows treats a bare `"E:"` as the current directory
/// on that drive, not its root; callers that pass drive letters must append a
/// separator (BUG-0019).
pub fn normalize_root(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed.len() == 2 && trimmed.ends_with(':') {
        PathBuf::from(format!("{trimmed}\\"))
    } else {
        PathBuf::from(trimmed)
    }
}

/// Generic executable stems that make poor display names (BUG-0024): the app is
/// usually identified by its containing folder instead.
const GENERIC_EXE_NAMES: &[&str] = &[
    "game", "launcher", "start", "app", "main", "client", "play", "run", "player",
    "loader", "program", "unity", "unreal", "index", "boot", "engine", "application",
];

/// Folders with these names are generic containers: a loose exe directly inside
/// them is not a meaningful app candidate. `bin` is intentionally absent (many
/// real apps keep their main exe in `bin`).
const GENERIC_CONTAINER_NAMES: &[&str] = &[
    "game", "app", "client", "launcher", "application", "program", "engine",
];

/// Tool/library subfolders whose loose exes are support files, not apps.
/// `bin` is intentionally absent: many real apps live in `bin`.
const TOOL_COMPONENT_NAMES: &[&str] = &[
    "tool", "tools", "loader", "loaders", "redist", "runtime", "plugins", "mods", "patch",
    "crack",
];

/// Unambiguous immediate-parent names for support files. `tool` is intentionally
/// absent so a real app in a folder literally named `Tool` still appears.
const TOOL_PARENT_NAMES: &[&str] = &[
    "loaders", "loader", "redist", "runtime", "plugins", "mods", "patch", "crack",
];

/// Helper executables shipped inside game folders; never portable apps.
const TOOL_EXE_NAMES: &[&str] = &[
    "chromedriver",
    "editbin",
    "getpefileinfo",
    "geckodriver",
    "msedgedriver",
    "dumpbin",
    "sigcheck",
];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Discovered {
    pub name: String,
    pub path: String,
    pub exe: String,
    pub size: u64,
    pub kind: String,
    pub score: i32,
}

pub fn drives() -> Vec<PathBuf> {
    ('C'..='Z')
        .filter_map(|c| {
            let p = PathBuf::from(format!("{c}:\\"));
            if p.is_dir() {
                Some(p)
            } else {
                None
            }
        })
        .collect()
}

pub fn default_roots() -> Vec<PathBuf> {
    drives()
        .into_iter()
        .filter(|p| !p.to_string_lossy().to_uppercase().starts_with("C:"))
        .collect()
}

/// Collapse everything inside a game library (`...\Games\<game>\...`) into one
/// item per game folder.
///
/// Real libraries often look like `K:\GAMES\hGame\<game>\Tool\...`; without this
/// pass the hub lists the game's helpers (`nwjc.exe`, `zsyncmake`, ...) as
/// separate apps, and the classifier then labels each helper by its own
/// language. One game must be one card. The library container (`hGame`) is
/// detected when it has no main exe but many game directories.
fn collapse_game_libraries(items: Vec<Discovered>) -> Vec<Discovered> {
    let mut cache: HashMap<String, bool> = HashMap::new();
    let mut out: Vec<Discovered> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();

    for item in items {
        let Some(unit) = game_library_unit(Path::new(&item.path), &mut cache) else {
            out.push(item);
            continue;
        };
        let key = normalized_path_key(&unit);
        if let Some(&slot) = index.get(&key) {
            let existing = &mut out[slot];
            // Prefer a folder candidate's exe over a helper file discovered first.
            if item.kind == "folder" && existing.kind != "folder" {
                existing.exe = item.exe;
                existing.kind = item.kind;
                existing.score = item.score.max(existing.score);
            } else if item.score > existing.score {
                existing.score = item.score;
            }
            continue;
        }
        let unit_name = unit
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let name = folder_display_name(&unit, &unit_name);
        let size = if unit.is_dir() { dir_size(&unit) } else { item.size };
        // The first discovered item may be an internal helper; prefer the
        // game's main exe so a collapsed card launches the game, not a tool.
        let exe = if item.kind == "folder" {
            item.exe
        } else {
            scan::find_main_exe(&unit)
                .map(|(path, _)| path.to_string_lossy().to_string())
                .unwrap_or(item.exe)
        };
        out.push(Discovered {
            name,
            path: unit.to_string_lossy().to_string(),
            exe,
            size,
            kind: "folder".to_string(),
            score: 900 + (size / 1_048_576).min(50) as i32,
        });
        index.insert(key, out.len() - 1);
    }
    out
}

/// Return the game folder that owns `path`, when `path` lives below a folder
/// named `Games`. A normal library (`D:\Games\<game>`) maps to `<game>`; a
/// collection (`K:\GAMES\hGame\<game>`) maps to `<game>` as well.
fn game_library_unit(path: &Path, cache: &mut HashMap<String, bool>) -> Option<PathBuf> {
    let components: Vec<_> = path.components().collect();
    let games_idx = components.iter().position(|component| match component {
        std::path::Component::Normal(name) => {
            name.to_string_lossy().eq_ignore_ascii_case("games")
        }
        _ => false,
    })?;
    if components.len() <= games_idx + 1 {
        return None;
    }
    let child: PathBuf = components[..=games_idx + 1].iter().collect();
    if !child.is_dir() {
        return None;
    }
    let key = normalized_path_key(&child);
    let collection = *cache
        .entry(key)
        .or_insert_with(|| is_game_collection(&child));
    if collection {
        if components.len() <= games_idx + 2 {
            return Some(child);
        }
        let second: PathBuf = components[..=games_idx + 2].iter().collect();
        if second.is_dir() {
            return Some(second);
        }
    }
    Some(child)
}

/// A library container has no main exe directly and many child game folders.
fn is_game_collection(dir: &Path) -> bool {
    if scan::find_main_exe_direct(dir).is_some() {
        return false;
    }
    let games = fs::read_dir(dir)
        .map(|entries| entries.flatten().filter(|entry| entry.path().is_dir()).count())
        .unwrap_or(0);
    games >= 6
}

pub fn discover(
    roots: &[PathBuf],
    exclude: &[PathBuf],
    limit: usize,
    cancel: &AtomicBool,
) -> Vec<Discovered> {
    // Enumerating the registry is not free: do it once for the whole scan.
    let installed = registry::installed_locations();
    discover_with(roots, exclude, limit, cancel, &installed)
}

/// [`discover`] with an explicit installed-location set.
///
/// The command path feeds the real registry result; tests inject synthetic
/// values so behaviour stays deterministic and independent of the machine.
pub fn discover_with(
    roots: &[PathBuf],
    exclude: &[PathBuf],
    limit: usize,
    cancel: &AtomicBool,
    installed: &[PathBuf],
) -> Vec<Discovered> {
    let installed_keys = installed_keys(installed);
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut budget = VISIT_BUDGET;
    for root in roots {
        scan_dir(
            root,
            0,
            root,
            exclude,
            &installed_keys,
            &mut out,
            &mut seen,
            &mut budget,
            cancel,
        );
        if budget == 0 || cancel.load(Ordering::Relaxed) {
            break;
        }
    }

    out = collapse_game_libraries(out);
    out.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out.truncate(limit);
    out
}

/// Drop discovered items that are already registered apps.
///
/// `existing` holds the stored app paths, which may be relative to `root` or
/// absolute depending on how each app was imported. A discovered item points at
/// a concrete exe (both loose files and folders set `exe`); when that field is
/// empty the item `path` is compared instead. Comparison goes through
/// [`scan::paths_equal`] so the same file registered under another import mode
/// is still recognized, while two different programs that happen to share a
/// display name are both kept. Order of the remaining items is preserved.
pub fn filter_registered(
    root: &Path,
    existing: &[String],
    items: Vec<Discovered>,
) -> Vec<Discovered> {
    if existing.is_empty() {
        return items;
    }
    let registered: Vec<PathBuf> = existing.iter().map(|p| resolve(root, p)).collect();
    items
        .into_iter()
        .filter(|item| {
            let target = if item.exe.is_empty() {
                PathBuf::from(&item.path)
            } else {
                PathBuf::from(&item.exe)
            };
            !registered
                .iter()
                .any(|known| scan::paths_equal(known, &target))
        })
        .collect()
}

fn scan_dir(
    dir: &Path,
    depth: usize,
    drive_root: &Path,
    exclude: &[PathBuf],
    installed_keys: &[String],
    out: &mut Vec<Discovered>,
    seen: &mut HashSet<String>,
    budget: &mut usize,
    cancel: &AtomicBool,
) {
    if *budget == 0 || depth >= MAX_DEPTH || cancel.load(Ordering::Relaxed) {
        return;
    }
    if exclude.iter().any(|e| same_path(e, dir)) {
        return;
    }
    if is_system_area(dir) {
        return;
    }
    if is_installed_path(dir, installed_keys) {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    *budget = budget.saturating_sub(1);

    let is_top = depth == 0;
    for entry in entries.flatten() {
        if *budget == 0 {
            return;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_file() {
            if depth > 0
                && scan::is_exe_file(&path)
                && !scan::is_blocked_exe(&path)
                && !is_system_area(&path)
                && !is_installed_path(&path, installed_keys)
                && !looks_like_installer(&path)
                && !is_download_area(&path)
                && !is_tool_loose_exe(&path)
            {
                push_file(&path, out, seen);
            }
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        let lower = name.to_lowercase();
        if is_top && SKIP_TOP.contains(&lower.as_str()) {
            continue;
        }
        if lower.starts_with('$') || lower.starts_with('.') {
            continue;
        }
        if exclude.iter().any(|e| same_path(e, &path)) {
            continue;
        }
        if is_system_area(&path) {
            continue;
        }
        if is_installed_path(&path, installed_keys) {
            continue;
        }
        if scan::looks_installed(&path) {
            continue;
        }
        if is_tool_folder(&path) {
            // A folder that is really a helper/tool directory must not become a
            // card (and must not be descended into either).
            continue;
        }
        if let Some((exe, score)) = scan::find_main_exe_direct(&path) {
            push_folder(&path, &name, &exe, score, out, seen);
            continue;
        }
        scan_dir(
            &path,
            depth + 1,
            drive_root,
            exclude,
            installed_keys,
            out,
            seen,
            budget,
            cancel,
        );
    }
    let _ = drive_root;
}

/// Directory names that mark installed/system software or build artifacts,
/// skipped at any depth.
fn is_system_area(path: &Path) -> bool {
    path.components().any(|component| match component {
        std::path::Component::Normal(name) => {
            let name = name.to_string_lossy().to_lowercase();
            SYSTEM_COMPONENTS.contains(&name.as_str()) || BUILD_COMPONENTS.contains(&name.as_str())
        }
        _ => false,
    })
}

/// Pre-normalize injected install locations once per scan.
fn installed_keys(installed: &[PathBuf]) -> Vec<String> {
    let mut keys = Vec::new();
    for location in installed {
        let key = normalized_path_key(location);
        let key = key.trim_end_matches('\\').to_string();
        if !key.is_empty() {
            keys.push(key);
        }
    }
    keys
}

fn normalized_path_key(path: &Path) -> String {
    dunce::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('/', "\\")
        .to_lowercase()
}

/// True when `path` equals or lives below one of the installed locations.
///
/// The separator check keeps `...\App` from swallowing a sibling `...\AppOther`.
fn is_installed_path(path: &Path, installed_keys: &[String]) -> bool {
    if installed_keys.is_empty() {
        return false;
    }
    let candidate = normalized_path_key(path);
    installed_keys.iter().any(|key| {
        candidate == *key
            || (candidate.len() > key.len()
                && candidate.starts_with(key.as_str())
                && candidate.as_bytes()[key.len()] == b'\\')
    })
}

/// Filename fragments of installer / updater / redistributable packages.
///
/// Matching is case-insensitive on the compacted stem (separators removed), so
/// `AppSetup.exe`, `MyInstaller.exe`, `vcredist_x64.exe` and `install_app.exe`
/// are all recognised. Short, collision-prone words like `patch` are matched as
/// whole tokens only, so `Dispatch.exe` is not mistaken for a patch.
const INSTALLER_FRAGMENTS: &[&str] = &[
    "setup",
    "install",
    "unins",
    "update",
    "updater",
    "vcredist",
    "redist",
    "repair",
];
const INSTALLER_TOKENS: &[&str] = &["patch"];
/// Chinese markers are matched as substrings: `更新程序`, `安装包`, `卸载工具`.
const INSTALLER_CJK_MARKERS: &[&str] = &["安装", "更新", "卸载"];
/// A name that advertises portability is never treated as an installer, e.g.
/// `InstallerToolPortable.exe` stays discoverable.
const PORTABLE_HINTS: &[&str] = &["portable", "green", "免安装", "绿色"];

/// True when a loose exe looks like an installer/updater package rather than a
/// runnable portable program. Folder candidates are unaffected: there the
/// uninstaller heuristic and the main-exe scoring already decide.
fn looks_like_installer(path: &Path) -> bool {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if stem.is_empty() {
        return false;
    }
    if PORTABLE_HINTS.iter().any(|hint| stem.contains(hint)) {
        return false;
    }
    if INSTALLER_CJK_MARKERS
        .iter()
        .any(|marker| stem.contains(marker))
    {
        return true;
    }
    let compact: String = stem.chars().filter(|c| c.is_alphanumeric()).collect();
    if INSTALLER_FRAGMENTS
        .iter()
        .any(|fragment| compact.contains(fragment))
    {
        return true;
    }
    let tokens: Vec<&str> = stem
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect();
    tokens
        .iter()
        .any(|token| INSTALLER_TOKENS.contains(token))
}

/// Directory names of download/staging folders. A loose exe lying directly in
/// one of these is an installer package far more often than a portable app, but
/// folders inside them are still scanned so extracted portable apps are found.
const DOWNLOAD_AREA_MARKERS: &[&str] = &["download", "softmgr", "incoming", "下载"];

fn is_download_area(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    parent.components().any(|component| match component {
        std::path::Component::Normal(name) => {
            let name = name.to_string_lossy().to_lowercase();
            DOWNLOAD_AREA_MARKERS
                .iter()
                .any(|marker| name.contains(marker))
        }
        _ => false,
    })
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
    norm(a) == norm(b)
}

fn is_generic_display_name(name: &str) -> bool {
    let name = name.trim().to_lowercase();
    GENERIC_EXE_NAMES.contains(&name.as_str())
}

/// Display name for a loose exe: generic stems (`Game.exe`, `Launcher.exe`)
/// borrow the (non-generic) parent folder name so different games/tools stay
/// distinguishable (BUG-0024).
fn loose_exe_display_name(exe: &Path) -> String {
    let stem = exe
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    if is_generic_display_name(&stem) {
        if let Some(parent) = exe.parent().and_then(|parent| parent.file_name()) {
            let parent = parent.to_string_lossy().to_string();
            if !parent.trim().is_empty() && !is_generic_display_name(&parent) {
                return parent;
            }
        }
    }
    stem
}

/// Display name for a folder candidate: a generic folder name
/// (`...\\Pokemon\\Game`) borrows the parent folder name (`Pokemon`).
fn folder_display_name(dir: &Path, name: &str) -> String {
    if is_generic_display_name(name) {
        if let Some(parent) = dir.parent().and_then(|parent| parent.file_name()) {
            let parent = parent.to_string_lossy().to_string();
            if !parent.trim().is_empty() && !is_generic_display_name(&parent) {
                return parent;
            }
        }
    }
    name.to_string()
}

/// True when a loose exe is a support file rather than a portable app: it lives
/// under a tool/library folder, or has a well-known helper executable name.
fn is_tool_loose_exe(exe: &Path) -> bool {
    let stem = exe
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if TOOL_EXE_NAMES.contains(&stem.as_str()) {
        return true;
    }
    let Some(parent) = exe.parent() else {
        return false;
    };
    // Ancestors above the immediate parent: `...\Tool\loaders\driver.exe`.
    for ancestor in parent.ancestors().skip(1) {
        if let Some(name) = ancestor.file_name() {
            let name = name.to_string_lossy().to_lowercase();
            if TOOL_COMPONENT_NAMES.contains(&name.as_str()) {
                return true;
            }
        }
    }
    // Immediate parent: only unambiguous support names and generic containers,
    // so `Tool/Good.exe` (a real app in a folder called Tool) still appears.
    if let Some(name) = parent.file_name() {
        let name = name.to_string_lossy().to_lowercase();
        if TOOL_PARENT_NAMES.contains(&name.as_str())
            || GENERIC_CONTAINER_NAMES.contains(&name.as_str())
        {
            return true;
        }
    }
    false
}

/// True when a *folder* candidate is really a helper/tool directory, e.g.
/// `...\Tool\loaders\editbin` (BUG-0024 follow-up). Checked before
/// `push_folder`, so such folders are neither listed nor descended into.
///
/// `Tools/Good` is not affected: only unambiguous support parents
/// (`loaders`/`redist`/…) count, never `tool(s)` itself.
fn is_tool_folder(dir: &Path) -> bool {
    if let Some(name) = dir.file_name() {
        let name = name.to_string_lossy().to_lowercase();
        if TOOL_EXE_NAMES.contains(&name.as_str()) {
            return true;
        }
    }
    if let Some(parent) = dir.parent().and_then(|parent| parent.file_name()) {
        let parent = parent.to_string_lossy().to_lowercase();
        if TOOL_PARENT_NAMES.contains(&parent.as_str()) {
            return true;
        }
    }
    false
}

fn push_file(exe: &Path, out: &mut Vec<Discovered>, seen: &mut HashSet<String>) {
    let key = exe.to_string_lossy().to_lowercase();
    if !seen.insert(key) {
        return;
    }
    let name = loose_exe_display_name(exe);
    if name.is_empty() {
        return;
    }
    out.push(Discovered {
        name,
        path: exe.to_string_lossy().to_string(),
        exe: exe.to_string_lossy().to_string(),
        size: fs::metadata(exe).map(|m| m.len()).unwrap_or(0),
        kind: "file".to_string(),
        score: 100 + (fs::metadata(exe).map(|m| m.len()).unwrap_or(0) / 1_048_576).min(50) as i32,
    });
}

fn push_folder(
    dir: &Path,
    name: &str,
    exe: &Path,
    score: i32,
    out: &mut Vec<Discovered>,
    seen: &mut HashSet<String>,
) {
    let key = exe.to_string_lossy().to_lowercase();
    if !seen.insert(key) {
        return;
    }
    out.push(Discovered {
        name: folder_display_name(dir, name),
        path: dir.to_string_lossy().to_string(),
        exe: exe.to_string_lossy().to_string(),
        size: dir_size(dir),
        kind: "folder".to_string(),
        score: score + 500,
    });
}

fn dir_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    let mut count = 0usize;
    for entry in WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            count += 1;
            if count >= SIZE_ENTRY_BUDGET {
                break;
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ph-discover-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn fake_exe(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut d = vec![0u8; 256];
        d[0] = b'M';
        d[1] = b'Z';
        d[0x3C..0x40].copy_from_slice(&(128u32).to_le_bytes());
        d[128..132].copy_from_slice(b"PE\0\0");
        d[152..154].copy_from_slice(&(0x10bu16).to_le_bytes());
        d[196..198].copy_from_slice(&(2u16).to_le_bytes());
        fs::write(path, d).unwrap();
    }

    #[test]
    fn finds_nested_portable_folder() {
        let root = tempdir("nested");
        fake_exe(&root.join("tools/Alpha/Alpha.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        assert!(found.iter().any(|d| d.name == "Alpha"), "{found:?}");
    }

    #[test]
    fn finds_deeply_nested_portable_folder() {
        // BUG-0017: real portable apps can live five levels below the drive
        // root (e.g. <drive>\<workspace>\<env>\<bucket>\<tool>).
        let root = tempdir("deep");
        fake_exe(&root.join("workspace/env/shared/tool/Tool/Tool.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        assert!(found.iter().any(|d| d.name == "Tool"), "{found:?}");
    }

    #[test]
    fn skips_build_directories_at_any_depth() {
        let root = tempdir("builddirs");
        fake_exe(&root.join("project/node_modules/pkg/bin.exe"));
        fake_exe(&root.join("project/target/release/tool.exe"));
        fake_exe(&root.join("project/tools/Good/Good.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        assert!(found.iter().any(|d| d.name == "Good"), "{found:?}");
        assert!(
            found.iter().all(|d| d.name != "bin" && d.name != "tool"),
            "{found:?}"
        );
    }

    #[test]
    fn normalizes_bare_drive_roots() {
        // BUG-0019: "E:" is drive-relative; only "E:\" is the drive root.
        assert_eq!(normalize_root("E:"), PathBuf::from("E:\\"));
        assert_eq!(normalize_root("E:\\"), PathBuf::from("E:\\"));
        assert_eq!(normalize_root(r"E:\Tools"), PathBuf::from(r"E:\Tools"));
        assert_eq!(normalize_root("Apps"), PathBuf::from("Apps"));
    }

    #[test]
    fn skips_installed_folder() {
        let root = tempdir("installed");
        fake_exe(&root.join("tool/tool.exe"));
        fake_exe(&root.join("tool/unins000.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        assert!(found.iter().all(|d| d.name != "tool"), "{found:?}");
    }

    #[test]
    fn finds_single_exe_folder() {
        let root = tempdir("single");
        fake_exe(&root.join("Delta/Delta.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        assert!(found.iter().any(|d| d.kind == "folder" && d.name == "Delta"), "{found:?}");
    }

    #[test]
    fn matches_folder_name_variants() {
        let root = tempdir("variants");
        fake_exe(&root.join("tools/SampleTool/SampleTool2020Compat.exe"));
        fake_exe(&root.join("tools/SampleTool/SampleTool2022.exe"));
        fake_exe(&root.join("tools/NestedTool/Program/NestedTool.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        let sample = found.iter().find(|d| d.name == "SampleTool").expect("SampleTool");
        assert!(sample.exe.ends_with("SampleTool2022.exe"), "{sample:?}");
        assert!(found.iter().any(|d| d.name == "NestedTool"), "{found:?}");
    }

    #[test]
    fn finds_loose_exe_in_container() {
        let root = tempdir("container");
        fake_exe(&root.join("tools/a.exe"));
        fake_exe(&root.join("tools/b.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        assert!(found.iter().any(|d| d.kind == "file" && d.name == "a"), "{found:?}");
        assert!(found.iter().any(|d| d.kind == "file" && d.name == "b"), "{found:?}");
    }

    #[test]
    fn respects_exclude() {
        let root = tempdir("exclude");
        fake_exe(&root.join("App/App.exe"));
        let found = discover_with(
            &[root.clone()],
            &[root.join("App")],
            50,
            &AtomicBool::new(false),
            &[],
        );
        assert!(found.iter().all(|d| d.name != "App"), "{found:?}");
    }

    // --- installed / system-area filtering -------------------------------------

    #[test]
    fn skips_system_areas_at_any_depth() {
        let root = tempdir("system-depth");
        fake_exe(&root.join("Program Files/InstalledApp/InstalledApp.exe"));
        fake_exe(&root.join("Tools/WindowsApps/StoreApp/StoreApp.exe"));
        fake_exe(&root.join("Tools/Portable/Portable.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        assert!(!found.iter().any(|d| d.name == "InstalledApp"), "{found:?}");
        assert!(!found.iter().any(|d| d.name == "StoreApp"), "{found:?}");
        assert!(
            found.iter().any(|d| d.name == "Portable"),
            "portable app next to system areas must still be found: {found:?}"
        );
    }

    #[test]
    fn skips_injected_install_locations() {
        let root = tempdir("installed-locations");
        fake_exe(&root.join("InstallDir/Installed/Installed.exe"));
        fake_exe(&root.join("Green/Green.exe"));
        let installed = vec![root.join("InstallDir")];
        let found = discover_with(
            &[root.clone()],
            &[],
            50,
            &AtomicBool::new(false),
            &installed,
        );
        assert!(!found.iter().any(|d| d.name == "Installed"), "{found:?}");
        assert!(found.iter().any(|d| d.name == "Green"), "{found:?}");

        // A scan rooted directly inside an installed location yields nothing.
        let rooted = discover_with(
            &[root.join("InstallDir")],
            &[],
            50,
            &AtomicBool::new(false),
            &installed,
        );
        assert!(rooted.is_empty(), "{rooted:?}");
    }

    #[test]
    fn installed_location_prefix_respects_component_boundary() {
        let root = tempdir("installed-prefix");
        fake_exe(&root.join("App/App.exe"));
        fake_exe(&root.join("AppOther/AppOther.exe"));
        let installed = vec![root.join("App")];
        let found = discover_with(
            &[root.clone()],
            &[],
            50,
            &AtomicBool::new(false),
            &installed,
        );
        assert!(
            !found.iter().any(|d| d.name == "App"),
            "installed directory must be hidden: {found:?}"
        );
        assert!(
            found.iter().any(|d| d.name == "AppOther"),
            "sibling `AppOther` must not be mistaken for `App`: {found:?}"
        );
    }

    #[test]
    fn installed_path_check_handles_loose_files_and_boundaries() {
        let root = tempdir("installed-path-check");
        let keys = installed_keys(&[root.join("Tools/App")]);
        assert!(is_installed_path(&root.join("Tools/App"), &keys));
        assert!(is_installed_path(&root.join("Tools/App/App.exe"), &keys));
        assert!(!is_installed_path(&root.join("Tools/AppOther/App.exe"), &keys));
        assert!(!is_installed_path(&root.join("Tools/AppOther.exe"), &keys));
        assert!(!is_installed_path(&root.join("Tools/App"), &[]));
    }

    #[test]
    fn keeps_portable_folders_and_still_skips_uninstallers() {
        let root = tempdir("portable-vs-uninstaller");
        fake_exe(&root.join("Green/Green.exe"));
        fake_exe(&root.join("Gray/Gray.exe"));
        fake_exe(&root.join("Gray/uninstall.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        assert!(found.iter().any(|d| d.name == "Green"), "{found:?}");
        assert!(!found.iter().any(|d| d.name == "Gray"), "{found:?}");
    }

    #[test]
    fn skips_launcher_libraries_at_any_depth() {
        let root = tempdir("launcher-libraries");
        fake_exe(&root.join("Library/steamapps/common/Game/Game.exe"));
        fake_exe(&root.join("Library/steamapps/loose-tool.exe"));
        fake_exe(&root.join("Library/XboxGames/StoreGame/StoreGame.exe"));
        fake_exe(&root.join("Library/Portable/Portable.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        assert!(!found.iter().any(|d| d.name == "Game"), "{found:?}");
        assert!(!found.iter().any(|d| d.name == "loose-tool"), "{found:?}");
        assert!(!found.iter().any(|d| d.name == "StoreGame"), "{found:?}");
        assert!(found.iter().any(|d| d.name == "Portable"), "{found:?}");
    }

    #[test]
    fn skips_installer_like_loose_exes() {
        let root = tempdir("installer-names");
        fake_exe(&root.join("Bucket/AppSetup.exe"));
        fake_exe(&root.join("Bucket/MyInstaller.exe"));
        fake_exe(&root.join("Bucket/更新程序.exe"));
        fake_exe(&root.join("Bucket/CleanTool.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        let names: Vec<&str> = found.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"CleanTool"), "{names:?}");
        assert!(!names.contains(&"AppSetup"), "{names:?}");
        assert!(!names.contains(&"MyInstaller"), "{names:?}");
        assert!(!names.contains(&"更新程序"), "{names:?}");
    }

    #[test]
    fn installer_filename_rules() {
        for name in [
            "AppSetup.exe",
            "MyInstaller.exe",
            "install_app.exe",
            "vcredist_x64.exe",
            "App-Patch.exe",
            "更新程序.exe",
            "安装包.exe",
            "卸载工具.exe",
        ] {
            assert!(looks_like_installer(Path::new(name)), "{name} should be excluded");
        }
        for name in [
            "CleanTool.exe",
            "Dispatch.exe",
            "PatchWork.exe",
            "InstallerToolPortable.exe",
            "PortableEditor.exe",
        ] {
            assert!(!looks_like_installer(Path::new(name)), "{name} should be kept");
        }
    }

    #[test]
    fn skips_loose_exes_in_download_staging_dirs() {
        let root = tempdir("download-area");
        fake_exe(&root.join("Downloads/Versioned-Package_1.2.3.exe"));
        fake_exe(&root.join("Downloads/Another_Package-v2.exe"));
        fake_exe(&root.join("Downloads/GreenTool/GreenTool.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        assert!(
            !found.iter().any(|d| d.kind == "file"),
            "loose package exes in a download folder must be skipped: {found:?}"
        );
        assert!(
            found.iter().any(|d| d.name == "GreenTool"),
            "portable apps extracted into folders inside downloads stay discoverable: {found:?}"
        );
    }

    // --- naming / tool filtering (BUG-0024) ------------------------------------

    #[test]
    fn generic_loose_exe_uses_parent_folder_name() {
        let root = tempdir("generic-file-name");
        // Two exes in the folder so it is a container, not a single-app folder.
        fake_exe(&root.join("Library/okayu_130/Game.exe"));
        fake_exe(&root.join("Library/okayu_130/extra.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        let game = found
            .iter()
            .find(|d| d.kind == "file" && d.exe.ends_with("Game.exe"))
            .unwrap_or_else(|| panic!("Game.exe should be discovered: {found:?}"));
        assert_eq!(game.name, "okayu_130");
        assert!(game.exe.ends_with("okayu_130\\Game.exe") || game.exe.ends_with("okayu_130/Game.exe"));
    }

    #[test]
    fn generic_folder_uses_parent_folder_name() {
        let root = tempdir("generic-folder-name");
        fake_exe(&root.join("Collection/Pokemon/Game/Game.exe"));
        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        let item = found
            .iter()
            .find(|d| d.kind == "folder" && d.exe.ends_with("Game\\Game.exe"))
            .unwrap_or_else(|| panic!("Game folder should be discovered: {found:?}"));
        assert_eq!(item.name, "Pokemon");
    }

    #[test]
    fn skips_tool_and_loader_loose_exes() {
        let root = tempdir("tool-exes");
        fake_exe(&root.join("okayu/Tool/loaders/chromedriver.exe"));
        fake_exe(&root.join("okayu/Tool/loaders/editbin.exe"));
        fake_exe(&root.join("okayu/Tool/editbin.exe"));
        fake_exe(&root.join("okayu/Tool/Good.exe"));
        fake_exe(&root.join("okayu/Game/SomeLooseApp.exe"));
        fake_exe(&root.join("okayu/Game/another.exe"));
        // `bin` must stay discoverable (many real apps keep their exe there).
        fake_exe(&root.join("okayu/bin/Real.exe"));
        fake_exe(&root.join("okayu/bin/Other.exe"));

        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        let exes = found
            .iter()
            .map(|d| d.exe.to_lowercase())
            .collect::<Vec<_>>();
        assert!(
            !exes.iter().any(|exe| exe.contains("chromedriver")),
            "loaders/chromedriver.exe must be skipped: {found:?}"
        );
        assert!(
            !exes.iter().any(|exe| exe.contains("editbin")),
            "editbin.exe must be skipped: {found:?}"
        );
        assert!(
            !exes.iter().any(|exe| exe.contains("somelooseapp")),
            "loose exes in a generic Game container must be skipped: {found:?}"
        );
        assert!(
            exes.iter().any(|exe| exe.ends_with("real.exe")),
            "loose exes in bin/ must stay discoverable: {found:?}"
        );
        assert!(
            found
                .iter()
                .any(|d| d.exe.ends_with("Good.exe") && d.name == "Good"),
            "a real app in Tool/ must still appear: {found:?}"
        );
    }

    #[test]
    fn skips_tool_folders_but_keeps_tools_good() {
        let root = tempdir("tool-folders");
        // Folder candidates: `...\Tool\loaders\editbin` etc. (real scan hit
        // five such `editbin` rows).
        fake_exe(&root.join("Tool/loaders/editbin/editbin.exe"));
        fake_exe(&root.join("Tool/loaders/chromedriver/chromedriver.exe"));
        // A normal app in a folder literally named Tools must survive.
        fake_exe(&root.join("Tools/Good/Good.exe"));

        let found = discover_with(&[root.clone()], &[], 50, &AtomicBool::new(false), &[]);
        assert!(
            !found
                .iter()
                .any(|d| d.kind == "folder" && d.name.eq_ignore_ascii_case("editbin")),
            "editbin tool folder must be skipped: {found:?}"
        );
        assert!(
            !found.iter().any(|d| {
                d.kind == "folder"
                    && (d.name.eq_ignore_ascii_case("editbin")
                        || d.name.eq_ignore_ascii_case("chromedriver")
                        || d.path.to_lowercase().contains("editbin")
                        || d.path.to_lowercase().contains("chromedriver"))
            }),
            "loader tool folders must be skipped: {found:?}"
        );
        assert!(
            found
                .iter()
                .any(|d| d.kind == "folder" && d.name == "Good"),
            "Tools/Good must still appear: {found:?}"
        );
    }

    // --- filter_registered -----------------------------------------------------

    fn discovered(name: &str, dir: &Path, exe: &Path) -> Discovered {
        Discovered {
            name: name.to_string(),
            path: dir.to_string_lossy().to_string(),
            exe: exe.to_string_lossy().to_string(),
            size: 0,
            kind: "folder".to_string(),
            score: 0,
        }
    }

    fn abs_str(p: &Path) -> String {
        p.to_string_lossy().replace('\\', "/")
    }

    #[test]
    fn filter_registered_excludes_relative_and_absolute_registration() {
        let root = tempdir("filter-registered");
        let exe = root.join("Tools/Alpha/Alpha.exe");
        fake_exe(&exe);
        let items = vec![discovered("Alpha", &root.join("Tools/Alpha"), &exe)];

        // Relative registration (app stored inside the portable root).
        let relative = vec!["Tools/Alpha/Alpha.exe".to_string()];
        assert!(
            filter_registered(&root, &relative, items.clone()).is_empty(),
            "relative registration must hide the discovered app"
        );

        // Absolute registration of the same file (register import mode).
        let absolute = vec![abs_str(&exe)];
        assert!(
            filter_registered(&root, &absolute, items).is_empty(),
            "absolute registration must hide the discovered app"
        );
    }

    #[test]
    fn filter_registered_keeps_same_name_different_file() {
        let root = tempdir("filter-registered-name");
        let one = root.join("One/Shared.exe");
        let two = root.join("Two/Shared.exe");
        fake_exe(&one);
        fake_exe(&two);
        let items = vec![
            discovered("Shared", &root.join("One"), &one),
            discovered("Shared", &root.join("Two"), &two),
        ];
        let kept = filter_registered(&root, &["One/Shared.exe".to_string()], items);
        assert_eq!(kept.len(), 1, "only the registered file may be removed");
        assert!(
            Path::new(&kept[0].exe).ends_with("Two/Shared.exe"),
            "{:?}",
            kept[0]
        );
    }

    #[test]
    fn filter_registered_keeps_unregistered_and_preserves_order() {
        let root = tempdir("filter-registered-keep");
        let a = root.join("Apps/Alpha/Alpha.exe");
        let b = root.join("Apps/Beta/Beta.exe");
        fake_exe(&a);
        fake_exe(&b);
        let items = vec![
            discovered("Alpha", &root.join("Apps/Alpha"), &a),
            discovered("Beta", &root.join("Apps/Beta"), &b),
        ];
        let kept = filter_registered(&root, &["Other/Other.exe".to_string()], items);
        let names: Vec<&str> = kept.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["Alpha", "Beta"]);
    }

    #[test]
    fn filter_registered_uses_path_when_exe_is_empty() {
        let root = tempdir("filter-registered-noexe");
        let exe = root.join("Loose/Only.exe");
        fake_exe(&exe);
        let items = vec![Discovered {
            name: "Only".to_string(),
            path: abs_str(&exe),
            exe: String::new(),
            size: 0,
            kind: "file".to_string(),
            score: 0,
        }];
        assert!(
            filter_registered(&root, &["Loose/Only.exe".to_string()], items).is_empty(),
            "empty exe must fall back to item.path"
        );
    }

    // --- game library collapsing ------------------------------------------------

    #[test]
    fn collapses_game_library_tools_into_one_item_per_game() {
        let root = tempdir("game-library-collapse");
        // A collection: six child games, no direct exe.
        for index in 1..=6 {
            let game = root.join(format!("Games/hGame/Game{index}"));
            fake_exe(&game.join("sub/Game.exe"));
            fake_exe(&game.join("Tool/nwjc.exe"));
            fake_exe(&game.join("lib/py2/zsyncmake.exe"));
        }
        fake_exe(&root.join("Games/Standalone/Standalone.exe"));
        let found = discover_with(&[root.clone()], &[], 100, &AtomicBool::new(false), &[]);

        for index in 1..=6 {
            assert!(
                found.iter().any(|item| item.name == format!("Game{index}")),
                "game {index} must be one item: {found:?}"
            );
        }
        assert!(
            found.iter().any(|item| item.name == "Standalone"),
            "normal library game must stay: {found:?}"
        );
        assert!(
            found
                .iter()
                .all(|item| item.name != "nwjc" && item.name != "zsyncmake" && item.name != "sub"),
            "game internals must not be separate candidates: {found:?}"
        );
        // One item per game folder, not one per internal file.
        assert_eq!(
            found.iter().filter(|item| item.name.starts_with("Game")).count(),
            6,
            "{found:?}"
        );
    }

    #[test]
    fn keeps_normal_library_game_with_direct_exe() {
        let root = tempdir("game-library-normal");
        fake_exe(&root.join("Games/SingleGame/SingleGame.exe"));
        let found = discover_with(&[root.clone()], &[], 20, &AtomicBool::new(false), &[]);
        assert!(
            found.iter().any(|item| item.name == "SingleGame"),
            "{found:?}"
        );
        assert_eq!(found.len(), 1, "{found:?}");
    }
}
