<script lang="ts">
  import type { AppView, Collection, TagStyle } from '../lib/types'
  import type { Translator } from '../lib/i18n'
  import { fallbackLetter, hashColor } from '../lib/theme'
  import { tagCss, tagText } from '../lib/tag-style'

  let {
    apps,
    selectedIds,
    primary,
    collections,
    tagStyles,
    t,
    onlaunch,
    onreveal,
    onedit,
    ontags,
    oncategory,
    onautostart,
    onbatchtag,
    onbatchcategory,
    onbatchcollection,
    onclear
  }: {
    apps: AppView[]
    selectedIds: string[]
    primary: AppView | null
    collections: Collection[]
    tagStyles: Record<string, TagStyle>
    t: Translator
    onlaunch: (app: AppView) => void
    onreveal: (app: AppView) => void
    onedit: (app: AppView) => void
    ontags: (app: AppView) => void
    oncategory: () => void
    onautostart: (app: AppView) => void
    onbatchtag: () => void
    onbatchcategory: () => void
    onbatchcollection: () => void
    onclear: () => void
  } = $props()

  let count = $derived(selectedIds.length)
  let autoCount = $derived(apps.filter((app) => app.autostart).length)
  let missingCount = $derived(apps.filter((app) => !app.exists).length)
  let groupsOf = $derived(
    primary ? collections.filter((collection) => collection.appIds.includes(primary.id)) : []
  )
</script>

<aside class="ph-inspector" data-testid="inspector" aria-label={t('inspector.title')}>
  <div class="inspector-head">
    <span>{t('inspector.title')}</span>
    {#if count === 1}<span class="key">↵ {t('inspector.launch')}</span>{/if}
  </div>

  {#if count > 1}
    <div class="multi">
      <div class="multi-count">{count}</div>
      <div class="multi-label">{t('inspector.selectedApps')}</div>
      <div class="multi-names">{selectedIds.map((id) => apps.find((app) => app.id === id)?.name).filter(Boolean).slice(0, 5).join(' · ')}</div>
      <div class="actions column">
        <button class="btn primary" onclick={onbatchtag}>{t('tags.quick')}</button>
        <button class="btn" onclick={onbatchcollection}>{t('collections.addSelected')}</button>
        <button class="btn" onclick={onbatchcategory}>{t('categoryPicker.title')}</button>
        <button class="btn quiet" onclick={onclear}>{t('app.selectionClear')}</button>
      </div>
    </div>
  {:else if primary}
    <div class="hero">
      <span class="hero-icon" style:background={primary.icon ? undefined : hashColor(primary.name)}>
        {#if primary.icon}<img src={primary.icon} alt="" />{:else}{fallbackLetter(primary.name)}{/if}
      </span>
      <div class="hero-copy">
        <div class="hero-name">{primary.name}</div>
        <div class="hero-meta">
          {#if !primary.exists}<span class="bad">{t('card.missing')}</span>{:else}<span class="ok">● {t('inspector.ready')}</span>{/if}
          {#if primary.autostart} · {t('inspector.autostartOn')}{/if}
        </div>
      </div>
    </div>
    <div class="actions">
      <button class="btn primary" onclick={() => onlaunch(primary)}>{t('inspector.launch')}</button>
      <button class="btn" onclick={() => onreveal(primary)}>{t('inspector.reveal')}</button>
    </div>
    <div class="detail">
      <div class="row"><span>{t('inspector.path')}</span><b title={primary.path}>{primary.path}</b></div>
      <div class="row"><span>{t('inspector.category')}</span><b class="amber">{primary.category ? t(`category.${primary.category}`) : t('category.unclassified')}</b></div>
      <div class="row"><span>{t('inspector.groups')}</span><b>{groupsOf.length ? groupsOf.map((group) => group.name).join(' · ') : '—'}</b></div>
      <div class="row"><span>{t('inspector.autostart')}</span><b class:amber={primary.autostart}>{primary.autostart ? t('inspector.on') : t('inspector.off')}</b></div>
      <div class="row"><span>{t('inspector.admin')}</span><b class:amber={primary.runAsAdmin}>{primary.runAsAdmin ? t('inspector.on') : t('inspector.off')}</b></div>
    </div>
    {#if primary.tags.length}
      <div class="tag-list">
        {#each primary.tags as tag (tag)}
          {@const style = tagStyles[tag]}
          <span class="tag" style={tagCss(style)}>{tagText(style, tag)}</span>
        {/each}
      </div>
    {/if}
    <div class="actions column secondary">
      <button class="btn" onclick={() => ontags(primary)}>{t('tags.edit')}</button>
      <button class="btn" onclick={oncategory}>{t('menu.category')}</button>
      <button class="btn" onclick={() => onautostart(primary)}>
        {primary.autostart ? t('inspector.turnOffAutostart') : t('inspector.turnOnAutostart')}
      </button>
      <button class="btn" onclick={() => onedit(primary)}>{t('menu.edit')}</button>
    </div>
    <div class="shortcuts">↑↓ {t('inspector.select')} · Shift+Enter {t('inspector.reveal')} · {t('inspector.rightClick')}</div>
  {:else}
    <div class="empty">
      <div class="empty-art">[ + ]</div>
      <div class="empty-title">{t('inspector.emptyTitle')}</div>
      <div class="empty-hint">{t('inspector.emptyHint')}</div>
    </div>
    <div class="summary">
      <div><b>{apps.length}</b><span>{t('inspector.apps')}</span></div>
      <div><b class="amber">{autoCount}</b><span>{t('inspector.auto')}</span></div>
      <div><b class:bad={missingCount > 0}>{missingCount}</b><span>{t('inspector.missing')}</span></div>
    </div>
  {/if}
</aside>

<style>
  .ph-inspector {
    flex: 0 0 282px;
    width: 282px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    border-left: var(--ph-border-width) solid var(--ph-border);
    background: var(--ph-panel);
    overflow-y: auto;
  }

  .inspector-head {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 44px;
    padding: 0 15px;
    border-bottom: var(--ph-border-width) solid var(--ph-border);
    color: var(--ph-text-muted);
    font-family: var(--ph-font-pixel);
    font-size: 11px;
    letter-spacing: .06em;
  }

  .inspector-head .key { color: var(--ph-text-muted); font-size: 10px; }

  .hero { display: flex; gap: 11px; padding: 15px; border-bottom: var(--ph-border-width) solid var(--ph-border); }
  .hero-icon {
    width: 58px; height: 58px; flex: none;
    display: grid; place-items: center; overflow: hidden;
    border: 1px solid var(--ph-border-light);
    background: var(--ph-bg-alt);
    color: var(--ph-accent);
    font-family: var(--ph-font-pixel); font-size: 22px;
  }
  .hero-icon img { width: 100%; height: 100%; object-fit: contain; }
  .hero-name { font-family: var(--ph-font-pixel); font-size: 15px; margin: 2px 0 7px; color: var(--ph-text); }
  .hero-meta { color: var(--ph-text-muted); font-size: 11px; line-height: 1.6; }
  .hero-meta .ok { color: var(--ph-accent); }
  .hero-meta .bad { color: var(--ph-danger); }

  .actions { display: flex; gap: 6px; padding: 12px 15px 0; }
  .actions.column { flex-direction: column; }
  .actions.secondary { padding-top: 10px; }
  .btn {
    flex: 1;
    min-height: 30px;
    padding: 0 9px;
    border: 1px solid var(--ph-border-light);
    background: var(--ph-bg-alt);
    color: var(--ph-text);
    font: 12px var(--ph-font);
    cursor: pointer;
    text-align: center;
  }
  .btn:hover { border-color: var(--ph-accent); color: var(--ph-accent); }
  .btn.primary { background: var(--ph-accent); border-color: var(--ph-accent); color: #1a1409; font-weight: 600; }
  .btn.primary:hover { background: #ffd06b; color: #1a1409; }
  .btn.quiet { color: var(--ph-text-muted); background: transparent; }

  .detail { padding: 7px 15px 0; }
  .detail .row {
    display: flex; align-items: baseline; justify-content: space-between; gap: 8px;
    padding: 6px 0; border-bottom: 1px solid var(--ph-border);
    font-size: 11px;
  }
  .detail .row span { color: var(--ph-text-muted); flex: none; }
  .detail .row b { color: var(--ph-text); font-weight: 400; max-width: 170px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .detail .row b.amber, .amber { color: var(--ph-accent); }
  .bad { color: var(--ph-danger); }

  .tag-list { display: flex; flex-wrap: wrap; gap: 4px; padding: 10px 15px 0; }
  .tag { border: 1px solid var(--ph-border); padding: 1px 5px; font-size: 10px; color: var(--ph-text-muted); background: var(--ph-bg-alt); }

  .shortcuts { margin-top: auto; border-top: var(--ph-border-width) solid var(--ph-border); padding: 10px 15px; color: var(--ph-text-muted); font-family: var(--ph-font-pixel); font-size: 10px; line-height: 1.8; }

  .multi { padding: 18px 15px; }
  .multi-count { font-family: var(--ph-font-pixel); font-size: 42px; line-height: 1; color: var(--ph-accent); }
  .multi-label { margin-top: 5px; font-family: var(--ph-font-pixel); font-size: 11px; color: var(--ph-text-muted); }
  .multi-names { margin-top: 12px; color: var(--ph-text-muted); font-size: 11px; line-height: 1.7; overflow: hidden; display: -webkit-box; -webkit-line-clamp: 4; line-clamp: 4; -webkit-box-orient: vertical; }
  .multi .actions { padding-left: 0; padding-right: 0; }

  .empty { padding: 26px 16px 16px; text-align: center; }
  .empty-art { font-family: var(--ph-font-pixel); color: var(--ph-border-light); font-size: 28px; }
  .empty-title { margin-top: 12px; color: var(--ph-text); font-family: var(--ph-font-pixel); font-size: 12px; }
  .empty-hint { margin-top: 7px; color: var(--ph-text-muted); font-size: 11px; line-height: 1.6; }
  .summary { margin-top: auto; display: grid; grid-template-columns: repeat(3, 1fr); border-top: var(--ph-border-width) solid var(--ph-border); }
  .summary div { padding: 12px 10px; text-align: center; border-right: 1px solid var(--ph-border); }
  .summary div:last-child { border-right: 0; }
  .summary b { display: block; font-family: var(--ph-font-pixel); font-size: 20px; font-weight: 400; color: var(--ph-text); }
  .summary span { display: block; margin-top: 3px; font-family: var(--ph-font-pixel); font-size: 9px; color: var(--ph-text-muted); }
  .summary b.amber { color: var(--ph-accent); }
  .summary b.bad { color: var(--ph-danger); }

  @media (max-width: 1120px) { .ph-inspector { display: none; } }
</style>
