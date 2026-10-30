import { invoke } from '@tauri-apps/api/core'
import { mockApi } from './api-mock'
import type {
  AiCliCandidate,
  AiConfigView,
  AiSavePayload,
  AiTestResult,
  AppAudit,
  AppEditPatch,
  AppView,
  Bootstrap,
  Candidate,
  CategoryApply,
  CategorySuggestion,
  Config,
  Collection,
  DiscoverReport,
  Discovered,
  ExportReport,
  TagStyle,
  ImportItem,
  ImportMode,
  ImportReport,
  LaunchReceipt,
  LaunchReport,
  StartupStatus,
  Theme,
  ThemeSummary
} from './types'

const realApi = {
  bootstrap: () => invoke<Bootstrap>('bootstrap'),
  listApps: () => invoke<AppView[]>('list_apps'),
  getIcons: (ids: string[]) => invoke<Record<string, string | null>>('get_icons', { ids }),
  scanApps: () => invoke<Candidate[]>('scan_apps'),
  discoverApps: (roots: string[]) => invoke<DiscoverReport>('discover_apps', { roots }),
  cancelDiscover: () => invoke<void>('cancel_discover'),
  listDrives: () => invoke<string[]>('list_drives'),
  importApps: (items: ImportItem[], mode: ImportMode) =>
    invoke<ImportReport>('import_apps', { items, mode }),
  addApps: (candidates: Candidate[]) => invoke<AppView[]>('add_apps', { candidates }),
  updateApp: (id: string, patch: Partial<AppEditPatch>) =>
    invoke<AppView>('update_app', { id, patch }),
  removeApp: (id: string) => invoke<AppView[]>('remove_app', { id }),
  launchApp: (id: string) => invoke<LaunchReceipt>('launch_app', { id }),
  revealApp: (id: string) => invoke<void>('reveal_app', { id }),
  openThemeDir: () => invoke<void>('open_theme_dir'),
  listThemes: () => invoke<ThemeSummary[]>('list_themes'),
  getTheme: (id: string) => invoke<Theme>('get_theme', { id }),
  setTheme: (id: string) => invoke<void>('set_theme', { id }),
  setLocale: (locale: string) => invoke<void>('set_locale', { locale }),
  setImportMode: (mode: string) => invoke<void>('set_import_mode', { mode }),
  setAppsDir: (path: string) => invoke<Config>('set_apps_dir', { path }),
  setHotkey: (accel: string) => invoke<void>('set_hotkey', { accel }),
  setPixelIcons: (enabled: boolean) => invoke<void>('set_pixel_icons', { enabled }),
  setPixelFont: (enabled: boolean) => invoke<void>('set_pixel_font', { enabled }),
  setDiscoverRoots: (roots: string[]) =>
    invoke<string[]>('set_discover_roots', { roots }),
  saveCollections: (collections: Collection[]) =>
    invoke<Collection[]>('save_collections', { collections }),
  setFavorites: (favorites: string[]) =>
    invoke<string[]>('set_favorites', { favorites }),
  suspendHotkey: () => invoke<void>('suspend_hotkey'),
  resumeHotkey: () => invoke<void>('resume_hotkey'),
  setLaunchAtLogin: (enabled: boolean) =>
    invoke<StartupStatus>('set_launch_at_login', { enabled }),
  getStartupStatus: () => invoke<StartupStatus>('get_startup_status'),
  setAppAutostart: (id: string, enabled: boolean) =>
    invoke<void>('set_app_autostart', { id, enabled }),
  setAppTags: (id: string, tags: string[]) =>
    invoke<AppView[]>('set_app_tags', { id, tags }),
  setTagStyle: (tag: string, color: string, icon: string) =>
    invoke<Record<string, TagStyle>>('set_tag_style', { tag, color, icon }),
  setCloseToTray: (enabled: boolean) => invoke<void>('set_close_to_tray', { enabled }),
  setStartHiddenAtLogin: (enabled: boolean) =>
    invoke<void>('set_start_hidden_at_login', { enabled }),
  auditApps: () => invoke<AppAudit[]>('audit_apps'),
  exportBundle: (target: string) => invoke<ExportReport>('export_bundle', { target }),
  runAutostartNow: () => invoke<LaunchReport[]>('run_autostart_now'),
  showMainWindow: () => invoke<void>('show_main_window'),
  hideMainWindow: () => invoke<void>('hide_main_window'),
  // ---- AI providers (REQ-0035) ----
  aiGetConfig: () => invoke<AiConfigView>('ai_get_config'),
  aiSaveConfig: (payload: AiSavePayload) => invoke<AiConfigView>('ai_save_config', { payload }),
  aiTestConnection: () => invoke<AiTestResult>('ai_test_connection'),
  aiSetConsent: (enabled: boolean) => invoke<AiConfigView>('ai_set_consent', { enabled }),
  aiDetectClis: () => invoke<AiCliCandidate[]>('ai_detect_clis'),
  aiBindCli: (provider: 'claude-cli' | 'codex-cli', path: string) =>
    invoke<AiConfigView>('ai_bind_cli', { provider, path }),
  classifyApps: () => invoke<CategorySuggestion[]>('classify_apps'),
  classifyDiscovered: (items: Discovered[]) =>
    invoke<CategorySuggestion[]>('classify_discovered', { items }),
  applyCategories: (items: CategoryApply[]) => invoke<AppView[]>('apply_categories', { items }),
  quit: () => invoke<void>('quit_app')
}

export const api = import.meta.env.VITE_E2E === "1" ? mockApi : realApi
