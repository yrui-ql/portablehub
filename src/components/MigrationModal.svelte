<script lang="ts">
  // REQ-0023 · 迁移弹窗：应用来源体检 + 自包含迁移包导出。
  //
  // 契约：props { t, onclose }；数据通过 api.auditApps() / api.exportBundle(target)
  // 获取，弹窗自行管理忙碌态、错误与报告展示。
  import { onMount, tick } from 'svelte'
  import type { AppAudit, AppSourceKind, ExportReport } from '../lib/types'
  import type { Translator } from '../lib/i18n'
  import { api } from '../lib/api'
  import { pickDirectory } from '../lib/desktop'
  import InfoTip from './InfoTip.svelte'

  let { t, onclose }: { t: Translator; onclose: () => void } = $props()

  let audit = $state<AppAudit[]>([])
  let auditing = $state(true)
  let target = $state('')
  let exporting = $state(false)
  let report = $state<ExportReport | null>(null)
  let error = $state('')
  let copied = $state(false)
  let clipFallback = $state('')
  let fallbackEl: HTMLTextAreaElement | undefined = $state()

  let counts = $derived({
    local: audit.filter((item) => item.kind === 'local').length,
    linked: audit.filter((item) => item.kind === 'linked').length,
    external: audit.filter((item) => item.kind === 'external').length,
    missing: audit.filter((item) => item.kind === 'missing').length
  })
  let failures = $derived(report ? report.items.filter((item) => !item.ok) : [])

  onMount(() => {
    void loadAudit()
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      if (!exporting) onclose()
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  })

  async function loadAudit() {
    auditing = true
    try {
      audit = await api.auditApps()
    } catch (reason) {
      error = readableError(reason)
    } finally {
      auditing = false
    }
  }

  function kindLabel(kind: AppSourceKind): string {
    return t(`migrate.kind.${kind}`)
  }

  /**
   * Backend errors are already human readable (Chinese in this build); map the
   * two documented guards to the dialog's own strings so both the real command
   * and the E2E mock produce the same message.
   */
  function readableError(reason: unknown): string {
    const raw = reason instanceof Error ? reason.message : String(reason)
    const lower = raw.toLowerCase()
    if (lower.includes('inside') || raw.includes('程序目录内')) return t('migrate.guardInside')
    if (lower.includes('not empty') || raw.includes('必须为空') || raw.includes('非空')) {
      return t('migrate.guardEmpty')
    }
    return raw
  }

  function reportText(value: ExportReport): string {
    const lines = [t('migrate.report')]
    for (const item of value.items) {
      lines.push(
        `${item.name}\t${item.ok ? 'ok' : 'failed'}\t${item.from}→${item.to}\t${item.message}`
      )
    }
    lines.push(value.reportPath)
    return lines.join('\n')
  }

  async function copyReport() {
    if (!report) return
    copied = false
    clipFallback = ''
    const text = reportText(report)
    try {
      await navigator.clipboard.writeText(text)
      copied = true
    } catch {
      // Clipboard blocked (permission / plain browser): expose selectable text.
      clipFallback = text
      await tick()
      fallbackEl?.focus()
      fallbackEl?.select()
    }
  }

  async function browseTarget() {
    const picked = await pickDirectory(target.trim() || undefined)
    if (picked) target = picked
  }

  async function runExport() {
    if (exporting) return
    error = ''
    copied = false
    clipFallback = ''
    report = null
    const value = target.trim()
    if (!value) {
      error = t('migrate.targetRequired')
      return
    }
    exporting = true
    try {
      report = await api.exportBundle(value)
    } catch (reason) {
      error = readableError(reason)
    } finally {
      exporting = false
    }
  }
</script>

<div
  class="ph-overlay"
  role="presentation"
  onclick={(event) => {
    if (event.target === event.currentTarget && !exporting) onclose()
  }}
>
  <div
    class="ph-settings ph-migrate"
    role="dialog"
    aria-modal="true"
    aria-label={t('migrate.title')}
    data-testid="migration-modal"
  >
    <div class="ph-settings-head">
      <strong>{t('migrate.title')}</strong>
      <button class="ph-btn" data-testid="migrate-close" disabled={exporting} onclick={onclose}>
        {t('migrate.close')}
      </button>
    </div>

    <div class="ph-migrate-body">
      <section class="ph-section">
        <h3>
          {t('migrate.audit')}
          <InfoTip
            testid="migrate-hint-info"
            align="left"
            label={t('migrate.hintLabel')}
            text={t('migrate.hint')}
          />
        </h3>

        {#if auditing}
          <p class="ph-hint" data-testid="audit-loading">{t('migrate.auditing')}</p>
        {:else if !audit.length}
          <p class="ph-hint" data-testid="audit-empty">{t('migrate.empty')}</p>
        {:else}
          <p class="ph-hint" data-testid="audit-summary">
            {t('migrate.kindSummary', counts)}
          </p>
          <div class="ph-migrate-list" data-testid="audit-list">
            {#each audit as item (item.id)}
              <div class="ph-migrate-row" data-testid="audit-row" data-kind={item.kind}>
                <span class="ph-migrate-name" title={item.name}>{item.name}</span>
                <span
                  class="ph-badge"
                  class:missing={item.kind === 'missing'}
                  data-testid="audit-kind"
                  data-kind={item.kind}
                >
                  {kindLabel(item.kind)}
                </span>
                <span class="ph-migrate-path" data-testid="audit-path" title={item.path}>
                  {item.path}
                </span>
                <span class="ph-migrate-path" data-testid="audit-target" title={item.target}>
                  {item.target}
                </span>
              </div>
            {/each}
          </div>
        {/if}
      </section>

      <section class="ph-section">
        <h3>{t('migrate.target')}</h3>
        <div class="ph-row">
          <input
            class="ph-input"
            data-testid="export-target"
            bind:value={target}
            spellcheck="false"
            autocomplete="off"
            disabled={exporting}
            placeholder={t('migrate.target')}
          />
          <button
            type="button"
            class="ph-btn"
            data-testid="export-target-browse"
            disabled={exporting}
            onclick={browseTarget}
          >
            {t('migrate.browse')}
          </button>
        </div>
        <p class="ph-hint">{t('migrate.exportHint')}</p>

        <div class="ph-row">
          <button
            type="button"
            class="ph-btn accent"
            data-testid="export-btn"
            disabled={exporting || auditing}
            onclick={runExport}
          >
            {t('migrate.export')}
          </button>
          {#if exporting}
            <span class="ph-hint" data-testid="export-busy">{t('migrate.exporting')}</span>
          {/if}
        </div>

        {#if error}
          <p class="ph-hint ph-danger" role="alert" data-testid="migrate-error">{error}</p>
        {/if}
      </section>

      {#if report}
        <section class="ph-section" data-testid="export-report">
          <h3>{t('migrate.report')}</h3>
          <div class="ph-row">
            <strong data-testid="export-done">{t('migrate.done', { n: report.copied })}</strong>
            {#if report.failed > 0}
              <span class="ph-badge missing" data-testid="export-failures">
                {t('migrate.failures', { n: report.failed })}
              </span>
            {/if}
          </div>
          <p class="ph-hint ph-migrate-report-path" data-testid="export-report-path" title={report.reportPath}>
            {report.reportPath}
          </p>

          <div class="ph-migrate-list ph-migrate-results">
            {#each report.items as item (item.name)}
              <div
                class="ph-migrate-row"
                class:failed={!item.ok}
                data-testid="export-row"
                data-ok={item.ok ? 'true' : 'false'}
              >
                <span class="ph-migrate-name" title={item.name}>{item.name}</span>
                <span class="ph-migrate-status ph-danger" class:ok={item.ok}>
                  {item.message}
                </span>
                <span class="ph-migrate-path" title={`${item.from} → ${item.to}`}>
                  {item.from} → {item.to}
                </span>
              </div>
            {/each}
          </div>

          {#if failures.length}
            <p class="ph-hint ph-danger" data-testid="export-failure-names">
              {failures.map((item) => item.name).join(', ')}
            </p>
          {/if}

          <div class="ph-row" style="margin-top:10px">
            <button type="button" class="ph-btn" data-testid="copy-report" onclick={copyReport}>
              {t('migrate.copyReport')}
            </button>
            {#if copied}
              <span class="ph-hint" data-testid="copy-report-hint">{t('migrate.copied')}</span>
            {/if}
          </div>
          {#if clipFallback}
            <textarea
              class="ph-migrate-fallback"
              data-testid="report-fallback"
              readonly
              bind:this={fallbackEl}
            >{clipFallback}</textarea>
          {/if}
        </section>
      {/if}
    </div>
  </div>
</div>

<style>
  .ph-migrate {
    width: min(760px, 94vw);
  }

  .ph-migrate-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .ph-migrate-list {
    max-height: 30vh;
    overflow-y: auto;
    border: var(--ph-border-width) solid var(--ph-border);
    background: var(--ph-panel);
  }

  .ph-migrate-results {
    max-height: 24vh;
  }

  .ph-migrate-row {
    display: grid;
    grid-template-columns: minmax(72px, 0.8fr) auto minmax(0, 1.3fr) minmax(0, 1.3fr);
    gap: 10px;
    align-items: center;
    padding: 6px 8px;
    border-bottom: 1px solid var(--ph-border);
  }

  .ph-migrate-row:last-child {
    border-bottom: 0;
  }

  .ph-migrate-row:hover {
    background: var(--ph-panel-hover);
  }

  .ph-migrate-row.failed {
    border-left: 3px solid var(--ph-danger);
  }

  /* .ph-badge is absolute for grid cards; in these rows it belongs in the flow. */
  .ph-migrate-row .ph-badge {
    position: static;
    top: auto;
    right: auto;
    white-space: nowrap;
  }

  .ph-migrate-row .ph-badge[data-kind='local'] {
    border-color: var(--ph-accent-2);
    color: var(--ph-accent-2);
  }

  .ph-migrate-row .ph-badge[data-kind='linked'] {
    border-color: var(--ph-accent);
    color: var(--ph-accent);
  }

  .ph-migrate-row .ph-badge[data-kind='external'] {
    border-color: var(--ph-border-light);
    color: var(--ph-text-muted);
  }

  .ph-migrate-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: bold;
  }

  .ph-migrate-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ph-text-muted);
    font-size: 11px;
  }

  .ph-migrate-status {
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ph-migrate-status.ok {
    color: var(--ph-accent-2);
  }

  .ph-migrate-results .ph-migrate-row {
    grid-template-columns: minmax(72px, 0.7fr) minmax(0, 0.9fr) minmax(0, 1.6fr);
  }

  .ph-migrate-report-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ph-migrate-fallback {
    width: 100%;
    height: 120px;
    margin-top: 8px;
    padding: 8px;
    background: var(--ph-panel);
    border: var(--ph-border-width) solid var(--ph-border);
    color: var(--ph-text);
    font-family: var(--ph-font);
    font-size: 11px;
    resize: vertical;
  }

  @media (max-width: 720px) {
    .ph-migrate-row {
      grid-template-columns: minmax(64px, 0.8fr) auto minmax(0, 1.2fr);
    }

    /* Keep the resolved target readable in narrow windows. */
    .ph-migrate-row .ph-migrate-path:last-child {
      display: none;
    }
  }
</style>
