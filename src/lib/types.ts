export interface AppEntry {
  id: string
  name: string
  path: string
  args: string[]
  workDir: string
  autostart: boolean
  autostartDelayMs: number
  skipIfRunning: boolean
  hidden: boolean
  tags: string[]
  /** Semantic primary category (REQ-0039); empty = unclassified. */
  category: string
  /** Launch elevated via UAC (BUG-0023). */
  runAsAdmin: boolean
}

export interface AppView extends AppEntry {
  exists: boolean
  icon: string | null
}

/** Fields the edit dialog can change via the `update_app` command. */
export interface AppEditPatch {
  name: string
  path: string
  args: string[]
  workDir: string
  runAsAdmin?: boolean
  category?: string
}

export interface Config {
  schemaVersion: number
  theme: string
  locale: string
  importMode: string
  hotkey: string
  launchAtLogin: boolean
  startHiddenAtLogin: boolean
  closeToTray: boolean
  autostartStaggerMs: number
  appsDir: string
  apps: AppEntry[]
  /** Global style per tag: color (#rrggbb) and optional emoji/icon. */
  tagStyles: Record<string, TagStyle>
  /** Keep app icons pixelated (user preference, independent of the theme). */
  pixelIcons: boolean
  /** Use the pixel font across the UI. */
  pixelFont: boolean
  /** Last directories selected in the Discover scope. */
  discoverRoots: string[]
  collections: Collection[]
  favorites: string[]
}

export interface Collection {
  id: string
  name: string
  appIds: string[]
}

export interface TagStyle {
  color: string
  icon: string
}

export interface Theme {
  schemaVersion: number
  id: string
  name: string
  author: string
  vars: Record<string, string>
  css?: string | null
}

export interface ThemeSummary {
  id: string
  name: string
  author: string
  builtin: boolean
  error: string | null
}

export interface StartupStatus {
  enabled: boolean
  linkExists: boolean
  targetOk: boolean
  linkPath: string
  detail: string
}

export interface Candidate {
  name: string
  relPath: string
  score: number
  hasManifest: boolean
}

export interface Bootstrap {
  root: string
  appName: string
  version: string
  config: Config
  warning: string | null
  themes: ThemeSummary[]
  startup: StartupStatus
  webview2: string | null
  registryAutostart: boolean
  startHidden: boolean
}

export interface LaunchReceipt {
  pid: number
  commandLine: string
  /** True when the app was started through UAC (no pid is available). */
  elevated?: boolean
}

export interface LaunchReport {
  name: string
  ok: boolean
  message: string
}

export interface Discovered {
  name: string
  path: string
  exe: string
  size: number
  kind: 'folder' | 'file'
  score: number
  /** REQ-0041: semantic suggestion attached after AI/rules classification. */
  category?: string
  /** UI-only: how many additional same-name locations were merged away. */
  duplicateCount?: number
}

export type ImportMode = 'link' | 'move' | 'register'

export interface DiscoverReport {
  items: Discovered[]
  cancelled: boolean
}

export interface ImportItem {
  path: string
  name: string
  kind: string
  /** Optional semantic category to store with the imported app. */
  category?: string
}

export interface ImportOutcome {
  name: string
  ok: boolean
  message: string
}

export interface ImportReport {
  moved: number
  added: number
  skipped: number
  failed: number
  items: ImportOutcome[]
}

// ---- migration (REQ-0023) ----

export type AppSourceKind = 'local' | 'linked' | 'external' | 'missing'

export interface AppAudit {
  id: string
  name: string
  kind: AppSourceKind
  path: string
  target: string
}

export interface ExportItem {
  name: string
  ok: boolean
  from: string
  to: string
  message: string
}

export interface ExportReport {
  ok: boolean
  target: string
  total: number
  copied: number
  failed: number
  items: ExportItem[]
  reportPath: string
}

// ---- AI providers (REQ-0035 / REQ-0038) ----

export type AiProvider = 'anthropic' | 'openai' | 'claude-cli' | 'codex-cli'

/** Never contains the raw key: the backend returns a masked hint only. */
export interface AiConfigView {
  provider: AiProvider
  baseUrl: string
  model: string
  hasKey: boolean
  keyHint: string
  consent: boolean
  /** Bound CLI executable (empty for API providers). */
  cliPath: string
  /** Version reported by the bound CLI, for display only. */
  cliVersion: string
}

export interface AiCliCandidate {
  provider: 'claude-cli' | 'codex-cli'
  path: string
  version: string
  source: string
}

export interface AiSavePayload {
  provider: AiProvider
  baseUrl: string
  model: string
  /** Omit/empty keeps the stored key; set only when the user types a new one. */
  apiKey?: string
  clearKey?: boolean
  cliPath?: string
}

export interface AiTestResult {
  ok: boolean
  message: string
}

// ---- semantic classification (REQ-0039) ----

export type AppCategory =
  | 'system'
  | 'network'
  | 'dev'
  | 'game'
  | 'media'
  | 'document'
  | 'security'
  | 'storage'
  | 'comm'
  | 'runtime'
  | 'other'

export interface CategorySuggestion {
  id: string
  category: AppCategory
  confidence: number
  reason: string
  source: 'rules' | 'ai'
}

export interface CategoryApply {
  id: string
  category: AppCategory
}

