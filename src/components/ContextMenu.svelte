<script lang="ts">
  import type { AppView } from '../lib/types'
  import type { Translator } from '../lib/i18n'
  import type { TagCount } from '../lib/tags'

  let {
    x,
    y,
    app,
    tags,
    t,
    onclose,
    onlaunch,
    onreveal,
    onedit,
    ontags,
    oncollection,
    oncategory,
    onquicktag,
    onquicktags,
    onautostart,
    onremove
  }: {
    x: number
    y: number
    app: AppView
    tags: TagCount[]
    t: Translator
    onclose: () => void
    onlaunch: (app: AppView) => void
    onreveal: (app: AppView) => void
    onedit: (app: AppView) => void
    ontags: (app: AppView) => void
    oncollection: (app: AppView) => void
    oncategory: (app: AppView) => void
    onquicktag: (tag: string) => void
    onquicktags: (app: AppView) => void
    onautostart: (app: AppView) => void
    onremove: (app: AppView) => void
  } = $props()

  let left = $derived(Math.min(x, window.innerWidth - 190))
  // Base items + up to 8 quick-tag entries; keep the whole menu inside the viewport.
  let top = $derived(Math.min(y, window.innerHeight - (330 + Math.min(tags.length, 8) * 30)))
</script>

<div class="ph-menu" style:left={`${left}px`} style:top={`${top}px`} role="menu">
  <button class="ph-menu-item" onclick={() => { onlaunch(app); onclose() }}>{t('menu.launch')}</button>
  <button class="ph-menu-item" onclick={() => { onreveal(app); onclose() }}>{t('menu.reveal')}</button>
  <button class="ph-menu-item" data-testid="menu-edit" onclick={() => { onedit(app); onclose() }}>{t('menu.edit')}</button>
  <button class="ph-menu-item" data-testid="menu-tags" onclick={() => { ontags(app); onclose() }}>{t('tags.edit')}</button>
  <button class="ph-menu-item" data-testid="menu-category" onclick={() => { oncategory(app); onclose() }}>{t('menu.category')}</button>
  <button class="ph-menu-item" data-testid="menu-collection" onclick={() => { oncollection(app); onclose() }}>{t('collections.addSelected')}…</button>
  <div class="ph-menu-sep"></div>
  <div class="ph-menu-label">{t('tags.quick')}</div>
  {#each tags.slice(0, 8) as item (item.tag)}
    <button
      class="ph-menu-item ph-menu-quick-tag"
      data-testid="menu-quick-tag"
      data-tag={item.tag}
      onclick={() => { onquicktag(item.tag); onclose() }}
    >
      <span class="ph-menu-mark">{app.tags.includes(item.tag) ? '✓' : '+'}</span>
      {item.tag}
    </button>
  {/each}
  <button
    class="ph-menu-item"
    data-testid="menu-quick-tags-new"
    onclick={() => { onquicktags(app); onclose() }}
  >{t('tags.quickNew')}</button>
  <div class="ph-menu-sep"></div>
  <button class="ph-menu-item" onclick={() => { onautostart(app); onclose() }}>
    {app.autostart ? t('menu.autostartOff') : t('menu.autostartOn')}
  </button>
  <div class="ph-menu-sep"></div>
  <button class="ph-menu-item ph-danger" onclick={() => { onremove(app); onclose() }}>
    {t('menu.remove')}
  </button>
</div>
