//! Import discovered apps: optionally move files into the apps folder, then
//! register them in the config. Shared by the Tauri command and the CLI mode.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::classify;
use super::config::AppEntry;
use super::path::to_rel;
use super::scan;
use crate::infra::{link, transfer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportMode {
    Link,
    Move,
    Register,
}

impl ImportMode {
    pub fn label(self) -> &'static str {
        match self {
            ImportMode::Link => "已链接并添加",
            ImportMode::Move => "已移动并添加",
            ImportMode::Register => "已添加（未移动文件）",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportItem {
    pub path: String,
    pub name: String,
    pub kind: String,
    /// Optional pre-assigned category (e.g. from an external import list).
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    pub name: String,
    pub ok: bool,
    pub message: String,
}

pub fn run(
    root: &Path,
    apps_dir: &Path,
    existing: HashSet<String>,
    mut existing_names: HashSet<String>,
    items: Vec<ImportItem>,
    mode: ImportMode,
    backup_source: bool,
    progress: &mut dyn FnMut(&ImportOutcome),
) -> (Vec<AppEntry>, Vec<ImportOutcome>) {
    let mut entries: Vec<AppEntry> = Vec::new();
    let mut outcomes: Vec<ImportOutcome> = Vec::new();
    let mut seen = existing;

    for item in items {
        let source = PathBuf::from(&item.path);
        let name = item.name.clone();

        let outcome = if !source.exists() {
            err(name, format!("源路径不存在：{}", source.display()))
        } else if is_protected(&source) {
            err(name, "源路径位于受保护的系统目录，拒绝移动".to_string())
        } else if source.starts_with(apps_dir) {
            ok(name, "已在应用目录，跳过".to_string())
        } else if existing_names.contains(&name.to_lowercase()) {
            ok(name, "同名应用已存在，跳过".to_string())
        } else {
            let target = if item.kind == "file" {
                match source.file_name() {
                    Some(file) => apps_dir.join(file),
                    None => {
                        let o = err(name, "无法确定目标文件名".to_string());
                        progress(&o);
                        outcomes.push(o);
                        continue;
                    }
                }
            } else {
                apps_dir.join(safe_name(&name))
            };

            if mode != ImportMode::Register && target.exists() {
                err(name, format!("目标已存在：{}", target.display()))
            } else {
                let final_path = match mode {
                    ImportMode::Move => {
                        match transfer::move_path(&source, &target, backup_source) {
                            Ok(()) => target,
                            Err(e) => {
                                let o = err(name, e);
                                progress(&o);
                                outcomes.push(o);
                                continue;
                            }
                        }
                    }
                    ImportMode::Link => match link::create_link(&source, &target) {
                        Ok(()) => target,
                        Err(e) => {
                            let o = err(name, e);
                            progress(&o);
                            outcomes.push(o);
                            continue;
                        }
                    },
                    ImportMode::Register => source,
                };

                let exe = if final_path.is_dir() {
                    scan::find_main_exe_direct(&final_path)
                        .or_else(|| scan::find_main_exe(&final_path))
                        .map(|(p, _)| p)
                } else {
                    Some(final_path.clone())
                };

                match exe {
                    None => err(name, "找不到主程序".to_string()),
                    Some(exe) => {
                        let rel = to_rel(root, &exe);
                        if !seen.insert(rel.clone()) {
                            ok(name, "已在列表中，跳过".to_string())
                        } else {
                            existing_names.insert(name.to_lowercase());
                            let mut entry = AppEntry::new(name.clone(), rel);
                            // An invalid/unknown category from the import list is
                            // ignored rather than trusted.
                            if let Some(category) = item
                                .category
                                .as_deref()
                                .map(str::trim)
                                .filter(|category| classify::is_valid_category(category))
                            {
                                entry.category = category.to_string();
                            }
                            entries.push(entry);
                            ok(name, mode.label().to_string())
                        }
                    }
                }
            }
        };

        progress(&outcome);
        outcomes.push(outcome);
    }

    (entries, outcomes)
}

fn ok(name: String, message: String) -> ImportOutcome {
    ImportOutcome { name, ok: true, message }
}

fn err(name: String, message: String) -> ImportOutcome {
    ImportOutcome { name, ok: false, message }
}

pub fn safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.').to_string();
    if trimmed.is_empty() {
        "App".to_string()
    } else {
        trimmed
    }
}

pub fn is_protected(path: &Path) -> bool {
    let s = path.to_string_lossy().to_lowercase().replace('/', "\\");
    ["c:\\windows", "c:\\program files", "c:\\programdata"]
        .iter()
        .any(|prefix| s.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ph-import-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn fake_exe(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut d = vec![0u8; 256];
        d[0] = b'M';
        d[1] = b'Z';
        d[0x3C..0x40].copy_from_slice(&(128u32).to_le_bytes());
        d[128..132].copy_from_slice(b"PE\0\0");
        d[152..154].copy_from_slice(&(0x10bu16).to_le_bytes());
        std::fs::write(path, d).unwrap();
    }

    #[test]
    fn moves_and_registers_folder() {
        let base = tempdir("folder");
        let src = base.join("src/Demo");
        fake_exe(&src.join("Demo.exe"));
        let apps = base.join("apps");

        let (entries, outcomes) = run(
            &base,
            &apps,
            HashSet::new(),
            HashSet::new(),
            vec![ImportItem { path: src.to_string_lossy().to_string(), name: "Demo".into(), kind: "folder".into(), category: None }],
            ImportMode::Move,
            false,
            &mut |_| {},
        );
        assert_eq!(entries.len(), 1);
        assert!(outcomes[0].ok, "{outcomes:?}");
        assert!(apps.join("Demo/Demo.exe").exists());
        assert!(!src.exists());
        assert_eq!(entries[0].path, "apps/Demo/Demo.exe");
    }

    #[test]
    fn adds_without_moving() {
        let base = tempdir("nomove");
        let src = base.join("src/Tool/Tool.exe");
        fake_exe(&src);
        let apps = base.join("apps");

        let (entries, outcomes) = run(
            &base,
            &apps,
            HashSet::new(),
            HashSet::new(),
            vec![ImportItem { path: base.join("src/Tool").to_string_lossy().to_string(), name: "Tool".into(), kind: "folder".into(), category: None }],
            ImportMode::Register,
            false,
            &mut |_| {},
        );
        assert_eq!(entries.len(), 1);
        assert!(outcomes[0].ok);
        assert!(src.exists(), "source must be kept when move=false");
    }

    #[test]
    fn skips_existing_target() {
        let base = tempdir("exists");
        let src = base.join("src/Demo");
        fake_exe(&src.join("Demo.exe"));
        let apps = base.join("apps");
        std::fs::create_dir_all(apps.join("Demo")).unwrap();

        let (entries, outcomes) = run(
            &base,
            &apps,
            HashSet::new(),
            HashSet::new(),
            vec![ImportItem { path: src.to_string_lossy().to_string(), name: "Demo".into(), kind: "folder".into(), category: None }],
            ImportMode::Move,
            false,
            &mut |_| {},
        );
        assert!(entries.is_empty());
        assert!(!outcomes[0].ok);
        assert!(src.exists());
    }
    #[test]
    fn links_folder_without_moving() {
        let base = tempdir("link");
        let src = base.join("src/Demo");
        fake_exe(&src.join("Demo.exe"));
        let apps = base.join("apps");

        let (entries, outcomes) = run(
            &base,
            &apps,
            HashSet::new(),
            HashSet::new(),
            vec![ImportItem { path: src.to_string_lossy().to_string(), name: "Demo".into(), kind: "folder".into(), category: None }],
            ImportMode::Link,
            false,
            &mut |_| {},
        );
        assert_eq!(entries.len(), 1);
        assert!(outcomes[0].ok, "{outcomes:?}");
        assert!(src.exists(), "link mode must keep the source");
        assert!(apps.join("Demo/Demo.exe").exists(), "junction should expose the app");
    }

    #[test]
    fn import_applies_valid_category_and_ignores_unknown() {
        let base = tempdir("category");
        let src = base.join("src/DevTool");
        fake_exe(&src.join("DevTool.exe"));
        let apps = base.join("apps");

        let (entries, _) = run(
            &base,
            &apps,
            HashSet::new(),
            HashSet::new(),
            vec![ImportItem {
                path: src.to_string_lossy().to_string(),
                name: "DevTool".into(),
                kind: "folder".into(),
                category: Some("dev".into()),
            }],
            ImportMode::Move,
            false,
            &mut |_| {},
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].category, "dev");

        let src2 = base.join("src/BadTool");
        fake_exe(&src2.join("BadTool.exe"));
        let (entries, _) = run(
            &base,
            &apps,
            HashSet::new(),
            HashSet::new(),
            vec![ImportItem {
                path: src2.to_string_lossy().to_string(),
                name: "BadTool".into(),
                kind: "folder".into(),
                category: Some("not-a-category".into()),
            }],
            ImportMode::Move,
            false,
            &mut |_| {},
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].category, "", "unknown categories are ignored");
    }
}
