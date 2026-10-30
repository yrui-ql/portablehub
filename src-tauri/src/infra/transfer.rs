//! Move a file/folder with cross-volume fallback and verification.
//!
//! Same-volume moves use rename. Cross-volume copies recursively, compares
//! (file count, total bytes), and only then removes the source.

use std::fs;
use std::path::{Path, PathBuf};

pub fn move_path(src: &Path, dst: &Path, backup_source: bool) -> Result<(), String> {
    if dst.exists() {
        return Err(format!("目标已存在：{}", dst.display()));
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目标目录失败：{e}"))?;
    }

    // Same-volume rename is atomic and leaves nothing behind, but it cannot keep
    // a backup of the source. Backup mode therefore always copies.
    if !backup_source && fs::rename(src, dst).is_ok() {
        return Ok(());
    }

    copy_tree(src, dst)?;
    if tree_stats(src) != tree_stats(dst) {
        let _ = remove_path(dst);
        return Err("复制后校验不一致，已清理目标".to_string());
    }

    if backup_source {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let backup = PathBuf::from(format!("{}.migrated-{stamp}", src.to_string_lossy()));
        fs::rename(src, &backup)
            .map_err(|e| format!("源目录改名备份失败：{}（{e}）", src.display()))?;
    } else {
        remove_path(src)?;
    }
    Ok(())
}

pub(crate) fn copy_tree(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_file() {
        fs::copy(src, dst).map_err(|e| format!("复制失败：{} → {}（{e}）", src.display(), dst.display()))?;
        return Ok(());
    }
    fs::create_dir_all(dst).map_err(|e| format!("创建目录失败：{}（{e}）", dst.display()))?;
    for entry in fs::read_dir(src).map_err(|e| format!("读取目录失败：{}（{e}）", src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| {
                format!("复制失败：{} → {}（{e}）", from.display(), to.display())
            })?;
        }
    }
    Ok(())
}

pub(crate) fn tree_stats(path: &Path) -> (u64, u64) {
    if path.is_file() {
        return (1, fs::metadata(path).map(|m| m.len()).unwrap_or(0));
    }
    let mut files = 0u64;
    let mut bytes = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let (f, b) = tree_stats(&entry.path());
            files += f;
            bytes += b;
        }
    }
    (files, bytes)
}

pub fn remove_path(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|e| format!("删除源目录失败：{}（{e}）", path.display()))
    } else {
        fs::remove_file(path).map_err(|e| format!("删除源文件失败：{}（{e}）", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn backup_keeps_source_renamed() {
        let base = tempdir("backup");
        let src = base.join("src/App");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("App.exe"), b"MZ").unwrap();
        let dst = base.join("dst/App");
        move_path(&src, &dst, true).unwrap();
        assert!(dst.join("App.exe").exists());
        assert!(!src.exists(), "source should be renamed away");
        let renamed = fs::read_dir(base.join("src")).unwrap().flatten().any(|e| {
            e.file_name().to_string_lossy().contains(".migrated-")
        });
        assert!(renamed, "backup directory should exist");
    }

    fn tempdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ph-transfer-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn moves_folder_and_verifies() {
        let base = tempdir("folder");
        let src = base.join("src/App");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("App.exe"), b"MZ").unwrap();
        fs::write(src.join("sub/data.bin"), b"1234").unwrap();

        let dst = base.join("dst/App");
        move_path(&src, &dst, false).unwrap();
        assert!(!src.exists());
        assert!(dst.join("App.exe").exists());
        assert!(dst.join("sub/data.bin").exists());
    }

    #[test]
    fn refuses_existing_target() {
        let base = tempdir("exists");
        let src = base.join("a.txt");
        fs::write(&src, b"x").unwrap();
        let dst = base.join("b.txt");
        fs::write(&dst, b"y").unwrap();
        assert!(move_path(&src, &dst, false).is_err());
        assert!(src.exists());
    }

    #[test]
    fn moves_single_file() {
        let base = tempdir("file");
        let src = base.join("tool.exe");
        fs::write(&src, b"MZ").unwrap();
        let dst = base.join("apps/tool.exe");
        move_path(&src, &dst, false).unwrap();
        assert!(dst.exists());
        assert!(!src.exists());
    }
}
