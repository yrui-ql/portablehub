<script lang="ts">
  import { tick } from 'svelte'
  import type { AppCategory, Discovered, ImportMode, ImportReport } from '../lib/types'
  import type { Translator } from '../lib/i18n'
  import { api } from '../lib/api'
  import { summarizeImportReport, type ImportReportSummary } from '../lib/import-report'
  import { groupDiscovered, type DiscoverGroup } from '../lib/discover-groups'
  import { formatSize, shortPath } from '../lib/format'
  import InfoTip from './InfoTip.svelte'
  import Icon from './Icon.svelte'

  let {
    items,
    appsDir,
    defaultMode,
    drives,
    initialRoots,
    existingNames,
    scanning,
    folderScanning,
    scanKind,
    cancelled,
    busy,
    progress,
    report,
    t,
    onscan,
    onfolderscan,
    oncancel,
    onrescan,
    onclose,
    onclosereport,
    onimport
  }: {
    items: Discovered[]
    appsDir: string
    defaultMode: ImportMode
    drives: string[]
    initialRoots: string[]
    existingNames: string[]
    scanning: boolean
    folderScanning: boolean
    scanKind: 'discover' | 'folder'
    /** The last scan was cancelled but kept a partial result set. */
    cancelled: boolean
    busy: boolean
    progress: string
    report: ImportReport | null
    t: Translator
    onscan: (roots: string[]) => void
    onfolderscan: () => void
    oncancel: () => void
    onrescan: () => void
    onclose: () => void
    onclosereport: () => void
    onimport: (items: Discovered[], mode: ImportMode) => void
  } = $props()

  let selected = $state<Record<string, boolean>>({})
  let mode = $state<ImportMode>('link')
  let roots = $state<Record<string, boolean>>({})
  let rootsReady = $state(false)
  let query = $state('')
  let writeCategories = $state(false)
  // "Change scope" is view navigation, not a rescan — the results stay
  // in App.svelte until the user explicitly clicks "Rescan".
  let view = $state<'results' | 'scope'>('results')
  let collapsed = $state<Record<string, boolean>>({})

  // ---- REQ-0041 · AI classification of discover results ----
  // Fixed category order shared with the main screen (src/App.svelte).
  const CATEGORY_ORDER: AppCategory[] = [
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
  ]
  const UNCLASSIFIED_KEY = 'unclassified'

  let classifying = $state(false)
  let classifyError = $state('')
  /** Suggestion map keyed by item path: the props themselves are read-only. */
  let categoryMap = $state<Record<string, string>>({})
  /** After classification the default view is "group by category". */
  let groupMode = $state<'category' | 'source'>('source')

  $effect(() => {
    mode = defaultMode
  })

  $effect(() => {
    if (scanKind === 'folder') mode = 'register'
  })

  $effect(() => {
    if (!rootsReady && drives.length) {
      rootsReady = true
      const saved = new Set(initialRoots)
      const next: Record<string, boolean> = {}
      for (const drive of drives) {
        next[drive] = saved.size
          ? saved.has(drive)
          : !drive.toUpperCase().startsWith('C:')
      }
      roots = next
    }
  })

  $effect(() => {
    items
    selected = {}
    collapsed = {}
    // A new result set invalidates the previous classification.
    categoryMap = {}
    classifyError = ''
    groupMode = 'source'
  })

  let existing = $derived(new Set(existingNames.map((n) => n.toLowerCase())))
  let existingHidden = $derived(
    items.filter((item) => existing.has(item.name.toLowerCase())).length
  )
  // A rescan is a diff: already registered apps never enter the result list.
  let visible = $derived(
    items.filter((item) => {
      if (existing.has(item.name.toLowerCase())) return false
      const q = query.trim().toLowerCase()
      if (!q) return true
      return item.name.toLowerCase().includes(q) || item.path.toLowerCase().includes(q)
    })
  )
  let chosen = $derived(visible.filter((item) => selected[item.path]))
  let mergedCount = $derived(items.reduce((sum, item) => sum + (item.duplicateCount ?? 0), 0))
  // REQ-0021: groups derive from the filtered set, so groups whose visible items
  // are all filtered out disappear automatically. Folder scans are a single
  // logical source, so they do not get one group per nested folder.
  let visibleGroups = $derived(
    scanKind === 'folder' && visible.length
      ? [{ key: 'folder', label: t('discover.folderSource'), hint: appsDir, items: visible }]
      : groupDiscovered(visible)
  )

  // ---- REQ-0041 · AI classification ----

  function categoryOf(item: Discovered): string {
    return (categoryMap[item.path] ?? item.category ?? '').trim()
  }

  let hasCategories = $derived(items.some((item) => categoryOf(item) !== ''))

  /** Groups by semantic category: fixed order, unknown categories then unclassified. */
  let categoryGroups = $derived.by((): DiscoverGroup[] => {
    const buckets = new Map<string, Discovered[]>()
    for (const item of visible) {
      const key = categoryOf(item) || UNCLASSIFIED_KEY
      const bucket = buckets.get(key)
      if (bucket) bucket.push(item)
      else buckets.set(key, [item])
    }
    const rank = (key: string): number => {
      if (key === UNCLASSIFIED_KEY) return CATEGORY_ORDER.length + 1
      const index = CATEGORY_ORDER.indexOf(key as AppCategory)
      return index < 0 ? CATEGORY_ORDER.length : index
    }
    return [...buckets.entries()]
      .sort((a, b) => rank(a[0]) - rank(b[0]))
      .map(([key, bucket]) => ({
        key,
        label: t(`category.${key}`),
        hint: t(`category.${key}`),
        items: [...bucket].sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'))
      }))
  })

  let groups = $derived(groupMode === 'category' ? categoryGroups : visibleGroups)

  async function classifyResults() {
    if (classifying || busy || !items.length) return
    classifying = true
    classifyError = ''
    try {
      // Classify the whole result set, not only the filtered subset.
      const suggestions = await api.classifyDiscovered(items)
      const next: Record<string, string> = { ...categoryMap }
      for (const suggestion of suggestions) next[suggestion.id] = suggestion.category
      categoryMap = next
      if (items.some((item) => (next[item.path] ?? '').trim() !== '')) groupMode = 'category'
    } catch (e) {
      // A failed classification never clears the scanned results.
      classifyError = e instanceof Error ? e.message : String(e)
    } finally {
      classifying = false
    }
  }

  /** REQ-0041: carry the category suggestion into the import payload. */
  function importChosen() {
    const payload = chosen.map((item) => {
      // AI/rule categories stay suggestions unless the user opts in.
      if (!writeCategories) {
        const { category: _category, ...rest } = item
        return rest
      }
      const category = categoryOf(item)
      return category ? { ...item, category } : item
    })
    onimport(payload, mode)
  }

  function toggle(path: string) {
    selected = { ...selected, [path]: !selected[path] }
  }

  function selectAll(value: boolean) {
    const next: Record<string, boolean> = {}
    for (const item of visible) next[item.path] = value
    selected = next
  }

  function toggleGroup(key: string) {
    collapsed = { ...collapsed, [key]: !collapsed[key] }
  }

  function isCollapsed(key: string): boolean {
    return collapsed[key] ?? false
  }

  /** Select only the currently visible items of one group; a second click clears
   *  them again so the same button works as "select all / clear this group". */
  function selectGroup(group: DiscoverGroup) {
    const allSelected = group.items.every((item) => selected[item.path])
    const next = { ...selected }
    for (const item of group.items) next[item.path] = !allSelected
    selected = next
  }

  function startScan() {
    const picked = drives.filter((d) => roots[d])
    if (!picked.length) return
    // Starting a scan leaves the scope view; the progress/result views take over.
    view = 'results'
    onscan(picked)
  }

  // BUG-0012: the ONLY action that drops the previous results.
  function retryScan() {
    onrescan()
    if (scanKind === 'folder') onfolderscan()
    else startScan()
  }

  // ---- import result panel (REQ-0014) ----

  let copyHint = $state('')
  let fallbackText = $state('')
  let fallbackVisible = $state(false)
  let fallbackEl = $state<HTMLTextAreaElement | undefined>()

  const emptySummary: ImportReportSummary = { added: 0, skipped: 0, failed: 0, rows: [] }

  // All classification lives in the pure helper so it stays unit-testable.
  let summary = $derived(report ? summarizeImportReport(report) : emptySummary)

  function reportText(r: ImportReport): string {
    const s = summarizeImportReport(r)
    const lines = [
      t('report.copyHeader', { added: s.added, skipped: s.skipped, failed: s.failed })
    ]
    for (const row of s.rows) {
      lines.push(`${row.name}\t${t(`report.state.${row.state}`)}\t${row.message}`)
    }
    return lines.join('\n')
  }

  async function copyReport() {
    if (!report) return
    const text = reportText(report)
    copyHint = ''
    fallbackVisible = false
    const clip = typeof navigator === 'undefined' ? undefined : navigator.clipboard
    try {
      if (!clip?.writeText) throw new Error('clipboard unavailable')
      await clip.writeText(text)
      copyHint = t('report.copied')
    } catch {
      // Clipboard blocked/unavailable: expose the text and select it so the
      // user can copy manually.
      fallbackText = text
      fallbackVisible = true
      copyHint = t('report.copyFailed')
      await tick()
      fallbackEl?.focus()
      fallbackEl?.select()
    }
  }
</script>

<div class="ph-overlay" role="presentation" onclick={(e) => { if (e.target === e.currentTarget && !busy) onclose() }}>
  <div class="ph-settings ph-discover" data-testid="discover-modal" role="dialog" aria-label={t('discover.title')}>
    <div class="ph-settings-head">
      <strong>{t('discover.title')}</strong>
      <button class="ph-btn" data-testid="discover-close" disabled={busy} onclick={onclose}>
        {t('settings.close')}
      </button>
    </div>

    {#if scanning}
      <div class="ph-section">
        <p class="ph-hint" data-testid="scan-progress">
          {folderScanning ? t('discover.scanningFolder') : t('discover.scanning')}
        </p>
        {#if !folderScanning}
          <div class="ph-row">
            <button class="ph-btn" data-testid="scan-cancel" onclick={oncancel}>
              {t('discover.cancelScan')}
            </button>
          </div>
        {/if}
      </div>
    {:else if report}
      <div class="ph-section" data-testid="import-report">
        <h3>{t('report.title')}</h3>
        <p class="ph-hint" data-testid="report-summary">
          {t('report.summary', {
            added: summary.added,
            skipped: summary.skipped,
            failed: summary.failed
          })}
        </p>
        {#if summary.rows.length}
          <h3 style="margin-top:12px">{t('report.details')}</h3>
          <div class="ph-report-list" data-testid="report-list">
            {#each summary.rows as row, index (index)}
              <div class="ph-report-row" data-testid="report-row" data-state={row.state}>
                <span class="ph-discover-name">{row.name}</span>
                <span
                  class="ph-badge"
                  class:on={row.state === 'added'}
                  class:missing={row.state === 'failed'}
                >
                  {t(`report.state.${row.state}`)}
                </span>
                <span
                  class="ph-discover-path"
                  class:ph-danger={row.state === 'failed'}
                  title={row.message}
                >
                  {row.message}
                </span>
              </div>
            {/each}
          </div>
        {:else}
          <p class="ph-hint">{t('report.empty')}</p>
        {/if}
        {#if copyHint}
          <p class="ph-hint" data-testid="report-copy-hint">{copyHint}</p>
        {/if}
        <div class="ph-row" style="margin-top:10px">
          <button class="ph-btn" data-testid="report-copy" onclick={copyReport}>
            {t('report.copy')}
          </button>
          <button class="ph-btn accent" data-testid="report-close" onclick={onclosereport}>
            {t('report.done')}
          </button>
        </div>
        {#if fallbackVisible}
          <textarea
            class="ph-report-fallback"
            readonly
            bind:this={fallbackEl}
            value={fallbackText}
            aria-label={t('report.title')}
          ></textarea>
        {/if}
      </div>
    {:else if view === 'scope' || !items.length}
      <div class="ph-section">
        <h3>
          {t('discover.scope')}
          <InfoTip
            testid="discover-scope-info"
            label={t('discover.hintLabel')}
            text={t('discover.hint', { dir: appsDir })}
          />
        </h3>
        <div class="ph-quick-scan" data-testid="quick-scan">
          <div class="ph-quick-copy">
            <strong>{t('discover.quickScan')}</strong>
            <span>{t('discover.quickScanHint', { dir: appsDir })}</span>
          </div>
          <button
            type="button"
            class="ph-btn accent"
            data-testid="scan-folder"
            disabled={scanning || busy}
            onclick={onfolderscan}
          >{t('discover.quickScanAction')}</button>
        </div>
        <p class="ph-hint ph-source-divider">{t('discover.orDrives')}</p>
        {#if items.length}
          <p class="ph-hint" data-testid="kept-results">
            {t('discover.keptResults', { n: items.length })}
          </p>
          <p class="ph-hint">{t('discover.rescanHint')}</p>
          <div class="ph-row">
            <button class="ph-btn" data-testid="back-to-results" onclick={() => (view = 'results')}>
              {t('discover.backToResults')}
            </button>
            <button class="ph-btn" data-testid="retry-scan" disabled={busy} onclick={retryScan}>
              {t('discover.rescan')}
            </button>
          </div>
        {/if}
        <div class="ph-scope">
          {#each drives as drive (drive)}
            <label class="ph-scope-item">
              <input
                class="ph-check"
                type="checkbox"
                data-testid="scope-drive"
                data-drive={drive}
                checked={roots[drive] ?? false}
                onchange={() => (roots = { ...roots, [drive]: !roots[drive] })}
              />
              {drive}
            </label>
          {/each}
        </div>
        <div class="ph-row" style="margin-top:10px">
          <button class="ph-btn accent" data-testid="start-scan" disabled={!drives.some((d) => roots[d])} onclick={startScan}>
            {t('discover.startScan')}
          </button>
        </div>
      </div>
    {:else}
    <div class="ph-discover-results">
      <div class="ph-section ph-discover-scroll">
        {#if cancelled}
          <p class="ph-hint ph-danger" data-testid="scan-incomplete">
            {t('discover.incompleteHint')}
          </p>
        {/if}
        <div class="ph-row">
          <div class="ph-search-wrap">
            <Icon name="search" size={15} />
            <input class="ph-search" bind:value={query} placeholder={t('discover.filterPlaceholder')} />
            {#if query}
              <button
                type="button"
                class="ph-input-clear"
                data-testid="discover-filter-clear"
                aria-label={t('app.clearSearch')}
                title={t('app.clearSearch')}
                onclick={() => (query = '')}
              >
                <Icon name="close" size={13} />
              </button>
            {/if}
          </div>
          {#if existingHidden > 0}
            <span class="ph-hint" data-testid="existing-hidden">
              {t('discover.existingHidden', { n: existingHidden })}
            </span>
          {/if}
        </div>
        <div class="ph-discover-tools">
          <button class="ph-btn" onclick={() => selectAll(true)}>{t('discover.selectAll')}</button>
          <button class="ph-btn" onclick={() => selectAll(false)}>{t('discover.clear')}</button>
          <span class="ph-hint" data-testid="found-count">
            {t('discover.foundCount', { n: visible.length })}
          </span>
          {#if mergedCount > 0}
            <span class="ph-hint" data-testid="merged-count" title={t('discover.mergedHint')}>
              {t('discover.merged', { n: mergedCount })}
            </span>
          {/if}
          <button
            class="ph-btn"
            data-testid="discover-classify"
            title={t('discover.classifyHint')}
            aria-busy={classifying}
            disabled={busy || classifying || !items.length}
            onclick={classifyResults}
          >
            {t('discover.classify')}
          </button>
          {#if classifying}
            <span
              class="ph-hint ph-classifying"
              data-testid="discover-classifying"
              role="status"
            >
              <span class="ph-spinner" data-testid="discover-spinner" aria-hidden="true"></span>
              {t('discover.classifying')}
            </span>
          {/if}
          {#if hasCategories}
            <label class="ph-hint" for="group-mode">{t('discover.groupMode')}</label>
            <select
              id="group-mode"
              class="ph-select ph-group-mode"
              data-testid="discover-group-mode"
              bind:value={groupMode}
            >
              <option value="category">{t('discover.groupCategory')}</option>
              <option value="source">{t('discover.groupSource')}</option>
            </select>
          {/if}
          <button class="ph-btn" data-testid="rescan-btn" disabled={busy} onclick={() => (view = 'scope')}>
            {t('discover.changeScope')}
          </button>
        </div>
        {#if classifyError}
          <p class="ph-hint ph-danger" data-testid="discover-classify-error">{classifyError}</p>
        {/if}
        {#if !visible.length}
          <p class="ph-hint">{t('discover.empty')}</p>
        {/if}
        <p class="ph-hint" data-testid="group-hint">
          {groupMode === 'category' ? t('discover.groupCategory') : t('discover.byFolder')}
        </p>
        <div class="ph-discover-list">
          {#each groups as group (group.key)}
            <div
              class="ph-discover-group"
              data-testid="discover-group"
              data-key={group.key}
              data-collapsed={isCollapsed(group.key) ? 'true' : 'false'}
            >
              <div class="ph-group-head">
                <button
                  class="ph-btn ph-group-toggle"
                  data-testid="discover-group-toggle"
                  type="button"
                  aria-expanded={!isCollapsed(group.key)}
                  onclick={() => toggleGroup(group.key)}
                >
                  {isCollapsed(group.key) ? '▸' : '▾'}
                </button>
                <span
                  class="ph-discover-name ph-group-label"
                  data-testid="discover-group-label"
                  title={group.hint}
                >
                  {group.key === '' ? t('discover.unknownSource') : group.label}
                </span>
                <span class="ph-hint" data-testid="discover-group-count">
                  {t('discover.groupCount', { n: group.items.length })}
                </span>
                <button
                  class="ph-btn"
                  data-testid="discover-group-select"
                  onclick={() => selectGroup(group)}
                >
                  {t('discover.groupSelect')}
                </button>
              </div>
              {#if !isCollapsed(group.key)}
                {#each group.items as item (item.path)}
                  <label class="ph-discover-row" data-testid="discover-row">
                    <input
                      class="ph-check"
                      type="checkbox"
                      checked={selected[item.path] ?? false}
                      disabled={busy}
                      onchange={() => toggle(item.path)}
                    />
                    <span class="ph-discover-name" class:ph-name-with-cat={hasCategories}>
                      {#if hasCategories}
                        <span class="ph-name-text">{item.name}</span>
                        {#if categoryOf(item)}
                          <span class="ph-badge" data-testid="discover-category">
                            {t(`category.${categoryOf(item)}`)}
                          </span>
                        {/if}
                      {:else}
                        {item.name}
                      {/if}
                    </span>
                    <span class="ph-badge">{item.kind === 'folder' ? t('discover.kindFolder') : t('discover.kindFile')}</span>
                    {#if item.duplicateCount}
                      <span class="ph-badge" data-testid="merged-badge" title={t('discover.mergedHint')}>
                        +{item.duplicateCount}
                      </span>
                    {/if}
                    <span class="ph-discover-size">{formatSize(item.size)}</span>
                    <span class="ph-discover-path" title={item.path}>{shortPath(item.path, 72)}</span>
                  </label>
                {/each}
              {/if}
            </div>
          {/each}
        </div>
      </div>

      <div class="ph-discover-actions" data-testid="discover-actions">
        <div class="ph-actions-status">
          <span class="ph-hint ph-discover-selected" data-testid="selected-count" aria-live="polite">
            {t('discover.selected', { n: chosen.length })}
          </span>
          {#if busy}
            <span class="ph-hint" data-testid="import-progress">{t('discover.working')} {progress}</span>
          {:else}
            <span class="ph-hint ph-mode-summary" data-testid="mode-summary">
              {t(`discover.modeShort.${mode}`)}
            </span>
          {/if}
        </div>
        <div class="ph-actions-controls">
          <label for="mode" class="ph-hint">{t('discover.mode')}</label>
          <select id="mode" class="ph-select ph-mode-select" bind:value={mode} disabled={busy}>
            <option value="link">{t('discover.modeLink')}</option>
            <option value="move">{t('discover.modeMove')}</option>
            <option value="register">{t('discover.modeRegister')}</option>
          </select>
          <InfoTip
            testid="mode-info"
            align="left"
            label={t('discover.modeInfo')}
            text={t(`discover.modeDetail.${mode}`)}
          />
          <label class="ph-scope-item">
            <input
              class="ph-check"
              type="checkbox"
              data-testid="import-apply-categories"
              bind:checked={writeCategories}
              disabled={busy}
            />
            {t('discover.writeCategories')}
          </label>
          <button
            class="ph-btn accent ph-discover-import"
            data-testid="import-btn"
            disabled={busy || !chosen.length}
            onclick={importChosen}
          >
            {t('discover.import')}{#if chosen.length}{' '}({chosen.length}){/if}
          </button>
        </div>
      </div>
    </div>
    {/if}
  </div>
</div>

<style>
  /* Layout only: colours/badges come from the existing global .ph-* classes. */

  .ph-quick-scan {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 11px;
    margin-bottom: 4px;
    border: var(--ph-border-width) solid var(--ph-accent);
    background: var(--ph-bg-alt);
  }

  .ph-quick-copy {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .ph-quick-copy strong {
    color: var(--ph-text);
    font-family: var(--ph-font-pixel);
    font-size: 12px;
    font-weight: 400;
  }

  .ph-quick-copy span {
    color: var(--ph-text-muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ph-source-divider {
    margin: 9px 0 5px;
  }

  /* REQ-0021 result groups */
  .ph-discover-group {
    border-bottom: var(--ph-border-width) solid var(--ph-border);
  }

  .ph-discover-group:last-child {
    border-bottom: none;
  }

  .ph-group-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    background: var(--ph-bg-alt);
    border-bottom: 1px solid var(--ph-border);
  }

  .ph-group-toggle {
    min-width: 26px;
    padding: 0 6px;
  }

  .ph-group-label {
    flex: 1;
  }

  /* REQ-0041: category chip lives inside the name cell so the row grid is unchanged. */
  .ph-discover-name.ph-name-with-cat {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  /* BUG-0025: "classifying…" gets a CSS spinner (no image/dependency). */
  .ph-classifying {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .ph-spinner {
    width: 12px;
    height: 12px;
    flex: none;
    border: 2px solid var(--ph-border);
    border-top-color: var(--ph-accent);
    border-radius: 50%;
    animation: ph-spin 0.8s linear infinite;
  }

  @keyframes ph-spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .ph-spinner {
      animation: none;
    }
  }

  .ph-name-text {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ph-discover-name.ph-name-with-cat .ph-badge {
    flex: none;
  }

  .ph-discover-tools {
    flex-wrap: wrap;
  }

  .ph-select.ph-group-mode {
    flex: none;
    min-width: 120px;
  }

  .ph-report-list {
    max-height: 46vh;
    overflow-y: auto;
    margin-top: 8px;
    border: var(--ph-border-width) solid var(--ph-border);
    background: var(--ph-panel);
  }

  .ph-report-row {
    display: grid;
    grid-template-columns: 1.2fr auto 2fr;
    gap: 10px;
    align-items: center;
    padding: 6px 8px;
    border-bottom: 1px solid var(--ph-border);
  }

  .ph-report-row:last-child {
    border-bottom: none;
  }

  .ph-report-fallback {
    width: 100%;
    min-height: 96px;
    margin-top: 8px;
    padding: 6px 8px;
    background: var(--ph-panel);
    color: var(--ph-text);
    border: var(--ph-border-width) solid var(--ph-border);
    font-size: 11px;
    resize: vertical;
  }

  /* BUG-0020: keep the import action visible while the result list scrolls. */
  .ph-discover-results {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }

  .ph-discover-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .ph-discover-scroll .ph-discover-list {
    max-height: none;
    overflow: visible;
  }

  .ph-discover-actions {
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
    padding: 8px 14px;
    border-top: var(--ph-border-width) solid var(--ph-border);
    background: var(--ph-bg-alt);
  }

  .ph-actions-status {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .ph-actions-controls {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .ph-mode-summary {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ph-mode-select {
    flex: 0 0 auto;
    width: auto;
    min-width: 140px;
  }

  .ph-discover-selected {
    color: var(--ph-text);
    font-weight: bold;
    white-space: nowrap;
  }

  .ph-discover-import {
    min-width: 120px;
    margin-left: auto;
  }
</style>
