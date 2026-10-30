//! Headless CLI for automation:
//!   PortableHub.exe --discover [--out file]
//!   PortableHub.exe --import <file> [--move] [--out file]

use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::domain::config;
use crate::domain::discover;
use crate::domain::import::{self, ImportItem, ImportMode};
use crate::domain::path::resolve;

pub fn run(args: &[String]) -> i32 {
    let out = arg_value(args, "--out");

    if args.iter().any(|a| a == "--discover") {
        let root = current_root();
        let mut exclude = vec![root];
        if let Some(list) = arg_value(args, "--exclude") {
            for part in list.to_string_lossy().split(',') {
                let part = part.trim();
                if !part.is_empty() {
                    exclude.push(PathBuf::from(part));
                }
            }
        }

        let roots = match arg_value(args, "--roots") {
            Some(list) => list
                .to_string_lossy()
                .split(',')
                .filter_map(|part| {
                    let part = part.trim();
                    if part.is_empty() {
                        return None;
                    }
                    let normalized = if part.len() == 2 && part.ends_with(':') {
                        format!("{part}\\")
                    } else {
                        part.to_string()
                    };
                    Some(PathBuf::from(normalized))
                })
                .collect::<Vec<_>>(),
            None => discover::drives(),
        };

        let found = discover::discover(&roots, &exclude, 500, &AtomicBool::new(false));
        return emit(&serde_json::to_string_pretty(&found).unwrap_or_default(), out);
    }

    if let Some(file) = arg_value(args, "--import") {
        let text = match fs::read_to_string(&file) {
            Ok(t) => t,
            Err(e) => return fail(&format!("读取导入清单失败：{e}")),
        };
        let items: Vec<ImportItem> = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => return fail(&format!("解析导入清单失败：{e}")),
        };

        let root = current_root();
        let loaded = config::load(&root);
        let apps_dir = resolve(&root, &loaded.config.apps_dir);
        let existing = loaded.config.apps.iter().map(|a| a.path.clone()).collect();
        let existing_names = loaded.config.apps.iter().map(|a| a.name.to_lowercase()).collect();
        let mode = match arg_value(args, "--mode").map(|p| p.to_string_lossy().to_string()) {
            Some(m) if m == "link" => ImportMode::Link,
            Some(m) if m == "move" => ImportMode::Move,
            Some(m) if m == "register" => ImportMode::Register,
            Some(m) => return fail(&format!("未知 --mode：{m}（link|move|register）")),
            None => match loaded.config.import_mode.as_str() {
                "move" => ImportMode::Move,
                "register" => ImportMode::Register,
                _ => ImportMode::Link,
            },
        };
        let backup_source = args.iter().any(|a| a == "--backup");

        let mut progress = |o: &import::ImportOutcome| {
            println!("{}: {}", o.name, o.message);
        };
        let (entries, outcomes) =
            import::run(&root, &apps_dir, existing, existing_names, items, mode, backup_source, &mut progress);

        if !entries.is_empty() {
            let mut cfg = loaded.config.clone();
            cfg.apps.extend(entries);
            if let Err(e) = config::save(&root, &cfg) {
                return fail(&format!("保存配置失败：{e}"));
            }
        }

        let payload = json!({ "mode": format!("{mode:?}").to_lowercase(), "items": outcomes });
        return emit(&serde_json::to_string_pretty(&payload).unwrap_or_default(), out);
    }

    fail("未知 CLI 参数")
}

fn current_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default()
}

fn arg_value(args: &[String], key: &str) -> Option<PathBuf> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
}

fn emit(text: &str, out: Option<PathBuf>) -> i32 {
    match out {
        Some(path) => match fs::write(&path, text) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("写入输出文件失败：{e}");
                1
            }
        },
        None => {
            println!("{text}");
            0
        }
    }
}

fn fail(message: &str) -> i32 {
    eprintln!("{message}");
    1
}
