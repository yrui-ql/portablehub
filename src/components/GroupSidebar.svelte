<script lang="ts">
  // REQ-0022 · source-group navigation for the main screen.
  // Controlled component: the parent owns the active group key and the counts.
  import type { Translator } from '../lib/i18n'

  const ALL_GROUP_KEY = 'all'

  let {
    groups,
    active,
    total,
    t,
    onselect
  }: {
    groups: { key: string; label: string; hint: string; count: number }[]
    active: string
    total: number
    t: Translator
    onselect: (key: string) => void
  } = $props()

  // The grouping module leaves `label` blank for the hub's own folder so the
  // component can localize it here.
  const LOCAL_GROUP_KEY = 'local'

  function labelOf(group: { key: string; label: string }): string {
    return group.key === LOCAL_GROUP_KEY ? t('app.groupLocal') : group.label
  }
</script>

<aside class="group-sidebar" data-testid="group-sidebar" aria-label={t('app.groupsTitle')}>
  <div class="group-sidebar-title">{t('app.groupsTitle')}</div>
  <nav class="group-nav">
    <button
      type="button"
      class="group-item"
      class:active={active === ALL_GROUP_KEY}
      data-testid="group-nav-all"
      aria-current={active === ALL_GROUP_KEY ? 'true' : undefined}
      onclick={() => onselect(ALL_GROUP_KEY)}
    >
      <span class="group-name">{t('app.groupAll')}</span>
      <span class="group-count">{total}</span>
    </button>
    {#each groups as group (group.key)}
      <button
        type="button"
        class="group-item"
        class:active={active === group.key}
        data-testid="group-nav-item"
        data-group-key={group.key}
        title={group.hint || undefined}
        aria-current={active === group.key ? 'true' : undefined}
        onclick={() => onselect(group.key)}
      >
        <span class="group-name">{labelOf(group)}</span>
        <span class="group-count">{group.count}</span>
      </button>
    {/each}
  </nav>
</aside>

<style>
  .group-sidebar {
    display: flex;
    flex-direction: column;
    flex: 0 0 196px;
    width: 196px;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 8px;
    background: var(--ph-bg-alt);
    border-right: var(--ph-border-width) solid var(--ph-border);
  }

  .group-sidebar-title {
    padding: 0 6px 8px;
    color: var(--ph-accent);
    font-size: 13px;
    letter-spacing: 1px;
  }

  .group-nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .group-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    width: 100%;
    min-width: 0;
    padding: 6px 8px;
    border: 0;
    border-left: 3px solid transparent;
    background: transparent;
    color: var(--ph-text-muted);
    text-align: left;
    cursor: pointer;
  }

  .group-item:hover {
    background: var(--ph-panel-hover);
    color: var(--ph-text);
  }

  .group-item.active {
    border-left-color: var(--ph-accent);
    background: var(--ph-panel);
    color: var(--ph-accent);
    font-weight: bold;
  }

  .group-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .group-count {
    flex: 0 0 auto;
    padding: 0 4px;
    border: 1px solid var(--ph-border);
    background: var(--ph-bg-alt);
    color: var(--ph-text-muted);
    font-size: 10px;
  }

  .group-item.active .group-count {
    border-color: var(--ph-accent);
    color: var(--ph-accent);
  }
</style>
