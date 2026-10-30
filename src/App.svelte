<script lang="ts">
  import { onMount } from 'svelte'
  import TitleBar from './components/TitleBar.svelte'
  import AppCard from './components/AppCard.svelte'
  import CollectionModal from './components/CollectionModal.svelte'
  import CategoryPicker from './components/CategoryPicker.svelte'
  import ContextMenu from './components/ContextMenu.svelte'
  import QuickTag from './components/QuickTag.svelte'
  import EditModal from './components/EditModal.svelte'
  import TagEditor from './components/TagEditor.svelte'
  import TagManager from './components/TagManager.svelte'
  import SmartOrganize from './components/SmartOrganize.svelte'
  import GroupSidebar from './components/GroupSidebar.svelte'
  import InspectorPanel from './components/InspectorPanel.svelte'
  import Icon from './components/Icon.svelte'
  import Settings from './components/Settings.svelte'
  import DiscoverModal from './components/DiscoverModal.svelte'
  import { api } from './lib/api'
  import { groupApps } from './lib/app-groups'
  import { listenSafe } from './lib/desktop'
  import { filterApps, type AutostartFilter } from './lib/filter'
  import { collectTags } from './lib/tags'
  import { tagCss, tagText } from './lib/tag-style'
  import { PAGE_SIZE, nextPage, pageSlice } from './lib/pagination'
  import { makeT } from './lib/i18n'
  import { displayPath } from './lib/path'
  import { applyPixelFont, applyPixelIcons, applyTheme } from './lib/theme'
  import type {
    AppEditPatch,
    AppView,
    Bootstrap,
    Candidate,
    Collection,
    Discovered,
    ImportMode,
    ImportOutcome,
    ImportReport,
    StartupStatus,
    TagStyle,
    ThemeSummary
  } from './lib/types'

  // REQ-0039: grouping can be driven by source folder, tag or semantic category.
  type GroupMode = 'off' | 'source' | 'tag' | 'smart' | 'collection'

  interface UiGroup {
    key: string
    label: string
    hint: string
    apps: AppView[]
  }

  // Fixed category order shared by the smart groups and their sidebar entries.
  const CATEGORY_ORDER = [
    'system',
    'network',
    'dev',
    'game',
    'media',
    'document',
    'security',
    'storage',
    'comm',
    'runtime',
    'other'
  ] as const

  const UNCLASSIFIED_KEY = 'unclassified'

  let boot = $state<Bootstrap | null>(null)
  let apps = $state<AppView[]>([])
  let selectedId = $state<string | null>(null)
  // Multi-select lives alongside the keyboard cursor (selectedId). Ctrl/Shift
  // click, Shift+Arrow and Ctrl+A grow selectedIds; selectedId stays the anchor
  // for keyboard navigation and single-app actions.
  let selectedIds = $state<string[]>([])
  let selectionAnchor = $state<string | null>(null)
  let themes = $state<ThemeSummary[]>([])
  let startup = $state<StartupStatus>({
    enabled: false,
    linkExists: false,
    targetOk: false,
    linkPath: '',
    detail: ''
  })
  let candidates = $state<Candidate[]>([])
  let scanning = $state(false)
  let query = $state('')
  let activeGroup = $state('all')
  // REQ-0029: grouping is opt-in. REQ-0039 adds the mode behind the toggle.
  let groupMode = $state<GroupMode>('off')
  let smartEditor = $state(false)
  let showSettings = $state(false)
  let showDiscover = $state(false)
  let pixelIcons = $state(false)
  let pixelFont = $state(true)
  let discoverRoots = $state<string[]>([])
  let tagManager = $state(false)
  let tagStyles = $state<Record<string, TagStyle>>({})
  let collections = $state<Collection[]>([])
  let collectionModal = $state<{ apps: AppView[] } | null>(null)
  let categoryPicker = $state<AppView[] | null>(null)
  let editing = $state<AppView | null>(null)
  let importReport = $state<ImportReport | null>(null)
  let discovering = $state(false)
  let importing = $state(false)
  let importProgress = $state('')
  let discovered = $state<Discovered[]>([])
  // True when the last discover scan was cancelled but kept partial results.
  let scanCancelled = $state(false)
  let drives = $state<string[]>([])
  let closeToTray = $state(true)
  let startHiddenAtLogin = $state(false)
  let hotkeyDraft = $state('')
  let locale = $state('zh-CN')
  let toast = $state<{ text: string; error: boolean } | null>(null)
  let menu = $state<{ x: number; y: number; app: AppView } | null>(null)
  let quickTag = $state<{
    apps: AppView[]
    anchor: { x: number; y: number; bottom: number }
  } | null>(null)
  // REQ-0032: tag editing and tag filtering. Tags can be included (AND) or
  // excluded; the filter scopes to the active group when one is selected.
  let activeTags = $state<string[]>([])
  let excludedTags = $state<string[]>([])
  let autostartFilter = $state<AutostartFilter>('off')
  let tagEditor = $state<AppView | null>(null)
  let tagFilterOpen = $state(false)
  // REQ-0042: bounded render window. At most PAGE_SIZE cards are mounted per
  // grid / group section; "show more" grows that section's window.
  let flatShown = $state(PAGE_SIZE)
  let groupShown = $state<Record<string, number>>({})
  let searchEl: HTMLInputElement | undefined = $state()

  let t = $derived(makeT(locale))
  let grouped = $derived(groupMode !== 'off')
  // REQ-0022: source groups are derived from the stored app paths.
  let appGroups = $derived(groupApps(apps))
  // User-curated groups: flat collections of app ids, ordered as stored.
  let collectionGroups = $derived<UiGroup[]>(
    collections.map((collection) => ({
      key: `collection:${collection.id}`,
      label: collection.name,
      hint: t('collections.manage'),
      apps: collection.appIds
        .map((id) => apps.find((app) => app.id === id))
        .filter((app): app is AppView => Boolean(app))
    }))
  )
  // REQ-0039: the active mode decides which group list the sidebar and grid use.
  let modeGroups = $derived<UiGroup[]>(
    groupMode === 'smart'
      ? smartGroupsFor(apps)
      : groupMode === 'tag'
        ? tagGroupsFor(apps)
        : groupMode === 'collection'
          ? collectionGroups
          : appGroups
  )
  let sidebarGroups = $derived(
    modeGroups.map((g) => ({ key: g.key, label: g.label, hint: g.hint, count: g.apps.length }))
  )
  // Groups visible under the active navigation entry, filtered by the search box.
  // When grouping is off the whole list is one implicit group (flat grid).
  let visibleGroups = $derived(
    grouped
      ? modeGroups
          .filter((g) => activeGroup === 'all' || g.key === activeGroup)
          .map((g) => ({ ...g, apps: filterApps(g.apps, query, activeTags, excludedTags, autostartFilter) }))
          .filter((g) => g.apps.length > 0)
      : [
          {
            key: 'all',
            label: '',
            hint: '',
            apps: filterApps(apps, query, activeTags, excludedTags, autostartFilter)
          }
        ].filter((g) => g.apps.length > 0)
  )
  let filtered = $derived(visibleGroups.flatMap((g) => g.apps))
  // REQ-0042: only this slice is mounted in the flat grid.
  let flatVisible = $derived(pageSlice(filtered, flatShown))
  // Icons are loaded lazily for the cards that are actually mounted. `list_apps`
  // stays cheap for large libraries; a slow icon extraction never blocks the UI.
  let iconCandidates = $derived<AppView[]>(
    grouped && activeGroup === 'all'
      ? visibleGroups.flatMap((group) =>
          group.apps.slice(0, groupShown[group.key] ?? PAGE_SIZE)
        )
      : flatVisible
  )
  let requestedIcons = new Set<string>()

  async function ensureIcons(list: AppView[]) {
    const need = [...new Set(list.map((app) => app.id))].filter(
      (id) => !requestedIcons.has(id)
    )
    if (!need.length) return
    for (const id of need) requestedIcons.add(id)
    try {
      const icons = await api.getIcons(need)
      apps = apps.map((app) =>
        Object.prototype.hasOwnProperty.call(icons, app.id)
          ? { ...app, icon: icons[app.id] }
          : app
      )
    } catch {
      for (const id of need) requestedIcons.delete(id)
    }
  }

  $effect(() => {
    void ensureIcons(iconCandidates)
  })
  // REQ-0032: tags offered by the filter belong to the current group, so a
  // "game" group only shows game tags (e.g. played / backlog / dropped).
  let tagScopeApps = $derived.by(() => {
    if (!grouped || activeGroup === 'all') return apps
    return modeGroups.find((group) => group.key === activeGroup)?.apps ?? apps
  })
  // Every tag in use, most-used first; the toolbar entry only exists once an
  // app carries a tag.
  let appTags = $derived(collectTags(tagScopeApps))
  // Virtual built-in filter: apps with "start when hub starts" enabled.
  let autostartCount = $derived(tagScopeApps.filter((app) => app.autostart).length)
  let tagFilterActiveCount = $derived(
    activeTags.length + excludedTags.length + (autostartFilter === 'off' ? 0 : 1)
  )
  // Quick-tag menu/popover is not group-scoped: it offers every known tag.
  let allTagOptions = $derived(collectTags(apps))

  /** Case-insensitive name order inside one group. */
  function byName(a: AppView, b: AppView): number {
    return a.name.localeCompare(b.name, 'zh-CN')
  }

  /** REQ-0039: group by semantic category; fixed order, unclassified last. */
  function smartGroupsFor(list: AppView[]): UiGroup[] {
    const buckets = new Map<string, AppView[]>()
    for (const app of list) {
      const key = (app.category ?? '').trim() || UNCLASSIFIED_KEY
      const bucket = buckets.get(key)
      if (bucket) bucket.push(app)
      else buckets.set(key, [app])
    }
    const rank = (key: string): number => {
      if (key === UNCLASSIFIED_KEY) return CATEGORY_ORDER.length + 1
      const index = CATEGORY_ORDER.indexOf(key as (typeof CATEGORY_ORDER)[number])
      return index < 0 ? CATEGORY_ORDER.length : index
    }
    return [...buckets.entries()]
      .sort((a, b) => rank(a[0]) - rank(b[0]) || a[0].localeCompare(b[0], 'zh-CN'))
      .map(([key, bucketApps]) => ({
        key,
        label: key === UNCLASSIFIED_KEY ? t('category.unclassified') : t(`category.${key}`),
        hint: key === UNCLASSIFIED_KEY ? '' : t(`category.${key}`),
        apps: [...bucketApps].sort(byName)
      }))
  }

  /** REQ-0032/0039: group by the first tag; untagged apps go to `unclassified`. */
  function tagGroupsFor(list: AppView[]): UiGroup[] {
    const buckets = new Map<string, AppView[]>()
    for (const app of list) {
      const tag = (app.tags ?? []).find((item) => item.trim().length > 0) ?? UNCLASSIFIED_KEY
      const bucket = buckets.get(tag)
      if (bucket) bucket.push(app)
      else buckets.set(tag, [app])
    }
    return [...buckets.entries()]
      .sort((a, b) => {
        const aUn = a[0] === UNCLASSIFIED_KEY
        const bUn = b[0] === UNCLASSIFIED_KEY
        if (aUn || bUn) return aUn === bUn ? 0 : aUn ? 1 : -1
        return b[1].length - a[1].length || a[0].localeCompare(b[0], 'zh-CN')
      })
      .map(([key, bucketApps]) => ({
        key,
        label: key === UNCLASSIFIED_KEY ? t('category.unclassified') : key,
        hint: key === UNCLASSIFIED_KEY ? '' : key,
        apps: [...bucketApps].sort(byName)
      }))
  }

  function persistGroupMode(mode: GroupMode) {
    try {
      localStorage.setItem('ph.groupMode', mode)
    } catch {
      /* UI preference only; ignore storage failures */
    }
  }

  function setGroupMode(mode: GroupMode) {
    groupMode = mode
    if (mode === 'off') activeGroup = 'all'
    persistGroupMode(mode)
    if (mode === 'collection' && collections.length === 0) {
      collectionModal = { apps: [] }
    }
  }

  function toggleGrouping() {
    if (groupMode !== 'off') {
      setGroupMode('off')
      return
    }
    // Smart mode is the useful default once categories exist; source otherwise,
    // so pre-classification behaviour (and main-groups e2e) is unchanged.
    setGroupMode(apps.some((app) => (app.category ?? '').trim().length > 0) ? 'smart' : 'source')
  }

  /** Smart organize finished: adopt the new categories and switch to smart mode. */
  function applySmartCategories(updated: AppView[]) {
    if (!smartEditor) return
    const before = new Map(apps.map((app) => [app.id, app.category ?? '']))
    const changed = updated.filter(
      (app) => (app.category ?? '') !== (before.get(app.id) ?? '')
    ).length
    apps = updated
    smartEditor = false
    activeGroup = 'all'
    setGroupMode('smart')
    say(t('smart.applied', { n: changed }))
  }

  type TagFilterState = 'off' | 'include' | 'exclude'

  function tagFilterState(tag: string): TagFilterState {
    if (excludedTags.includes(tag)) return 'exclude'
    return activeTags.includes(tag) ? 'include' : 'off'
  }

  /** off → include → exclude → off */
  function cycleTagFilter(tag: string) {
    const state = tagFilterState(tag)
    if (state === 'off') {
      activeTags = [...activeTags, tag]
    } else if (state === 'include') {
      activeTags = activeTags.filter((item) => item !== tag)
      excludedTags = [...excludedTags, tag]
    } else {
      excludedTags = excludedTags.filter((item) => item !== tag)
    }
  }

  function clearTagFilter() {
    activeTags = []
    excludedTags = []
    autostartFilter = 'off'
  }

  function cycleAutostartFilter() {
    autostartFilter =
      autostartFilter === 'off' ? 'include' : autostartFilter === 'include' ? 'exclude' : 'off'
  }

  async function saveTags(tags: string[]) {
    if (!tagEditor) return
    const target = tagEditor
    try {
      apps = await api.setAppTags(target.id, tags)
      tagEditor = null
      say(t('edit.saved', { name: target.name }))
    } catch (e) {
      say(String(e), true)
    }
  }

  // One-click tagging from the context menu / quick-tag popover: write
  // immediately instead of opening the full editor and pressing Save.
  function quickTargets(): AppView[] {
    if (quickTag) {
      return quickTag.apps.map((app) => apps.find((item) => item.id === app.id) ?? app)
    }
    const target = menu?.app
    if (!target) return []
    if (selectedIds.length > 1 && selectedIds.includes(target.id)) {
      return apps.filter((app) => selectedIds.includes(app.id))
    }
    return [apps.find((app) => app.id === target.id) ?? target]
  }

  async function applyQuickTag(tag: string, mode: 'toggle' | 'add') {
    const targets = quickTargets()
    if (!targets.length) return
    const remove = mode === 'toggle' && targets.every((app) => app.tags.includes(tag))
    try {
      for (const target of targets) {
        const current = apps.find((app) => app.id === target.id) ?? target
        const has = current.tags.includes(tag)
        if (mode === 'add' && has) continue
        const next = remove
          ? current.tags.filter((item) => item !== tag)
          : has
            ? current.tags
            : [...current.tags, tag]
        apps = await api.setAppTags(target.id, next)
      }
      if (quickTag) {
        quickTag = {
          ...quickTag,
          apps: quickTag.apps.map((app) => apps.find((item) => item.id === app.id) ?? app)
        }
      }
    } catch (e) {
      say(String(e), true)
    }
  }

  async function toggleQuickTag(tag: string) {
    await applyQuickTag(tag, 'toggle')
  }

  async function addQuickTag(tag: string) {
    await applyQuickTag(tag, 'add')
  }

  async function saveTagStyle(tag: string, color: string, icon: string) {
    tagStyles = await api.setTagStyle(tag, color, icon)
  }

  function openQuickTag(app: AppView, anchor: { x: number; y: number; bottom: number }) {
    quickTag = { apps: [app], anchor }
  }

  function openBatchTag(event?: MouseEvent) {
    if (!selectedIds.length) return
    const targets = apps.filter((app) => selectedIds.includes(app.id))
    if (!targets.length) return
    const rect = event ? (event.currentTarget as HTMLElement).getBoundingClientRect() : null
    quickTag = {
      apps: targets,
      anchor: rect
        ? { x: rect.left, y: rect.bottom, bottom: rect.bottom }
        : { x: Math.max(8, window.innerWidth - 580), y: 120, bottom: 120 }
    }
  }

  function openBatchTagFromInspector() {
    openBatchTag()
  }

  function selectRailGroup(key: string) {
    if (!grouped) setGroupMode('source')
    activeGroup = key
  }

  async function applyCategory(category: string) {
    const targets = categoryPicker
    if (!targets?.length) return
    try {
      let next = apps
      for (const target of targets) {
        const updated = await api.updateApp(target.id, { category })
        next = next.map((app) => (app.id === updated.id ? updated : app))
      }
      apps = next
      categoryPicker = null
      say(t('categoryPicker.applied', { n: targets.length }))
    } catch (e) {
      say(String(e), true)
    }
  }

  async function saveCollections(list: Collection[]) {
    try {
      collections = await api.saveCollections(list)
      if (boot) boot.config.collections = JSON.parse(JSON.stringify(collections))
      collectionModal = null
      if (groupMode === 'collection' && !collections.some((item) => `collection:${item.id}` === activeGroup)) {
        activeGroup = 'all'
      }
      say(t('collections.saved', { n: collections.length }))
    } catch (e) {
      say(String(e), true)
    }
  }

  // Keep the navigation valid when the selected group disappears (e.g. the last
  // app of a group was removed, retagged, or the grouping mode changed).
  $effect(() => {
    if (activeGroup !== 'all' && !modeGroups.some((g) => g.key === activeGroup)) {
      activeGroup = 'all'
    }
  })

  // Drop selected tag filters that no longer exist (e.g. the last app carrying
  // that tag was removed or retagged), so the grid cannot get stuck empty.
  $effect(() => {
    if (!activeTags.length && !excludedTags.length) return
    const known = new Set(appTags.map((item) => item.tag))
    const nextInclude = activeTags.filter((tag) => known.has(tag))
    const nextExclude = excludedTags.filter((tag) => known.has(tag))
    if (nextInclude.length !== activeTags.length) activeTags = nextInclude
    if (nextExclude.length !== excludedTags.length) excludedTags = nextExclude
  })

  // REQ-0042: a new search / tag filter / grouping mode / nav entry starts from
  // the first page again. This effect reads only the context and writes the
  // render limits, so it can never re-trigger itself.
  $effect(() => {
    const context = `${query}\u0000${activeTags.join('\u0000')}\u0000${excludedTags.join('\u0000')}\u0000${autostartFilter}\u0000${groupMode}\u0000${activeGroup}`
    void context
    flatShown = PAGE_SIZE
    groupShown = {}
  })

  // A filter/search/group change can hide selected apps; drop them from the
  // selection so the batch action never touches invisible cards.
  $effect(() => {
    if (!selectedIds.length) return
    const visible = new Set(filtered.map((app) => app.id))
    const next = selectedIds.filter((id) => visible.has(id))
    if (next.length !== selectedIds.length) {
      selectedIds = next
      if (!selectedId || !visible.has(selectedId)) selectedId = next.at(-1) ?? null
      if (!selectionAnchor || !visible.has(selectionAnchor)) selectionAnchor = next.at(-1) ?? null
    }
  })
  let toastTimer: number | undefined
  // Generation token so a cancelled / superseded discover scan can never
  // overwrite the state of a newer one.
  let scanToken = 0

  function say(text: string, error = false) {
    toast = { text, error }
    clearTimeout(toastTimer)
    toastTimer = window.setTimeout(() => (toast = null), 3200)
  }

  async function loadTheme(id: string) {
    try {
      applyTheme(await api.getTheme(id))
    } catch (e) {
      // A theme can disappear (e.g. the removed Game Boy built-in). Fall back to
      // the default instead of leaving the app with no theme.
      if (id !== 'pixel-dark') {
        try {
          applyTheme(await api.getTheme('pixel-dark'))
          if (boot) boot.config.theme = 'pixel-dark'
          await api.setTheme('pixel-dark')
          return
        } catch {
          // fall through to the readable error below
        }
      }
      say(t('app.themeFailed', { error: String(e) }), true)
    }
  }

  async function refresh() {
    requestedIcons = new Set()
    apps = await api.listApps()
  }

  onMount(async () => {
    try {
      const storedMode = localStorage.getItem('ph.groupMode')
      if (
        storedMode === 'source' ||
        storedMode === 'tag' ||
        storedMode === 'smart' ||
        storedMode === 'collection'
      ) {
        groupMode = storedMode
      } else if (localStorage.getItem('ph.grouped') === '1') {
        // Backward compatibility with the pre-REQ-0039 boolean preference.
        groupMode = 'source'
      }
    } catch {
      groupMode = 'off'
    }
    try {
      const b = await api.bootstrap()
      boot = b
      themes = b.themes
      startup = b.startup
      closeToTray = b.config.closeToTray
      startHiddenAtLogin = b.config.startHiddenAtLogin
      locale = b.config.locale || 'zh-CN'
      hotkeyDraft = b.config.hotkey
      pixelIcons = b.config.pixelIcons ?? false
      applyPixelIcons(pixelIcons)
      pixelFont = b.config.pixelFont ?? true
      applyPixelFont(pixelFont)
      discoverRoots = [...(b.config.discoverRoots ?? [])]
      collections = structuredClone(b.config.collections ?? [])
      if (groupMode === 'collection' && collections.length === 0) groupMode = 'source'
      tagStyles = { ...(b.config.tagStyles ?? {}) }
      await refresh()
      await loadTheme(b.config.theme)
      if (b.warning) say(b.warning, true)
      if (!b.startHidden) await api.showMainWindow()
    } catch (e) {
      say(String(e), true)
      await api.showMainWindow().catch(() => {})
    }
  })

  async function launch(app: AppView) {
    try {
      const r = await api.launchApp(app.id)
      say(t('app.started', { name: app.name, pid: r.pid }))
    } catch (e) {
      say(t('app.launchFailed', { name: app.name, error: String(e) }), true)
    }
  }

  async function reveal(app: AppView) {
    try {
      await api.revealApp(app.id)
    } catch (e) {
      say(String(e), true)
    }
  }

  function selectApp(app: AppView, event: MouseEvent) {
    // Right-clicking an already multi-selected card keeps the batch selection so
    // the context menu's quick tag applies to every selected app.
    if (event.button === 2 && selectedIds.length > 1 && selectedIds.includes(app.id)) {
      selectedId = app.id
      return
    }
    if (event.shiftKey && selectionAnchor) {
      selectedIds = rangeSelection(selectionAnchor, app.id)
      selectedId = app.id
      return
    }
    if (event.ctrlKey || event.metaKey) {
      selectedIds = selectedIds.includes(app.id)
        ? selectedIds.filter((id) => id !== app.id)
        : [...selectedIds, app.id]
      selectedId = app.id
      selectionAnchor = app.id
      return
    }
    selectedIds = [app.id]
    selectedId = app.id
    selectionAnchor = app.id
  }

  function rangeSelection(from: string, to: string): string[] {
    const start = filtered.findIndex((app) => app.id === from)
    const end = filtered.findIndex((app) => app.id === to)
    if (start < 0 || end < 0) return [to]
    const [lo, hi] = start <= end ? [start, end] : [end, start]
    return filtered.slice(lo, hi + 1).map((app) => app.id)
  }

  function selectAllVisible() {
    selectedIds = filtered.map((app) => app.id)
    selectedId = filtered.at(-1)?.id ?? null
    selectionAnchor = selectedId
  }

  function clearSelection() {
    selectedIds = []
    selectedId = null
    selectionAnchor = null
  }

  function launchSelected() {
    const app = filtered.find((a) => a.id === selectedId) ?? filtered[0]
    if (app) launch(app)
  }

  function revealSelected() {
    const app = filtered.find((a) => a.id === selectedId) ?? filtered[0]
    if (app) reveal(app)
  }

  function moveSelection(step: number, extend = false) {
    if (!filtered.length) return
    const index = filtered.findIndex((a) => a.id === selectedId)
    const next =
      index < 0
        ? step > 0 ? 0 : filtered.length - 1
        : (index + step + filtered.length) % filtered.length
    const app = filtered[next]
    selectedId = app.id
    if (extend && selectionAnchor) {
      selectedIds = rangeSelection(selectionAnchor, app.id)
    } else {
      selectedIds = [app.id]
      selectionAnchor = app.id
    }
    // REQ-0042: keyboard navigation may leave the mounted window; grow it so the
    // newly selected card actually exists in the DOM.
    ensureSelectedVisible(app)
  }

  /** Grow the render window so the given app's card is mounted (REQ-0042). */
  function ensureSelectedVisible(app: AppView) {
    const flatMode = !grouped || activeGroup !== 'all'
    if (flatMode) {
      const index = filtered.findIndex((item) => item.id === app.id)
      if (index < flatShown) return
      flatShown = Math.ceil((index + 1) / PAGE_SIZE) * PAGE_SIZE
      return
    }
    const group = visibleGroups.find((item) =>
      item.apps.some((entry) => entry.id === app.id)
    )
    if (!group) return
    const limit = groupShown[group.key] ?? PAGE_SIZE
    const index = group.apps.findIndex((item) => item.id === app.id)
    if (index < limit) return
    groupShown = {
      ...groupShown,
      [group.key]: Math.ceil((index + 1) / PAGE_SIZE) * PAGE_SIZE
    }
  }

  /** "Show more" inside one group section: grow only that group's window. */
  function moreInGroup(key: string) {
    groupShown = { ...groupShown, [key]: nextPage(groupShown[key] ?? PAGE_SIZE) }
  }

  async function toggleAutostart(app: AppView) {
    try {
      await api.setAppAutostart(app.id, !app.autostart)
      await refresh()
      say(t(app.autostart ? 'app.autostartOff' : 'app.autostartOn', { name: app.name }))
    } catch (e) {
      say(String(e), true)
    }
  }

  async function remove(app: AppView) {
    try {
      apps = await api.removeApp(app.id)
      say(t('app.removed', { name: app.name }))
    } catch (e) {
      say(String(e), true)
    }
  }

  async function saveEdit(patch: AppEditPatch) {
    if (!editing) return
    const updated = await api.updateApp(editing.id, patch)
    apps = apps.map((a) => (a.id === updated.id ? updated : a))
    say(t('edit.saved', { name: updated.name }))
    editing = null
  }

  async function scan() {
    if (scanning) return
    scanning = true
    try {
      candidates = await api.scanApps()
      if (!candidates.length) say(t('app.noCandidates'))
    } catch (e) {
      say(String(e), true)
    } finally {
      scanning = false
    }
  }

  // REQ-0019: opening the dialog must never reset an in-flight or finished scan.
  // Closing it merely hides the UI; the scan keeps running here in App.svelte.
  async function openDiscover() {
    showDiscover = true
    try {
      if (!drives.length) drives = await api.listDrives()
    } catch (e) {
      say(String(e), true)
    }
  }

  async function startScan(roots: string[]) {
    if (discovering) return
    const token = ++scanToken
    discovering = true
    scanCancelled = false
    importProgress = ''
    importReport = null
    try {
      discoverRoots = await api.setDiscoverRoots(roots)
      if (boot) boot.config.discoverRoots = [...discoverRoots]
      const report = await api.discoverApps(roots)
      if (token !== scanToken) return
      discovered = report.items
      if (report.cancelled) {
        // Keep whatever the backend found before the cancellation, and mark the
        // results as incomplete (BUG-0008/REQ-0019 M4).
        scanCancelled = report.items.length > 0
        if (scanCancelled) say(t('discover.cancelledPartial', { n: report.items.length }))
        else say(t('discover.cancelled'))
      } else if (!report.items.length) {
        scanCancelled = false
        say(t('discover.empty'))
      } else {
        scanCancelled = false
        if (!showDiscover) {
          // AC-19-3: the dialog is closed, so report the result on the main screen.
          say(t('app.discoverDone', { n: report.items.length }))
        }
      }
    } catch (e) {
      if (token === scanToken) say(String(e), true)
    } finally {
      if (token === scanToken) discovering = false
    }
  }

  async function cancelScan() {
    if (!discovering) return
    try {
      await api.cancelDiscover()
    } catch (e) {
      say(String(e), true)
    }
  }

  // Explicit "scan again" from the results view: the only place that drops the
  // previous outcome (the dialog close button intentionally does not).
  function resetDiscover() {
    if (discovering) return
    discovered = []
    scanCancelled = false
    importProgress = ''
    importReport = null
  }

  async function changeImportMode(mode: string) {
    try {
      await api.setImportMode(mode)
      if (boot) boot.config.importMode = mode
    } catch (e) {
      say(String(e), true)
    }
  }

  async function importSelected(items: Discovered[], mode: ImportMode) {
    importing = true
    importProgress = ''
    let unlisten: (() => void) | undefined
    try {
      await api.setImportMode(mode)
      if (boot) boot.config.importMode = mode
      unlisten = await listenSafe<ImportOutcome>('import://progress', (p) => {
        importProgress = `${p.name}: ${p.message}`
      })
      const report = await api.importApps(
        items.map((i) => ({ path: i.path, name: i.name, kind: i.kind, category: i.category })),
        mode
      )
      // Import already succeeded: show the result panel even if the refresh fails.
      importReport = report
      try {
        apps = await api.listApps()
      } catch (e) {
        say(String(e), true)
      }
    } catch (e) {
      say(String(e), true)
    } finally {
      importing = false
      unlisten?.()
    }
  }

  async function addAll() {
    // Report what actually happened: the backend/mock skip already-added
    // candidates, so candidates.length is NOT the added count (BUG-0009 AC-B9-4).
    const batch = candidates
    try {
      const before = apps.length
      apps = await api.addApps(batch)
      const added = Math.max(0, apps.length - before)
      const skipped = Math.max(0, batch.length - added)
      if (skipped > 0) say(t('app.addedSkipped', { added, skipped }))
      else say(t('app.added', { n: added }))
      candidates = []
    } catch (e) {
      say(String(e), true)
    }
  }

  async function changeTheme(id: string) {
    try {
      await api.setTheme(id)
      await loadTheme(id)
      themes = await api.listThemes()
      if (boot) boot.config.theme = id
    } catch (e) {
      say(String(e), true)
    }
  }

  async function togglePixelIcons(enabled: boolean) {
    try {
      await api.setPixelIcons(enabled)
      pixelIcons = enabled
      applyPixelIcons(enabled)
      if (boot) boot.config.pixelIcons = enabled
    } catch (e) {
      say(String(e), true)
    }
  }

  async function togglePixelFont(enabled: boolean) {
    try {
      await api.setPixelFont(enabled)
      pixelFont = enabled
      applyPixelFont(enabled)
      if (boot) boot.config.pixelFont = enabled
    } catch (e) {
      say(String(e), true)
    }
  }

  async function changeAppsDir(path: string) {
    if (!boot) return
    try {
      boot.config = await api.setAppsDir(path)
      candidates = await api.scanApps()
      say(t('app.appsDirChanged', { path: displayPath(boot.root, boot.config.appsDir) }))
    } catch (e) {
      say(String(e), true)
    }
  }

  async function changeLocale(id: string) {
    try {
      await api.setLocale(id)
      locale = id
    } catch (e) {
      say(String(e), true)
    }
  }

  async function toggleLogin(enabled: boolean) {
    try {
      startup = await api.setLaunchAtLogin(enabled)
      say(t(enabled ? 'app.loginOn' : 'app.loginOff'))
    } catch (e) {
      say(String(e), true)
      startup = await api.getStartupStatus()
    }
  }

  async function toggleTray(enabled: boolean) {
    closeToTray = enabled
    await api.setCloseToTray(enabled)
  }

  async function toggleStartHidden(enabled: boolean) {
    startHiddenAtLogin = enabled
    try {
      await api.setStartHiddenAtLogin(enabled)
    } catch (e) {
      startHiddenAtLogin = !enabled
      say(String(e), true)
    }
  }

  async function saveHotkey() {
    const previous = boot?.config.hotkey ?? ''
    try {
      await api.setHotkey(hotkeyDraft)
      if (boot) boot.config.hotkey = hotkeyDraft
      say(t('app.hotkeySaved'))
    } catch (e) {
      // A conflicting shortcut must not silently replace the saved one.
      hotkeyDraft = previous
      say(String(e), true)
    }
  }

  // BUG-0013: while the shortcut recorder is active the global hotkey is
  // unregistered so pressing the old combination cannot hide the window.
  async function beginHotkeyRecord() {
    try {
      await api.suspendHotkey()
    } catch (e) {
      say(String(e), true)
    }
  }

  async function endHotkeyRecord() {
    try {
      await api.resumeHotkey()
    } catch (e) {
      say(String(e), true)
    }
  }

  async function runAutostartNow() {
    try {
      const reports = await api.runAutostartNow()
      say(
        reports.length
          ? reports.map((r) => `${r.name}:${r.ok ? 'ok' : r.message}`).join('; ')
          : t('app.noAutostart')
      )
    } catch (e) {
      say(String(e), true)
    }
  }

  function onMenu(event: MouseEvent, app: AppView) {
    menu = { x: event.clientX, y: event.clientY, app }
  }

  function handleKey(event: KeyboardEvent) {
    // Any open dialog owns the keyboard: global shortcuts must not launch apps,
    // steal focus into search, or stack another dialog on top (REQ-0013 F1).
    if (editing || tagEditor || quickTag || tagManager || collectionModal || categoryPicker || smartEditor || showSettings || showDiscover) return
    const tag = document.activeElement?.tagName
    const typing =
      tag === 'INPUT' ||
      tag === 'TEXTAREA' ||
      (document.activeElement as HTMLElement | null)?.isContentEditable === true
    if (event.ctrlKey && event.key.toLowerCase() === 'a' && !typing) {
      event.preventDefault()
      selectAllVisible()
      return
    }
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
      event.preventDefault()
      moveSelection(1, event.shiftKey)
      return
    }
    if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
      event.preventDefault()
      moveSelection(-1, event.shiftKey)
      return
    }
    if (event.key === 'Enter') {
      event.preventDefault()
      if (event.shiftKey) revealSelected()
      else launchSelected()
      return
    }
    if (event.key === 'Escape') {
      if (menu) menu = null
      else if (query) query = ''
      else clearSelection()
      return
    }
    if (event.key === '/' && document.activeElement !== searchEl) {
      event.preventDefault()
      searchEl?.focus()
      return
    }
    if (event.ctrlKey && event.key === ',') {
      event.preventDefault()
      showSettings = true
      return
    }
    }
</script>

<svelte:window
  onkeydown={handleKey}
  onclick={(e) => {
    menu = null
    const el = e.target as HTMLElement
    if (!el.closest('.ph-card') && !el.closest('.ph-menu') && !el.closest('[data-testid="selection-bar"]')) {
      clearSelection()
    }
  }}
/>

<TitleBar {t} />

<div class="ph-content">
  <div class="ph-toolbar">
    <div class="ph-search-wrap">
      <Icon name="search" size={15} />
      <input
        class="ph-search"
        bind:this={searchEl}
        bind:value={query}
        placeholder={t('app.searchPlaceholder')}
        aria-label={t('app.searchLabel')}
        spellcheck="false"
      />
      {#if query}
        <button
          type="button"
          class="ph-input-clear"
          data-testid="search-clear"
          aria-label={t('app.clearSearch')}
          title={t('app.clearSearch')}
          onclick={() => {
            query = ''
            searchEl?.focus()
          }}
        >
          <Icon name="close" size={13} />
        </button>
      {/if}
    </div>
    <div class="ph-toolbar-cluster ph-toolbar-primary">
      <button class="ph-btn" data-testid="discover-btn" onclick={openDiscover}>
        <Icon name="discover" />{t('app.discover')}
      </button>
      <button class="ph-btn" data-testid="scan-btn" disabled={scanning} onclick={scan}>
        <Icon name="scan" />{scanning ? t('app.scanning') : t('app.scan')}
      </button>
      <button
        class="ph-btn"
        class:accent={grouped}
        data-testid="group-toggle"
        aria-pressed={grouped}
        title={t('app.groupingHint')}
        onclick={toggleGrouping}
      >
        <Icon name="layers" />{t('app.grouping')}
      </button>
      {#if grouped}
        <label class="ph-group-mode">
          <span>{t('smart.groupMode')}</span>
          <select
            class="ph-select"
            data-testid="group-mode"
            value={groupMode}
            onchange={(event) => setGroupMode(event.currentTarget.value as GroupMode)}
          >
            <option value="source">{t('smart.groupSource')}</option>
            <option value="tag">{t('smart.groupTag')}</option>
            <option value="smart">{t('smart.groupSmart')}</option>
            <option value="collection">{t('smart.groupCollection')}</option>
          </select>
        </label>
        {#if groupMode === 'collection'}
          <button
            class="ph-btn"
            data-testid="collections-manage"
            onclick={() => (collectionModal = { apps: [] })}
          >
            <Icon name="layers" />{t('collections.manage')}
          </button>
        {/if}
      {/if}
    </div>
    <div class="ph-toolbar-cluster ph-toolbar-actions">
      {#if appTags.length || autostartCount > 0}
        <button
          class="ph-btn"
          class:accent={tagFilterActiveCount > 0}
          data-testid="tag-filter-btn"
          aria-pressed={tagFilterActiveCount > 0}
          title={t('app.tagFilter')}
          onclick={() => (tagFilterOpen = !tagFilterOpen)}
        ><Icon name="tag" />{tagFilterActiveCount
          ? `${t('app.tags')} (${tagFilterActiveCount})`
          : t('app.tags')}</button>
      {/if}
      {#if apps.length}
        <button class="ph-btn" data-testid="smart-organize" onclick={() => (smartEditor = true)}>
          <Icon name="sparkles" />{t('smart.organize')}
        </button>
      {/if}
      {#if discovering}
        <span class="ph-badge on" data-testid="scan-status">{t('app.discoverRunning')}</span>
        <button class="ph-btn" data-testid="scan-cancel-top" onclick={cancelScan}>
          {t('app.discoverCancel')}
        </button>
      {/if}
      <button class="ph-btn accent" data-testid="settings-btn" onclick={() => (showSettings = true)}>
        <Icon name="settings" />{t('app.settings')}
      </button>
    </div>
  </div>

  {#if selectedIds.length > 1}
    <div class="ph-toolbar ph-selection-bar" data-testid="selection-bar">
      <span class="ph-hint" data-testid="selection-count">
        {t('app.selectedCount', { n: selectedIds.length })}
      </span>
      <button class="ph-btn" data-testid="selection-all" onclick={selectAllVisible}>
        {t('app.selectionAll')}
      </button>
      <button class="ph-btn" data-testid="selection-clear" onclick={clearSelection}>
        {t('app.selectionClear')}
      </button>
      <button class="ph-btn accent" data-testid="batch-tag" onclick={openBatchTag}>
        {t('tags.quick')} ({selectedIds.length})
      </button>
      <button
        class="ph-btn"
        data-testid="selection-collection"
        onclick={() =>
          (collectionModal = {
            apps: apps.filter((app) => selectedIds.includes(app.id))
          })}
      >
        {t('collections.addSelected')}
      </button>
      <button
        class="ph-btn"
        data-testid="selection-category"
        onclick={() =>
          (categoryPicker = apps.filter((app) => selectedIds.includes(app.id)))}
      >
        {t('categoryPicker.title')}
      </button>
      <span class="ph-hint ph-selection-hint">{t('app.selectionHint')}</span>
    </div>
  {/if}

  {#if tagFilterOpen && (appTags.length || autostartCount > 0)}
    <div class="ph-toolbar ph-tag-filter-bar" data-testid="tag-filter-bar">
      <span class="ph-hint">{t('app.tagFilter')}</span>
      {#if grouped && activeGroup !== 'all'}
        <span class="ph-hint" data-testid="tag-scope-hint">{t('app.tagFilterGroup')}</span>
      {/if}
      <button
        type="button"
        class="ph-tag-chip"
        class:active={autostartFilter === 'include'}
        class:excluded={autostartFilter === 'exclude'}
        data-testid="tag-filter-autostart"
        data-state={autostartFilter}
        aria-pressed={autostartFilter !== 'off'}
        title={t('app.autostartTagCycle')}
        onclick={cycleAutostartFilter}
      >{t('app.autostartTag')} ({autostartCount})</button>
      {#each appTags as item (item.tag)}
        {@const state = tagFilterState(item.tag)}
        {@const style = tagStyles[item.tag]}
        <button
          type="button"
          class="ph-tag-chip"
          class:active={state === 'include'}
          class:excluded={state === 'exclude'}
          data-testid="tag-filter-chip"
          data-tag={item.tag}
          data-state={state}
          aria-pressed={state !== 'off'}
          aria-label={`${item.tag} (${item.count})`}
          title={t('app.tagFilterCycle')}
          style={tagCss(style)}
          onclick={() => cycleTagFilter(item.tag)}
        >{tagText(style, item.tag)} ({item.count})</button>
      {/each}
      <button
        type="button"
        class="ph-btn"
        data-testid="tag-clear-btn"
        disabled={tagFilterActiveCount === 0}
        onclick={clearTagFilter}
      >{t('app.tagClear')}</button>
      <button
        type="button"
        class="ph-btn"
        data-testid="tag-manage"
        onclick={() => (tagManager = true)}
      >{t('tags.manage')}</button>
    </div>
  {/if}

  {#if candidates.length}
    <div
      class="ph-toolbar ph-candidates"
      data-testid="candidates-bar"
      style="border-bottom:var(--ph-border-width) solid var(--ph-border)"
    >
      <span class="ph-hint ph-candidate-count">{t('app.found', { n: candidates.length })}</span>
      <div class="ph-candidate-list" data-testid="candidate-list">
        {#each candidates as c (c.relPath)}
          <span class="ph-badge on" data-testid="candidate-badge" title={c.relPath}>{c.name}</span>
        {/each}
      </div>
      <button class="ph-btn accent" data-testid="add-all" onclick={addAll}>{t('app.addAll')}</button>
      <button class="ph-btn" data-testid="ignore-candidates" onclick={() => (candidates = [])}>{t('app.ignore')}</button>
    </div>
  {/if}

  <div class="ph-main">
    <GroupSidebar
      groups={sidebarGroups}
      active={activeGroup}
      total={apps.length}
      {t}
      onselect={selectRailGroup}
    />
    <div class="ph-grid-wrap">
      <div class="ph-main-head" data-testid="main-heading">
        <div class="ph-main-title">
          <div class="ph-main-eyebrow">
            {#if grouped && activeGroup !== 'all'}{t('app.tagFilterGroup')}{:else}{t('app.groupAll')}{/if}
            {#if query.trim()} / “{query.trim()}”{/if}
          </div>
          <h1>
            {#if grouped && activeGroup !== 'all'}
              {modeGroups.find((group) => group.key === activeGroup)?.label ?? t('app.groupAll')}
            {:else}{t('app.groupAll')}{/if}
            <span>{filtered.length}</span>
          </h1>
          <div class="ph-main-sub">{t('app.showing', { shown: filtered.length, total: apps.length })}</div>
        </div>
        <div class="ph-main-filters">
          <button
            type="button"
            class="ph-main-chip"
            class:active={autostartFilter === 'include'}
            data-testid="main-autostart-filter"
            onclick={() => (autostartFilter = autostartFilter === 'include' ? 'off' : 'include')}
          ><span class="dot"></span>{t('app.autostartTag')}<b>{autostartCount}</b></button>
          <button
            type="button"
            class="ph-main-chip"
            class:active={tagFilterOpen}
            onclick={() => (tagFilterOpen = !tagFilterOpen)}
          >{t('app.tags')}<b>{appTags.length}</b></button>
        </div>
      </div>
      {#if filtered.length}
        {#if grouped && activeGroup === 'all'}
          {#each visibleGroups as group (group.key)}
            <section class="ph-group" data-testid="grid-group">
              <h3 class="ph-group-title">
                <span data-testid="grid-group-label">
                  {group.key === 'local' ? t('app.groupLocal') : group.label}
                </span>
                <span class="ph-hint">{t('app.groupCount', { n: group.apps.length })}</span>
              </h3>
              <div class="ph-grid">
                {#each pageSlice(group.apps, groupShown[group.key] ?? PAGE_SIZE) as app (app.id)}
                  <AppCard {app} {t} {tagStyles} selected={selectedIds.includes(app.id)} onselect={selectApp} onlaunch={launch} onmenu={onMenu} />
                {/each}
              </div>
              {#if group.apps.length > (groupShown[group.key] ?? PAGE_SIZE)}
                <button
                  type="button"
                  class="ph-btn ph-more"
                  data-testid="group-more"
                  onclick={() => moreInGroup(group.key)}
                >{t('app.showMore', { n: group.apps.length - (groupShown[group.key] ?? PAGE_SIZE) })}</button>
              {/if}
            </section>
          {/each}
        {:else}
          <div class="ph-grid">
            {#each flatVisible as app (app.id)}
              <AppCard {app} {t} {tagStyles} selected={selectedIds.includes(app.id)} onselect={selectApp} onlaunch={launch} onmenu={onMenu} />
            {/each}
          </div>
          {#if filtered.length > flatShown}
            <button
              type="button"
              class="ph-btn ph-more"
              data-testid="grid-more"
              onclick={() => (flatShown = nextPage(flatShown))}
            >{t('app.showMore', { n: filtered.length - flatShown })}</button>
          {/if}
          <p class="ph-hint ph-visible-count" data-testid="grid-visible-count">
            {t('app.showing', { shown: flatVisible.length, total: filtered.length })}
          </p>
        {/if}
      {:else}
        <div class="ph-empty">
          <div class="big">[ + ]</div>
          <div>
            {apps.length
              ? grouped && activeGroup !== 'all' && !query.trim()
                ? t('app.groupEmpty')
                : t('app.emptyNoMatch')
              : t('app.emptyNoApps')}
          </div>
          <div class="ph-hint">
            {t('app.emptyHint', {
              path: displayPath(boot?.root ?? '', boot?.config.appsDir ?? '')
            })}
          </div>
          <button class="ph-btn" onclick={scan}>{t('app.scanDir')}</button>
        </div>
      {/if}
    </div>
    <InspectorPanel
      {apps}
      {selectedIds}
      primary={selectedIds.length === 1 ? apps.find((app) => app.id === selectedIds[0]) ?? null : null}
      {collections}
      {tagStyles}
      {t}
      onlaunch={launch}
      onreveal={reveal}
      onedit={(app) => (editing = app)}
      ontags={(app) => (tagEditor = app)}
      oncategory={() =>
        (categoryPicker = apps.filter((app) => selectedIds.includes(app.id)))}
      onautostart={toggleAutostart}
      onbatchtag={openBatchTagFromInspector}
      onbatchcategory={() =>
        (categoryPicker = apps.filter((app) => selectedIds.includes(app.id)))}
      onbatchcollection={() =>
        (collectionModal = {
          apps: apps.filter((app) => selectedIds.includes(app.id))
        })}
      onclear={clearSelection}
    />
  </div>
</div>

{#if showDiscover && boot}
  <DiscoverModal
    items={discovered}
    appsDir={displayPath(boot.root, boot.config.appsDir)}
    defaultMode={boot.config.importMode as ImportMode}
    initialRoots={discoverRoots}
    {drives}
    existingNames={apps.map((a) => a.name)}
    scanning={discovering}
    cancelled={scanCancelled}
    busy={importing}
    progress={importProgress}
    report={importReport}
    {t}
    onscan={startScan}
    oncancel={cancelScan}
    onrescan={resetDiscover}
    onclose={() => (showDiscover = false)}
    onclosereport={() => {
      // The import report is the end of this discover round: close the dialog and
      // consume the scan results, so the next open starts from the scope step.
      showDiscover = false
      resetDiscover()
    }}
    onimport={importSelected}
  />
{/if}

{#if menu}
  <ContextMenu
    x={menu.x}
    y={menu.y}
    app={menu.app}
    tags={allTagOptions}
    {t}
    onclose={() => (menu = null)}
    onlaunch={launch}
    onreveal={reveal}
    onedit={(app) => (editing = app)}
    ontags={(app) => (tagEditor = app)}
    oncategory={(app) => {
      categoryPicker =
        selectedIds.length > 1 && selectedIds.includes(app.id)
          ? apps.filter((item) => selectedIds.includes(item.id))
          : [app]
    }}
    oncollection={(app) => {
      const targets =
        selectedIds.length > 1 && selectedIds.includes(app.id)
          ? apps.filter((item) => selectedIds.includes(item.id))
          : [app]
      collectionModal = { apps: targets }
    }}
    onquicktag={toggleQuickTag}
    onquicktags={(app) => {
      const anchor = { x: menu?.x ?? 120, y: menu?.y ?? 120, bottom: menu?.y ?? 120 }
      menu = null
      openQuickTag(app, anchor)
    }}
    onautostart={toggleAutostart}
    onremove={remove}
  />
{/if}

{#if quickTag}
  <QuickTag
    apps={quickTag.apps}
    allTags={allTagOptions}
    {tagStyles}
    anchor={quickTag.anchor}
    {t}
    onclose={() => (quickTag = null)}
    ontoggle={toggleQuickTag}
    onadd={addQuickTag}
  />
{/if}

{#if editing}
  <EditModal app={editing} {t} onclose={() => (editing = null)} onsave={saveEdit} />
{/if}

{#if tagEditor}
  <TagEditor
    app={tagEditor}
    allTags={appTags}
    {tagStyles}
    {t}
    onsave={saveTags}
    onclose={() => (tagEditor = null)}
  />
{/if}

{#if categoryPicker}
  <CategoryPicker
    apps={categoryPicker}
    {t}
    onapply={applyCategory}
    onclose={() => (categoryPicker = null)}
  />
{/if}

{#if collectionModal}
  <CollectionModal
    collections={collections}
    selectedApps={collectionModal.apps}
    {t}
    onsave={saveCollections}
    onclose={() => (collectionModal = null)}
  />
{/if}

{#if tagManager}
  <TagManager
    tags={allTagOptions}
    {tagStyles}
    {t}
    onsave={saveTagStyle}
    onclose={() => (tagManager = false)}
  />
{/if}

{#if smartEditor}
  <SmartOrganize
    apps={apps}
    {t}
    onapply={applySmartCategories}
    onclose={() => (smartEditor = false)}
  />
{/if}

{#if showSettings && boot}
  <Settings
    {boot}
    {themes}
    {startup}
    {closeToTray}
    {pixelIcons}
    {pixelFont}
    {locale}
    importMode={boot.config.importMode}
    {t}
    bind:hotkeyDraft
    onrecordstart={beginHotkeyRecord}
    onrecordstop={endHotkeyRecord}
    onclose={() => {
      showSettings = false
      void endHotkeyRecord()
    }}
    ontheme={changeTheme}
    onpixelicons={togglePixelIcons}
    onpixelfont={togglePixelFont}
    onlocale={changeLocale}
    onappsdir={changeAppsDir}
    onimportmode={changeImportMode}
    onhotkey={saveHotkey}
    onautostart={toggleLogin}
    onclosetotray={toggleTray}
    {startHiddenAtLogin}
    onstarthidden={toggleStartHidden}
    onrunautostart={runAutostartNow}
    onquit={() => api.quit()}
  />
{/if}

{#if toast}
  <div
    class="ph-toast"
    class:error={toast.error}
    role="status"
    aria-live={toast.error ? 'assertive' : 'polite'}
    aria-atomic="true"
  >{toast.text}</div>
{/if}

<style>
  /* REQ-0032: tag filter bar under the toolbar. */
  .ph-tag-filter-bar {
    flex-wrap: wrap;
    row-gap: 6px;
  }

  .ph-selection-bar {
    flex-wrap: wrap;
    row-gap: 6px;
    background: var(--ph-bg-alt);
  }

  .ph-main-head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    padding: 2px 2px 14px;
    margin-bottom: 4px;
    border-bottom: 1px solid var(--ph-border);
  }

  .ph-main-eyebrow {
    color: var(--ph-accent);
    font-family: var(--ph-font-pixel);
    font-size: 11px;
    letter-spacing: .06em;
  }

  .ph-main-title h1 {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin: 7px 0 0;
    font-family: var(--ph-font-pixel);
    font-size: 26px;
    font-weight: 400;
    color: var(--ph-text);
  }

  .ph-main-title h1 span {
    color: var(--ph-text-muted);
    font-size: 15px;
  }

  .ph-main-sub {
    margin-top: 6px;
    color: var(--ph-text-muted);
    font-size: 12px;
  }

  .ph-main-filters {
    display: flex;
    gap: 6px;
    padding-bottom: 2px;
  }

  .ph-main-chip {
    height: 27px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    border: 1px solid var(--ph-border);
    background: var(--ph-panel);
    color: var(--ph-text-muted);
    font-family: var(--ph-font);
    font-size: 11px;
    cursor: pointer;
  }

  .ph-main-chip b {
    color: var(--ph-text);
    font-family: var(--ph-font-pixel);
    font-size: 10px;
    font-weight: 400;
  }

  .ph-main-chip:hover,
  .ph-main-chip.active {
    border-color: var(--ph-accent);
    color: var(--ph-accent);
  }

  .ph-main-chip .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--ph-accent);
  }

  .ph-visible-count {
    display: none;
  }

  .ph-toolbar-cluster {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 0 0 auto;
  }

  .ph-toolbar-actions {
    margin-left: auto;
    padding-left: 8px;
    border-left: var(--ph-border-width) solid var(--ph-border);
  }

  .ph-selection-hint {
    margin-left: auto;
  }

  .ph-tag-filter-bar .ph-hint {
    margin: 0;
  }

  .ph-tag-chip {
    height: 26px;
    padding: 0 8px;
    background: var(--ph-panel);
    border: var(--ph-border-width) solid var(--ph-border);
    color: var(--ph-text-muted);
    font-size: 11px;
    cursor: pointer;
  }

  .ph-tag-chip:hover {
    background: var(--ph-panel-hover);
    color: var(--ph-text);
  }

  .ph-tag-chip.active {
    border-color: var(--ph-accent);
    color: var(--ph-accent);
    background: var(--ph-bg-alt);
  }

  .ph-tag-chip.excluded {
    border-color: var(--ph-danger);
    color: var(--ph-danger);
    background: var(--ph-bg-alt);
    text-decoration: line-through;
  }

  .ph-tag-chip.excluded::before {
    content: '− ';
  }

  .ph-tag-filter-bar .ph-tag-chip {
    min-width: 72px;
    text-align: center;
  }

  /* REQ-0039: grouping-mode select in the toolbar. */
  .ph-group-mode {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--ph-text-muted);
    font-size: 11px;
    white-space: nowrap;
  }

  .ph-group-mode .ph-select {
    flex: 0 0 auto;
    min-width: 0;
    width: 132px;
    height: 26px;
  }

  /* REQ-0042: render-window "show more" affordances. */
  .ph-more {
    display: block;
    margin: 10px auto 0;
  }

  .ph-visible-count {
    margin: 8px 0 0;
    text-align: center;
  }
</style>
