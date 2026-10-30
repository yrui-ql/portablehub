//! Theme model: CSS variables + optional CSS override.
//!
//! Themes are data, not code: a JSON file maps `--ph-*` variables to values and may
//! carry a small CSS block. CSS is injected as text (never HTML) and rejected if it
//! tries to pull remote resources.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub const DEFAULT_THEME: &str = "pixel-dark";
const MAX_CSS_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    #[serde(default = "one")]
    pub schema_version: u32,
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
    #[serde(default)]
    pub css: Option<String>,
}

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSummary {
    pub id: String,
    pub name: String,
    pub author: String,
    pub builtin: bool,
    pub error: Option<String>,
}

const BUILTIN: &[(&str, &str)] = &[
    (
        "pixel-dark",
        include_str!("../../themes/pixel-dark.json"),
    ),
    (
        "pixel-paper",
        include_str!("../../themes/pixel-paper.json"),
    ),
    (
        "pixel-slate",
        include_str!("../../themes/pixel-slate.json"),
    ),
];

pub fn builtin_all() -> Vec<Theme> {
    BUILTIN
        .iter()
        .filter_map(|(_, raw)| serde_json::from_str::<Theme>(raw).ok())
        .collect()
}

pub fn builtin(id: &str) -> Option<Theme> {
    BUILTIN
        .iter()
        .find(|(bid, _)| *bid == id)
        .and_then(|(_, raw)| serde_json::from_str::<Theme>(raw).ok())
}

pub fn default_theme() -> Theme {
    builtin(DEFAULT_THEME).unwrap_or_else(|| Theme {
        schema_version: 1,
        id: DEFAULT_THEME.to_string(),
        name: "Pixel Dark".to_string(),
        author: "portablehub".to_string(),
        vars: BTreeMap::new(),
        css: None,
    })
}

pub fn validate(theme: &Theme) -> Result<(), String> {
    if theme.id.is_empty() || theme.id.len() > 64 {
        return Err("主题 id 长度非法".into());
    }
    if !theme
        .id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err("主题 id 只允许小写字母、数字、- 和 _".into());
    }
    for key in theme.vars.keys() {
        if !key.starts_with("--ph-") {
            return Err(format!("主题变量必须以 --ph- 开头：{key}"));
        }
        if !key[5..]
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(format!("主题变量名非法：{key}"));
        }
    }
    if let Some(css) = &theme.css {
        if css.len() > MAX_CSS_BYTES {
            return Err("主题 CSS 超过 64KB".into());
        }
        let lower = css.to_lowercase();
        for bad in ["@import", "javascript:", "expression(", "url(http", "url(https", "url(//"] {
            if lower.contains(bad) {
                return Err(format!("主题 CSS 含被禁止的内容：{bad}"));
            }
        }
    }
    Ok(())
}

pub fn list(dir: &Path) -> Vec<ThemeSummary> {
    let mut out: Vec<ThemeSummary> = builtin_all()
        .into_iter()
        .map(|t| ThemeSummary {
            id: t.id.clone(),
            name: t.name.clone(),
            author: t.author.clone(),
            builtin: true,
            error: None,
        })
        .collect();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|s| serde_json::from_str::<Theme>(&s).map_err(|e| e.to_string()))
            {
                Ok(t) => {
                    let error = validate(&t).err();
                    let summary = ThemeSummary {
                        id: t.id.clone(),
                        name: t.name.clone(),
                        author: t.author.clone(),
                        builtin: false,
                        error,
                    };
                    if let Some(slot) = out.iter_mut().find(|s| s.id == summary.id) {
                        *slot = summary;
                    } else {
                        out.push(summary);
                    }
                }
                Err(e) => out.push(ThemeSummary {
                    id: path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    name: "(损坏)".to_string(),
                    author: String::new(),
                    builtin: false,
                    error: Some(e),
                }),
            }
        }
    }

    out.sort_by(|a, b| b.builtin.cmp(&a.builtin).then(a.id.cmp(&b.id)));
    out
}

pub fn get(dir: &Path, id: &str) -> Result<Theme, String> {
    let user = dir.join(format!("{id}.json"));
    if user.exists() {
        let text = fs::read_to_string(&user).map_err(|e| e.to_string())?;
        let theme: Theme = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        validate(&theme)?;
        if theme.id != id {
            return Err(format!("主题文件名与 id 不一致：{id} vs {}", theme.id));
        }
        return Ok(theme);
    }
    builtin(id).ok_or_else(|| format!("主题不存在：{id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "portablehub-theme-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn builtin_themes_are_valid() {
        for t in builtin_all() {
            validate(&t).unwrap_or_else(|e| panic!("{}: {e}", t.id));
        }
        assert!(builtin(DEFAULT_THEME).is_some());
    }

    #[test]
    fn user_theme_overrides_builtin() {
        let dir = tempdir("override");
        let mut t = builtin(DEFAULT_THEME).unwrap();
        t.name = "Custom".into();
        fs::write(dir.join("pixel-dark.json"), serde_json::to_string(&t).unwrap()).unwrap();
        let got = get(&dir, "pixel-dark").unwrap();
        assert_eq!(got.name, "Custom");
    }

    #[test]
    fn theme_fallback_when_broken() {
        let dir = tempdir("broken");
        fs::write(dir.join("pixel-dark.json"), "{ broken").unwrap();
        assert!(get(&dir, "pixel-dark").is_err());
        // caller falls back to the built-in copy
        assert!(builtin("pixel-dark").is_some());
    }

    #[test]
    fn theme_css_guard() {
        let mut t = default_theme();
        t.css = Some("@import url(https://evil)".into());
        assert!(validate(&t).is_err());
        t.css = Some(".ph-card{border-color:var(--ph-accent)}".into());
        assert!(validate(&t).is_ok());
    }
}
