// In-memory API used by the Playwright E2E suite (VITE_E2E=1).
// Fixtures are synthetic: never put real app names or machine paths here.
import type {
  AiCliCandidate,
  AiConfigView,
  AiSavePayload,
  AiTestResult,
  AppAudit,
  AppEditPatch,
  AppSourceKind,
  AppView,
  Bootstrap,
  Candidate,
  CategoryApply,
  CategorySuggestion,
  Config,
  DiscoverReport,
  Discovered,
  ExportReport,
  ImportItem,
  ImportMode,
  ImportReport,
  LaunchReceipt,
  StartupStatus,
  TagStyle,
  Theme,
  ThemeSummary
} from './types'

const theme: Theme = {
  schemaVersion: 1,
  id: 'pixel-dark',
  name: 'Pixel Dark',
  author: 'test',
  vars: {},
  css: ''
}

const themes: ThemeSummary[] = [
  { id: 'pixel-dark', name: 'Pixel Dark', author: 'test', builtin: true, error: null }
]

let config: Config = {
  schemaVersion: 1,
  theme: 'pixel-dark',
  locale: 'zh-CN',
  importMode: 'link',
  hotkey: 'Ctrl+Alt+P',
  launchAtLogin: false,
  startHiddenAtLogin: false,
  closeToTray: true,
  autostartStaggerMs: 2000,
  appsDir: 'Apps',
  apps: [],
  tagStyles: {},
  pixelIcons: false,
  pixelFont: true,
  discoverRoots: [],
  collections: [],
  favorites: []
}

// ---- AI provider config (REQ-0035) ----
let aiConfig: AiConfigView = {
  provider: 'openai',
  baseUrl: 'https://api.deepseek.com/v1',
  model: 'deepseek-chat',
  hasKey: false,
  keyHint: '',
  consent: false,
  cliPath: '',
  cliVersion: ''
}
let aiCliCandidates: AiCliCandidate[] = [
  {
    provider: 'claude-cli',
    path: 'C:/mock/claude.cmd',
    version: '2.1.296 (Claude Code)',
    source: 'mock'
  }
]
let aiTestOk = true
let aiTestMessage = '连接成功（mock）'

function maskKey(key: string): string {
  if (key.length <= 8) return '****'
  return `${key.slice(0, 4)}…${key.slice(-4)}`
}

function normalizeTags(tags: string[]): string[] {
  const out: string[] = []
  for (const raw of tags) {
    const tag = raw.trim()
    if (!tag || tag.length > 24) continue
    if (out.some((x) => x.toLowerCase() === tag.toLowerCase())) continue
    out.push(tag)
    if (out.length >= 20) break
  }
  return out
}

function makeApp(name: string, index: number): AppView {
  return {
    id: `id-${index}`,
    name,
    path: `Apps/${name}/${name}.exe`,
    args: [],
    workDir: '',
    autostart: false,
    autostartDelayMs: 0,
    skipIfRunning: true,
    hidden: false,
    tags: [],
    category: '',
    runAsAdmin: false,
    exists: true,
    icon: null
  }
}

// REQ-0022: specs that need several source folders seed the app list before the
// bundle loads (`page.addInitScript`); the default fixture stays Alpha/Beta/Gamma
// so app.spec.ts keeps expecting exactly three cards.
const seededApps = (globalThis as { __PH_APPS_SEED__?: AppView[] }).__PH_APPS_SEED__
let apps: AppView[] = Array.isArray(seededApps)
  ? structuredClone(seededApps)
  : ['Alpha', 'Beta', 'Gamma'].map(makeApp)

const defaultDiscovered: Discovered[] = [
  { name: 'Delta', path: 'E:/tools/Delta', exe: 'E:/tools/Delta/Delta.exe', size: 20 * 1024 * 1024, kind: 'folder', score: 800 },
  { name: 'Beta', path: 'E:/tools/Beta', exe: 'E:/tools/Beta/Beta.exe', size: 5 * 1024 * 1024, kind: 'folder', score: 700 }
]
// REQ-0021: specs that need several source directories replace the fixture via
// `__PH_MOCK__.setDiscoverItems`; other specs keep the default pair (same dir).
let discovered: Discovered[] = structuredClone(defaultDiscovered)

// Synthetic candidates for the toolbar "Scan" action (BUG-0008 geometry and
// BUG-0009 add-all tests). The E2E suite can replace them through
// `__PH_MOCK__.setScanCandidates`; the defaults stay disjoint from the
// app/discover fixtures so app.spec.ts card counts are unaffected.
const defaultScanCandidates: Candidate[] = [
  { name: 'Omega', relPath: 'Apps/Omega/Omega.exe', score: 900, hasManifest: false },
  { name: 'Sigma', relPath: 'Apps/Sigma/Sigma.exe', score: 850, hasManifest: true }
]
let scanCandidates: Candidate[] = structuredClone(defaultScanCandidates)

let launches = 0
let cancelled = false
let cancelResolve: (() => void) | null = null

// REQ-0019: E2E can stretch the discover scan so the "running in background"
// state is observable. Default 0 keeps every existing spec instantaneous.
let discoverDelayMs = 0
let discoverCalls = 0

// Cancelling returns a partial result set, matching the real backend: the scan
// stops at its next checkpoint instead of discarding what it already found.
function partialDiscover(): Discovered[] {
  return structuredClone(discovered.slice(0, 1))
}

function waitForDiscoverDelay(ms: number): Promise<void> {
  if (ms <= 0) return Promise.resolve()
  return new Promise<void>((resolve) => {
    let timer: ReturnType<typeof setTimeout>
    const finish = () => {
      clearTimeout(timer)
      if (cancelResolve === finish) cancelResolve = null
      resolve()
    }
    timer = setTimeout(finish, ms)
    cancelResolve = finish
  })
}

// Names the E2E suite can force to fail, so the import report's failure path is
// reachable without a real broken source folder. Lowercased on purpose.
const failNames = new Set<string>()

// ---- migration (REQ-0023) ----
const MOCK_ROOT = 'C:/PortableHub'
let auditOverride: AppAudit[] | null = null
const exportFailures = new Set<string>()
let targetNotEmpty = false

// ---- hotkey recording (BUG-0013) ----
let hotkeySuspended = false
let suspendCalls = 0
let resumeCalls = 0
let hotkeyConflict: string | null = null
let openThemeDirCalls = 0

function auditFromApps(): AppAudit[] {
  return apps.map((a) => {
    const normalized = a.path.replace(/\\/g, '/')
    const inside = normalized.startsWith('Apps/')
    const kind: AppSourceKind = !a.exists ? 'missing' : inside ? 'local' : 'external'
    return { id: a.id, name: a.name, kind, path: a.path, target: a.path }
  })
}

function startupStatus(): StartupStatus {
  return {
    enabled: config.launchAtLogin,
    linkExists: config.launchAtLogin,
    targetOk: config.launchAtLogin,
    linkPath: config.launchAtLogin ? 'Startup/PortableHub.lnk' : '',
    detail: config.launchAtLogin ? 'enabled' : 'disabled'
  }
}

export const mockApi = {
  bootstrap: async (): Promise<Bootstrap> => ({
    root: 'C:/PortableHub',
    appName: 'PortableHub',
    version: 'test',
    config: structuredClone(config),
    warning: null,
    themes: structuredClone(themes),
    startup: startupStatus(),
    webview2: 'test',
    registryAutostart: false,
    startHidden: true
  }),
  listApps: async (): Promise<AppView[]> => structuredClone(apps),
  getIcons: async (ids: string[]): Promise<Record<string, string | null>> => {
    const wanted = new Set(ids)
    const out: Record<string, string | null> = {}
    for (const app of apps) {
      if (wanted.has(app.id)) out[app.id] = app.icon ?? null
    }
    return out
  },
  scanApps: async (): Promise<Candidate[]> => structuredClone(scanCandidates),
  discoverApps: async (_roots: string[]): Promise<DiscoverReport> => {
    discoverCalls++
    // A fresh scan must not inherit the previous cancellation.
    cancelled = false
    await waitForDiscoverDelay(discoverDelayMs)
    const wasCancelled = cancelled
    cancelled = false
    return wasCancelled
      ? { items: partialDiscover(), cancelled: true }
      : { items: structuredClone(discovered), cancelled: false }
  },
  cancelDiscover: async (): Promise<void> => {
    cancelled = true
    // Resolve any in-flight scan delay now: the backend stops at its next
    // checkpoint and reports the partial result set.
    cancelResolve?.()
  },
  listDrives: async (): Promise<string[]> => ['C:', 'D:', 'E:'],
  importApps: async (items: ImportItem[], mode: ImportMode): Promise<ImportReport> => {
    let added = 0
    let skipped = 0
    const outcomes = items.map((item) => {
      if (failNames.has(item.name.toLowerCase())) {
        return { name: item.name, ok: false, message: 'mock import failed' }
      }
      if (apps.some((a) => a.name.toLowerCase() === item.name.toLowerCase())) {
        skipped++
        return { name: item.name, ok: true, message: 'already exists' }
      }
      const created = makeApp(item.name, apps.length)
      apps = [...apps, item.category ? { ...created, category: item.category } : created]
      added++
      return { name: item.name, ok: true, message: mode }
    })
    const failed = outcomes.filter((o) => !o.ok).length
    return { moved: mode === 'move' ? added : 0, added, skipped, failed, items: outcomes }
  },
  addApps: async (candidates: Candidate[]): Promise<AppView[]> => {
    // Mirrors commands::add_apps: skip candidates whose name or normalized path
    // is already registered, so the frontend's added/skipped toast is honest
    // (BUG-0009 AC-B9-4).
    const normalize = (path: string) =>
      path.replace(/\\/g, '/').replace(/\/+/g, '/').toLowerCase()
    const knownNames = new Set(apps.map((a) => a.name.toLowerCase()))
    const knownPaths = new Set(apps.map((a) => normalize(a.path)))
    for (const c of candidates) {
      const name = c.name.toLowerCase()
      const path = normalize(c.relPath)
      if (knownNames.has(name) || knownPaths.has(path)) continue
      knownNames.add(name)
      knownPaths.add(path)
      apps = [...apps, makeApp(c.name, apps.length)]
    }
    return structuredClone(apps)
  },
  updateApp: async (id: string, patch: Partial<AppEditPatch>): Promise<AppView> => {
    apps = apps.map((a) => (a.id === id ? { ...a, ...patch } : a))
    return structuredClone(apps.find((a) => a.id === id)!)
  },
  removeApp: async (id: string): Promise<AppView[]> => {
    apps = apps.filter((a) => a.id !== id)
    return structuredClone(apps)
  },
  launchApp: async (_id: string): Promise<LaunchReceipt> => {
    launches++
    return { pid: 1000 + launches, commandLine: 'mock' }
  },
  revealApp: async (): Promise<void> => {},
  openThemeDir: async (): Promise<void> => {
    openThemeDirCalls++
  },
  listThemes: async (): Promise<ThemeSummary[]> => structuredClone(themes),
  getTheme: async (_id: string): Promise<Theme> => structuredClone(theme),
  setTheme: async (id: string): Promise<void> => {
    config = { ...config, theme: id }
  },
  setLocale: async (locale: string): Promise<void> => {
    config = { ...config, locale }
  },
  setAppsDir: async (path: string): Promise<Config> => {
    config = { ...config, appsDir: path }
    return structuredClone(config)
  },
  setImportMode: async (mode: string): Promise<void> => {
    config = { ...config, importMode: mode }
  },
  setHotkey: async (hotkey: string): Promise<void> => {
    if (hotkeyConflict && hotkey.trim() === hotkeyConflict) {
      throw new Error(`hotkey conflict: ${hotkey}`)
    }
    config = { ...config, hotkey }
    hotkeySuspended = false
  },
  suspendHotkey: async (): Promise<void> => {
    suspendCalls++
    hotkeySuspended = true
  },
  resumeHotkey: async (): Promise<void> => {
    resumeCalls++
    hotkeySuspended = false
  },
  setLaunchAtLogin: async (enabled: boolean): Promise<StartupStatus> => {
    config = { ...config, launchAtLogin: enabled }
    return startupStatus()
  },
  getStartupStatus: async (): Promise<StartupStatus> => startupStatus(),
  setAppAutostart: async (id: string, enabled: boolean): Promise<void> => {
    apps = apps.map((a) => (a.id === id ? { ...a, autostart: enabled } : a))
  },
  setAppTags: async (id: string, tags: string[]): Promise<AppView[]> => {
    const clean = normalizeTags(tags)
    apps = apps.map((a) => (a.id === id ? { ...a, tags: clean } : a))
    return structuredClone(apps)
  },
  setTagStyle: async (tag: string, color: string, icon: string): Promise<Record<string, TagStyle>> => {
    const key = tag.trim()
    const next = { ...config.tagStyles }
    if (!color && !icon) delete next[key]
    else next[key] = { color, icon }
    config = { ...config, tagStyles: next }
    return structuredClone(config.tagStyles)
  },
  setPixelIcons: async (enabled: boolean): Promise<void> => {
    config = { ...config, pixelIcons: enabled }
  },
  setPixelFont: async (enabled: boolean): Promise<void> => {
    config = { ...config, pixelFont: enabled }
  },
  setDiscoverRoots: async (roots: string[]): Promise<string[]> => {
    config = { ...config, discoverRoots: [...roots] }
    return structuredClone(config.discoverRoots)
  },
  saveCollections: async (collections: any[]): Promise<any[]> => {
    const plain = JSON.parse(JSON.stringify(collections))
    config = { ...config, collections: plain }
    return JSON.parse(JSON.stringify(config.collections))
  },
  setFavorites: async (favorites: string[]): Promise<string[]> => {
    config = { ...config, favorites: [...favorites] }
    return structuredClone(config.favorites)
  },
  setCloseToTray: async (enabled: boolean): Promise<void> => {
    config = { ...config, closeToTray: enabled }
  },
  setStartHiddenAtLogin: async (enabled: boolean): Promise<void> => {
    config = { ...config, startHiddenAtLogin: enabled }
  },
  auditApps: async (): Promise<AppAudit[]> => structuredClone(auditOverride ?? auditFromApps()),
  exportBundle: async (target: string): Promise<ExportReport> => {
    const normalizedTarget = target.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase()
    const normalizedRoot = MOCK_ROOT.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase()
    if (!normalizedTarget) throw new Error('target required')
    if (normalizedTarget === normalizedRoot || normalizedTarget.startsWith(`${normalizedRoot}/`)) {
      throw new Error('guard: target inside program folder')
    }
    if (targetNotEmpty) throw new Error('guard: target not empty')
    const items = apps.map((a) => {
      const failed = exportFailures.has(a.name)
      return {
        name: a.name,
        ok: !failed,
        from: a.path,
        to: `${target}/Apps/${a.name}`,
        message: failed ? 'mock export failed' : 'copied'
      }
    })
    const failed = items.filter((i) => !i.ok).length
    return {
      ok: failed === 0,
      target,
      total: items.length,
      copied: items.length - failed,
      failed,
      items,
      reportPath: `${target}/migration-report.json`
    }
  },
  runAutostartNow: async () => [],
  showMainWindow: async (): Promise<void> => {},
  hideMainWindow: async (): Promise<void> => {},
  aiGetConfig: async (): Promise<AiConfigView> => structuredClone(aiConfig),
  aiSaveConfig: async (payload: AiSavePayload): Promise<AiConfigView> => {
    const apiKey = payload.apiKey?.trim()
    const hasKey = payload.clearKey ? false : !!apiKey || aiConfig.hasKey
    aiConfig = {
      provider: payload.provider,
      baseUrl: payload.baseUrl.trim(),
      model: payload.model.trim(),
      hasKey,
      keyHint: apiKey ? maskKey(apiKey) : payload.clearKey ? '' : aiConfig.keyHint,
      consent: aiConfig.consent,
      cliPath: payload.cliPath ?? aiConfig.cliPath,
      cliVersion: aiConfig.cliVersion
    }
    return structuredClone(aiConfig)
  },
  aiDetectClis: async (): Promise<AiCliCandidate[]> => structuredClone(aiCliCandidates),
  classifyApps: async (): Promise<CategorySuggestion[]> => {
    const categories: CategorySuggestion['category'][] = ['dev', 'media', 'game']
    return apps.map((app, index) => ({
      id: app.id,
      category: categories[index % categories.length],
      confidence: 0.92,
      reason: 'mock rule match',
      source: 'rules'
    }))
  },
  classifyDiscovered: async (items: Discovered[]): Promise<CategorySuggestion[]> => {
    const categories: CategorySuggestion['category'][] = ['dev', 'media', 'game', 'network']
    return items.map((item, index) => ({
      id: item.path,
      category: categories[index % categories.length],
      confidence: 0.9,
      reason: 'mock AI suggestion',
      source: 'ai'
    }))
  },
  applyCategories: async (items: CategoryApply[]): Promise<AppView[]> => {
    const byId = new Map(items.map((item) => [item.id, item.category]))
    apps = apps.map((app) => (byId.has(app.id) ? { ...app, category: byId.get(app.id)! } : app))
    return structuredClone(apps)
  },
  aiBindCli: async (
    provider: 'claude-cli' | 'codex-cli',
    path: string
  ): Promise<AiConfigView> => {
    const candidate = aiCliCandidates.find((c) => c.provider === provider && c.path === path)
    if (!candidate) throw new Error('CLI not found')
    aiConfig = {
      ...aiConfig,
      provider,
      cliPath: candidate.path,
      cliVersion: candidate.version
    }
    return structuredClone(aiConfig)
  },
  aiTestConnection: async (): Promise<AiTestResult> => ({
    ok: aiTestOk,
    message: aiTestMessage
  }),
  aiSetConsent: async (enabled: boolean): Promise<AiConfigView> => {
    aiConfig = { ...aiConfig, consent: enabled }
    return structuredClone(aiConfig)
  },
  quit: async (): Promise<void> => {}
}

;(globalThis as unknown as Record<string, unknown>).__PH_MOCK__ = {
  get apps() {
    return apps
  },
  get launches() {
    return launches
  },
  get config() {
    return config
  },
  get failNames() {
    return failNames
  },
  /** REQ-0019: how long the mock discover scan takes (ms). Default 0. */
  get discoverDelayMs() {
    return discoverDelayMs
  },
  set discoverDelayMs(value: number) {
    discoverDelayMs = Number.isFinite(value) && value > 0 ? value : 0
  },
  /** How many discover scans have been started; lets E2E prove no re-scan. */
  get discoverCalls() {
    return discoverCalls
  },
  /** BUG-0008 AC-B8-4: replace the candidates returned by the next scanApps(). */
  setScanCandidates(list: Candidate[]) {
    scanCandidates = structuredClone(list)
  },
  /** REQ-0022: replace the app list (multi-source grouping E2E fixtures). */
  setApps(list: AppView[]) {
    apps = structuredClone(list)
  },
  /** REQ-0021: replace the items returned by discoverApps() (grouping E2E). */
  setDiscoverItems(list: Discovered[]) {
    discovered = structuredClone(list)
  },
  /** Restore the default two-item discover fixture. */
  resetDiscoverItems() {
    discovered = structuredClone(defaultDiscovered)
  },
  /** Force the next imports of `name` to fail (case-insensitive). */
  failOn(name: string) {
    failNames.add(name.toLowerCase())
  },
  /** Clear all forced failures. */
  clearFailures() {
    failNames.clear()
  },
  /** REQ-0023: replace the audit rows shown by the migration dialog. */
  setAudit(list: AppAudit[]) {
    auditOverride = structuredClone(list)
  },
  /** REQ-0023: names whose export row must fail. */
  setExportFailures(names: string[]) {
    exportFailures.clear()
    for (const name of names) exportFailures.add(name)
  },
  /** REQ-0023: simulate a non-empty export target (guard test). */
  setTargetNotEmpty(value: boolean) {
    targetNotEmpty = !!value
  },
  /** BUG-0013: is the global hotkey currently suspended for recording? */
  get hotkeySuspended() {
    return hotkeySuspended
  },
  get suspendCalls() {
    return suspendCalls
  },
  get resumeCalls() {
    return resumeCalls
  },
  /** BUG-0013: make setHotkey reject this accelerator to test conflict handling. */
  setHotkeyConflict(accel: string | null) {
    hotkeyConflict = accel
  },
  /** REQ: theme hint row opens the folder instead of printing a path. */
  get openThemeDirCalls() {
    return openThemeDirCalls
  },
  /** REQ-0035: current AI config (masked key) for assertions. */
  get aiConfig() {
    return structuredClone(aiConfig)
  },
  /** REQ-0035: force the connection-test result. */
  setAiTestResult(ok: boolean, message: string) {
    aiTestOk = ok
    aiTestMessage = message
  }
}
