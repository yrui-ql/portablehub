<script lang="ts">
  // REQ-0039 · smart categorize review dialog.
  //
  // Nothing is written until the user confirms: classifyApps() proposes a
  // category per app (local rules, or AI when configured), the rows can be
  // re-categorized / deselected, and only the selected rows are applied.
  import { onMount } from 'svelte'
  import type { AppCategory, AppView, CategoryApply } from '../lib/types'
  import type { Translator } from '../lib/i18n'
  import { api } from '../lib/api'
  import InfoTip from './InfoTip.svelte'

  // Fixed category order: review rows, selects and smart groups all use it.
  const CATEGORIES: AppCategory[] = [
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

  interface Row {
    id: string
    name: string
    category: AppCategory
    confidence: number
    reason: string
    source: 'rules' | 'ai'
    selected: boolean
  }

  let {
    apps,
    t,
    onapply,
    onclose
  }: {
    apps: AppView[]
    t: Translator
    onapply: (apps: AppView[]) => void
    onclose: () => void
  } = $props()

  let rows = $state<Row[]>([])
  let loading = $state(true)
  let applying = $state(false)
  let error = $state('')

  let selectedCount = $derived(rows.filter((row) => row.selected).length)
  let canApply = $derived(!loading && !applying && selectedCount > 0)

  onMount(() => {
    void load()
    // Esc closes the dialog; capture so the main screen's global Esc handler
    // (clear search / selection) never fires while the dialog is open.
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      if (!applying) onclose()
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  })

  function errorText(reason: unknown): string {
    return reason instanceof Error ? reason.message : String(reason)
  }

  async function load() {
    loading = true
    error = ''
    try {
      const known = new Map(apps.map((app) => [app.id, app]))
      const suggestions = await api.classifyApps()
      rows = suggestions
        .filter((suggestion) => known.has(suggestion.id))
        .map((suggestion) => ({
          id: suggestion.id,
          name: known.get(suggestion.id)!.name,
          category: suggestion.category,
          confidence: suggestion.confidence,
          reason: suggestion.reason,
          source: suggestion.source,
          selected: true
        }))
    } catch (reason) {
      error = errorText(reason)
    } finally {
      loading = false
    }
  }

  function setAll(value: boolean) {
    rows = rows.map((row) => ({ ...row, selected: value }))
  }

  function toggleRow(id: string) {
    rows = rows.map((row) => (row.id === id ? { ...row, selected: !row.selected } : row))
  }

  function changeCategory(id: string, value: string) {
    rows = rows.map((row) =>
      row.id === id ? { ...row, category: value as AppCategory } : row
    )
  }

  async function applySelected() {
    if (applying || !selectedCount) return
    const chosen: CategoryApply[] = rows
      .filter((row) => row.selected)
      .map((row) => ({ id: row.id, category: row.category }))
    applying = true
    error = ''
    try {
      onapply(await api.applyCategories(chosen))
    } catch (reason) {
      error = errorText(reason)
    } finally {
      applying = false
    }
  }
</script>

<div
  class="ph-overlay"
  role="presentation"
  onclick={(event) => {
    if (event.target === event.currentTarget && !applying) onclose()
  }}
>
  <div
    class="ph-settings ph-smart"
    role="dialog"
    aria-modal="true"
    aria-label={t('smart.title')}
    data-testid="smart-modal"
  >
    <div class="ph-settings-head">
      <strong>{t('smart.title')}</strong>
      <InfoTip testid="smart-hint-info" label={t('smart.title')} text={t('smart.hint')} />
    </div>

    <div class="ph-smart-body">
      <section class="ph-section">
        {#if loading}
          <p class="ph-hint" data-testid="smart-loading" aria-live="polite">{t('smart.loading')}</p>
        {:else if !rows.length}
          <p class="ph-hint" data-testid="smart-empty">{t('smart.empty')}</p>
          {#if error}
            <p class="ph-hint ph-danger" role="alert" data-testid="smart-error">{error}</p>
          {/if}
        {:else}
          <div class="ph-row">
            <button
              type="button"
              class="ph-btn"
              data-testid="smart-select-all"
              onclick={() => setAll(true)}
            >{t('smart.selectAll')}</button>
            <button
              type="button"
              class="ph-btn"
              data-testid="smart-clear"
              onclick={() => setAll(false)}
            >{t('smart.clear')}</button>
            <span class="ph-hint">{t('smart.applySelected', { n: selectedCount })}</span>
          </div>

          <div class="ph-smart-list">
            {#each rows as row (row.id)}
              <div class="ph-smart-row" data-testid="smart-row" data-app-name={row.name}>
                <input
                  type="checkbox"
                  class="ph-check"
                  data-testid="smart-row-select"
                  aria-label={row.name}
                  checked={row.selected}
                  onchange={() => toggleRow(row.id)}
                />
                <span class="ph-smart-name" title={row.name}>{row.name}</span>
                <select
                  class="ph-select"
                  data-testid="smart-category"
                  aria-label={t('smart.category')}
                  value={row.category}
                  onchange={(event) => changeCategory(row.id, event.currentTarget.value)}
                >
                  {#each CATEGORIES as category (category)}
                    <option value={category}>{t(`category.${category}`)}</option>
                  {/each}
                </select>
                <span class="ph-smart-confidence" title={row.reason}>
                  {t('smart.confidence', { pct: Math.round(row.confidence * 100) })}
                </span>
                <span class="ph-smart-source">
                  {row.source === 'ai' ? t('smart.sourceAi') : t('smart.sourceRules')}
                </span>
              </div>
            {/each}
          </div>

          {#if error}
            <p class="ph-hint ph-danger" role="alert" data-testid="smart-error">{error}</p>
          {/if}
        {/if}
      </section>
    </div>

    <div class="ph-smart-foot">
      <button type="button" class="ph-btn" data-testid="smart-cancel" disabled={applying} onclick={onclose}>
        {t('tags.cancel')}
      </button>
      <button
        type="button"
        class="ph-btn accent"
        data-testid="smart-apply"
        disabled={!canApply}
        onclick={applySelected}
      >{t('smart.applySelected', { n: selectedCount })}</button>
    </div>
  </div>
</div>

<style>
  .ph-smart {
    width: min(760px, 94vw);
  }

  .ph-smart-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .ph-smart-list {
    max-height: 46vh;
    overflow-y: auto;
    border: var(--ph-border-width) solid var(--ph-border);
    background: var(--ph-panel);
  }

  .ph-smart-row {
    display: grid;
    grid-template-columns: 18px minmax(64px, 0.9fr) minmax(120px, 1fr) auto auto;
    gap: 10px;
    align-items: center;
    padding: 6px 8px;
    border-bottom: 1px solid var(--ph-border);
  }

  .ph-smart-row:last-child {
    border-bottom: 0;
  }

  .ph-smart-row:hover {
    background: var(--ph-panel-hover);
  }

  .ph-smart-row .ph-select {
    flex: 0 0 auto;
    min-width: 0;
    height: 26px;
  }

  .ph-smart-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: bold;
  }

  .ph-smart-confidence,
  .ph-smart-source {
    color: var(--ph-text-muted);
    font-size: 11px;
    white-space: nowrap;
    text-align: right;
  }

  .ph-smart-foot {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding: 12px 14px;
    border-top: var(--ph-border-width) solid var(--ph-border);
  }
</style>
