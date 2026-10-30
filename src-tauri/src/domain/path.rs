//! Path helpers. Config stores paths relative to the executable directory.

use std::path::{Path, PathBuf};

pub fn resolve(root: &Path, rel: &str) -> PathBuf {
    let p = Path::new(rel);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

/// Convert to a forward-slash relative path when it lives under `root`.
pub fn to_rel(root: &Path, p: &Path) -> String {
    let abs = dunce::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let root_abs = dunce::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    match abs.strip_prefix(&root_abs) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => abs.to_string_lossy().replace('\\', "/"),
    }
}

pub fn is_absolute(s: &str) -> bool {
    Path::new(s).is_absolute()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative() {
        let root = Path::new("C:/base"); // portable:allow fixture
        assert_eq!(resolve(root, "Apps/A/A.exe"), root.join("Apps/A/A.exe"));
    }

    #[test]
    fn keeps_absolute() {
        let root = Path::new("C:/base"); // portable:allow fixture
        assert_eq!(resolve(root, "D:/x/a.exe"), PathBuf::from("D:/x/a.exe")); // portable:allow fixture
    }

    #[test]
    fn stores_relative_path_inside_root() {
        let root = std::env::temp_dir().join(format!("ph-path-in-{}", std::process::id()));
        let apps = root.join("Apps");
        std::fs::create_dir_all(&apps).unwrap();
        let stored = to_rel(&root, &apps);
        assert_eq!(stored, "Apps");
    }

    #[test]
    fn stores_absolute_path_outside_root() {
        let root = std::env::temp_dir().join(format!("ph-path-root-{}", std::process::id()));
        let outside = std::env::temp_dir().join(format!("ph-path-out-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let stored = to_rel(&root, &outside);
        assert!(Path::new(&stored).is_absolute(), "expected absolute, got {stored}");
    }
}
