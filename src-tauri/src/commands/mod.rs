//! Tauri command surface. All heavy work is delegated to domain/infra.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::domain::config::{self, AppEntry, Collection, Config, TagStyle};
use crate::domain::path::resolve;
use crate::domain::{ai, classify, launch, migrate, scan, theme};
use crate::infra::{autostart, icon, registry};

pub struct AppState {
    pub root: PathBuf,
    pub config: Mutex<Config>,
    pub warning: Mutex<Option<String>>,
    pub discover_cancel: Arc<AtomicBool>,
    /// True while the global hotkey is temporarily unregistered for recording.
    pub hotkey_suspended: Mutex<bool>,
}

impl AppState {
    pub fn new(root: PathBuf, cfg: Config, warning: Option<String>) -> Self {
        Self {
            root,
            config: Mutex::new(cfg),
            warning: Mutex::new(warning),
            discover_cancel: Arc::new(AtomicBool::new(false)),
            hotkey_suspended: Mutex::new(false),
        }
    }
    pub fn cfg(&self) -> Config {
        self.config.lock().unwrap().clone()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppView {
    pub id: String,
    pub name: String,
    pub path: String,
    pub args: Vec<String>,
    pub work_dir: String,
    pub autostart: bool,
    pub autostart_delay_ms: u64,
    pub skip_if_running: bool,
    pub hidden: bool,
    pub tags: Vec<String>,
    /// Semantic category (REQ-0039); empty = unclassified.
    pub category: String,
    /// Launch elevated via UAC (BUG-0023).
    pub run_as_admin: bool,
    pub exists: bool,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub root: String,
    pub app_name: String,
    pub version: String,
    pub config: Config,
    pub warning: Option<String>,
    pub themes: Vec<theme::ThemeSummary>,
    pub startup: autostart::StartupStatus,
    pub webview2: Option<String>,
    pub registry_autostart: bool,
    pub start_hidden: bool,
}

fn view_with_icon(root: &std::path::Path, app: &AppEntry, with_icon: bool) -> AppView {
    let exe = resolve(root, &app.path);
    AppView {
        id: app.id.clone(),
        name: app.name.clone(),
        path: app.path.clone(),
        args: app.args.clone(),
        work_dir: app.work_dir.clone(),
        autostart: app.autostart,
        autostart_delay_ms: app.autostart_delay_ms,
        skip_if_running: app.skip_if_running,
        hidden: app.hidden,
        tags: app.tags.clone(),
        category: app.category.clone(),
        run_as_admin: app.run_as_admin,
        exists: exe.exists(),
        icon: if with_icon { icon::get(root, &exe) } else { None },
    }
}

fn view(root: &std::path::Path, app: &AppEntry) -> AppView {
    view_with_icon(root, app, true)
}

fn persist(state: &AppState) -> Result<(), String> {
    let cfg = state.cfg();
    config::save(&state.root, &cfg)
}

#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> Bootstrap {
    let cfg = state.cfg();
    let from_login = std::env::args().any(|a| a == autostart::AUTOSTART_ARG);
    Bootstrap {
        root: state.root.to_string_lossy().to_string(),
        app_name: crate::domain::identity::app_name(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        themes: theme::list(&state.root.join("Themes")),
        startup: autostart::status(),
        webview2: registry::webview2_version(),
        registry_autostart: registry::run_key_has(&crate::domain::identity::app_name()),
        start_hidden: from_login && cfg.start_hidden_at_login,
        warning: state.warning.lock().unwrap().clone(),
        config: cfg,
    }
}

fn list_apps_from(root: &std::path::Path, cfg: &Config) -> Vec<AppView> {
    cfg.apps
        .iter()
        .filter(|a| !a.hidden)
        .map(|a| view_with_icon(root, a, false))
        .collect()
}

#[tauri::command]
pub async fn list_apps(state: State<'_, AppState>) -> Result<Vec<AppView>, String> {
    let root = state.root.clone();
    let cfg = state.cfg();
    tauri::async_runtime::spawn_blocking(move || list_apps_from(&root, &cfg))
        .await
        .map_err(|e| format!("读取应用列表失败：{e}"))
}

#[tauri::command]
pub async fn scan_apps(state: State<'_, AppState>) -> Result<Vec<scan::Candidate>, String> {
    let root = state.root.clone();
    let cfg = state.cfg();
    tauri::async_runtime::spawn_blocking(move || {
        let found = scan::scan(&root, &cfg.apps_dir);
        let existing: Vec<String> = cfg.apps.iter().map(|a| a.path.clone()).collect();
        scan::filter_new_candidates(&root, &existing, found)
    })
    .await
    .map_err(|e| format!("扫描任务失败：{e}"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverReport {
    pub items: Vec<crate::domain::discover::Discovered>,
    pub cancelled: bool,
}

#[tauri::command]
pub async fn discover_apps(
    state: State<'_, AppState>,
    roots: Vec<String>,
) -> Result<DiscoverReport, String> {
    let cancel = state.discover_cancel.clone();
    cancel.store(false, Ordering::Relaxed);
    let cfg = state.cfg();
    let mut exclude = vec![state.root.clone()];
    exclude.push(resolve(&state.root, &cfg.apps_dir));
    let root = state.root.clone();
    let registered: Vec<String> = cfg.apps.iter().map(|a| a.path.clone()).collect();

    let roots: Vec<PathBuf> = if roots.is_empty() {
        crate::domain::discover::default_roots()
    } else {
        // The frontend passes drive letters like "E:"; on Windows that is a
        // drive-relative path (BUG-0019), so normalize it to "E:\".
        roots
            .into_iter()
            .map(|r| crate::domain::discover::normalize_root(&r))
            .collect()
    };

    tauri::async_runtime::spawn_blocking(move || {
        let items = crate::domain::discover::discover(&roots, &exclude, 500, &cancel);
        // Discovery runs across whole drives, so it also returns apps that are
        // already registered (e.g. added with an absolute path). Remove them
        // here: the frontend only detects duplicates by name, and users rename
        // cards, so that check misses them.
        let items = crate::domain::discover::filter_registered(&root, &registered, items);
        let cancelled = cancel.load(Ordering::Relaxed);
        DiscoverReport { items, cancelled }
    })
    .await
    .map_err(|e| format!("发现任务失败：{e}"))
}

#[tauri::command]
pub fn cancel_discover(state: State<'_, AppState>) {
    state.discover_cancel.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub fn list_drives() -> Vec<String> {
    crate::domain::discover::drives()
        .into_iter()
        .map(|p| p.to_string_lossy().trim_end_matches('\\').to_string())
        .collect()
}

pub use crate::domain::import::{ImportItem, ImportMode, ImportOutcome};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub moved: usize,
    pub added: usize,
    pub skipped: usize,
    pub failed: usize,
    pub items: Vec<ImportOutcome>,
}

#[tauri::command]
pub async fn import_apps(
    app: AppHandle,
    state: State<'_, AppState>,
    items: Vec<ImportItem>,
    mode: ImportMode,
) -> Result<ImportReport, String> {
    use tauri::Emitter;

    let root = state.root.clone();
    let cfg = state.cfg();
    let apps_dir = resolve(&root, &cfg.apps_dir);
    let existing: std::collections::HashSet<String> =
        cfg.apps.iter().map(|a| a.path.clone()).collect();
    let existing_names: std::collections::HashSet<String> =
        cfg.apps.iter().map(|a| a.name.to_lowercase()).collect();

    let (entries, outcomes) = tauri::async_runtime::spawn_blocking(move || {
        let mut progress = |outcome: &ImportOutcome| {
            let _ = app.emit("import://progress", outcome);
        };
        // The GUI's Move mode is a real move: same-volume rename, otherwise
        // copy → verify → delete the source. `backup_source` stays false here on
        // purpose (only the CLI's `--backup` flag asks for the renamed backup),
        // so the UI must never claim a backup is kept (BUG-0016).
        crate::domain::import::run(&root, &apps_dir, existing, existing_names, items, mode, false, &mut progress)
    })
    .await
    .map_err(|e| format!("导入任务失败：{e}"))?;

    if !entries.is_empty() {
        {
            let mut cfg = state.config.lock().unwrap();
            cfg.apps.extend(entries);
        }
        persist(&state)?;
    }

    let added = outcomes.iter().filter(|o| o.ok).count();
    let failed = outcomes.iter().filter(|o| !o.ok).count();
    let skipped = outcomes
        .iter()
        .filter(|o| o.ok && o.message.contains("跳过"))
        .count();
    Ok(ImportReport {
        moved: if mode == ImportMode::Move { added } else { 0 },
        added,
        skipped,
        failed,
        items: outcomes,
    })
}

#[tauri::command]
pub fn add_apps(state: State<'_, AppState>, candidates: Vec<scan::Candidate>) -> Result<Vec<AppView>, String> {
    {
        let mut cfg = state.config.lock().unwrap();
        let mut existing: Vec<(String, String)> = cfg
            .apps
            .iter()
            .map(|a| (a.name.clone(), a.path.clone()))
            .collect();
        for c in candidates {
            // Imported apps may store an absolute path while a scan candidate
            // carries a root-relative one, so compare canonicalized paths.
            // Same-name entries are skipped too: that matches the import-side
            // dedupe and keeps "add all" from creating duplicate cards.
            if scan::should_skip_add(&state.root, &existing, &c.name, &c.rel_path) {
                continue;
            }
            existing.push((c.name.clone(), c.rel_path.clone()));
            cfg.apps.push(AppEntry::new(c.name, c.rel_path));
        }
    }
    persist(&state)?;
    Ok(list_apps_from(&state.root, &state.cfg()))
}

/// REQ-0023: classify where each app's content really lives.
#[tauri::command]
pub fn audit_apps(state: State<'_, AppState>) -> Vec<migrate::AppAudit> {
    let cfg = state.cfg();
    migrate::audit(&state.root, &cfg)
}

/// REQ-0023: export the hub as a self-contained, movable bundle.
#[tauri::command]
pub async fn export_bundle(
    state: State<'_, AppState>,
    target: String,
) -> Result<migrate::ExportReport, String> {
    let root = state.root.clone();
    let cfg = state.cfg();
    tauri::async_runtime::spawn_blocking(move || {
        migrate::export_bundle(&root, &cfg, std::path::Path::new(&target))
    })
    .await
    .map_err(|e| format!("导出任务失败：{e}"))?
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppPatch {
    pub name: Option<String>,
    pub path: Option<String>,
    pub args: Option<Vec<String>>,
    pub work_dir: Option<String>,
    pub autostart: Option<bool>,
    pub autostart_delay_ms: Option<u64>,
    pub skip_if_running: Option<bool>,
    /// BUG-0023: launch elevated via UAC.
    pub run_as_admin: Option<bool>,
    /// Manual semantic category; empty string clears it.
    pub category: Option<String>,
}

impl Default for AppPatch {
    fn default() -> Self {
        Self {
            name: None,
            path: None,
            args: None,
            work_dir: None,
            autostart: None,
            autostart_delay_ms: None,
            skip_if_running: None,
            run_as_admin: None,
            category: None,
        }
    }
}

#[tauri::command]
pub fn update_app(state: State<'_, AppState>, id: String, patch: AppPatch) -> Result<AppView, String> {
    {
        let mut cfg = state.config.lock().unwrap();
        let app = cfg
            .apps
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| format!("应用不存在：{id}"))?;
        if let Some(v) = patch.name {
            app.name = v;
        }
        if let Some(v) = patch.path {
            app.path = v;
        }
        if let Some(v) = patch.args {
            app.args = v;
        }
        if let Some(v) = patch.work_dir {
            app.work_dir = v;
        }
        if let Some(v) = patch.autostart {
            app.autostart = v;
        }
        if let Some(v) = patch.autostart_delay_ms {
            app.autostart_delay_ms = v;
        }
        if let Some(v) = patch.skip_if_running {
            app.skip_if_running = v;
        }
        if let Some(v) = patch.run_as_admin {
            app.run_as_admin = v;
        }
        if let Some(v) = patch.category {
            let category = v.trim();
            if !category.is_empty() && !classify::is_valid_category(category) {
                return Err(format!("未知分类：{category}"));
            }
            app.category = category.to_string();
        }
    }
    persist(&state)?;
    let cfg = state.cfg();
    let app = cfg.apps.iter().find(|a| a.id == id).unwrap();
    Ok(view(&state.root, app))
}

#[tauri::command]
pub fn remove_app(state: State<'_, AppState>, id: String) -> Result<Vec<AppView>, String> {
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.apps.retain(|a| a.id != id);
    }
    persist(&state)?;
    Ok(list_apps_from(&state.root, &state.cfg()))
}

// ---- AI providers (REQ-0035 / REQ-0038) ----
//
// The raw API key stays in the backend (`_private/ai.json`); every command
// returns the masked `AiConfigView` instead. CLI providers run the locally
// installed `claude` / `codex` executable instead of an HTTP API.

#[tauri::command]
pub fn ai_get_config(state: State<'_, AppState>) -> Result<ai::AiConfigView, String> {
    let cfg = ai::load(&state.root);
    Ok(ai::AiConfigView::from_config(&cfg))
}

#[tauri::command]
pub fn ai_save_config(
    state: State<'_, AppState>,
    payload: ai::AiSavePayload,
) -> Result<ai::AiConfigView, String> {
    let mut cfg = ai::load(&state.root);
    cfg.provider = payload.provider.trim().to_string();
    cfg.base_url = payload.base_url.trim().to_string();
    cfg.model = payload.model.trim().to_string();
    if let Some(cli_path) = payload
        .cli_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        cfg.cli_path = cli_path.to_string();
    }
    ai::validate(&cfg)?;
    if payload.clear_key.unwrap_or(false) {
        cfg.api_key.clear();
    } else if let Some(key) = payload
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
    {
        cfg.api_key = key.to_string();
    }
    ai::save(&state.root, &cfg)?;
    Ok(ai::AiConfigView::from_config(&cfg))
}

#[tauri::command]
pub fn ai_set_consent(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<ai::AiConfigView, String> {
    let mut cfg = ai::load(&state.root);
    cfg.consent = enabled;
    ai::save(&state.root, &cfg)?;
    Ok(ai::AiConfigView::from_config(&cfg))
}

#[tauri::command]
pub async fn ai_test_connection(
    state: State<'_, AppState>,
) -> Result<ai::AiTestResult, String> {
    let root = state.root.clone();
    // HTTP requests (20s) and CLI runs (up to 120s) block; keep them off the
    // UI thread.
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = ai::load(&root);
        ai::test_connection(&cfg)
    })
    .await
    .map_err(|e| format!("AI 连接测试失败：{e}"))
}

/// Scan `PATH` for locally installed Claude Code / Codex CLIs.
#[tauri::command]
pub async fn ai_detect_clis() -> Result<Vec<ai::AiCliCandidate>, String> {
    // Each candidate runs `--version` (up to 10s), so never block the UI thread.
    tauri::async_runtime::spawn_blocking(ai::detect_clis)
        .await
        .map_err(|e| format!("检测本机 CLI 失败：{e}"))
}

/// Bind a detected CLI as the active provider.
#[tauri::command]
pub async fn ai_bind_cli(
    state: State<'_, AppState>,
    provider: String,
    path: String,
) -> Result<ai::AiConfigView, String> {
    let root = state.root.clone();
    tauri::async_runtime::spawn_blocking(move || ai::bind_cli(&root, &provider, &path))
        .await
        .map_err(|e| format!("绑定 CLI 失败：{e}"))?
}

// ---- semantic classification (REQ-0039 / REQ-0040) ----

/// Suggest categories for every app: rules by default, AI when enabled.
///
/// AI requires `consent` and a usable provider; any failure falls back to the
/// local rules, so this command never fails because the model/CLI misbehaved.
#[tauri::command]
pub async fn classify_apps(
    state: State<'_, AppState>,
) -> Result<Vec<classify::Suggestion>, String> {
    let cfg = state.cfg();
    let root = state.root.clone();
    let inputs: Vec<classify::ClassifyInput> = cfg
        .apps
        .iter()
        .map(|app| {
            let exe = resolve(&root, &app.path);
            classify::ClassifyInput {
                id: app.id.clone(),
                name: app.name.clone(),
                path: app.path.clone(),
                exe: exe.to_string_lossy().to_string(),
                size: std::fs::metadata(&exe).map(|meta| meta.len()).unwrap_or(0),
            }
        })
        .collect();

    // AI classification blocks on network/CLI; keep it off the UI thread.
    let ai_cfg = ai::load(&root);
    tauri::async_runtime::spawn_blocking(move || {
        if ai_cfg.consent && ai::available(&ai_cfg) {
            classify::classify_with_ai(&root, &ai_cfg, &inputs)
        } else {
            inputs.iter().map(classify::classify_rules).collect()
        }
    })
    .await
    .map_err(|e| format!("分类任务失败：{e}"))
}

/// Normalize discovered items for classification.
///
/// Discovery items carry no id, so an empty id falls back to the path; items
/// without a path are unusable and dropped; duplicate paths keep the first
/// occurrence. Input order is preserved.
fn inputs_from_discovered(
    items: Vec<classify::ClassifyInput>,
) -> Vec<classify::ClassifyInput> {
    let mut out: Vec<classify::ClassifyInput> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for mut item in items {
        let path = item.path.trim().to_string();
        if path.is_empty() {
            continue;
        }
        let id = item.id.trim();
        item.id = if id.is_empty() {
            path.clone()
        } else {
            id.to_string()
        };
        item.path = path;
        let key = item.path.replace('\\', "/").to_lowercase();
        if !seen.insert(key) {
            continue;
        }
        out.push(item);
    }
    out
}

/// Suggest categories for discovered (not yet imported) apps.
///
/// The frontend sends `Discovered` items; `id`/`size` are optional on the wire.
/// AI is used when consent is on and the provider is usable, otherwise the
/// local rules run. Every failure falls back inside `classify`, so the command
/// never fails because the model/CLI misbehaved.
#[tauri::command]
pub async fn classify_discovered(
    state: State<'_, AppState>,
    items: Vec<classify::ClassifyInput>,
) -> Result<Vec<classify::Suggestion>, String> {
    let root = state.root.clone();
    let inputs = inputs_from_discovered(items);
    let ai_cfg = ai::load(&root);
    tauri::async_runtime::spawn_blocking(move || {
        if ai_cfg.consent && ai::available(&ai_cfg) {
            classify::classify_with_ai(&root, &ai_cfg, &inputs)
        } else {
            inputs.iter().map(classify::classify_rules).collect()
        }
    })
    .await
    .map_err(|e| format!("发现结果分类失败：{e}"))
}

/// Persist accepted suggestions; unknown categories/ids are skipped.
#[tauri::command]
pub fn apply_categories(
    state: State<'_, AppState>,
    items: Vec<classify::CategoryApply>,
) -> Result<Vec<AppView>, String> {
    {
        let mut cfg = state.config.lock().unwrap();
        for item in items {
            let category = item.category.trim();
            if !classify::is_valid_category(category) {
                continue;
            }
            if let Some(app) = cfg.apps.iter_mut().find(|app| app.id == item.id) {
                app.category = category.to_string();
            }
        }
    }
    persist(&state)?;
    Ok(list_apps_from(&state.root, &state.cfg()))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchReceipt {
    pub pid: u32,
    pub command_line: String,
    /// True when the app was started through the UAC prompt (no pid then).
    pub elevated: bool,
}

#[tauri::command]
pub fn launch_app(state: State<'_, AppState>, id: String) -> Result<LaunchReceipt, String> {
    let cfg = state.cfg();
    let app = cfg
        .apps
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| format!("应用不存在：{id}"))?;
    let plan = launch::plan(&state.root, app)?;
    let outcome = launch::spawn_with_receipt(&plan)?;
    Ok(LaunchReceipt {
        pid: outcome.pid,
        command_line: plan.command_line,
        elevated: outcome.elevated,
    })
}

#[tauri::command]
pub fn reveal_app(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let cfg = state.cfg();
    let app = cfg
        .apps
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| format!("应用不存在：{id}"))?;
    let exe = resolve(&state.root, &app.path);
    let target = if exe.exists() {
        exe
    } else {
        state.root.join(&cfg.apps_dir)
    };
    std::process::Command::new("explorer")
        .arg(format!("/select,{}", target.to_string_lossy()))
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Open the `Themes` folder in Explorer (used by the appearance settings row
/// instead of printing a long absolute path as help text).
#[tauri::command]
pub fn open_theme_dir(state: State<'_, AppState>) -> Result<(), String> {
    let dir = state.root.join("Themes");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建主题目录失败：{e}"))?;
    std::process::Command::new("explorer")
        .arg(&dir)
        .spawn()
        .map_err(|e| format!("无法打开主题目录：{e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn get_icons(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<HashMap<String, Option<String>>, String> {
    let root = state.root.clone();
    let wanted: std::collections::HashSet<String> = ids.into_iter().collect();
    let apps: Vec<AppEntry> = state
        .cfg()
        .apps
        .into_iter()
        .filter(|a| wanted.contains(&a.id))
        .collect();
    tauri::async_runtime::spawn_blocking(move || {
        apps.iter()
            .map(|a| {
                let exe = resolve(&root, &a.path);
                (a.id.clone(), icon::get(&root, &exe))
            })
            .collect()
    })
    .await
    .map_err(|e| format!("图标任务失败：{e}"))
}

#[tauri::command]
pub fn list_themes(state: State<'_, AppState>) -> Vec<theme::ThemeSummary> {
    theme::list(&state.root.join("Themes"))
}

#[tauri::command]
pub fn get_theme(state: State<'_, AppState>, id: String) -> Result<theme::Theme, String> {
    let dir = state.root.join("Themes");
    match theme::get(&dir, &id) {
        Ok(t) => Ok(t),
        Err(e) => {
            // fall back to the built-in copy so the UI always has a theme
            theme::builtin(&id).ok_or(e)
        }
    }
}

#[tauri::command]
pub fn set_theme(state: State<'_, AppState>, id: String) -> Result<(), String> {
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.theme = id;
    }
    persist(&state)
}

/// Temporarily unregister the global hotkey while the user records a new one.
///
/// Without this, pressing the old accelerator during recording would still
/// toggle the main window and make the recorder look broken.
#[tauri::command]
pub fn suspend_hotkey(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let mut suspended = state.hotkey_suspended.lock().unwrap();
    if *suspended {
        return Ok(());
    }
    let hotkey = state.cfg().hotkey;
    // The accelerator may already be unregistered (e.g. registration failed at
    // startup); recording must still be enterable in that case.
    let _ = app.global_shortcut().unregister(hotkey.as_str());
    *suspended = true;
    Ok(())
}

/// Re-register the global hotkey once recording finished (or was cancelled).
///
/// On failure the suspended flag is kept so a later attempt can retry instead
/// of silently leaving the app without a global hotkey.
#[tauri::command]
pub fn resume_hotkey(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let mut suspended = state.hotkey_suspended.lock().unwrap();
    if !*suspended {
        return Ok(());
    }
    let hotkey = state.cfg().hotkey;
    if let Err(e) = app.global_shortcut().register(hotkey.as_str()) {
        return Err(format!("热键恢复失败：{e}"));
    }
    *suspended = false;
    Ok(())
}

#[tauri::command]
pub fn set_hotkey(
    state: State<'_, AppState>,
    app: AppHandle,
    accel: String,
) -> Result<(), String> {
    let gs = app.global_shortcut();
    let old = state.cfg().hotkey;
    if old == accel {
        return Ok(());
    }
    let _ = gs.unregister(old.as_str());
    if let Err(e) = gs.register(accel.as_str()) {
        let _ = gs.register(old.as_str());
        // The old accelerator was restored, so recording is over either way.
        *state.hotkey_suspended.lock().unwrap() = false;
        return Err(format!("热键注册失败：{e}"));
    }
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.hotkey = accel;
    }
    *state.hotkey_suspended.lock().unwrap() = false;
    persist(&state)
}

#[tauri::command]
pub fn set_launch_at_login(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<autostart::StartupStatus, String> {
    let status = if enabled {
        autostart::enable()?
    } else {
        autostart::disable()?
    };
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.launch_at_login = enabled;
    }
    persist(&state)?;
    Ok(status)
}

#[tauri::command]
pub fn get_startup_status() -> autostart::StartupStatus {
    autostart::status()
}

#[tauri::command]
pub fn set_app_autostart(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    {
        let mut cfg = state.config.lock().unwrap();
        let app = cfg
            .apps
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| format!("应用不存在：{id}"))?;
        app.autostart = enabled;
    }
    persist(&state)
}

#[tauri::command]
pub fn set_apps_dir(state: State<'_, AppState>, path: String) -> Result<Config, String> {
    let input = path.trim();
    if input.is_empty() {
        return Err("应用目录不能为空".into());
    }
    let abs = crate::domain::path::resolve(&state.root, input);
    if !abs.is_dir() {
        return Err(format!("目录不存在或不是文件夹：{}", abs.display()));
    }
    let stored = crate::domain::path::to_rel(&state.root, &abs);
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.apps_dir = stored;
    }
    persist(&state)?;
    Ok(state.cfg())
}

#[tauri::command]
pub fn set_import_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    if !["link", "move", "register"].contains(&mode.as_str()) {
        return Err(format!("未知导入方式：{mode}"));
    }
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.import_mode = mode;
    }
    persist(&state)
}

#[tauri::command]
pub fn set_locale(state: State<'_, AppState>, locale: String) -> Result<(), String> {
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.locale = locale;
    }
    persist(&state)
}

#[tauri::command]
pub fn set_close_to_tray(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.close_to_tray = enabled;
    }
    persist(&state)
}

#[tauri::command]
pub fn set_pixel_icons(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.pixel_icons = enabled;
    }
    persist(&state)
}

#[tauri::command]
pub fn set_pixel_font(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.pixel_font = enabled;
    }
    persist(&state)
}

#[tauri::command]
pub fn set_discover_roots(
    state: State<'_, AppState>,
    roots: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut clean: Vec<String> = Vec::new();
    for root in roots {
        let root = root.trim().trim_end_matches(['\\', '/']).to_string();
        if root.is_empty() || root.chars().count() > 260 {
            continue;
        }
        if !clean.iter().any(|item| item.eq_ignore_ascii_case(&root)) {
            clean.push(root);
        }
        if clean.len() >= 32 {
            break;
        }
    }
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.discover_roots = clean.clone();
    }
    persist(&state)?;
    Ok(clean)
}

#[tauri::command]
pub fn save_collections(
    state: State<'_, AppState>,
    collections: Vec<Collection>,
) -> Result<Vec<Collection>, String> {
    let known: std::collections::HashSet<String> =
        state.config.lock().unwrap().apps.iter().map(|a| a.id.clone()).collect();
    let mut clean: Vec<Collection> = Vec::new();
    for collection in collections {
        let name = collection.name.trim().to_string();
        if name.is_empty() || name.chars().count() > 40 {
            continue;
        }
        let id = if collection.id.trim().is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            collection.id.trim().to_string()
        };
        if clean.iter().any(|item| item.id == id) {
            continue;
        }
        let mut app_ids: Vec<String> = Vec::new();
        for app_id in collection.app_ids {
            if known.contains(&app_id) && !app_ids.contains(&app_id) {
                app_ids.push(app_id);
            }
        }
        clean.push(Collection {
            id,
            name,
            app_ids,
        });
        if clean.len() >= 64 {
            break;
        }
    }
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.collections = clean.clone();
    }
    persist(&state)?;
    Ok(clean)
}

#[tauri::command]
pub fn set_favorites(
    state: State<'_, AppState>,
    favorites: Vec<String>,
) -> Result<Vec<String>, String> {
    let known: std::collections::HashSet<String> =
        state.config.lock().unwrap().apps.iter().map(|a| a.id.clone()).collect();
    let mut clean: Vec<String> = Vec::new();
    for app_id in favorites {
        if known.contains(&app_id) && !clean.contains(&app_id) {
            clean.push(app_id);
        }
    }
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.favorites = clean.clone();
    }
    persist(&state)?;
    Ok(clean)
}

#[tauri::command]
pub fn set_start_hidden_at_login(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.start_hidden_at_login = enabled;
    }
    persist(&state)
}

#[tauri::command]
pub fn run_autostart_now(state: State<'_, AppState>) -> Vec<autostart::LaunchReport> {
    let cfg = state.cfg();
    autostart::launch_flagged(&state.root, &cfg, false)
}

/// Trim, de-duplicate (case-insensitive) and cap user tags. Pure helper so the
/// normalization rules are unit-testable.
fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() || tag.chars().count() > 24 {
            continue;
        }
        if out.iter().any(|existing| existing.eq_ignore_ascii_case(tag)) {
            continue;
        }
        out.push(tag.to_string());
        if out.len() >= 20 {
            break;
        }
    }
    out
}

/// REQ-0032: replace an app's tag list. Tags are logical categories and do not
/// touch the files on disk.
#[tauri::command]
pub fn set_app_tags(
    state: State<'_, AppState>,
    id: String,
    tags: Vec<String>,
) -> Result<Vec<AppView>, String> {
    {
        let mut cfg = state.config.lock().unwrap();
        let app = cfg
            .apps
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| format!("应用不存在：{id}"))?;
        app.tags = normalize_tags(tags);
    }
    persist(&state)?;
    Ok(list_apps_from(&state.root, &state.cfg()))
}

fn valid_tag_color(color: &str) -> bool {
    if color.is_empty() {
        return true;
    }
    color.len() == 7
        && color.starts_with('#')
        && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Persist a user-defined style for one tag. Styles are global (all chips share
/// them), so this lives on Config rather than on an individual app.
#[tauri::command]
pub fn set_tag_style(
    state: State<'_, AppState>,
    tag: String,
    color: String,
    icon: String,
) -> Result<BTreeMap<String, TagStyle>, String> {
    let tag = tag.trim().to_string();
    if tag.is_empty() || tag.chars().count() > 24 {
        return Err("标签名无效".to_string());
    }
    let color = color.trim().to_ascii_lowercase();
    if !valid_tag_color(&color) {
        return Err("颜色必须是 #rrggbb 或留空".to_string());
    }
    let icon = icon.trim().chars().take(8).collect::<String>();
    {
        let mut cfg = state.config.lock().unwrap();
        if color.is_empty() && icon.is_empty() {
            cfg.tag_styles.remove(&tag);
        } else {
            cfg.tag_styles.insert(tag, TagStyle { color, icon });
        }
    }
    persist(&state)?;
    Ok(state.config.lock().unwrap().tag_styles.clone())
}

pub fn toggle_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let visible = w.is_visible().unwrap_or(false);
        if visible {
            let _ = w.hide();
        } else {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[tauri::command]
pub fn hide_main_window(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn log_frontend(state: State<'_, AppState>, message: String) {
    let dir = state.root.join("_cache").join("logs");
    let _ = std::fs::create_dir_all(&dir);
    let line = format!(
        "{} {}\n",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        message
    );
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("frontend.log"))
    {
        let _ = f.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_starts_with_hotkey_active() {
        let state = AppState::new(
            std::env::temp_dir(),
            Config::default(),
            Some("warning".to_string()),
        );
        assert!(!*state.hotkey_suspended.lock().unwrap());
        assert_eq!(state.cfg().hotkey, "Ctrl+Alt+P");
        assert_eq!(state.warning.lock().unwrap().as_deref(), Some("warning"));
    }

    #[test]
    fn normalizes_user_tags() {
        let tags = normalize_tags(vec![
            "  游戏 ".to_string(),
            "游戏".to_string(),
            "".to_string(),
            "Tool".to_string(),
            "tool".to_string(),
            "x".repeat(25),
        ]);
        assert_eq!(tags, vec!["游戏".to_string(), "Tool".to_string()]);
    }

    #[test]
    fn discovered_inputs_fill_ids_and_dedupe_paths() {
        let inputs = inputs_from_discovered(vec![
            classify::ClassifyInput {
                id: String::new(),
                name: "Alpha".to_string(),
                path: "E:/tools/Alpha".to_string(),
                exe: "E:/tools/Alpha/Alpha.exe".to_string(),
                size: 10,
            },
            // Duplicate path (different casing/separators) must be skipped.
            classify::ClassifyInput {
                id: "second".to_string(),
                name: "Alpha again".to_string(),
                path: "e:\\tools\\alpha".to_string(),
                exe: "e:\\tools\\alpha\\Alpha.exe".to_string(),
                size: 10,
            },
            // Path-less items cannot be classified.
            classify::ClassifyInput {
                id: "no-path".to_string(),
                name: "NoPath".to_string(),
                path: "   ".to_string(),
                exe: String::new(),
                size: 0,
            },
            classify::ClassifyInput {
                id: "keep-id".to_string(),
                name: "Beta".to_string(),
                path: "E:/tools/Beta".to_string(),
                exe: "E:/tools/Beta/Beta.exe".to_string(),
                size: 20,
            },
        ]);
        assert_eq!(inputs.len(), 2, "{inputs:?}");
        assert_eq!(inputs[0].id, "E:/tools/Alpha", "empty id falls back to path");
        assert_eq!(inputs[0].name, "Alpha");
        assert_eq!(inputs[1].id, "keep-id", "existing ids are kept");
        assert_eq!(inputs[1].size, 20);
    }

    #[test]
    fn discovered_inputs_accept_frontend_payload_without_id() {
        // `api.classifyDiscovered` sends `Discovered` objects: no id, extra fields.
        let payload = serde_json::json!([
            { "name": "Alpha", "path": "E:/tools/Alpha", "exe": "E:/tools/Alpha/Alpha.exe", "size": 42, "kind": "folder", "score": 500 },
            { "name": "Beta", "path": "E:/tools/Beta", "exe": "E:/tools/Beta/Beta.exe", "size": 0, "kind": "file", "score": 100 }
        ]);
        let items: Vec<classify::ClassifyInput> = serde_json::from_value(payload).unwrap();
        let inputs = inputs_from_discovered(items);
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].id, "E:/tools/Alpha");
        assert_eq!(inputs[0].size, 42);
        assert_eq!(inputs[1].id, "E:/tools/Beta");
    }
}
