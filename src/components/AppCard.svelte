<script lang="ts">
  import type { AppView } from '../lib/types'
  import type { TagStyle } from '../lib/types'
  import type { Translator } from '../lib/i18n'
  import { fallbackLetter, hashColor } from '../lib/theme'
  import { tagCss, tagText } from '../lib/tag-style'

  let {
    app,
    t,
    selected,
    tagStyles,
    onselect,
    onlaunch,
    onmenu
  }: {
    app: AppView
    t: Translator
    selected: boolean
    tagStyles: Record<string, TagStyle>
    onselect: (app: AppView, event: MouseEvent) => void
    onlaunch: (app: AppView) => void
    onmenu: (event: MouseEvent, app: AppView) => void
  } = $props()

  // REQ-0032: keep the card compact — at most three tag chips plus a +N marker.
  const MAX_CARD_TAGS = 3
  let tags = $derived((app.tags ?? []).filter((tag) => tag.trim().length > 0))
  let shownTags = $derived(tags.slice(0, MAX_CARD_TAGS))
  let hiddenTagCount = $derived(Math.max(0, tags.length - MAX_CARD_TAGS))

  // BUG-0021 follow-up: if a data URL fails to decode, fall back to the letter
  // instead of leaving an empty square.
  let iconFailed = $state(false)
  $effect(() => {
    app.icon
    iconFailed = false
  })
</script>

<button
  class="ph-card"
  data-testid="app-card"
  data-app-name={app.name}
  class:selected
  data-selected={selected ? 'true' : 'false'}
  data-autostart={app.autostart ? 'true' : 'false'}
  aria-current={selected ? 'true' : undefined}
  title={app.exists ? app.path : `${app.path} (${t('card.missing')})`}
  onclick={(e) => onselect(app, e)}
  ondblclick={() => onlaunch(app)}
  oncontextmenu={(e) => {
    e.preventDefault()
    onselect(app, e)
    onmenu(e, app)
  }}
>
  <span class="ph-card-top">
    <span
      class="ph-icon"
      style:background={app.icon && !iconFailed ? undefined : hashColor(app.name)}
    >
      {#if app.icon && !iconFailed}
        <img src={app.icon} alt="" onerror={() => (iconFailed = true)} />
      {:else}
        {fallbackLetter(app.name)}
      {/if}
    </span>
    <span class="ph-card-text">
      <span class="ph-name">{app.name}</span>
      {#if app.category}
        <span class="ph-category" data-testid="card-category" title={t(`category.${app.category}`)}>
          {t(`category.${app.category}`)}
        </span>
      {/if}
    </span>
  </span>
  {#if shownTags.length}
    <span class="ph-tags">
      {#each shownTags as tag (tag)}
        {@const style = tagStyles[tag]}
        <span class="ph-tag" data-testid="card-tag" title={tag} style={tagCss(style)}>{tagText(style, tag)}</span>
      {/each}
      {#if hiddenTagCount > 0}
        <span
          class="ph-tag ph-tag-more"
          data-testid="card-tag-more"
          title={tags.slice(MAX_CARD_TAGS).join(', ')}
        >+{hiddenTagCount}</span>
      {/if}
    </span>
  {/if}
  {#if !app.exists || app.runAsAdmin || app.autostart}
    <span class="ph-card-badges">
      {#if !app.exists}
        <span class="ph-badge missing">{t('card.missing')}</span>
      {/if}
      {#if app.runAsAdmin}
        <span class="ph-badge admin" data-testid="card-admin" title={t('edit.runAsAdmin')}>
          {t('card.admin')}
        </span>
      {/if}
      {#if app.autostart}
        <span class="ph-badge on">{t('card.autostart')}</span>
      {/if}
    </span>
  {/if}
  <span class="ph-card-foot">
    <span class="ph-card-state" class:bad={!app.exists}>
      <i></i>{app.exists ? t('card.ready') : t('card.missing')}
    </span>
  </span>
</button>

<style>
  /* BUG-0023: the card can carry several badges (missing / admin / autostart);
     they stack in the top-right corner instead of overlapping. */
  .ph-card-badges {
    position: absolute;
    top: 4px;
    right: 4px;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    max-width: calc(100% - 8px);
  }

  .ph-card-badges .ph-badge {
    position: static;
    top: auto;
    right: auto;
  }

  .ph-card-badges .ph-badge.admin {
    border-color: var(--ph-accent);
    color: var(--ph-accent);
  }

  /* REQ-0039: semantic category chip. */
  .ph-category {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0 4px;
    font-size: 10px;
    line-height: 1.6;
    color: var(--ph-text-muted);
    background: transparent;
    border: 1px solid var(--ph-border);
    align-self: flex-start;
  }

  .ph-card-top {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    min-width: 0;
    padding-right: 28px;
  }

  .ph-card-text {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    min-width: 0;
    padding-top: 1px;
  }

  .ph-card-text .ph-name {
    max-width: 100%;
  }

  .ph-card-foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    min-height: 13px;
    margin-top: auto;
  }

  .ph-card-state {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--ph-text-muted);
    font-family: var(--ph-font-pixel);
    font-size: 9px;
    letter-spacing: .03em;
  }

  .ph-card-state i {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--ph-accent);
  }

  .ph-card-state.bad { color: var(--ph-danger); }
  .ph-card-state.bad i { background: var(--ph-danger); }

  .ph-tags {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-start;
    gap: 3px;
    max-width: 100%;
    margin-top: 0;
  }

  .ph-tag {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0 4px;
    font-size: 10px;
    line-height: 1.6;
    color: var(--ph-text-muted);
    background: var(--ph-bg-alt);
    border: 1px solid var(--ph-border);
  }

  .ph-tag-more {
    color: var(--ph-accent);
    border-color: var(--ph-accent);
  }
</style>
