//! Directory junctions and hard links (no admin required on Windows).
//!
//! Junctions are used for folders, hard links for single files on the same volume.
//! Both keep the original data in place; the apps folder only gains a link.

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn create_dir_junction(link: &Path, target: &Path) -> Result<(), String> {
    if link.exists() {
        return Err(format!("链接路径已存在：{}", link.display()));
    }
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建链接父目录失败：{e}"))?;
    }
    // cmd.exe parses the whole command as one string reliably; passing mklink's
    // arguments separately breaks on some Windows versions.
    let cmdline = format!(
        "mklink /J \"{}\" \"{}\"",
        link.display(),
        target.display()
    );
    let out = Command::new("cmd")
        .raw_arg("/C")
        .raw_arg(&cmdline)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("无法调用 mklink：{e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "创建目录联接失败：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

pub fn create_link(source: &Path, link: &Path) -> Result<(), String> {
    if source.is_dir() {
        create_dir_junction(link, source)
    } else {
        if let Some(parent) = link.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建链接父目录失败：{e}"))?;
        }
        std::fs::hard_link(source, link).map_err(|e| {
            format!(
                "创建硬链接失败（跨卷不支持，请改用移动或仅登记）：{e}"
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tempdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ph-link-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn junction_points_to_source() {
        let base = tempdir("junction");
        let target = base.join("real-app");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("App.exe"), b"MZ").unwrap();

        let link = base.join("apps/App");
        create_dir_junction(&link, &target).unwrap();
        assert!(link.join("App.exe").exists(), "junction should expose the target");
        assert!(target.exists(), "target must remain in place");
    }

    #[test]
    fn hard_link_keeps_source() {
        let base = tempdir("hard");
        let src = base.join("tool.exe");
        std::fs::write(&src, b"MZ").unwrap();
        let link = base.join("apps/tool.exe");
        std::fs::create_dir_all(base.join("apps")).unwrap();
        create_link(&src, &link).unwrap();
        assert!(link.exists());
        assert!(src.exists());
    }
}
