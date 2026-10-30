//! Semantic classification of apps: local keyword rules (REQ-0039) plus the
//! optional AI path (REQ-0040).
//!
//! The fixed category ids match the frontend `AppCategory` union. Rule
//! classification is intentionally simple and deterministic; the AI path sends
//! batches of at most [`BATCH_SIZE`] apps to the configured provider, caches the
//! answers under `<root>/_cache/ai-classify.json`, and falls back to the rules
//! per batch (never failing the whole call).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::ai;

/// Fixed category ids. The order is also the tie-break priority.
pub const CATEGORIES: &[&str] = &[
    "system",
    "network",
    "dev",
    "game",
    "media",
    "document",
    "security",
    "storage",
    "comm",
    "runtime",
    "other",
];

/// Keyword table, aligned with [`CATEGORIES`] (excluding the `other` fallback).
const RULES: &[(&str, &[&str])] = &[
    (
        "system",
        &[
            "driver", "驱动", "清理", "卸载", "优化", "硬件检测", "硬件", "工具箱",
            "注册表", "启动项", "修复", "cleaner", "tweak", "boost", "care", "control", "fan",
        ],
    ),
    (
        "network",
        &[
            "vpn", "proxy", "dns", "代理", "网络", "下载", "telegram", "共享", "远程",
            "v2ray", "trojan", "ssr", "机场", "加速器", "browser", "浏览器", "torrent", "bt",
        ],
    ),
    (
        "dev",
        &[
            "code", "ide", "git", "python", "node", "docker", "编辑器", "开发", "调试",
            "vscode", "编程", "终端", "terminal", "ssh", "数据库", "database", "postman",
        ],
    ),
    (
        "game",
        &[
            "game", "游戏", "修改器", "trainer", "模拟器", "emulator", "steam", "启动器",
            "游戏库", "mod", "retroarch",
        ],
    ),
    (
        "media",
        &[
            "播放", "截图", "录屏", "视频", "音频", "图像", "音乐", "ffmpeg", "obs",
            "player", "photo", "video", "audio", "music", "image", "screenshot", "record",
            "剪辑", "直播", "potplayer", "vlc",
        ],
    ),
    (
        "document",
        &[
            "office", "word", "excel", "pdf", "笔记", "文档", "阅读", "ppt", "markdown",
            "note", "reader", "办公", "wps", "翻译", "translate",
        ],
    ),
    (
        "security",
        &[
            "杀毒", "安全", "密码", "防火墙", "取证", "antivirus", "firewall", "password",
            "加密", "密钥", "security", "vault", "keeper",
        ],
    ),
    (
        "storage",
        &[
            "备份", "同步", "磁盘", "分区", "镜像", "压缩", "解压", "backup", "sync",
            "disk", "partition", "iso", "zip", "7z", "rar", "winrar", "clone",
        ],
    ),
    (
        "comm",
        &[
            "微信", "qq", "邮箱", "聊天", "会议", "wechat", "mail", "chat", "meeting",
            "discord", "slack", "zoom", "telegram-desktop", "钉钉", "飞书",
        ],
    ),
    (
        "runtime",
        &[
            "runtime", "运行库", "vcredist", "dotnet", ".net", "java", "redist",
            "directx", "jre", "jdk", "vc++", "msvc",
        ],
    ),
];

/// Input for [`classify_rules`], built from a stored app or a discovery item.
///
/// `#[serde(default)]` lets discovery payloads in: they carry no `id` (filled
/// from the path by the command) and may omit `size`.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ClassifyInput {
    pub id: String,
    pub name: String,
    pub path: String,
    pub exe: String,
    pub size: u64,
}

/// One classification suggestion for an app.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub id: String,
    pub category: String,
    pub confidence: f64,
    pub reason: String,
    pub source: String,
}

/// App payload for `apply_categories`; unknown categories are ignored.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryApply {
    pub id: String,
    pub category: String,
}

pub fn is_valid_category(category: &str) -> bool {
    CATEGORIES.contains(&category)
}

/// Classify one app with the local keyword rules.
///
/// A match against the display name or executable file name is "strong"
/// (confidence 0.9); a match that only appears in the stored path is "weak"
/// (0.75). When several categories match, the highest score wins and the order
/// of [`CATEGORIES`] breaks ties. No match falls back to `other` (0.3).
pub fn classify_rules(input: &ClassifyInput) -> Suggestion {
    let name = input.name.to_lowercase();
    let exe = Path::new(&input.exe)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let path = input.path.to_lowercase().replace('\\', "/");

    let mut best: Option<(&'static str, u8, Vec<&'static str>)> = None;
    for (category, keywords) in RULES {
        let mut strong: Vec<&'static str> = Vec::new();
        let mut weak: Vec<&'static str> = Vec::new();
        for keyword in *keywords {
            if name.contains(keyword) || exe.contains(keyword) {
                if !strong.contains(keyword) {
                    strong.push(keyword);
                }
            } else if path.contains(keyword) && !weak.contains(keyword) {
                weak.push(keyword);
            }
        }
        let (score, matched) = if !strong.is_empty() {
            (2u8, strong)
        } else if !weak.is_empty() {
            (1u8, weak)
        } else {
            continue;
        };
        let better = match &best {
            None => true,
            Some((_, best_score, _)) => score > *best_score,
        };
        if better {
            best = Some((category, score, matched));
        }
    }

    match best {
        Some((category, score, matched)) => {
            let keywords: Vec<&str> = matched
                .iter()
                .map(|keyword| *keyword)
                .collect();
            let reason = if score == 2 {
                format!("匹配关键词：{}（名称/程序名）", keywords.join("、"))
            } else {
                format!("匹配关键词：{}（路径）", keywords.join("、"))
            };
            Suggestion {
                id: input.id.clone(),
                category: category.to_string(),
                confidence: if score == 2 { 0.9 } else { 0.75 },
                reason,
                source: "rules".to_string(),
            }
        }
        None => Suggestion {
            id: input.id.clone(),
            category: "other".to_string(),
            confidence: 0.3,
            reason: "未匹配到分类关键词".to_string(),
            source: "rules".to_string(),
        },
    }
}

// ---------------------------------------------------------------------------
// AI classification (REQ-0040)
// ---------------------------------------------------------------------------

/// Apps per AI request: small enough to keep responses parseable.
pub const BATCH_SIZE: usize = 20;

const CACHE_DIR: &str = "_cache";
const CACHE_NAME: &str = "ai-classify.json";
/// Hard cap so the cache cannot grow without bound.
const CACHE_LIMIT: usize = 2000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ClassifyCache {
    entries: BTreeMap<String, CachedEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct CachedEntry {
    category: String,
    confidence: f64,
    reason: String,
}

pub fn cache_path(root: &Path) -> PathBuf {
    root.join(CACHE_DIR).join(CACHE_NAME)
}

fn load_cache(root: &Path) -> ClassifyCache {
    fs::read_to_string(cache_path(root))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_cache(root: &Path, cache: &ClassifyCache) -> Result<(), String> {
    let path = cache_path(root);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("创建 AI 分类缓存目录失败：{e}"))?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(cache).map_err(|e| format!("序列化 AI 分类缓存失败：{e}"))?;
    fs::write(&tmp, text).map_err(|e| format!("写入 AI 分类缓存失败：{e}"))?;
    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("替换 AI 分类缓存失败：{e}")
    })
}

/// Engine fingerprint: a different provider/model/CLI version invalidates hits.
pub fn engine_label(cfg: &ai::AiConfig) -> String {
    format!(
        "{}|{}|{}",
        cfg.provider.trim(),
        cfg.model.trim(),
        cfg.cli_version.trim()
    )
}

/// Cache key: normalized path + size + name + engine.
fn cache_key(engine: &str, input: &ClassifyInput) -> String {
    let path = input.path.replace('\\', "/").to_lowercase();
    format!(
        "{engine}\u{1f}{path}\u{1f}{}\u{1f}{}",
        input.size,
        input.name.to_lowercase()
    )
}

fn prune_cache(cache: &mut ClassifyCache) {
    while cache.entries.len() > CACHE_LIMIT {
        let Some(key) = cache.entries.keys().next().cloned() else {
            break;
        };
        cache.entries.remove(&key);
    }
}

/// Prompt asking the model for a strict JSON array, one object per app.
pub fn build_prompt(inputs: &[ClassifyInput]) -> String {
    let mut prompt = String::new();
    prompt.push_str("Classify each Windows portable application into exactly one of these categories:\n");
    prompt.push_str(&CATEGORIES.join(", "));
    prompt.push_str("\n\nReturn a strict JSON array, one object per app, in this shape:\n");
    prompt.push_str("[{\"i\":0,\"category\":\"dev\",\"confidence\":0.9,\"reason\":\"short reason\"}]\n");
    prompt.push_str(
        "Rules: category must be one of the ids above; confidence is a number from 0 to 1; \
         classify by what the program does, not by where it is stored; \
         output only the JSON array (no markdown fences, no explanation).\n\nApps:\n",
    );
    for (index, input) in inputs.iter().enumerate() {
        prompt.push_str(&format!(
            "{index}. name={} | path={} | exe={} | size={}\n",
            input.name, input.path, input.exe, input.size
        ));
    }
    prompt
}

#[derive(Debug, Deserialize)]
struct RawSuggestion {
    #[serde(default)]
    i: Option<usize>,
    #[serde(default)]
    category: String,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    reason: Option<String>,
}

/// Parse the model's answer. Invalid entries (bad index/category) are dropped;
/// confidence is clamped to `0..=1`; a response without a usable entry is an
/// error so the caller can fall back to the rules.
pub fn parse_suggestions(
    text: &str,
    inputs: &[ClassifyInput],
) -> Result<Vec<Suggestion>, String> {
    let cleaned = text
        .replace("```json", "")
        .replace("```JSON", "")
        .replace("```", "");
    let start = cleaned
        .find('[')
        .ok_or_else(|| "AI 响应中没有 JSON 数组".to_string())?;
    let end = cleaned
        .rfind(']')
        .ok_or_else(|| "AI 响应中没有 JSON 数组".to_string())?;
    if end < start {
        return Err("AI 响应中没有 JSON 数组".to_string());
    }
    let raw: Vec<RawSuggestion> = serde_json::from_str(&cleaned[start..=end])
        .map_err(|e| format!("AI 分类结果解析失败：{e}"))?;

    let mut seen: HashSet<usize> = HashSet::new();
    let mut parsed: Vec<(usize, Suggestion)> = Vec::new();
    for item in raw {
        let Some(index) = item.i else {
            continue;
        };
        if index >= inputs.len() || !seen.insert(index) {
            continue;
        }
        let category = item.category.trim();
        if !is_valid_category(category) {
            continue;
        }
        let confidence = match item.confidence {
            Some(value) if value.is_finite() => value.clamp(0.0, 1.0),
            _ => 0.5,
        };
        parsed.push((
            index,
            Suggestion {
                id: inputs[index].id.clone(),
                category: category.to_string(),
                confidence,
                reason: item.reason.unwrap_or_default(),
                source: "ai".to_string(),
            },
        ));
    }
    if parsed.is_empty() {
        return Err("AI 未返回可用的分类结果".to_string());
    }
    parsed.sort_by_key(|(index, _)| *index);
    Ok(parsed.into_iter().map(|(_, suggestion)| suggestion).collect())
}

/// Classify with the configured provider, falling back to the rules per batch.
pub fn classify_with_ai(
    root: &Path,
    cfg: &ai::AiConfig,
    inputs: &[ClassifyInput],
) -> Vec<Suggestion> {
    let engine = engine_label(cfg);
    classify_with_ai_with(root, &engine, inputs, |prompt| ai::complete(cfg, prompt))
}

/// Testable core: `complete` stands in for the provider call.
pub fn classify_with_ai_with<F: Fn(&str) -> Result<String, String>>(
    root: &Path,
    engine: &str,
    inputs: &[ClassifyInput],
    complete: F,
) -> Vec<Suggestion> {
    if inputs.is_empty() {
        return Vec::new();
    }
    let mut cache = load_cache(root);
    let mut results: Vec<Option<Suggestion>> = vec![None; inputs.len()];
    let mut cache_dirty = false;

    for batch_start in (0..inputs.len()).step_by(BATCH_SIZE) {
        let batch_end = (batch_start + BATCH_SIZE).min(inputs.len());
        let mut pending: Vec<usize> = Vec::new();
        for index in batch_start..batch_end {
            let key = cache_key(engine, &inputs[index]);
            if let Some(entry) = cache.entries.get(&key) {
                if is_valid_category(&entry.category) {
                    results[index] = Some(Suggestion {
                        id: inputs[index].id.clone(),
                        category: entry.category.clone(),
                        confidence: entry.confidence.clamp(0.0, 1.0),
                        reason: entry.reason.clone(),
                        source: "ai".to_string(),
                    });
                    continue;
                }
            }
            pending.push(index);
        }
        if pending.is_empty() {
            continue;
        }

        let pending_inputs: Vec<ClassifyInput> = pending
            .iter()
            .map(|&index| inputs[index].clone())
            .collect();
        let prompt = build_prompt(&pending_inputs);
        let outcome = complete(&prompt).and_then(|text| parse_suggestions(&text, &pending_inputs));

        match outcome {
            Ok(suggestions) => {
                let mut by_id: HashMap<&str, usize> = pending_inputs
                    .iter()
                    .enumerate()
                    .map(|(local, input)| (input.id.as_str(), local))
                    .collect();
                for suggestion in suggestions {
                    if let Some(local) = by_id.remove(suggestion.id.as_str()) {
                        let global = pending[local];
                        cache.entries.insert(
                            cache_key(engine, &inputs[global]),
                            CachedEntry {
                                category: suggestion.category.clone(),
                                confidence: suggestion.confidence,
                                reason: suggestion.reason.clone(),
                            },
                        );
                        cache_dirty = true;
                        results[global] = Some(suggestion);
                    }
                }
                // Apps the model omitted still get a suggestion (rules).
                for &global in &pending {
                    if results[global].is_none() {
                        results[global] = Some(classify_rules(&inputs[global]));
                    }
                }
            }
            Err(_) => {
                // One failed batch never blocks the others.
                for &global in &pending {
                    results[global] = Some(classify_rules(&inputs[global]));
                }
            }
        }
    }

    if cache_dirty {
        prune_cache(&mut cache);
        let _ = save_cache(root, &cache);
    }

    results
        .into_iter()
        .enumerate()
        .map(|(index, suggestion)| suggestion.unwrap_or_else(|| classify_rules(&inputs[index])))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(name: &str, path: &str) -> ClassifyInput {
        ClassifyInput {
            id: "id-sample".to_string(),
            name: name.to_string(),
            path: path.to_string(),
            exe: String::new(),
            size: 0,
        }
    }

    fn input_with_exe(name: &str, path: &str, exe: &str) -> ClassifyInput {
        ClassifyInput {
            exe: exe.to_string(),
            ..input(name, path)
        }
    }

    #[test]
    fn categories_are_fixed_and_ordered() {
        assert_eq!(CATEGORIES.len(), 11);
        assert_eq!(CATEGORIES[0], "system");
        assert_eq!(CATEGORIES[10], "other");
        assert!(is_valid_category("dev"));
        assert!(!is_valid_category("unknown"));
        assert!(!is_valid_category(""));
    }

    #[test]
    fn classifies_by_name_in_english_and_chinese() {
        let cases = [
            ("ProxyTool", "Apps/ProxyTool/ProxyTool.exe", "network"),
            ("驱动精灵", "Apps/DriverGenius/DriverGenius.exe", "system"),
            ("Visual Studio Code", "Apps/VSCode/Code.exe", "dev"),
            ("Steam", "Apps/Steam/Steam.exe", "game"),
            ("OBS Studio", "Apps/OBS/OBS.exe", "media"),
            ("7-Zip", "Apps/7-Zip/7zFM.exe", "storage"),
            ("WPS Office", "Apps/WPS/wps.exe", "document"),
            ("密码管理", "Apps/Vault/vault.exe", "security"),
            ("微信", "Apps/WeChat/WeChat.exe", "comm"),
            ("Java Runtime", "Apps/Java/javaw.exe", "runtime"),
        ];
        for (name, path, expected) in cases {
            let suggestion = classify_rules(&input(name, path));
            assert_eq!(suggestion.category, expected, "name={name}");
            assert_eq!(suggestion.confidence, 0.9, "name={name}");
            assert_eq!(suggestion.source, "rules");
            assert!(!suggestion.reason.is_empty());
            assert_eq!(suggestion.id, "id-sample");
        }
    }

    #[test]
    fn classifies_by_executable_name() {
        let suggestion = classify_rules(&input_with_exe("Unknown Tool", "Apps/X", "OBS64.exe"));
        assert_eq!(suggestion.category, "media");
        assert_eq!(suggestion.confidence, 0.9);
    }

    #[test]
    fn path_only_match_is_lower_confidence() {
        let suggestion = classify_rules(&input("RetroArch", "Apps/游戏/RetroArch/retroarch.exe"));
        // The name/path root already hits the game keyword, still strong here.
        assert_eq!(suggestion.category, "game");

        let weak = classify_rules(&input("ToolX", "Apps/games/ToolX/tool.exe"));
        assert_eq!(weak.category, "game");
        assert_eq!(weak.confidence, 0.75);
        assert!(weak.reason.contains("路径"), "{}", weak.reason);
    }

    #[test]
    fn name_signal_beats_path_signal() {
        // "微信" is a strong comm match, the path only suggests network.
        let suggestion = classify_rules(&input("微信", "Apps/network-tools/WeChat/WeChat.exe"));
        assert_eq!(suggestion.category, "comm");
        assert_eq!(suggestion.confidence, 0.9);
    }

    #[test]
    fn ties_follow_stable_category_order() {
        // Matches a system keyword first in CATEGORIES order and a dev keyword.
        let suggestion = classify_rules(&input("cleaner-code", "Apps/cleaner-code/cleaner-code.exe"));
        assert_eq!(suggestion.category, "system");
    }

    #[test]
    fn unmatched_app_falls_back_to_other() {
        let suggestion = classify_rules(&input("Zzz", "Apps/Zzz/Zzz.exe"));
        assert_eq!(suggestion.category, "other");
        assert_eq!(suggestion.confidence, 0.3);
        assert!(suggestion.reason.contains("未匹配"), "{}", suggestion.reason);
    }

    #[test]
    fn reason_mentions_the_matched_keyword() {
        let suggestion = classify_rules(&input("ProxyTool", "Apps/ProxyTool/ProxyTool.exe"));
        assert!(suggestion.reason.contains("proxy"), "{}", suggestion.reason);
    }

    // --- AI classification (REQ-0040) -----------------------------------------

    fn ai_tempdir(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("ph-classify-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn ai_input(index: usize, name: &str) -> ClassifyInput {
        ClassifyInput {
            id: format!("id-{index}"),
            name: name.to_string(),
            path: format!("Apps/{name}/{name}.exe"),
            exe: format!("Apps/{name}/{name}.exe"),
            size: 1024,
        }
    }

    #[test]
    fn prompt_lists_categories_and_every_app() {
        let inputs = vec![ai_input(0, "Alpha"), ai_input(1, "Beta")];
        let prompt = build_prompt(&inputs);
        for category in CATEGORIES {
            assert!(prompt.contains(category), "missing category {category}");
        }
        assert!(prompt.contains("0. name=Alpha"), "{prompt}");
        assert!(prompt.contains("1. name=Beta"), "{prompt}");
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn parses_ai_suggestions_and_clamps_confidence() {
        let inputs = vec![ai_input(0, "Alpha"), ai_input(1, "Beta")];
        let text = r#"[{"i":1,"category":"media","confidence":0.8,"reason":"player"},{"i":0,"category":"game","confidence":1.7,"reason":"game"}]"#;
        let parsed = parse_suggestions(text, &inputs).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id, "id-0");
        assert_eq!(parsed[0].category, "game");
        assert_eq!(parsed[0].confidence, 1.0, "confidence must be clamped");
        assert_eq!(parsed[0].source, "ai");
        assert_eq!(parsed[1].id, "id-1");
        assert_eq!(parsed[1].category, "media");

        let missing_confidence =
            parse_suggestions(r#"[{"i":0,"category":"dev"}]"#, &inputs).unwrap();
        assert_eq!(missing_confidence[0].confidence, 0.5);
    }

    #[test]
    fn parses_fenced_json_and_ignores_invalid_entries() {
        let inputs = vec![ai_input(0, "Alpha"), ai_input(1, "Beta")];
        let text = "```json\n[{\"i\":0,\"category\":\"dev\",\"confidence\":0.4,\"reason\":\"code\"},{\"i\":5,\"category\":\"dev\"},{\"i\":1,\"category\":\"nope\"}]\n```";
        let parsed = parse_suggestions(text, &inputs).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, "id-0");
        assert_eq!(parsed[0].confidence, 0.4);
    }

    #[test]
    fn rejects_non_array_or_unusable_responses() {
        let inputs = vec![ai_input(0, "Alpha")];
        assert!(parse_suggestions("no json here", &inputs).is_err());
        assert!(parse_suggestions("{\"i\":0}", &inputs).is_err());
        assert!(parse_suggestions("[{\"i\":0,\"category\":\"unknown\"}]", &inputs).is_err());
        assert!(parse_suggestions("[]", &inputs).is_err());
        assert!(parse_suggestions("[{\"i\":0,", &inputs).is_err());
    }

    #[test]
    fn cache_roundtrip_and_hit_avoid_second_call() {
        let root = ai_tempdir("cache");
        let inputs = vec![ai_input(0, "Alpha")];
        let calls = std::cell::Cell::new(0usize);
        let first = classify_with_ai_with(&root, "engine-a", &inputs, |_| {
            calls.set(calls.get() + 1);
            Ok(r#"[{"i":0,"category":"dev","confidence":0.9,"reason":"mock"}]"#.to_string())
        });
        assert_eq!(first[0].category, "dev");
        assert_eq!(first[0].source, "ai");
        assert!(cache_path(&root).is_file(), "cache file must be written");

        let second = classify_with_ai_with(&root, "engine-a", &inputs, |_| {
            calls.set(calls.get() + 1);
            Err("must not be called".to_string())
        });
        assert_eq!(second[0].category, "dev");
        assert_eq!(second[0].source, "ai");
        assert_eq!(calls.get(), 1, "second run must hit the cache");
    }

    #[test]
    fn failed_batch_falls_back_to_rules() {
        let root = ai_tempdir("fallback");
        let inputs = vec![ai_input(0, "ProxyTool")];
        let results = classify_with_ai_with(&root, "engine-fail", &inputs, |_| {
            Err("network down".to_string())
        });
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].category, "network");
        assert_eq!(results[0].source, "rules");
    }

    #[test]
    fn batch_size_limits_requests_to_twenty_items() {
        let root = ai_tempdir("batch");
        let inputs: Vec<ClassifyInput> = (0..45)
            .map(|index| ai_input(index, &format!("App{index}")))
            .collect();
        let calls = std::cell::Cell::new(0usize);
        let prompts = std::cell::RefCell::new(Vec::<String>::new());
        let results = classify_with_ai_with(&root, "engine-batch", &inputs, |prompt| {
            calls.set(calls.get() + 1);
            prompts.borrow_mut().push(prompt.to_string());
            let response: Vec<serde_json::Value> = (0..20)
                .map(|index| {
                    serde_json::json!({
                        "i": index,
                        "category": "dev",
                        "confidence": 0.8,
                        "reason": "mock"
                    })
                })
                .collect();
            Ok(serde_json::to_string(&response).unwrap())
        });
        assert_eq!(calls.get(), 3, "45 apps must take three batches");
        assert_eq!(results.len(), 45);
        assert!(results.iter().all(|suggestion| suggestion.source == "ai"));
        let prompts = prompts.borrow();
        assert!(prompts[2].contains("App40"), "last batch holds the tail");
        assert!(
            !prompts[0].contains("App20"),
            "first batch must not exceed 20 apps"
        );
    }

    #[test]
    fn cache_key_changes_with_size() {
        let root = ai_tempdir("cache-size");
        let first = ai_input(0, "Alpha");
        let mut second = ai_input(1, "Alpha");
        second.size = 2048;
        let calls = std::cell::Cell::new(0usize);
        let run = classify_with_ai_with(
            &root,
            "engine-size",
            &[first.clone(), second.clone()],
            |_| {
                calls.set(calls.get() + 1);
                Ok(r#"[{"i":0,"category":"dev","confidence":0.9,"reason":"a"},{"i":1,"category":"media","confidence":0.8,"reason":"b"}]"#.to_string())
            },
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(run[0].category, "dev");
        assert_eq!(run[1].category, "media");

        // Identical inputs: every entry hits the cache, no provider call.
        let cached = classify_with_ai_with(
            &root,
            "engine-size",
            &[first.clone(), second.clone()],
            |_| {
                calls.set(calls.get() + 1);
                Err("must not be called".to_string())
            },
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(cached[0].category, "dev");
        assert_eq!(cached[1].category, "media");

        // Changing the size invalidates only that cache entry.
        let mut resized = second;
        resized.size = 4096;
        let after = classify_with_ai_with(&root, "engine-size", &[first, resized], |_| {
            calls.set(calls.get() + 1);
            Ok(r#"[{"i":0,"category":"game","confidence":0.9,"reason":"c"}]"#.to_string())
        });
        assert_eq!(calls.get(), 2, "only the resized app misses the cache");
        assert_eq!(after[0].category, "dev");
        assert_eq!(after[1].category, "game");
    }

    #[test]
    fn partial_ai_answer_falls_back_for_missing_entries() {
        let root = ai_tempdir("partial");
        let inputs = vec![ai_input(0, "Alpha"), ai_input(1, "Beta")];
        let results = classify_with_ai_with(&root, "engine-partial", &inputs, |_| {
            Ok(r#"[{"i":0,"category":"dev","confidence":0.9,"reason":"mock"}]"#.to_string())
        });
        assert_eq!(results[0].source, "ai");
        assert_eq!(results[0].category, "dev");
        assert_eq!(results[1].source, "rules");
    }

    #[test]
    fn engine_label_contains_provider_model_and_version() {
        let mut cfg = ai::AiConfig::default();
        cfg.provider = "claude-cli".to_string();
        cfg.cli_version = "2.1.296 (Claude Code)".to_string();
        let label = engine_label(&cfg);
        assert!(label.contains("claude-cli"));
        assert!(label.contains("2.1.296"));
    }
}
