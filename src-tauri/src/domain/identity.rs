//! Product identity derived at runtime, never hardcoded.
//!
//! The executable name is the single source of truth: renaming the binary
//! renames the window title, the startup shortcut and the registry guard.

use std::path::Path;

pub fn app_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| {
            p.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| env!("CARGO_PKG_NAME").to_string())
}

pub fn shortcut_file_name() -> String {
    format!("{}.lnk", app_name())
}

pub fn window_title_from(exe: &Path) -> String {
    exe.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_NAME").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_comes_from_file_name() {
        assert_eq!(window_title_from(Path::new("C:/x/MyTool.exe")), "MyTool"); // portable:allow fixture
        assert_eq!(window_title_from(Path::new("C:/")), env!("CARGO_PKG_NAME")); // portable:allow fixture
    }

    #[test]
    fn shortcut_matches_app_name() {
        assert!(shortcut_file_name().ends_with(".lnk"));
    }
}
