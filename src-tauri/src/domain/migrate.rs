//! Migration audit and self-contained bundle export (REQ-0023).
//!
//! `audit` classifies where each app actually lives (`local` / `linked` /
//! `external` / `missing`). `export_bundle` writes a fresh, portable copy of
//! the hub into an empty folder: the executable, `config.json`, `Themes/` and a
//! self-contained `Apps/` tree whose links and external registrations are
//! replaced by real content. The current program directory is only read.

use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::config::{self, AppEntry, Config};
use super::import::safe_name;
use super::path::resolve;
use super::scan;
use crate::infra::transfer;

pub const REPORT_NAME: &str = "migration-report.json";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppAudit {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub path: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportItem {
    pub name: String,
    pub ok: bool,
    pub from: String,
    pub to: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub ok: bool,
    pub target: String,
    pub total: usize,
    pub copied: usize,
    pub failed: usize,
    pub items: Vec<ExportItem>,
    pub report_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationReport<'a> {
    schema: u32,
    version: &'a str,
    exported_at: String,
    target: String,
    total: usize,
    copied: usize,
    failed: usize,
    items: &'a [ExportItem],
}

// ---------------------------------------------------------------------------
// Audit
// ---------------------------------------------------------------------------

/// Classify an app source from pure path facts.
///
/// * `app` — stored exe path resolved against the portable root (not
///   canonicalized, so a junction path is still visible);
/// * `resolved` — canonicalized real path (only meaningful when not missing);
/// * `apps_dir` — the configured apps directory (canonical form).
///
/// A stored path inside `apps_dir` whose real target escaped it is `linked`; a
/// stored path outside `apps_dir` is `external`; an unresolvable path is
/// `missing`.
pub fn classify(missing: bool, app: &Path, resolved: &Path, apps_dir: &Path) -> &'static str {
    if missing {
        return "missing";
    }
    if rel_under(app, apps_dir).is_none() {
        return "external";
    }
    if rel_under(resolved, apps_dir).is_some() {
        "local"
    } else {
        "linked"
    }
}

/// Classify every registered app. Reads metadata only, never writes.
pub fn audit(root: &Path, cfg: &Config) -> Vec<AppAudit> {
    let root = canonical_or_self(root);
    let apps_dir = resolve(&root, &cfg.apps_dir);
    cfg.apps
        .iter()
        .map(|a| {
            let app_path = resolve(&root, &a.path);
            let resolved = dunce::canonicalize(&app_path);
            let missing = resolved.is_err();
            let resolved_path = resolved.unwrap_or_default();
            AppAudit {
                id: a.id.clone(),
                name: a.name.clone(),
                kind: classify(missing, &app_path, &resolved_path, &apps_dir).to_string(),
                path: a.path.clone(),
                target: if missing {
                    String::new()
                } else {
                    to_slash(&resolved_path)
                },
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/// Copy the hub into `target` as a self-contained, movable package.
pub fn export_bundle(root: &Path, cfg: &Config, target: &Path) -> Result<ExportReport, String> {
    let exe = std::env::current_exe().map_err(|e| format!("无法定位当前程序：{e}"))?;
    export_bundle_with_exe(root, cfg, target, &exe)
}

fn export_bundle_with_exe(
    root: &Path,
    cfg: &Config,
    target: &Path,
    current_exe: &Path,
) -> Result<ExportReport, String> {
    let root = canonical_or_self(root);
    let target_input = if target.is_absolute() {
        target.to_path_buf()
    } else {
        root.join(target)
    };
    let target = normalize_abs(&target_input);

    // Guards: never write into the running directory, never mix with an
    // existing non-empty folder.
    if is_under(&target, &root) {
        return Err("目标不能在程序目录内".to_string());
    }
    if target.exists() {
        let empty = target.is_dir()
            && fs::read_dir(&target)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false);
        if !empty {
            return Err("目标目录必须为空或不存在".to_string());
        }
    }

    fs::create_dir_all(&target).map_err(|e| format!("创建目标目录失败：{e}"))?;
    let apps_out = target.join("Apps");
    let themes_out = target.join("Themes");
    fs::create_dir_all(&apps_out).map_err(|e| format!("创建 Apps 目录失败：{e}"))?;
    fs::create_dir_all(&themes_out).map_err(|e| format!("创建 Themes 目录失败：{e}"))?;

    if !current_exe.is_file() {
        return Err(format!("找不到当前程序：{}", current_exe.display()));
    }
    let exe_name = current_exe
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "无法确定当前程序文件名".to_string())?;
    fs::copy(current_exe, target.join(&exe_name))
        .map_err(|e| format!("复制程序文件失败：{e}"))?;

    // Copied first so any unknown fields survive if the rewrite below ever
    // fails; the rewritten config then replaces it.
    let config_src = root.join(config::CONFIG_NAME);
    if config_src.is_file() {
        let _ = fs::copy(&config_src, target.join(config::CONFIG_NAME));
    }
    let themes_src = root.join("Themes");
    if themes_src.is_dir() {
        transfer::copy_tree(&themes_src, &themes_out)
            .map_err(|e| format!("复制主题失败：{e}"))?;
    }

    let apps_dir = resolve(&root, &cfg.apps_dir);
    let mut new_cfg = cfg.clone();
    let mut items: Vec<ExportItem> = Vec::new();
    let mut used: HashSet<String> = HashSet::new();
    let mut copied = 0usize;
    let mut failed = 0usize;

    for (index, app) in cfg.apps.iter().enumerate() {
        let (new_rel, item) = export_one(&root, &apps_dir, &apps_out, app, &mut used);
        if let Some(new_rel) = new_rel {
            copied += 1;
            new_cfg.apps[index].path = new_rel;
        } else {
            failed += 1;
        }
        items.push(item);
    }

    let config_text =
        serde_json::to_string_pretty(&new_cfg).map_err(|e| format!("序列化配置失败：{e}"))?;
    fs::write(target.join(config::CONFIG_NAME), config_text)
        .map_err(|e| format!("写入目标配置失败：{e}"))?;

    let report_path = target.join(REPORT_NAME);
    let report = MigrationReport {
        schema: 1,
        version: env!("CARGO_PKG_VERSION"),
        exported_at: now_iso(),
        target: to_slash(&target),
        total: cfg.apps.len(),
        copied,
        failed,
        items: &items,
    };
    let report_text =
        serde_json::to_string_pretty(&report).map_err(|e| format!("序列化迁移报告失败：{e}"))?;
    fs::write(&report_path, report_text).map_err(|e| format!("写入迁移报告失败：{e}"))?;

    Ok(ExportReport {
        ok: failed == 0,
        target: to_slash(&target),
        total: cfg.apps.len(),
        copied,
        failed,
        items,
        report_path: to_slash(&report_path),
    })
}

/// Copy one app into `apps_out`, returning the new config-relative path on
/// success. The error side is the failed `ExportItem` that is still reported.
fn export_one(
    root: &Path,
    apps_dir: &Path,
    apps_out: &Path,
    app: &AppEntry,
    used: &mut HashSet<String>,
) -> (Option<String>, ExportItem) {
    let app_path = resolve(root, &app.path);
    let from = to_slash(&app_path);
    let failed = |message: String| ExportItem {
        name: app.name.clone(),
        ok: false,
        from: from.clone(),
        to: String::new(),
        message,
    };

    let resolved = match dunce::canonicalize(&app_path) {
        Ok(p) => p,
        Err(e) => return (None, failed(format!("源路径不存在或断链：{e}"))),
    };

    let plan = match plan_unit(&app_path, &resolved, apps_dir, &app.name) {
        Some(p) => p,
        None => return (None, failed("无法确定要复制的应用目录".to_string())),
    };

    let target_name = unique_target_name(&plan.base_name, plan.is_file, used);
    let destination = apps_out.join(&target_name);
    let copy_result = if plan.is_file {
        fs::copy(&plan.source, &destination)
            .map(|_| ())
            .map_err(|e| format!("复制失败：{}（{e}）", plan.source.display()))
    } else {
        transfer::copy_tree(&plan.source, &destination)
    };
    if let Err(e) = copy_result {
        return (None, failed(e));
    }
    if !destination.exists() {
        return (None, failed(format!("复制后目标不存在：{}", destination.display())));
    }

    let new_rel = if plan.is_file {
        format!("Apps/{target_name}")
    } else {
        let sub = to_slash(&plan.exe_rel_in_unit);
        format!("Apps/{target_name}/{sub}")
    };
    (
        Some(new_rel.clone()),
        ExportItem {
            name: app.name.clone(),
            ok: true,
            from,
            to: new_rel,
            message: plan
                .message
                .clone()
                .unwrap_or_else(|| "已复制为真实内容".to_string()),
        },
    )
}

struct UnitPlan {
    /// Real content to copy (directory or single file).
    source: PathBuf,
    is_file: bool,
    /// Name to use under `target/Apps` before uniqueness handling.
    base_name: String,
    /// Path of the exe relative to `source`; empty when `source` is the exe.
    exe_rel_in_unit: PathBuf,
    /// Optional note for the report (e.g. a shared folder copied exe-only).
    message: Option<String>,
}

fn plan_unit(app_path: &Path, resolved: &Path, apps_dir: &Path, name: &str) -> Option<UnitPlan> {
    if let Some(rel) = rel_under(app_path, apps_dir) {
        // Inside the apps dir: copy the first-level unit the app belongs to.
        // The unit path may itself be a junction; canonicalize resolves it to
        // the real external content that must be carried into the bundle.
        let first = rel.components().next()?.as_os_str().to_os_string();
        let unit_path = apps_dir.join(&first);
        let unit_is_file = unit_path.is_file();
        let source = dunce::canonicalize(&unit_path).unwrap_or_else(|_| unit_path.clone());
        let base_name = first.to_string_lossy().to_string();
        let exe_rel_in_unit = if unit_is_file {
            PathBuf::new()
        } else {
            exe_relative(resolved, &source)
        };
        Some(UnitPlan {
            source,
            is_file: unit_is_file,
            base_name,
            exe_rel_in_unit,
            message: None,
        })
    } else {
        // External registration: copy the exe's whole folder by default, so
        // DLLs/resources travel with the app. Only a large "shared bucket"
        // (dozens of unrelated exes) is exported as the bare executable.
        let (source, is_file) = external_unit(resolved);
        let base_name = if is_file {
            resolved
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| safe_name(name))
        } else {
            safe_name(name)
        };
        let exe_rel_in_unit = if is_file {
            PathBuf::new()
        } else {
            exe_relative(resolved, &source)
        };
        Some(UnitPlan {
            source,
            is_file,
            base_name,
            exe_rel_in_unit,
            message: if is_file && external_is_shared(resolved) {
                Some(SHARED_ONLY_MESSAGE.to_string())
            } else {
                None
            },
        })
    }
}

/// A canonicalized exe is relative to its real unit; when it is the unit
/// itself there is nothing to append.
fn exe_relative(resolved: &Path, source: &Path) -> PathBuf {
    resolved
        .strip_prefix(source)
        .ok()
        .map(Path::to_path_buf)
        .or_else(|| resolved.file_name().map(PathBuf::from))
        .unwrap_or_default()
}

/// Above this many `.exe` files the parent is treated as a shared tools folder
/// instead of a dedicated app folder (a typical app folder has a handful of
/// helper executables next to its main exe).
const SHARED_EXE_LIMIT: usize = 32;

const SHARED_ONLY_MESSAGE: &str = "shared folder: copied main exe only";

/// Decide the copy unit for an app registered outside the apps dir.
///
/// Default is the whole parent directory: portable apps keep their DLLs and
/// resources next to the exe. Only a drive root, the user profile directory or
/// a folder holding more than [`SHARED_EXE_LIMIT`] executables is a shared
/// bucket, and there copying just the registered exe is the safer choice.
fn external_unit(exe: &Path) -> (PathBuf, bool) {
    let Some(parent) = exe.parent() else {
        return (exe.to_path_buf(), true);
    };
    if parent.parent().is_none() || is_system_dir(parent) {
        // Drive root / user home: only the file belongs to the app.
        return (exe.to_path_buf(), true);
    }
    if count_exe_files(parent) > SHARED_EXE_LIMIT {
        (exe.to_path_buf(), true)
    } else {
        (parent.to_path_buf(), false)
    }
}

/// True when the exe would be exported as a single file from a shared bucket.
fn external_is_shared(exe: &Path) -> bool {
    let Some(parent) = exe.parent() else {
        return false;
    };
    parent.parent().is_none() || is_system_dir(parent) || count_exe_files(parent) > SHARED_EXE_LIMIT
}

fn count_exe_files(dir: &Path) -> usize {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    let path = entry.path();
                    path.is_file()
                        && path
                            .extension()
                            .map(|ext| ext.eq_ignore_ascii_case("exe"))
                            .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

/// Drive roots and the user profile are never an app folder.
fn is_system_dir(dir: &Path) -> bool {
    ["USERPROFILE", "HOME"].iter().any(|var| {
        std::env::var(var)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| scan::paths_equal(dir, Path::new(&value)))
            .unwrap_or(false)
    })
}

fn unique_target_name(base: &str, is_file: bool, used: &mut HashSet<String>) -> String {
    let (stem, ext) = if is_file {
        match base.rfind('.') {
            Some(index) if index > 0 => (&base[..index], &base[index..]),
            _ => (base, ""),
        }
    } else {
        (base, "")
    };
    let base = if base.is_empty() { "App" } else { base };
    let (stem, ext) = if base == "App" && stem.is_empty() {
        ("App", "")
    } else {
        (stem, ext)
    };
    let mut n = 1usize;
    loop {
        let candidate = if n == 1 {
            base.to_string()
        } else {
            format!("{stem}-{n}{ext}")
        };
        if used.insert(candidate.to_lowercase()) {
            return candidate;
        }
        n += 1;
    }
}

// ---------------------------------------------------------------------------
// Report helpers
// ---------------------------------------------------------------------------

fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rest = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (year + if month <= 2 { 1 } else { 0 }, month, day)
}

// ---------------------------------------------------------------------------
// Path helpers (pure, case- and separator-insensitive like Windows)
// ---------------------------------------------------------------------------

fn to_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn canonical_or_self(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Resolve a possibly non-existing path against its nearest existing ancestor,
/// then normalize `.`/`..` lexically.
fn normalize_abs(path: &Path) -> PathBuf {
    if let Ok(c) = dunce::canonicalize(path) {
        return c;
    }
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    let mut current = path.to_path_buf();
    while !current.exists() {
        let Some(name) = current.file_name() else { break };
        missing.push(name.to_os_string());
        let Some(parent) = current.parent() else { break };
        current = parent.to_path_buf();
    }
    let mut base = dunce::canonicalize(&current).unwrap_or(current);
    for part in missing.into_iter().rev() {
        base.push(part);
    }
    lexical_normalize(&base)
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn is_under(child: &Path, base: &Path) -> bool {
    rel_under(child, base).is_some()
}

/// `Some(rest)` when `child` equals `base` or lives below it.
fn rel_under(child: &Path, base: &Path) -> Option<PathBuf> {
    let child = to_slash(child);
    let base_raw = to_slash(base);
    let base = base_raw.trim_end_matches('/');
    if base.is_empty() {
        return Some(PathBuf::from(child.trim_start_matches('/')));
    }
    let mut end = 0usize;
    let mut child_chars = child.char_indices();
    for base_ch in base.chars() {
        match child_chars.next() {
            Some((index, ch)) if ch.eq_ignore_ascii_case(&base_ch) => {
                end = index + ch.len_utf8();
            }
            _ => return None,
        }
    }
    let rest = child.get(end..)?;
    if rest.is_empty() {
        Some(PathBuf::new())
    } else {
        rest.strip_prefix('/').map(PathBuf::from)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ph-migrate-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn fake_file(path: &Path, data: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }

    fn fake_exe(path: &Path) {
        fake_file(path, b"MZ fake executable");
    }

    fn write_config(root: &Path, apps: &[(&str, String)]) -> Config {
        let mut cfg = Config::default();
        for (name, path) in apps {
            cfg.apps.push(AppEntry::new(*name, path.clone()));
        }
        config::save(root, &cfg).unwrap();
        cfg
    }

    fn read_config(path: &Path) -> Config {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    // --- classify ----------------------------------------------------------

    #[test]
    fn classify_local_linked_external_missing() {
        let apps = Path::new("Root/Apps");
        let app = Path::new("Root/Apps/Demo/Demo.exe");

        assert_eq!(classify(false, app, app, apps), "local");
        // Windows-style separators and case differences still count as inside.
        assert_eq!(
            classify(
                false,
                Path::new("root\\apps\\Demo\\Demo.exe"),
                Path::new("Root/Apps/Demo/Demo.exe"),
                apps
            ),
            "local"
        );
        // Stored path inside the apps dir, real target outside: junction link.
        assert_eq!(
            classify(false, app, Path::new("Elsewhere/Real/Demo.exe"), apps),
            "linked"
        );
        // Stored path outside the apps dir: registered externally.
        assert_eq!(
            classify(
                false,
                Path::new("Elsewhere/Tool/Tool.exe"),
                Path::new("Elsewhere/Tool/Tool.exe"),
                apps
            ),
            "external"
        );
        assert_eq!(classify(true, app, Path::new(""), apps), "missing");
    }

    // --- audit -------------------------------------------------------------

    #[test]
    fn audit_classifies_and_resolves_targets() {
        let root = tempdir("audit");
        fake_exe(&root.join("Apps/Demo/Demo.exe"));
        let external = tempdir("audit-ext");
        fake_exe(&external.join("Tool/Tool.exe"));
        let cfg = write_config(
            &root,
            &[
                ("Demo", "Apps/Demo/Demo.exe".to_string()),
                ("Tool", to_slash(&external.join("Tool/Tool.exe"))),
                ("Gone", "Apps/Gone/Gone.exe".to_string()),
            ],
        );

        let audits = audit(&root, &cfg);
        assert_eq!(audits.len(), 3);
        assert_eq!(audits[0].kind, "local");
        assert_eq!(audits[1].kind, "external");
        assert!(audits[1].target.ends_with("Tool/Tool.exe"), "{:?}", audits[1]);
        assert_eq!(audits[2].kind, "missing");
        assert!(audits[2].target.is_empty());
    }

    // --- export ------------------------------------------------------------

    #[test]
    fn exports_local_app_and_writes_self_contained_bundle() {
        let root = tempdir("export-local");
        let base = tempdir("export-local-base");
        fake_exe(&root.join("Apps/Demo/Demo.exe"));
        fake_file(&root.join("Apps/Demo/data.txt"), b"payload");
        fake_file(&root.join("Themes/demo.css"), b":root{}");
        let cfg = write_config(&root, &[("Demo", "Apps/Demo/Demo.exe".to_string())]);
        let root_config_before = fs::read_to_string(root.join("config.json")).unwrap();

        let current_exe = base.join("SampleHub.exe");
        fake_exe(&current_exe);
        let target = base.join("bundle");

        let report =
            export_bundle_with_exe(&root, &cfg, &target, &current_exe).expect("export should work");
        assert!(report.ok);
        assert_eq!((report.total, report.copied, report.failed), (1, 1, 0));

        assert!(target.join("SampleHub.exe").is_file());
        assert!(target.join("config.json").is_file());
        assert!(target.join("migration-report.json").is_file());
        assert!(target.join("Apps/Demo/Demo.exe").is_file());
        assert!(target.join("Apps/Demo/data.txt").is_file());
        assert!(target.join("Themes/demo.css").is_file());

        let exported_cfg = read_config(&target.join("config.json"));
        assert_eq!(exported_cfg.apps[0].path, "Apps/Demo/Demo.exe");
        assert_eq!(report.items[0].to, "Apps/Demo/Demo.exe");

        let raw = fs::read_to_string(target.join("migration-report.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["schema"], 1);
        assert_eq!(value["copied"], 1);
        assert!(value["version"].as_str().is_some());
        assert!(value["exportedAt"].as_str().unwrap().ends_with('Z'));
        assert_eq!(value["items"].as_array().unwrap().len(), 1);

        // The running directory is read-only: nothing changed at the root.
        assert_eq!(
            fs::read_to_string(root.join("config.json")).unwrap(),
            root_config_before
        );
        assert!(root.join("Apps/Demo/Demo.exe").is_file());
        assert!(!root.join("Apps/Demo/Demo-2.exe").exists());
    }

    #[test]
    fn exports_external_app_into_apps_and_rewrites_its_path() {
        let root = tempdir("export-ext");
        let base = tempdir("export-ext-base");
        let external = base.join("ExternalTool");
        fake_exe(&external.join("Tool.exe"));
        fake_file(&external.join("data.bin"), b"data");
        let cfg = write_config(
            &root,
            &[("Tool", to_slash(&external.join("Tool.exe")))],
        );
        let current_exe = base.join("SampleHub.exe");
        fake_exe(&current_exe);
        let target = base.join("bundle");

        let report =
            export_bundle_with_exe(&root, &cfg, &target, &current_exe).expect("export should work");
        assert_eq!(report.copied, 1);
        assert!(target.join("Apps/Tool/Tool.exe").is_file());
        assert!(target.join("Apps/Tool/data.bin").is_file());
        let exported = read_config(&target.join("config.json"));
        assert_eq!(exported.apps[0].path, "Apps/Tool/Tool.exe");
    }

    #[test]
    fn exports_external_dedicated_folder_with_dependencies() {
        let root = tempdir("export-ext-dir");
        let base = tempdir("export-ext-dir-base");
        let app_dir = base.join("DedicatedApp");
        // A real app folder: a second exe and DLLs next to the main exe.
        fake_exe(&app_dir.join("Main.exe"));
        fake_exe(&app_dir.join("Helper.exe"));
        fake_file(&app_dir.join("runtime.dll"), b"dll payload");
        let cfg = write_config(
            &root,
            &[("Dedicated App", to_slash(&app_dir.join("Main.exe")))],
        );
        let current_exe = base.join("SampleHub.exe");
        fake_exe(&current_exe);
        let target = base.join("bundle");

        let report = export_bundle_with_exe(&root, &cfg, &target, &current_exe).unwrap();
        assert_eq!(report.copied, 1);
        assert!(target.join("Apps/Dedicated App/Main.exe").is_file());
        assert!(target.join("Apps/Dedicated App/Helper.exe").is_file());
        assert_eq!(
            fs::read(target.join("Apps/Dedicated App/runtime.dll")).unwrap(),
            b"dll payload"
        );
        let exported = read_config(&target.join("config.json"));
        assert_eq!(exported.apps[0].path, "Apps/Dedicated App/Main.exe");
        assert!(!report.items[0].message.contains("shared folder"));
    }

    #[test]
    fn exports_external_shared_folder_as_single_exe_only() {
        let root = tempdir("export-ext-shared");
        let base = tempdir("export-ext-shared-base");
        let bucket = base.join("ToolBucket");
        fake_exe(&bucket.join("Main.exe"));
        // 33 exes in total: above SHARED_EXE_LIMIT(32), so this is a bucket.
        for index in 1..=32 {
            fake_exe(&bucket.join(format!("tool-{index:02}.exe")));
        }
        let cfg = write_config(&root, &[("Bucket", to_slash(&bucket.join("Main.exe")))]);
        let current_exe = base.join("SampleHub.exe");
        fake_exe(&current_exe);
        let target = base.join("bundle");

        let report = export_bundle_with_exe(&root, &cfg, &target, &current_exe).unwrap();
        assert_eq!(report.copied, 1);
        assert!(target.join("Apps/Main.exe").is_file());
        assert!(
            !target.join("Apps/tool-01.exe").exists(),
            "unrelated bucket exes must not be exported"
        );
        assert!(
            report.items[0].message.contains("shared folder"),
            "shared-folder exports must be flagged: {}",
            report.items[0].message
        );
        let exported = read_config(&target.join("config.json"));
        assert_eq!(exported.apps[0].path, "Apps/Main.exe");
    }

    #[test]
    fn missing_app_fails_without_blocking_other_exports() {
        let root = tempdir("export-missing");
        let base = tempdir("export-missing-base");
        fake_exe(&root.join("Apps/Demo/Demo.exe"));
        let cfg = write_config(
            &root,
            &[
                ("Demo", "Apps/Demo/Demo.exe".to_string()),
                ("Gone", "Apps/Gone/Gone.exe".to_string()),
            ],
        );
        let current_exe = base.join("SampleHub.exe");
        fake_exe(&current_exe);
        let target = base.join("bundle");

        let report =
            export_bundle_with_exe(&root, &cfg, &target, &current_exe).expect("export runs");
        assert!(!report.ok, "a failed app marks the bundle report as not ok");
        assert_eq!((report.total, report.copied, report.failed), (2, 1, 1));
        let gone = report.items.iter().find(|i| i.name == "Gone").unwrap();
        assert!(!gone.ok);
        assert!(gone.to.is_empty());
        assert!(!gone.message.is_empty());

        let exported = read_config(&target.join("config.json"));
        assert_eq!(exported.apps[0].path, "Apps/Demo/Demo.exe");
        // A failed app keeps its original (unreachable) path for diagnosis.
        assert_eq!(exported.apps[1].path, "Apps/Gone/Gone.exe");
        assert!(target.join("migration-report.json").is_file());
    }

    #[cfg(windows)]
    #[test]
    fn exports_linked_app_as_real_content() {
        let root = tempdir("export-linked");
        let base = tempdir("export-linked-base");
        let real = base.join("RealDemo");
        fake_exe(&real.join("Demo.exe"));
        fake_file(&real.join("data.txt"), b"linked data");
        crate::infra::link::create_dir_junction(&root.join("Apps/Demo"), &real).unwrap();
        let cfg = write_config(&root, &[("Demo", "Apps/Demo/Demo.exe".to_string())]);

        let audits = audit(&root, &cfg);
        assert_eq!(audits[0].kind, "linked");
        assert!(audits[0].target.ends_with("RealDemo/Demo.exe"), "{:?}", audits[0]);

        let current_exe = base.join("SampleHub.exe");
        fake_exe(&current_exe);
        let target = base.join("bundle");
        let report = export_bundle_with_exe(&root, &cfg, &target, &current_exe).unwrap();
        assert_eq!(report.copied, 1);

        // The bundle must be self-contained: deleting the link target leaves it
        // intact, and the rewritten config points at the copied content.
        fs::remove_dir_all(&real).unwrap();
        assert!(target.join("Apps/Demo/Demo.exe").is_file());
        assert_eq!(
            fs::read(target.join("Apps/Demo/data.txt")).unwrap(),
            b"linked data"
        );
        let exported = read_config(&target.join("config.json"));
        assert_eq!(exported.apps[0].path, "Apps/Demo/Demo.exe");
    }

    #[test]
    fn refuses_target_inside_or_equal_to_root() {        let root = tempdir("export-guard-root");
        let base = tempdir("export-guard-root-base");
        fake_exe(&root.join("Apps/Demo/Demo.exe"));
        let cfg = write_config(&root, &[("Demo", "Apps/Demo/Demo.exe".to_string())]);
        let current_exe = base.join("SampleHub.exe");
        fake_exe(&current_exe);

        let inside = root.join("bundle");
        let err = export_bundle_with_exe(&root, &cfg, &inside, &current_exe).unwrap_err();
        assert!(err.contains("程序目录内"), "{err}");
        assert!(!inside.exists(), "guard must not create the target");

        let same = root.clone();
        let err = export_bundle_with_exe(&root, &cfg, &same, &current_exe).unwrap_err();
        assert!(err.contains("程序目录内"), "{err}");
    }

    #[test]
    fn refuses_non_empty_target_and_accepts_empty_one() {
        let root = tempdir("export-guard-empty");
        let base = tempdir("export-guard-empty-base");
        fake_exe(&root.join("Apps/Demo/Demo.exe"));
        let cfg = write_config(&root, &[("Demo", "Apps/Demo/Demo.exe".to_string())]);
        let current_exe = base.join("SampleHub.exe");
        fake_exe(&current_exe);

        let busy = base.join("busy");
        fs::create_dir_all(&busy).unwrap();
        fs::write(busy.join("existing.txt"), b"keep me").unwrap();
        let err = export_bundle_with_exe(&root, &cfg, &busy, &current_exe).unwrap_err();
        assert!(err.contains("空"), "{err}");
        assert!(busy.join("existing.txt").is_file(), "must not touch it");

        let empty = base.join("empty");
        fs::create_dir_all(&empty).unwrap();
        let report = export_bundle_with_exe(&root, &cfg, &empty, &current_exe).unwrap();
        assert_eq!(report.copied, 1);
    }

    #[test]
    fn duplicate_target_names_get_suffixes() {
        let root = tempdir("export-dupes");
        let base = tempdir("export-dupes-base");
        // Two different external folders registered under the same display name:
        // they must not overwrite each other inside Apps/.
        let first = base.join("first/Shared.exe");
        let second = base.join("second/Shared.exe");
        fake_exe(&first);
        fake_exe(&second);
        let cfg = write_config(
            &root,
            &[
                ("Shared", to_slash(&first)),
                ("Shared", to_slash(&second)),
            ],
        );
        let current_exe = base.join("SampleHub.exe");
        fake_exe(&current_exe);
        let target = base.join("bundle");

        let report = export_bundle_with_exe(&root, &cfg, &target, &current_exe).unwrap();
        assert_eq!(report.copied, 2);
        assert!(target.join("Apps/Shared/Shared.exe").is_file());
        assert!(target.join("Apps/Shared-2/Shared.exe").is_file());
        let exported = read_config(&target.join("config.json"));
        assert_eq!(exported.apps[0].path, "Apps/Shared/Shared.exe");
        assert_eq!(exported.apps[1].path, "Apps/Shared-2/Shared.exe");
    }
}
