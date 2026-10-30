//! Extract executable icons to PNG data URLs, with an on-disk cache.

use image::imageops::FilterType;
use image::{ImageFormat, RgbaImage};
use sha1::{Digest, Sha1};
use std::io::Cursor;
use std::path::{Path, PathBuf};

const ICON_PX: u32 = 64;

pub fn cache_dir(root: &Path) -> PathBuf {
    root.join("_cache").join("icons")
}

fn cache_key(exe: &Path) -> String {
    let mut h = Sha1::new();
    h.update(exe.to_string_lossy().as_bytes());
    if let Ok(meta) = std::fs::metadata(exe) {
        h.update(meta.len().to_le_bytes());
        if let Ok(t) = meta.modified() {
            if let Ok(d) = t.duration_since(std::time::UNIX_EPOCH) {
                h.update(d.as_secs().to_le_bytes());
            }
        }
    }
    h.finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

/// Return a `data:image/png;base64,...` string, or `None` when extraction fails.
pub fn get(root: &Path, exe: &Path) -> Option<String> {
    let dir = cache_dir(root);
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join(format!("{}.png", cache_key(exe)));

    let bytes = match std::fs::read(&file) {
        Ok(b) => b,
        Err(_) => {
            let png = render(exe)?;
            let _ = std::fs::write(&file, &png);
            png
        }
    };

    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
    Some(format!("data:image/png;base64,{b64}"))
}

fn render(exe: &Path) -> Option<Vec<u8>> {
    let icon = win_icon_extractor::extract_icon(exe).ok()?;
    let img: RgbaImage = RgbaImage::from_raw(icon.width, icon.height, icon.rgba)?;
    let resized = if icon.width == ICON_PX && icon.height == ICON_PX {
        img
    } else {
        image::imageops::resize(&img, ICON_PX, ICON_PX, FilterType::Nearest)
    };
    let mut buf = Vec::new();
    resized
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .ok()?;
    Some(buf)
}

/// Fallback colour derived from the app name, used by the UI when no icon exists.
pub fn fallback_color(name: &str) -> String {
    let mut h = Sha1::new();
    h.update(name.as_bytes());
    let d = h.finalize();
    format!("#{:02x}{:02x}{:02x}", 64 + d[0] % 128, 64 + d[1] % 128, 64 + d[2] % 128)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_exe_returns_none() {
        let dir = std::env::temp_dir();
        assert!(get(&dir, Path::new("definitely-missing.exe")).is_none());
    }

    #[test]
    fn fallback_colour_is_stable() {
        assert_eq!(fallback_color("Alpha"), fallback_color("Alpha"));
        assert_ne!(fallback_color("Alpha"), fallback_color("Beta"));
    }
}
