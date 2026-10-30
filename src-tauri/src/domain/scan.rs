//! Discover portable apps under `Apps/<App>/`.
//!
//! Heuristic order (highest wins):
//!   1. `portablehub.json` manifest inside the app folder
//!   2. exe name matches the folder name
//!   3. GUI subsystem (IMAGE_SUBSYSTEM_WINDOWS_GUI)
//!   4. direct child over nested
//!   5. larger file / newer mtime as tie-breakers

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::path::{resolve, to_rel};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub name: String,
    pub rel_path: String,
    pub score: i32,
    pub has_manifest: bool,
}

const SKIP_DIR_PREFIX: [&str; 2] = ["_", "."];

const EXE_BLOCKLIST: [&str; 12] = [
    "unins", "uninst", "update", "updater", "crash", "helper", "vcredist", "setup", "install",
    "uninstall", "repair", "elevate",
];

pub fn scan(root: &Path, apps_dir_rel: &str) -> Vec<Candidate> {
    let apps_dir = resolve(root, apps_dir_rel);
    let entries = match fs::read_dir(&apps_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if is_exe(&path) && !blocked(&path) {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !name.is_empty() {
                    out.push(Candidate {
                        name,
                        rel_path: to_rel(root, &path),
                        score: 100 + size_bonus(&path),
                        has_manifest: false,
                    });
                }
            }
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if SKIP_DIR_PREFIX.iter().any(|p| name.starts_with(p)) {
            continue;
        }
        if let Some(c) = scan_one(root, &path, &name) {
            out.push(c);
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Normalized textual key for path comparison: forward slashes + lowercase.
///
/// Windows paths are case-insensitive and may mix `/` and `\`, so raw string
/// equality is too strict; this keeps the fallback comparison meaningful.
fn path_key(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").to_lowercase()
}

/// True when both paths point at the same file on disk.
///
/// Prefers `dunce::canonicalize` (resolves `.`/`..`, links, and the real
/// file-name case on Windows); when either side cannot be canonicalized (e.g.
/// the registered target was deleted) it falls back to a separator-normalized,
/// case-insensitive string comparison.
pub fn paths_equal(a: &Path, b: &Path) -> bool {
    if let (Ok(ca), Ok(cb)) = (dunce::canonicalize(a), dunce::canonicalize(b)) {
        return path_key(&ca) == path_key(&cb);
    }
    path_key(a) == path_key(b)
}

/// Drop candidates that already have a registered app.
///
/// `existing` holds the stored app paths, which may be relative to `root`
/// (`Apps/Foo/Foo.exe`) or absolute (`E:/Tools/Foo/Foo.exe`), depending on how
/// the app was imported; a candidate only carries a root-relative `rel_path`.
/// Comparison goes through [`paths_equal`], so the two representations still
/// match. The order of the remaining candidates is preserved.
pub fn filter_new_candidates(
    root: &Path,
    existing: &[String],
    candidates: Vec<Candidate>,
) -> Vec<Candidate> {
    if existing.is_empty() {
        return candidates;
    }
    candidates
        .into_iter()
        .filter(|c| {
            let candidate = resolve(root, &c.rel_path);
            !existing
                .iter()
                .any(|e| paths_equal(&resolve(root, e), &candidate))
        })
        .collect()
}

/// True when adding a scan candidate would duplicate an existing app.
///
/// `existing` holds `(name, stored_path)` pairs of the registered apps. A
/// candidate is a duplicate when its display name matches case-insensitively
/// (same rule as importing) or when its exe resolves to the same file as a
/// registered path (which may be relative or absolute).
pub fn should_skip_add(
    root: &Path,
    existing: &[(String, String)],
    name: &str,
    rel_path: &str,
) -> bool {
    let candidate = resolve(root, rel_path);
    existing.iter().any(|(existing_name, existing_path)| {
        existing_name.eq_ignore_ascii_case(name)
            || paths_equal(&resolve(root, existing_path), &candidate)
    })
}

/// Best main executable inside `dir` (root or one level below), with its score.
pub fn find_main_exe(dir: &Path) -> Option<(PathBuf, i32)> {
    let name = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    if let Some(exe) = manifest_exe(dir) {
        return Some((exe, 1000));
    }

    let mut best: Option<(i32, PathBuf)> = None;
    let direct = list_exes(dir, true);
    let nested = list_exes(dir, false);

    for (depth, exes) in [(0, direct), (1, nested)] {
        for exe in exes {
            if blocked(&exe) {
                continue;
            }
            let mut score = 0;
            if normalized_stem(&exe) == normalized(&name) {
                score += 200;
            }
            if is_gui_subsystem(&exe) {
                score += 100;
            }
            if depth == 0 {
                score += 20;
            }
            score += size_bonus(&exe);
            if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                best = Some((score, exe));
            }
        }
    }
    best.map(|(s, p)| (p, s))
}

/// Like `find_main_exe`, but only looks at the folder root (plus a few role
/// subfolders such as `Program/`), so a container folder is not mistaken for an app.
pub fn find_main_exe_direct(dir: &Path) -> Option<(PathBuf, i32)> {
    if let Some(exe) = manifest_exe(dir) {
        return Some((exe, 1000));
    }
    let name = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let direct: Vec<PathBuf> = list_exes(dir, true)
        .into_iter()
        .filter(|e| !blocked(e))
        .collect();

    // An exe whose name overlaps the folder name is the strongest signal.
    // "SampleTool2022.exe" matches "SampleTool"; "my_tool.exe" matches
    // "My Tool (1.2)".
    let folder = normalized(&name);
    let matches_folder = |exe: &PathBuf| {
        let stem = normalized_stem(exe);
        if stem.is_empty() {
            return false;
        }
        stem == folder
            || (folder.chars().count() >= 3 && stem.contains(&folder))
            || (stem.chars().count() >= 5 && folder.contains(&stem))
    };

    // Among several matching names, the shortest is closest to the folder name
    // (e.g. prefer "SampleTool2022.exe" over "SampleTool2020Compat.exe").
    if let Some(exe) = direct
        .iter()
        .filter(|e| matches_folder(e))
        .min_by_key(|e| normalized_stem(e).chars().count())
    {
        let score = 320 + size_bonus(exe) + if is_gui_subsystem(exe) { 100 } else { 0 };
        return Some((exe.clone(), score));
    }

    // A single exe in the folder root is an app; several exes mean "container".
    if direct.len() == 1 {
        let exe = direct.into_iter().next().unwrap();
        let score = 20 + size_bonus(&exe) + if is_gui_subsystem(&exe) { 100 } else { 0 };
        return Some((exe, score));
    }

    // No root exe: accept a role subfolder whose exe matches the folder name,
    // or a role subfolder holding exactly one exe (e.g. NestedTool/Program).
    if direct.is_empty() {
        for role in ["program", "app", "bin", "x64", "x86"] {
            let sub = dir.join(role);
            if sub.is_dir() {
                let exes: Vec<PathBuf> = list_exes(&sub, true)
                    .into_iter()
                    .filter(|e| !blocked(e))
                    .collect();
                if let Some(exe) = exes.iter().find(|e| matches_folder(e)) {
                    let score = 200 + size_bonus(exe);
                    return Some((exe.clone(), score));
                }
                if exes.len() == 1 {
                    let exe = exes.into_iter().next().unwrap();
                    let score = size_bonus(&exe);
                    return Some((exe, score));
                }
            }
        }
    }

    None
}

/// True when the folder carries an uninstaller, i.e. it is an installed app, not portable.
pub fn looks_installed(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        let name = e.file_name().to_string_lossy().to_lowercase();
        let is_exe = name.ends_with(".exe");
        let is_lnk = name.ends_with(".lnk");
        // Only names that *start* with an uninstaller marker count; a tool called
        // "demo（软件卸载）.exe" must not mark the whole folder as installed.
        (name.starts_with("unins") && (is_exe || is_lnk))
            || (is_exe && name == "卸载.exe")
            || (is_lnk && name.starts_with("卸载"))
    })
}

fn manifest_exe(dir: &Path) -> Option<PathBuf> {
    let manifest = dir.join("portablehub.json");
    if !manifest.exists() {
        return None;
    }
    let text = fs::read_to_string(&manifest).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let exe = value.get("exe")?.as_str()?;
    let p = dir.join(exe.replace('/', "\\"));
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

pub fn scan_one(root: &Path, dir: &Path, name: &str) -> Option<Candidate> {
    if let Some(c) = from_manifest(root, dir, name) {
        return Some(c);
    }

    let mut best: Option<(i32, PathBuf)> = None;
    let direct = list_exes(dir, true);
    let nested = list_exes(dir, false);

    for (depth, exes) in [(0, direct), (1, nested)] {
        for exe in exes {
            if blocked(&exe) {
                continue;
            }
            let mut score = 0;
            if normalized_stem(&exe) == normalized(name) {
                score += 200;
            }
            if is_gui_subsystem(&exe) {
                score += 100;
            }
            if depth == 0 {
                score += 20;
            }
            score += size_bonus(&exe);
            if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                best = Some((score, exe));
            }
        }
    }

    let (score, exe) = best?;
    Some(Candidate {
        name: name.to_string(),
        rel_path: to_rel(root, &exe),
        score,
        has_manifest: false,
    })
}

fn from_manifest(root: &Path, dir: &Path, name: &str) -> Option<Candidate> {
    let manifest = dir.join("portablehub.json");
    if !manifest.exists() {
        return None;
    }
    let text = fs::read_to_string(&manifest).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let exe = value.get("exe")?.as_str()?;
    let exe_path = dir.join(exe.replace('/', "\\"));
    if !exe_path.exists() {
        return None;
    }
    Some(Candidate {
        name: value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(name)
            .to_string(),
        rel_path: to_rel(root, &exe_path),
        score: 1000,
        has_manifest: true,
    })
}

fn list_exes(dir: &Path, direct: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            if direct && is_exe(&p) {
                out.push(p);
            }
        } else if p.is_dir() && !direct {
            if let Ok(children) = fs::read_dir(&p) {
                for child in children.flatten() {
                    let cp = child.path();
                    if cp.is_file() && is_exe(&cp) {
                        out.push(cp);
                    }
                }
            }
        }
    }
    out
}

pub fn is_exe_file(p: &Path) -> bool {
    is_exe(p)
}

pub fn is_blocked_exe(p: &Path) -> bool {
    blocked(p)
}

fn is_exe(p: &Path) -> bool {
    p.extension()
        .map(|e| e.eq_ignore_ascii_case("exe"))
        .unwrap_or(false)
}

fn blocked(p: &Path) -> bool {
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    EXE_BLOCKLIST.iter().any(|b| stem.contains(b))
}

fn normalized(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn normalized_stem(p: &Path) -> String {
    p.file_stem()
        .map(|s| normalized(&s.to_string_lossy()))
        .unwrap_or_default()
}

fn size_bonus(p: &Path) -> i32 {
    let size = fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    ((size / 1_048_576).min(50)) as i32
}

/// Read the PE optional header subsystem field.
pub fn is_gui_subsystem(path: &Path) -> bool {
    let mut f = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut dos = [0u8; 64];
    if f.read_exact(&mut dos).is_err() || &dos[0..2] != b"MZ" {
        return false;
    }
    let e_lfanew = u32::from_le_bytes([dos[0x3C], dos[0x3D], dos[0x3E], dos[0x3F]]) as u64;
    if e_lfanew == 0 || e_lfanew > 4096 {
        return false;
    }
    let mut header = [0u8; 96];
    use std::io::{Seek, SeekFrom};
    if f.seek(SeekFrom::Start(e_lfanew)).is_err() || f.read_exact(&mut header).is_err() {
        return false;
    }
    if &header[0..4] != b"PE\0\0" {
        return false;
    }
    let magic = u16::from_le_bytes([header[24], header[25]]);
    if magic != 0x10b && magic != 0x20b {
        return false;
    }
    let subsystem = u16::from_le_bytes([header[24 + 68], header[24 + 69]]);
    subsystem == 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "portablehub-scan-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_dummy(path: &Path, gui: bool) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut data = vec![0u8; 256];
        data[0] = b'M';
        data[1] = b'Z';
        data[0x3C..0x40].copy_from_slice(&(128u32).to_le_bytes());
        data[128..132].copy_from_slice(b"PE\0\0");
        data[128 + 24..128 + 26].copy_from_slice(&(0x10bu16).to_le_bytes());
        let subsystem: u16 = if gui { 2 } else { 3 };
        data[128 + 24 + 68..128 + 24 + 70].copy_from_slice(&subsystem.to_le_bytes());
        fs::write(path, data).unwrap();
    }

    #[test]
    fn finds_direct_exe() {
        let root = tempdir("direct");
        write_dummy(&root.join("Apps/Demo/Demo.exe"), true);
        let found = scan(&root, "Apps");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rel_path, "Apps/Demo/Demo.exe");
    }

    #[test]
    fn finds_root_level_exe() {
        let root = tempdir("rootexe");
        write_dummy(&root.join("Apps/Standalone.exe"), true);
        let found = scan(&root, "Apps");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Standalone");
        assert_eq!(found[0].rel_path, "Apps/Standalone.exe");
    }

    #[test]
    fn finds_nested_exe() {
        let root = tempdir("nested");
        write_dummy(&root.join("Apps/Tool/Program/Tool.exe"), true);
        let found = scan(&root, "Apps");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rel_path, "Apps/Tool/Program/Tool.exe");
    }

    #[test]
    fn respects_manifest() {
        let root = tempdir("manifest");
        write_dummy(&root.join("Apps/Tool/bin/real.exe"), true);
        write_dummy(&root.join("Apps/Tool/other.exe"), true);
        fs::write(
            root.join("Apps/Tool/portablehub.json"),
            r#"{"name":"Tool","exe":"bin/real.exe"}"#,
        )
        .unwrap();
        let found = scan(&root, "Apps");
        assert_eq!(found[0].rel_path, "Apps/Tool/bin/real.exe");
        assert!(found[0].has_manifest);
    }

    #[test]
    fn skips_uninstallers() {
        let root = tempdir("skip");
        write_dummy(&root.join("Apps/Tool/unins000.exe"), true);
        let found = scan(&root, "Apps");
        assert!(found.is_empty());
    }

    #[test]
    fn uninstaller_tool_does_not_mark_folder_installed() {
        let dir = std::env::temp_dir().join(format!("ph-unins-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        write_dummy(&dir.join("Demo.exe"), true);
        write_dummy(&dir.join("demo（软件卸载）.exe"), true);
        assert!(!looks_installed(&dir));
        fs::write(dir.join("Uninstall Tool.lnk"), b"").unwrap();
        assert!(looks_installed(&dir));
    }

    #[test]
    fn handles_chinese_and_spaces() {
        let root = tempdir("cjk");
        write_dummy(&root.join("Apps/测试 应用/测试 应用.exe"), true);
        let found = scan(&root, "Apps");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "测试 应用");
    }

    // --- filter_new_candidates -------------------------------------------------

    fn candidate(rel_path: &str, name: &str) -> Candidate {
        Candidate {
            name: name.to_string(),
            rel_path: rel_path.to_string(),
            score: 0,
            has_manifest: false,
        }
    }

    fn abs_str(p: &Path) -> String {
        p.to_string_lossy().replace('\\', "/")
    }

    #[test]
    fn filter_excludes_already_registered_relative_path() {
        let root = tempdir("filter-rel");
        write_dummy(&root.join("Apps/Alpha/Alpha.exe"), true);
        write_dummy(&root.join("Apps/Beta/Beta.exe"), true);
        let existing = vec!["Apps/Alpha/Alpha.exe".to_string()];
        let fresh = filter_new_candidates(&root, &existing, scan(&root, "Apps"));
        assert_eq!(fresh.len(), 1, "only the unregistered app should remain");
        assert_eq!(fresh[0].rel_path, "Apps/Beta/Beta.exe");
    }

    #[test]
    fn filter_excludes_absolute_registration_of_same_file() {
        let root = tempdir("filter-abs");
        let exe = root.join("Apps/Alpha/Alpha.exe");
        write_dummy(&exe, true);
        let found = scan(&root, "Apps");
        assert_eq!(found.len(), 1);

        // Plain absolute path, as stored by "register" import mode.
        let absolute = vec![abs_str(&exe)];
        assert!(
            filter_new_candidates(&root, &absolute, found.clone()).is_empty(),
            "absolute registration must hide the candidate"
        );

        // Non-normalized absolute form: only canonicalization can match it,
        // so this proves the comparison is not pure string equality.
        let dotted = vec![abs_str(
            &root
                .join("Apps")
                .join("..")
                .join("Apps")
                .join("Alpha")
                .join("Alpha.exe"),
        )];
        assert!(
            filter_new_candidates(&root, &dotted, found).is_empty(),
            "canonicalized comparison must match an unnormalized absolute path"
        );
    }

    #[test]
    fn filter_keeps_unregistered_candidates_in_order() {
        let root = tempdir("filter-order");
        write_dummy(&root.join("Apps/Zeta/Zeta.exe"), true);
        write_dummy(&root.join("Apps/Alpha/Alpha.exe"), true);
        write_dummy(&root.join("Apps/Mike/Mike.exe"), true);
        let existing = vec!["Apps/Mike/Mike.exe".to_string()];
        let fresh = filter_new_candidates(&root, &existing, scan(&root, "Apps"));
        let rels: Vec<&str> = fresh.iter().map(|c| c.rel_path.as_str()).collect();
        assert_eq!(
            rels,
            vec!["Apps/Alpha/Alpha.exe", "Apps/Zeta/Zeta.exe"],
            "scan order must be preserved after filtering"
        );
    }

    #[test]
    fn filter_does_not_drop_candidates_by_name() {
        let root = tempdir("filter-name");
        write_dummy(&root.join("Apps/One/Shared.exe"), true);
        write_dummy(&root.join("Apps/Two/Shared.exe"), true);
        // Two distinct programs share a display name; only the registered path
        // may be removed. Name-based dedupe belongs to add_apps, not filtering.
        let candidates = vec![
            candidate("Apps/One/Shared.exe", "Shared"),
            candidate("Apps/Two/Shared.exe", "Shared"),
        ];
        let existing = vec!["Apps/One/Shared.exe".to_string()];
        let fresh = filter_new_candidates(&root, &existing, candidates);
        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].rel_path, "Apps/Two/Shared.exe");
    }

    #[test]
    fn filter_falls_back_to_case_insensitive_string_compare() {
        let root = tempdir("filter-fallback-case");
        // Neither file exists, so canonicalize fails on both sides and the
        // comparison falls back to normalized strings (case-folded).
        let existing = vec!["Apps/Missing/tool.exe".to_string()];
        let candidates = vec![candidate("Apps/Missing/Tool.exe", "Tool")];
        assert!(filter_new_candidates(&root, &existing, candidates).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn filter_falls_back_to_normalized_separators() {
        let root = tempdir("filter-fallback-sep");
        // Missing file + mixed separators + different case.
        let existing = vec!["apps\\missing\\TOOL.EXE".to_string()];
        let candidates = vec![candidate("Apps/Missing/Tool.exe", "Tool")];
        assert!(filter_new_candidates(&root, &existing, candidates).is_empty());
    }

    #[test]
    fn filter_keeps_everything_without_registered_apps() {
        let root = tempdir("filter-empty");
        write_dummy(&root.join("Apps/Alpha/Alpha.exe"), true);
        let found = scan(&root, "Apps");
        assert_eq!(filter_new_candidates(&root, &[], found.clone()).len(), 1);
    }

    // --- should_skip_add -------------------------------------------------------

    fn existing_pair(name: &str, path: &str) -> (String, String) {
        (name.to_string(), path.to_string())
    }

    #[test]
    fn skip_add_by_same_name_case_insensitive() {
        let root = tempdir("skip-name");
        let existing = vec![existing_pair("SomeTool", "Apps/Old/Old.exe")];
        assert!(should_skip_add(
            &root,
            &existing,
            "sometool",
            "Apps/New/New.exe"
        ));
    }

    #[test]
    fn skip_add_by_same_path_regardless_of_name_or_representation() {
        let root = tempdir("skip-path");
        let exe = root.join("Apps/Alpha/Alpha.exe");
        write_dummy(&exe, true);

        // Renamed app with a relative path: name differs, path matches.
        let relative = vec![existing_pair("Renamed Thing", "Apps/Alpha/Alpha.exe")];
        assert!(should_skip_add(
            &root,
            &relative,
            "Alpha",
            "Apps/Alpha/Alpha.exe"
        ));

        // Absolute registration of the same file: only canonicalize matches.
        let absolute = vec![existing_pair(
            "Different Name",
            &abs_str(&root.join("Apps").join("..").join("Apps").join("Alpha").join("Alpha.exe")),
        )];
        assert!(should_skip_add(
            &root,
            &absolute,
            "Alpha",
            "Apps/Alpha/Alpha.exe"
        ));
    }

    #[test]
    fn does_not_skip_add_when_name_and_path_differ() {
        let root = tempdir("skip-none");
        write_dummy(&root.join("Apps/Alpha/Alpha.exe"), true);
        write_dummy(&root.join("Apps/Beta/Beta.exe"), true);
        let existing = vec![existing_pair("Alpha", "Apps/Alpha/Alpha.exe")];
        assert!(!should_skip_add(
            &root,
            &existing,
            "Beta",
            "Apps/Beta/Beta.exe"
        ));
        // Empty registry never skips.
        assert!(!should_skip_add(&root, &[], "Beta", "Apps/Beta/Beta.exe"));
    }
}
