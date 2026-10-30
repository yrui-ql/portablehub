//! AI provider configuration, CLI binding and connectivity testing
//! (REQ-0035 / REQ-0038).
//!
//! Only the backend knows the raw API key: it is stored in
//! `<root>/_private/ai.json` and every view returned to the frontend carries a
//! masked hint instead. Besides remote APIs (`anthropic` / `openai`) the app can
//! bind a locally installed CLI (`claude-cli` / `codex-cli`) and route smart
//! features through it. CLI processes run hidden, with timeouts, and their
//! stderr is sanitized before it becomes an error message.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PRIVATE_DIR: &str = "_private";
const CONFIG_NAME: &str = "ai.json";
const COMPLETE_TIMEOUT_SECS: u64 = 60;
const COMPLETE_MAX_TOKENS: u64 = 2048;
const ANTHROPIC_VERSION: &str = "2023-06-01";
const CLI_VERSION_TIMEOUT_SECS: u64 = 10;
const CLI_PROMPT_TIMEOUT_SECS: u64 = 120;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const PROVIDER_ANTHROPIC: &str = "anthropic";
const PROVIDER_OPENAI: &str = "openai";
const PROVIDER_CLAUDE_CLI: &str = "claude-cli";
const PROVIDER_CODEX_CLI: &str = "codex-cli";

/// Shims looked up in `PATH`, per provider, in preference order (native exe
/// first so spawning never goes through `cmd.exe` quoting).
const CLI_SPECS: [(&str, [&str; 3]); 2] = [
    (
        PROVIDER_CLAUDE_CLI,
        ["claude.exe", "claude.cmd", "claude.ps1"],
    ),
    (PROVIDER_CODEX_CLI, ["codex.exe", "codex.cmd", "codex.ps1"]),
];

/// Stored configuration. `#[serde(default)]` keeps older/partial files usable.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AiConfig {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub consent: bool,
    /// Bound local CLI executable (empty for API providers).
    pub cli_path: String,
    /// Version reported by the bound CLI; display only.
    pub cli_version: String,
}

/// Frontend-facing configuration: the raw key never leaves the backend.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiConfigView {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub has_key: bool,
    pub key_hint: String,
    pub consent: bool,
    pub cli_path: String,
    pub cli_version: String,
}

/// A local CLI found on this machine.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiCliCandidate {
    pub provider: String,
    pub path: String,
    pub version: String,
    /// `"PATH"` for the discovered shim, `"native"` for an underlying exe.
    pub source: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiTestResult {
    pub ok: bool,
    pub message: String,
}

/// Payload for `ai_save_config`; the key is optional so the UI can leave it
/// untouched without ever having received it.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AiSavePayload {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub clear_key: Option<bool>,
    pub cli_path: Option<String>,
}

impl AiConfigView {
    pub fn from_config(cfg: &AiConfig) -> Self {
        Self {
            provider: cfg.provider.clone(),
            base_url: cfg.base_url.clone(),
            model: cfg.model.clone(),
            has_key: !cfg.api_key.trim().is_empty(),
            key_hint: mask_key(&cfg.api_key),
            consent: cfg.consent,
            cli_path: cfg.cli_path.clone(),
            cli_version: cfg.cli_version.clone(),
        }
    }
}

pub fn is_cli_provider(provider: &str) -> bool {
    matches!(provider.trim(), PROVIDER_CLAUDE_CLI | PROVIDER_CODEX_CLI)
}

pub fn is_api_provider(provider: &str) -> bool {
    matches!(provider.trim(), PROVIDER_ANTHROPIC | PROVIDER_OPENAI)
}

pub fn config_path(root: &Path) -> PathBuf {
    root.join(PRIVATE_DIR).join(CONFIG_NAME)
}

/// Missing or corrupt files fall back to defaults; the user's file is left
/// untouched (`save` is the only writer).
pub fn load(root: &Path) -> AiConfig {
    let path = config_path(root);
    let Ok(text) = fs::read_to_string(&path) else {
        return AiConfig::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// Atomic write: `_private/ai.json.tmp` → rename over the real file.
pub fn save(root: &Path, cfg: &AiConfig) -> Result<(), String> {
    let dir = root.join(PRIVATE_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("创建 AI 配置目录失败：{e}"))?;
    let path = dir.join(CONFIG_NAME);
    let tmp = dir.join("ai.json.tmp");
    let text = serde_json::to_string_pretty(cfg).map_err(|e| format!("序列化 AI 配置失败：{e}"))?;
    fs::write(&tmp, text).map_err(|e| format!("写入 AI 配置失败：{e}"))?;
    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("替换 AI 配置失败：{e}")
    })
}

pub fn validate(cfg: &AiConfig) -> Result<(), String> {
    let provider = cfg.provider.trim();
    if provider.is_empty() {
        return Err("请选择 AI 服务商".to_string());
    }
    if is_cli_provider(provider) {
        let path = cfg.cli_path.trim();
        if path.is_empty() {
            return Err("请先绑定本机 CLI".to_string());
        }
        let path = Path::new(path);
        if !path.is_file() {
            return Err(format!("CLI 路径不存在：{}", path.display()));
        }
        if !provider_matches_file(provider, path) {
            return Err("CLI 文件与所选服务商不匹配（claude / codex）".to_string());
        }
        return Ok(());
    }
    if !is_api_provider(provider) {
        return Err(format!("不支持的 AI 服务商：{provider}"));
    }
    if cfg.base_url.trim().is_empty() {
        return Err("请填写 API 地址".to_string());
    }
    if cfg.model.trim().is_empty() {
        return Err("请填写模型名称".to_string());
    }
    Ok(())
}

/// Masked hint shown in the UI (`sk-…abcd`); empty when no key is stored.
pub fn mask_key(key: &str) -> String {
    let key = key.trim();
    if key.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        return "****".to_string();
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

/// Provider-specific completion endpoint derived from the user's base URL.
pub fn endpoint(provider: &str, base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    match provider.trim() {
        PROVIDER_ANTHROPIC => {
            if base.ends_with("/v1") {
                format!("{base}/messages")
            } else {
                format!("{base}/v1/messages")
            }
        }
        _ => {
            if base.ends_with("/v1") {
                format!("{base}/chat/completions")
            } else {
                format!("{base}/v1/chat/completions")
            }
        }
    }
}

/// Pull the assistant text out of a provider response body.
pub fn extract_text(provider: &str, body: &serde_json::Value) -> Result<String, String> {
    let text = if provider.trim() == PROVIDER_ANTHROPIC {
        body.pointer("/content/0/text")
    } else {
        body.pointer("/choices/0/message/content")
    };
    match text.and_then(|value| value.as_str()) {
        Some(text) if !text.trim().is_empty() => Ok(text.trim().to_string()),
        Some(_) => Err("AI 返回了空回复".to_string()),
        None => Err("AI 响应格式不符合预期".to_string()),
    }
}

/// Send one minimal request to verify the configured provider.
///
/// API calls only happen when a key is configured and CLI calls only when a
/// provider is bound, so unit tests can run this without touching the network
/// or starting a process.
pub fn test_connection(cfg: &AiConfig) -> AiTestResult {
    if let Err(message) = validate(cfg) {
        return AiTestResult { ok: false, message };
    }
    if is_cli_provider(&cfg.provider) {
        return test_cli_connection(cfg);
    }
    match complete(cfg, PONG_PROMPT) {
        Ok(text) => AiTestResult {
            ok: true,
            message: format!(
                "连接成功：模型 {} 回复「{}」",
                cfg.model.trim(),
                summarize(&text)
            ),
        },
        Err(message) => AiTestResult { ok: false, message },
    }
}

/// Prompt used by `test_connection`: a single word keeps the test cheap.
const PONG_PROMPT: &str = "Reply with exactly one word: pong";

/// Full completion used by smart features (classification and later tasks).
///
/// API providers send one JSON request (max 2048 tokens, 60s timeout); CLI
/// providers reuse [`run_cli`]. Errors never contain the API key or headers.
pub fn complete(cfg: &AiConfig, prompt: &str) -> Result<String, String> {
    validate(cfg)?;
    if is_cli_provider(&cfg.provider) {
        return run_cli(cfg, prompt);
    }
    api_complete(cfg, prompt)
}

fn api_complete(cfg: &AiConfig, prompt: &str) -> Result<String, String> {
    let key = cfg.api_key.trim();
    if key.is_empty() {
        return Err("未配置 API Key".to_string());
    }
    let provider = cfg.provider.trim();
    let model = cfg.model.trim();
    let url = endpoint(provider, &cfg.base_url);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(COMPLETE_TIMEOUT_SECS))
        .build()
        .map_err(|e| redact(&format!("初始化网络客户端失败：{e}"), key))?;
    let request = if provider == PROVIDER_ANTHROPIC {
        client
            .post(&url)
            .header("x-api-key", key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&serde_json::json!({
                "model": model,
                "max_tokens": COMPLETE_MAX_TOKENS,
                "messages": [{ "role": "user", "content": prompt }],
            }))
    } else {
        client
            .post(&url)
            .bearer_auth(key)
            .json(&serde_json::json!({
                "model": model,
                "max_tokens": COMPLETE_MAX_TOKENS,
                "messages": [{ "role": "user", "content": prompt }],
            }))
    };
    let response = request.send().map_err(|e| {
        let message = if e.is_timeout() {
            format!("请求超时（{COMPLETE_TIMEOUT_SECS} 秒），请检查网络或 API 地址")
        } else if e.is_connect() {
            "无法连接到服务器，请检查网络或 API 地址".to_string()
        } else {
            format!("网络请求失败：{e}")
        };
        redact(&message, key)
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(status_message(status.as_u16()));
    }
    let body = response.text().unwrap_or_default();
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| "AI 服务返回了无效 JSON".to_string())?;
    extract_text(provider, &value).map_err(|message| redact(&message, key))
}

/// True when the configured provider can actually serve a request.
pub fn available(cfg: &AiConfig) -> bool {
    let provider = cfg.provider.trim();
    if is_cli_provider(provider) {
        let path = Path::new(cfg.cli_path.trim());
        return !cfg.cli_path.trim().is_empty()
            && path.is_file()
            && provider_matches_file(provider, path);
    }
    if is_api_provider(provider) {
        return !cfg.base_url.trim().is_empty()
            && !cfg.model.trim().is_empty()
            && !cfg.api_key.trim().is_empty();
    }
    false
}

fn test_cli_connection(cfg: &AiConfig) -> AiTestResult {
    match run_cli(cfg, "Reply with exactly: pong") {
        Ok(text) => {
            let label = cli_label(cfg.provider.trim());
            let version = cfg.cli_version.trim();
            // Version lines often repeat the product name (`2.1.296 (Claude Code)`).
            let who = if version.is_empty() {
                label.to_string()
            } else if version.to_lowercase().contains(&label.to_lowercase()) {
                version.to_string()
            } else {
                format!("{label} {version}")
            };
            AiTestResult {
                ok: true,
                message: format!("CLI 连接成功：{who} 回复「{}」", summarize(&text)),
            }
        }
        Err(message) => AiTestResult {
            ok: false,
            message: redact(&message, cfg.api_key.trim()),
        },
    }
}

// ---------------------------------------------------------------------------
// Local CLI discovery / binding
// ---------------------------------------------------------------------------

/// Find `claude` / `codex` shims on `PATH` and report their versions.
///
/// A generated npm shim is resolved to the native exe it wraps when the npm
/// layout is recognisable, so later invocations avoid `cmd.exe` argument
/// quoting. Custom shims are kept as candidates unchanged because they may
/// inject credentials/environment.
pub fn detect_clis() -> Vec<AiCliCandidate> {
    let Some(path_var) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    let dirs: Vec<PathBuf> = std::env::split_paths(&path_var).collect();
    detect_in_dirs(&dirs)
}

fn detect_in_dirs(dirs: &[PathBuf]) -> Vec<AiCliCandidate> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for dir in dirs {
        for (provider, file_names) in CLI_SPECS {
            for file_name in file_names {
                let shim = dir.join(file_name);
                if !shim.is_file() {
                    continue;
                }
                let (path, source) = resolve_cli_path(&shim);
                let key = path.to_string_lossy().replace('\\', "/").to_lowercase();
                if !seen.insert(key) {
                    continue;
                }
                out.push(AiCliCandidate {
                    provider: provider.to_string(),
                    path: path.to_string_lossy().to_string(),
                    version: cli_version(&path),
                    source: source.to_string(),
                });
            }
        }
    }
    out
}

/// Bind a local CLI: validate provider/file name, resolve the native exe, read
/// its version and persist `provider/cli_path/cli_version`. The API key is left
/// untouched.
pub fn bind_cli(root: &Path, provider: &str, path: &str) -> Result<AiConfigView, String> {
    let provider = provider.trim();
    if !is_cli_provider(provider) {
        return Err("只能绑定 claude-cli 或 codex-cli".to_string());
    }
    let resolved = resolve_bound_cli(provider, path)?;
    let version = cli_version(&resolved);
    let mut cfg = load(root);
    cfg.provider = provider.to_string();
    cfg.cli_path = resolved.to_string_lossy().to_string();
    cfg.cli_version = version;
    save(root, &cfg)?;
    Ok(AiConfigView::from_config(&cfg))
}

/// Validation half of [`bind_cli`] (no process is started).
fn resolve_bound_cli(provider: &str, path: &str) -> Result<PathBuf, String> {
    let raw = path.trim();
    if raw.is_empty() {
        return Err("请选择本机 CLI 路径".to_string());
    }
    let raw = Path::new(raw);
    if !raw.is_file() {
        return Err(format!("CLI 路径不存在：{}", raw.display()));
    }
    if !provider_matches_file(provider, raw) {
        return Err("CLI 文件与所选服务商不匹配（claude / codex）".to_string());
    }
    Ok(resolve_cli_path(raw).0)
}

/// One prepared CLI call: arguments plus an optional stdin prompt.
struct CliInvocation {
    args: Vec<String>,
    /// `claude-cli` gets the prompt on stdin so `cmd.exe` never parses it.
    stdin: Option<String>,
}

/// Run a prompt through the bound CLI and return its cleaned stdout.
///
/// This is the single entry point later smart features reuse; it is not exposed
/// as a Tauri command on purpose (the frontend gets task-specific commands).
pub(crate) fn run_cli(cfg: &AiConfig, prompt: &str) -> Result<String, String> {
    validate(cfg)?;
    let provider = cfg.provider.trim();
    let cli = Path::new(cfg.cli_path.trim());
    let invocation = cli_args(provider, prompt, cfg.model.trim())?;
    let mut command = Command::new(cli);
    command.args(&invocation.args);
    let output = run_command(
        command,
        Duration::from_secs(CLI_PROMPT_TIMEOUT_SECS),
        invocation.stdin.as_deref(),
    )?;
    if !output.success {
        let code = output.code.map(|c| c.to_string()).unwrap_or_else(|| "?".to_string());
        let detail = sanitize_error_text(&output.stderr);
        return if detail.is_empty() {
            Err(format!("CLI 执行失败（退出码 {code}）"))
        } else {
            Err(format!("CLI 执行失败（退出码 {code}）：{detail}"))
        };
    }
    let text = output.stdout.trim().to_string();
    if text.is_empty() {
        return Err("CLI 没有返回内容".to_string());
    }
    Ok(text)
}

/// Argument vector per provider (pure; unit-tested without spawning).
///
/// `claude-cli` receives the prompt on stdin (`-p` alone reads stdin), which
/// keeps long/quoted prompts away from `cmd.exe` argument parsing. `codex`
/// keeps the prompt in argv because its `exec` mode expects a positional prompt.
fn cli_args(provider: &str, prompt: &str, model: &str) -> Result<CliInvocation, String> {
    let model = model.trim();
    let mut args: Vec<String> = Vec::new();
    match provider.trim() {
        PROVIDER_CLAUDE_CLI => {
            args.push("-p".to_string());
            args.push("--output-format".to_string());
            args.push("text".to_string());
            args.push("--max-turns".to_string());
            args.push("1".to_string());
            if !model.is_empty() {
                args.push("--model".to_string());
                args.push(model.to_string());
            }
            Ok(CliInvocation {
                args,
                stdin: Some(prompt.to_string()),
            })
        }
        PROVIDER_CODEX_CLI => {
            args.push("exec".to_string());
            args.push("--skip-git-repo-check".to_string());
            if !model.is_empty() {
                args.push("--model".to_string());
                args.push(model.to_string());
            }
            args.push(prompt.to_string());
            Ok(CliInvocation { args, stdin: None })
        }
        _ => Err("不支持的本机 CLI 服务商".to_string()),
    }
}

fn cli_label(provider: &str) -> &'static str {
    match provider {
        PROVIDER_CLAUDE_CLI => "Claude Code",
        PROVIDER_CODEX_CLI => "Codex",
        _ => "CLI",
    }
}

/// `"claude"` / `"codex"` for a file name, else `None`.
fn cli_file_kind(path: &Path) -> Option<&'static str> {
    let stem = path.file_stem()?.to_string_lossy().to_lowercase();
    if stem.starts_with("claude") {
        Some("claude")
    } else if stem.starts_with("codex") {
        Some("codex")
    } else {
        None
    }
}

fn provider_matches_file(provider: &str, path: &Path) -> bool {
    let expected = match provider.trim() {
        PROVIDER_CLAUDE_CLI => "claude",
        PROVIDER_CODEX_CLI => "codex",
        _ => return false,
    };
    cli_file_kind(path) == Some(expected)
}

/// Resolve a shim to the native exe when it is a generated npm shim.
///
/// Custom shims are kept as-is: they may inject credentials or environment
/// (a real-world workstation wrapper does exactly that), so replacing them with
/// the bare executable would break authentication. Npm-generated shims only
/// forward to a local script, which is what "prefer native" is meant to avoid.
fn resolve_cli_path(shim: &Path) -> (PathBuf, &'static str) {
    let is_exe = shim
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("exe"))
        .unwrap_or(false);
    if is_exe {
        return (shim.to_path_buf(), "PATH");
    }
    if is_generated_npm_shim(shim) {
        if let Some(native) = native_exe_path(shim) {
            return (native, "native");
        }
    }
    (shim.to_path_buf(), "PATH")
}

/// True when the file looks like a package-manager generated wrapper that only
/// forwards to a sibling/nested node script (npm/pnpm `.cmd` / `.ps1` shims).
fn is_generated_npm_shim(shim: &Path) -> bool {
    let Ok(text) = fs::read_to_string(shim) else {
        return false;
    };
    if text.len() > 16 * 1024 {
        return false;
    }
    let lower = text.to_lowercase();
    if shim
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("cmd"))
        .unwrap_or(false)
    {
        (lower.contains("%dp0%\\node") || lower.contains("%~dp0\\node"))
            && lower.contains("node_modules")
    } else if shim
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("ps1"))
        .unwrap_or(false)
    {
        lower.contains("$basedir") && lower.contains("node_modules")
    } else {
        false
    }
}

/// Npm-style layout: a shim in `bin/` next to `node_modules/<pkg>/bin/<exe>`.
fn native_exe_path(shim: &Path) -> Option<PathBuf> {
    let kind = cli_file_kind(shim)?;
    let dir = shim.parent()?;
    let exe_name = format!("{kind}.exe");
    let same_dir = dir.join(&exe_name);
    if same_dir.is_file() {
        return Some(same_dir);
    }
    let relative = npm_relative_path(kind)?;
    let mut bases: Vec<&Path> = vec![dir];
    if let Some(parent) = dir.parent() {
        bases.push(parent);
    }
    for base in bases {
        let candidate = base.join(&relative);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn npm_relative_path(kind: &str) -> Option<PathBuf> {
    match kind {
        "claude" => Some(
            Path::new("node_modules")
                .join("@anthropic-ai")
                .join("claude-code")
                .join("bin")
                .join("claude.exe"),
        ),
        "codex" => Some(
            Path::new("node_modules")
                .join("@openai")
                .join("codex")
                .join("bin")
                .join("codex.exe"),
        ),
        _ => None,
    }
}

/// Run `--version`; any failure yields an empty version (the candidate stays
/// listed, the UI can still bind it).
fn cli_version(path: &Path) -> String {
    let mut command = Command::new(path);
    command.arg("--version");
    match run_command(
        command,
        Duration::from_secs(CLI_VERSION_TIMEOUT_SECS),
        None,
    ) {
        Ok(output) if output.success => parse_version_line(&output.stdout)
            .or_else(|| parse_version_line(&output.stderr))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// First non-empty line, ANSI cleaned and length-capped.
fn parse_version_line(text: &str) -> Option<String> {
    let cleaned = strip_ansi(text);
    let line = cleaned.lines().map(str::trim).find(|line| !line.is_empty())?;
    let mut line: String = line.chars().take(120).collect();
    if line.chars().count() >= 120 {
        line.push('…');
    }
    Some(line)
}

// ---------------------------------------------------------------------------
// Process helpers
// ---------------------------------------------------------------------------

struct CliOutput {
    success: bool,
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Spawn a hidden process, capture both streams and enforce a timeout.
///
/// Reader threads drain the pipes so a chatty CLI cannot block on a full pipe
/// while we wait for it to exit. When `stdin_data` is set it is written from a
/// helper thread (closing stdin afterwards so `-p` sees EOF).
fn run_command(
    mut command: Command,
    timeout: Duration,
    stdin_data: Option<&str>,
) -> Result<CliOutput, String> {
    command
        .stdin(if stdin_data.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().map_err(|e| format!("启动 CLI 失败：{e}"))?;
    if let Some(data) = stdin_data {
        if let Some(mut stdin) = child.stdin.take() {
            let data = data.to_string();
            std::thread::spawn(move || {
                let _ = stdin.write_all(data.as_bytes());
            });
        }
    }
    let mut stdout_reader = child.stdout.take().map(spawn_reader);
    let mut stderr_reader = child.stderr.take().map(spawn_reader);
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = join_reader(stdout_reader.take());
                let stderr = join_reader(stderr_reader.take());
                return Ok(CliOutput {
                    success: status.success(),
                    code: status.code(),
                    stdout: strip_ansi(&String::from_utf8_lossy(&stdout)),
                    stderr: strip_ansi(&String::from_utf8_lossy(&stderr)),
                });
            }
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = join_reader(stdout_reader.take());
                    let _ = join_reader(stderr_reader.take());
                    return Err(format!("CLI 执行超时（{} 秒）", timeout.as_secs()));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(format!("等待 CLI 失败：{e}"));
            }
        }
    }
}

fn spawn_reader<R: Read + Send + 'static>(mut stream: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stream.read_to_end(&mut buf);
        buf
    })
}

fn join_reader(handle: Option<std::thread::JoinHandle<Vec<u8>>>) -> Vec<u8> {
    handle.and_then(|h| h.join().ok()).unwrap_or_default()
}

/// Remove ANSI escape sequences (colours, spinner control codes).
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    while let Some(ch) = chars.next() {
                        if ('@'..='~').contains(&ch) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(ch) = chars.next() {
                        if ch == '\u{7}' {
                            break;
                        }
                        if ch == '\u{1b}' {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out
}

/// Short, sanitized stderr tail for error messages: no ANSI, no huge lines, no
/// secret-looking tokens.
fn sanitize_error_text(text: &str) -> String {
    let filtered: Vec<String> = strip_ansi(text)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && line.chars().count() <= 200)
        .map(redact_secrets)
        .collect();
    let tail: Vec<&str> = filtered
        .iter()
        .rev()
        .take(5)
        .rev()
        .map(String::as_str)
        .collect();
    let joined = tail.join(" | ");
    let mut out: String = joined.chars().take(600).collect();
    if joined.chars().count() > 600 {
        out.push('…');
    }
    out
}

/// Hide obvious API keys in diagnostic text (`sk-…`, `Bearer …`).
fn redact_secrets(line: &str) -> String {
    let line = redact_token(line, "sk-");
    redact_token(&line, "Bearer ")
}

fn redact_token(text: &str, marker: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let Some(index) = rest.find(marker) else {
            break;
        };
        // Do not treat embedded fragments ("task-force") as secrets.
        let boundary_ok = rest[..index]
            .chars()
            .last()
            .map(|c| !c.is_alphanumeric())
            .unwrap_or(true);
        out.push_str(&rest[..index]);
        out.push_str(marker);
        if boundary_ok {
            out.push_str("***");
            let after = &rest[index + marker.len()..];
            let end = after
                .find(|c: char| c.is_whitespace() || "\"',;)".contains(c))
                .unwrap_or(after.len());
            rest = &after[end..];
        } else {
            rest = &rest[index + marker.len()..];
        }
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn status_message(status: u16) -> String {
    match status {
        401 | 403 => "API Key 无效或没有访问权限（401/403）".to_string(),
        404 => "API 地址或模型不存在（404），请检查 Base URL 与模型名称".to_string(),
        429 => "请求过于频繁或额度不足（429），请稍后重试".to_string(),
        500..=599 => format!("AI 服务暂时不可用（HTTP {status}），请稍后重试"),
        _ => format!("连接失败（HTTP {status}）"),
    }
}

fn summarize(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = compact.chars().take(40).collect();
    if compact.chars().count() > 40 {
        out.push('…');
    }
    out
}

/// Never let the raw key leak through a library error message.
fn redact(message: &str, key: &str) -> String {
    if key.is_empty() {
        message.to_string()
    } else {
        message.replace(key, "***")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("ph-ai-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"placeholder").unwrap();
    }

    fn sample_config() -> AiConfig {
        AiConfig {
            provider: "openai".to_string(),
            base_url: "https://api.example.test/v1".to_string(),
            model: "sample-model".to_string(),
            api_key: "sk-sample-1234567890".to_string(),
            consent: true,
            cli_path: String::new(),
            cli_version: String::new(),
        }
    }

    fn cli_config(path: &Path) -> AiConfig {
        AiConfig {
            provider: PROVIDER_CLAUDE_CLI.to_string(),
            base_url: String::new(),
            model: String::new(),
            api_key: String::new(),
            consent: true,
            cli_path: path.to_string_lossy().to_string(),
            cli_version: "2.1.296".to_string(),
        }
    }

    #[test]
    fn endpoints_are_normalized_per_provider() {
        assert_eq!(
            endpoint("anthropic", "https://api.example.test"),
            "https://api.example.test/v1/messages"
        );
        assert_eq!(
            endpoint("anthropic", "https://api.example.test/v1/"),
            "https://api.example.test/v1/messages"
        );
        assert_eq!(
            endpoint("openai", "https://api.example.test"),
            "https://api.example.test/v1/chat/completions"
        );
        assert_eq!(
            endpoint("openai", "https://api.example.test/v1"),
            "https://api.example.test/v1/chat/completions"
        );
        assert_eq!(
            endpoint("openai", "  https://api.example.test/v1/  "),
            "https://api.example.test/v1/chat/completions"
        );
    }

    #[test]
    fn extracts_text_from_both_protocols() {
        let anthropic = serde_json::json!({ "content": [{ "type": "text", "text": "pong" }] });
        assert_eq!(extract_text("anthropic", &anthropic).unwrap(), "pong");

        let openai = serde_json::json!({
            "choices": [{ "message": { "role": "assistant", "content": "pong" } }]
        });
        assert_eq!(extract_text("openai", &openai).unwrap(), "pong");

        assert!(extract_text("anthropic", &serde_json::json!({})).is_err());
        assert!(extract_text("openai", &serde_json::json!({ "choices": [] })).is_err());
        let empty = serde_json::json!({
            "choices": [{ "message": { "content": "   " } }]
        });
        assert!(extract_text("openai", &empty).is_err());
    }

    #[test]
    fn masks_keys_without_leaking_them() {
        assert_eq!(mask_key(""), "");
        assert_eq!(mask_key("   "), "");
        assert_eq!(mask_key("short"), "****");
        assert_eq!(mask_key("12345678"), "****");
        let masked = mask_key("sk-sample-1234567890");
        assert_eq!(masked, "sk-s…7890");
        assert!(!masked.contains("ample-1234"));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let root = tempdir("roundtrip");
        let cfg = sample_config();
        save(&root, &cfg).unwrap();

        assert!(config_path(&root).is_file(), "config path must be _private/ai.json");
        assert_eq!(load(&root), cfg);
    }

    #[test]
    fn save_and_load_roundtrip_with_cli_fields() {
        let root = tempdir("roundtrip-cli");
        let mut cfg = sample_config();
        cfg.provider = PROVIDER_CODEX_CLI.to_string();
        cfg.cli_path = "Root/Tools/codex.exe".to_string();
        cfg.cli_version = "0.9.0".to_string();
        save(&root, &cfg).unwrap();
        assert_eq!(load(&root), cfg);
    }

    #[test]
    fn corrupt_config_falls_back_without_overwriting() {
        let root = tempdir("corrupt");
        let path = config_path(&root);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json").unwrap();

        assert_eq!(load(&root), AiConfig::default());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "{ not json",
            "load must never rewrite the user's file"
        );
    }

    #[test]
    fn missing_config_is_default() {
        let root = tempdir("missing");
        assert_eq!(load(&root), AiConfig::default());
        assert!(!config_path(&root).exists());
    }

    #[test]
    fn view_masks_the_key_and_exposes_cli_fields() {
        let view = AiConfigView::from_config(&sample_config());
        assert!(view.has_key);
        assert_eq!(view.key_hint, "sk-s…7890");
        assert_eq!(view.provider, "openai");
        assert_eq!(view.model, "sample-model");
        assert!(view.consent);
        assert_eq!(view.cli_path, "");
        assert_eq!(view.cli_version, "");

        let empty = AiConfigView::from_config(&AiConfig::default());
        assert!(!empty.has_key);
        assert_eq!(empty.key_hint, "");
    }

    #[test]
    fn validate_rejects_invalid_input() {
        let mut cfg = sample_config();
        assert!(validate(&cfg).is_ok());

        cfg.provider = "gemini".to_string();
        assert!(validate(&cfg).is_err());

        cfg.provider = "openai".to_string();
        cfg.base_url = "   ".to_string();
        assert!(validate(&cfg).is_err());

        cfg.base_url = "https://api.example.test".to_string();
        cfg.model = "".to_string();
        assert!(validate(&cfg).is_err());
    }

    #[test]
    fn cli_providers_are_recognised() {
        assert!(is_cli_provider("claude-cli"));
        assert!(is_cli_provider("codex-cli"));
        assert!(!is_cli_provider("openai"));
        assert!(is_api_provider("anthropic"));
        assert!(is_api_provider("openai"));
        assert!(!is_api_provider("claude-cli"));
    }

    #[test]
    fn cli_validation_requires_matching_existing_file() {
        let root = tempdir("cli-validate");
        let claude = root.join("bin/claude.cmd");
        touch(&claude);
        let codex = root.join("bin/codex.exe");
        touch(&codex);
        let other = root.join("bin/other.exe");
        touch(&other);

        assert!(validate(&cli_config(&claude)).is_ok());

        let mut codex_cfg = cli_config(&codex);
        codex_cfg.provider = PROVIDER_CODEX_CLI.to_string();
        assert!(validate(&codex_cfg).is_ok());

        let mut cfg = cli_config(&root.join("bin/missing.exe"));
        assert!(validate(&cfg).is_err(), "missing path is rejected");

        cfg = cli_config(&other);
        assert!(validate(&cfg).is_err(), "file name must match the provider");

        cfg = cli_config(&claude);
        cfg.provider = PROVIDER_CODEX_CLI.to_string();
        assert!(validate(&cfg).is_err(), "claude file is not a codex CLI");

        cfg = cli_config(&claude);
        cfg.cli_path.clear();
        assert!(validate(&cfg).is_err());
    }

    #[test]
    fn resolves_native_exe_in_npm_layout() {
        let root = tempdir("native-layout");
        let shim = root.join("package/bin/claude.cmd");
        touch(&shim);
        let native = root
            .join("package/node_modules/@anthropic-ai/claude-code/bin/claude.exe");
        touch(&native);

        assert_eq!(native_exe_path(&shim).unwrap(), native);

        let codex_shim = root.join("package/bin/codex.cmd");
        touch(&codex_shim);
        let codex_native = root.join("package/node_modules/@openai/codex/bin/codex.exe");
        touch(&codex_native);
        assert_eq!(native_exe_path(&codex_shim).unwrap(), codex_native);
    }

    #[test]
    fn resolves_native_exe_in_same_directory() {
        let root = tempdir("native-same-dir");
        let shim = root.join("claude.ps1");
        touch(&shim);
        let native = root.join("claude.exe");
        touch(&native);
        assert_eq!(native_exe_path(&shim).unwrap(), native);
    }

    #[test]
    fn native_resolution_returns_none_without_exe() {
        let root = tempdir("native-missing");
        let shim = root.join("bin/claude.cmd");
        touch(&shim);
        assert!(native_exe_path(&shim).is_none());
        // Unknown file names never resolve.
        let other = root.join("bin/other.cmd");
        touch(&other);
        assert!(native_exe_path(&other).is_none());
    }

    fn write_shim(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn resolve_bound_cli_prefers_native_exe() {
        let root = tempdir("bind-resolve");
        let shim = root.join("pkg/bin/claude.cmd");
        write_shim(
            &shim,
            "@ECHO off\r\n\"%dp0%\\node\" \"%dp0%\\node_modules\\@anthropic-ai\\claude-code\\cli.js\" %*",
        );
        let native = root.join("pkg/node_modules/@anthropic-ai/claude-code/bin/claude.exe");
        touch(&native);

        assert_eq!(
            resolve_bound_cli(PROVIDER_CLAUDE_CLI, &shim.to_string_lossy()).unwrap(),
            native
        );
        assert!(resolve_bound_cli(PROVIDER_ANTHROPIC, &shim.to_string_lossy()).is_err());
        assert!(resolve_bound_cli(PROVIDER_CLAUDE_CLI, "   ").is_err());
    }

    #[test]
    fn custom_credential_shim_is_not_replaced_by_native_exe() {
        let root = tempdir("custom-shim");
        let shim = root.join("pkg/bin/claude.cmd");
        write_shim(
            &shim,
            "@ECHO off\r\nREM wrapper that injects credentials\r\nnode \"%WS_ROOT%ws-run.mjs\" claude %*",
        );
        let native = root.join("pkg/node_modules/@anthropic-ai/claude-code/bin/claude.exe");
        touch(&native);

        // The native exe is resolvable, but the shim injects the credentials.
        assert_eq!(native_exe_path(&shim).unwrap(), native);
        assert_eq!(
            resolve_bound_cli(PROVIDER_CLAUDE_CLI, &shim.to_string_lossy()).unwrap(),
            shim
        );
    }

    #[test]
    fn only_generated_npm_shims_are_unwrapped() {
        let root = tempdir("npm-shim-detect");
        let npm_cmd = root.join("npm/claude.cmd");
        write_shim(
            &npm_cmd,
            "@ECHO off\r\n\"%dp0%\\node\" \"%dp0%\\node_modules\\@anthropic-ai\\claude-code\\cli.js\" %*",
        );
        assert!(is_generated_npm_shim(&npm_cmd));

        let npm_ps1 = root.join("npm/claude.ps1");
        write_shim(
            &npm_ps1,
            "#!/usr/bin/env pwsh\n& \"$basedir/node$exe\" \"$basedir/node_modules/@anthropic-ai/claude-code/cli.js\" $args",
        );
        assert!(is_generated_npm_shim(&npm_ps1));

        let custom = root.join("custom/claude.cmd");
        write_shim(&custom, "@ECHO off\r\nnode \"%ROOT%ws-run.mjs\" claude %*");
        assert!(!is_generated_npm_shim(&custom));

        let exe_shim = root.join("custom/claude.exe");
        touch(&exe_shim);
        assert!(!is_generated_npm_shim(&exe_shim));
    }

    #[test]
    fn cli_args_use_stdin_for_claude_and_argv_for_codex() {
        let claude = cli_args(PROVIDER_CLAUDE_CLI, "say pong", "").unwrap();
        assert_eq!(
            claude.args,
            vec!["-p", "--output-format", "text", "--max-turns", "1"]
        );
        assert_eq!(claude.stdin.as_deref(), Some("say pong"));
        assert!(!claude.args.iter().any(|arg| arg == "say pong"));

        let claude_model = cli_args(PROVIDER_CLAUDE_CLI, "say pong", "claude-sonnet").unwrap();
        assert_eq!(
            claude_model.args,
            vec![
                "-p",
                "--output-format",
                "text",
                "--max-turns",
                "1",
                "--model",
                "claude-sonnet"
            ]
        );
        assert_eq!(claude_model.stdin.as_deref(), Some("say pong"));

        let codex = cli_args(PROVIDER_CODEX_CLI, "say pong", "").unwrap();
        assert_eq!(codex.args, vec!["exec", "--skip-git-repo-check", "say pong"]);
        assert!(codex.stdin.is_none());

        let codex_model = cli_args(PROVIDER_CODEX_CLI, "say pong", "gpt-5").unwrap();
        assert_eq!(
            codex_model.args,
            vec!["exec", "--skip-git-repo-check", "--model", "gpt-5", "say pong"]
        );

        assert!(cli_args(PROVIDER_OPENAI, "hi", "").is_err());
    }

    #[test]
    fn claude_prompt_stays_out_of_argv() {
        // Quotes/newlines must never reach cmd.exe argument parsing.
        let prompt = "line1 \"quoted\" & line2\n[{\"i\":0,\"category\":\"dev\"}]";
        let invocation = cli_args(PROVIDER_CLAUDE_CLI, prompt, "").unwrap();
        assert_eq!(invocation.stdin.as_deref(), Some(prompt));
        assert!(!invocation.args.contains(&prompt.to_string()));
    }

    #[test]
    fn available_requires_a_complete_config() {
        let mut cfg = sample_config();
        assert!(available(&cfg));

        cfg.api_key.clear();
        assert!(!available(&cfg));

        let mut cfg = sample_config();
        cfg.model.clear();
        assert!(!available(&cfg));

        let mut cfg = sample_config();
        cfg.provider = PROVIDER_CLAUDE_CLI.to_string();
        assert!(!available(&cfg), "CLI without a path is unavailable");

        let root = tempdir("available-cli");
        let cli = root.join("claude.cmd");
        touch(&cli);
        cfg.cli_path = cli.to_string_lossy().to_string();
        assert!(available(&cfg));

        cfg.cli_path = root.join("codex.cmd").to_string_lossy().to_string();
        assert!(!available(&cfg), "file name must match the provider");
    }

    #[test]
    fn complete_without_key_does_not_touch_network() {
        let mut cfg = sample_config();
        cfg.api_key.clear();
        let err = complete(&cfg, "hi").unwrap_err();
        assert_eq!(err, "未配置 API Key");
    }

    #[test]
    fn run_cli_validates_before_spawning() {
        let mut cfg = cli_config(Path::new("Root/Tools/claude.exe"));
        cfg.cli_path.clear();
        let err = run_cli(&cfg, "hi").unwrap_err();
        assert!(err.contains("绑定"), "{err}");

        let cfg = sample_config();
        let err = run_cli(&cfg, "hi").unwrap_err();
        assert!(err.contains("CLI"), "{err}");
    }

    #[test]
    fn parses_version_first_line() {
        assert_eq!(parse_version_line("2.1.296\nmore"), Some("2.1.296".to_string()));
        assert_eq!(
            parse_version_line("\n\n  2.1.296 (Claude Code)  \nextra"),
            Some("2.1.296 (Claude Code)".to_string())
        );
        assert_eq!(
            parse_version_line("\u{1b}[32m1.2.3\u{1b}[0m"),
            Some("1.2.3".to_string())
        );
        assert_eq!(parse_version_line("   \n  "), None);
    }

    #[test]
    fn strips_ansi_sequences() {
        assert_eq!(strip_ansi("\u{1b}[31merror\u{1b}[0m"), "error");
        assert_eq!(strip_ansi("a\rb"), "ab");
        assert_eq!(strip_ansi("plain"), "plain");
    }

    #[test]
    fn sanitizes_cli_error_output() {
        let stderr = "\u{1b}[31mError: boom\u{1b}[0m\n\
                      api key sk-abcdef123456 leaked\n\
                      this line is fine";
        let cleaned = sanitize_error_text(stderr);
        assert!(cleaned.contains("Error: boom"), "{cleaned}");
        assert!(cleaned.contains("this line is fine"), "{cleaned}");
        assert!(!cleaned.contains("abcdef123456"), "{cleaned}");

        let long_line = "x".repeat(500);
        assert!(!sanitize_error_text(&long_line).contains(&long_line));
        assert_eq!(sanitize_error_text("   "), "");
    }

    #[test]
    fn test_connection_without_key_does_not_touch_network() {
        let mut cfg = sample_config();
        cfg.api_key.clear();
        let result = test_connection(&cfg);
        assert!(!result.ok);
        assert_eq!(result.message, "未配置 API Key");
    }

    #[test]
    fn test_connection_rejects_invalid_config_before_network() {
        let mut cfg = sample_config();
        cfg.provider = "gemini".to_string();
        let result = test_connection(&cfg);
        assert!(!result.ok);
        assert!(result.message.contains("服务商"), "{}", result.message);

        let mut cfg = sample_config();
        cfg.provider = PROVIDER_CLAUDE_CLI.to_string();
        let result = test_connection(&cfg);
        assert!(!result.ok);
        assert!(result.message.contains("绑定"), "{}", result.message);
    }

    #[test]
    fn http_errors_map_to_readable_messages() {
        assert!(status_message(401).contains("API Key"));
        assert!(status_message(403).contains("API Key"));
        assert!(status_message(404).contains("模型"));
        assert!(status_message(429).contains("频繁"));
        assert!(status_message(500).contains("不可用"));
        assert!(status_message(418).contains("418"));
    }

    #[test]
    fn redaction_hides_the_key() {
        assert_eq!(redact("failed for sk-secret-1234", "sk-secret-1234"), "failed for ***");
        assert_eq!(redact("plain", ""), "plain");
    }
}
