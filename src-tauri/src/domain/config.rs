//! Configuration model and persistence.
//!
//! Rules:
//! - all app paths are relative to the executable directory (absolute allowed, flagged separately)
//! - writes are atomic: tmp -> backup -> rename
//! - a corrupt file is preserved as `config.json.corrupt-<unix-time>` and defaults are used

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SCHEMA_VERSION: u32 = 1;
pub const CONFIG_NAME: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub work_dir: String,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub autostart_delay_ms: u64,
    #[serde(default = "default_true")]
    pub skip_if_running: bool,
    #[serde(default)]
    pub hidden: bool,
    /// User-defined category tags (logical classification, independent of paths).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Rule/AI-assigned semantic category (REQ-0039); empty = unclassified.
    #[serde(default)]
    pub category: String,
    /// Launch elevated via UAC (`ShellExecuteW` "runas", BUG-0023).
    #[serde(default)]
    pub run_as_admin: bool,
}

fn default_true() -> bool {
    true
}

impl AppEntry {
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            path: path.into(),
            args: Vec::new(),
            work_dir: String::new(),
            autostart: false,
            autostart_delay_ms: 0,
            skip_if_running: true,
            hidden: false,
            tags: Vec::new(),
            category: String::new(),
            run_as_admin: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct TagStyle {
    /// `#rrggbb`, or empty to use the theme default.
    pub color: String,
    /// Optional emoji / short glyph shown before the tag text.
    pub icon: String,
}

impl Default for TagStyle {
    fn default() -> Self {
        Self {
            color: String::new(),
            icon: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub app_ids: Vec<String>,
}

impl Default for Collection {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            app_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub schema_version: u32,
    pub theme: String,
    pub locale: String,
    pub import_mode: String,
    pub hotkey: String,
    pub launch_at_login: bool,
    pub start_hidden_at_login: bool,
    pub close_to_tray: bool,
    pub autostart_stagger_ms: u64,
    pub apps_dir: String,
    pub apps: Vec<AppEntry>,
    /// Global, user-defined style per tag (color / icon), keyed by tag text.
    pub tag_styles: BTreeMap<String, TagStyle>,
    /// Keep app icons pixelated (user preference, independent of the theme).
    pub pixel_icons: bool,
    /// Use the pixel font across the UI (user preference).
    pub pixel_font: bool,
    /// Last directories selected in the Discover scope; reused on the next scan.
    pub discover_roots: Vec<String>,
    /// User-curated, flat groups (manual collections of apps).
    pub collections: Vec<Collection>,
    /// Pinned app ids, shown first in the launcher / quick access.
    pub favorites: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            theme: "pixel-dark".to_string(),
            locale: "zh-CN".to_string(),
            import_mode: "link".to_string(),
            hotkey: "Ctrl+Alt+P".to_string(),
            launch_at_login: false,
            start_hidden_at_login: false,
            close_to_tray: true,
            autostart_stagger_ms: 2000,
            apps_dir: "Apps".to_string(),
            apps: Vec::new(),
            tag_styles: BTreeMap::new(),
            pixel_icons: false,
            pixel_font: true,
            discover_roots: Vec::new(),
            collections: Vec::new(),
            favorites: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadResult {
    pub config: Config,
    pub warning: Option<String>,
    pub config_path: String,
}

pub fn config_path(root: &Path) -> PathBuf {
    root.join(CONFIG_NAME)
}

pub fn load(root: &Path) -> LoadResult {
    let path = config_path(root);
    if !path.exists() {
        let cfg = Config::default();
        let _ = save(root, &cfg);
        return LoadResult {
            config: cfg,
            warning: None,
            config_path: path.to_string_lossy().to_string(),
        };
    }

    match fs::read_to_string(&path) {
        Ok(text) => {
            // Editors on Windows commonly save UTF-8 with a BOM; serde_json
            // rejects it, which used to quarantine the config and fall back to
            // defaults (BUG-0018). Strip it before parsing.
            let text = text.trim_start_matches('\u{feff}');
            match serde_json::from_str::<Config>(text) {
            Ok(cfg) => LoadResult {
                config: cfg,
                warning: None,
                config_path: path.to_string_lossy().to_string(),
            },
            Err(e) => {
                let backup = quarantine(&path);
                let cfg = Config::default();
                let _ = save(root, &cfg);
                LoadResult {
                    config: cfg,
                    warning: Some(format!(
                        "config.json 解析失败（{}），已备份为 {}",
                        e,
                        backup
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| "(备份失败)".to_string())
                    )),
                    config_path: path.to_string_lossy().to_string(),
                }
            }
            }
        }
        Err(e) => {
            let backup = quarantine(&path);
            let cfg = Config::default();
            let _ = save(root, &cfg);
            LoadResult {
                config: cfg,
                warning: Some(format!(
                    "config.json 读取失败（{}），已备份为 {}",
                    e,
                    backup
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "(备份失败)".to_string())
                )),
                config_path: path.to_string_lossy().to_string(),
            }
        }
    }
}

fn quarantine(path: &Path) -> Option<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let target = path.with_extension(format!("json.corrupt-{stamp}"));
    fs::rename(path, &target).ok().map(|_| target)
}

pub fn save(root: &Path, cfg: &Config) -> Result<(), String> {
    let path = config_path(root);
    let tmp = root.join("config.json.tmp");
    let bak = root.join("config.json.bak");

    let text = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(&tmp, text).map_err(|e| format!("写入临时配置失败：{e}"))?;

    if path.exists() {
        let _ = fs::copy(&path, &bak);
    }

    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("替换配置失败：{e}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "portablehub-test-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn roundtrip() {
        let dir = tempdir("roundtrip");
        let mut cfg = Config::default();
        cfg.apps.push(AppEntry::new("Demo", "Apps/Demo/Demo.exe"));
        save(&dir, &cfg).unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.config.apps.len(), 1);
        assert_eq!(loaded.config.apps[0].path, "Apps/Demo/Demo.exe");
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn default_apps_dir_is_relative_and_portable() {
        let cfg = Config::default();
        assert_eq!(cfg.apps_dir, "Apps");
        assert!(!Path::new(&cfg.apps_dir).is_absolute());
        assert!(cfg.discover_roots.is_empty());
    }

    #[test]
    fn tag_styles_roundtrip() {
        let dir = tempdir("tag-styles");
        let mut cfg = Config::default();
        cfg.tag_styles.insert(
            "已玩".to_string(),
            TagStyle {
                color: "#4ade80".to_string(),
                icon: "🐱".to_string(),
            },
        );
        save(&dir, &cfg).unwrap();
        let loaded = load(&dir);
        let style = loaded.config.tag_styles.get("已玩").unwrap();
        assert_eq!(style.color, "#4ade80");
        assert_eq!(style.icon, "🐱");
    }

    #[test]
    fn category_roundtrip_and_legacy_default() {
        let dir = tempdir("category");
        let mut cfg = Config::default();
        let mut app = AppEntry::new("Demo", "Apps/Demo/Demo.exe");
        app.category = "dev".to_string();
        cfg.apps.push(app);
        save(&dir, &cfg).unwrap();
        assert_eq!(load(&dir).config.apps[0].category, "dev");

        // Older configs have no category field: they load as unclassified.
        fs::write(
            config_path(&dir),
            r#"{"apps":[{"id":"x","name":"Old","path":"Apps/Old/Old.exe"}]}"#,
        )
        .unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.config.apps[0].category, "");
    }

    #[test]
    fn config_with_utf8_bom_loads() {
        // BUG-0018: a UTF-8 BOM must not be treated as corruption.
        let dir = tempdir("bom");
        let mut cfg = Config::default();
        cfg.apps.push(AppEntry::new("Demo", "Apps/Demo/Demo.exe"));
        let json = serde_json::to_string(&cfg).unwrap();
        fs::write(config_path(&dir), format!("\u{feff}{json}")).unwrap();
        let loaded = load(&dir);
        assert!(loaded.warning.is_none(), "{:?}", loaded.warning);
        assert_eq!(loaded.config.apps.len(), 1);
    }

    #[test]
    fn config_corrupt() {
        let dir = tempdir("corrupt");
        fs::write(config_path(&dir), "{ not json").unwrap();
        let loaded = load(&dir);
        assert!(loaded.warning.is_some());
        assert!(loaded.config.apps.is_empty());
        let entries: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".corrupt-"))
            .collect();
        assert!(!entries.is_empty(), "corrupt backup must exist");
    }

    #[test]
    fn missing_file_creates_default() {
        let dir = tempdir("missing");
        let loaded = load(&dir);
        assert!(config_path(&dir).exists());
        assert_eq!(loaded.config.theme, "pixel-dark");
    }
}
