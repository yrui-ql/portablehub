<script lang="ts">
  import { onMount } from 'svelte'
  import type { AppView, TagStyle } from '../lib/types'
  import type { Translator } from '../lib/i18n'
  import type { TagCount } from '../lib/tags'
  import { tagCss, tagText } from '../lib/tag-style'

  // One-click tagging: toggling an existing tag or adding a new one writes
  // immediately, so the user does not need the full editor dialog. When several
  // apps are selected the same tag is applied to all of them.
  let {
    apps,
    allTags,
    tagStyles,
    anchor,
    t,
    onclose,
    ontoggle,
    onadd
  }: {
    apps: AppView[]
    allTags: TagCount[]
    tagStyles: Record<string, TagStyle>
    anchor: { x: number; y: number; bottom: number }
    t: Translator
    onclose: () => void
    ontoggle: (tag: string) => void
    onadd: (tag: string) => void
  } = $props()

  let primary = $derived(apps[0])
  let title = $derived(
    apps.length > 1
      ? t('tags.quickTitleMany', { n: apps.length })
      : t('tags.quickTitle', { name: primary?.name ?? '' })
  )
  let draft = $state('')
  let inputEl: HTMLInputElement | undefined = $state()
  let width = 268
  let left = $derived(
    Math.max(8, Math.min(anchor.x, (typeof window === 'undefined' ? width : window.innerWidth) - width - 8))
  )
  let top = $derived.by(() => {
    if (typeof window === 'undefined') return anchor.bottom + 8
    const estimated = 260
    const below = anchor.bottom + 8
    if (below + estimated <= window.innerHeight - 8) return below
    return Math.max(8, anchor.y - estimated - 8)
  })

  function hasTag(tag: string): boolean {
    return apps.every((app) => app.tags.includes(tag))
  }

  function someTag(tag: string): boolean {
    return apps.some((app) => app.tags.includes(tag))
  }

  function add() {
    const tag = draft.trim()
    if (!tag) return
    onadd(tag)
    draft = ''
    inputEl?.focus()
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault()
      onclose()
    } else if (event.key === 'Enter') {
      event.preventDefault()
      add()
    }
  }

  function onPointerDown(event: PointerEvent) {
    const target = event.target as HTMLElement | null
    if (target?.closest('[data-testid="quick-tag"]')) return
    if (target?.closest('[data-testid="card-quick-tag"]')) return
    onclose()
  }

  onMount(() => {
    inputEl?.focus()
    document.addEventListener('pointerdown', onPointerDown, true)
    return () => document.removeEventListener('pointerdown', onPointerDown, true)
  })
</script>

<div
  class="ph-settings ph-quick-tag"
  data-testid="quick-tag"
  role="dialog"
  aria-modal="false"
  tabindex="-1"
  aria-label={title}
  style:left={`${left}px`}
  style:top={`${top}px`}
  style:width={`${width}px`}
  onkeydown={onKeydown}
>
  <div class="ph-quick-head">
    <strong>{title}</strong>
    <button type="button" class="ph-quick-close" aria-label={t('tags.cancel')} onclick={onclose}>
      ×
    </button>
  </div>
  {#if allTags.length}
    <div class="ph-quick-tags">
      {#each allTags as item (item.tag)}
        {@const style = tagStyles[item.tag]}
        <button
          type="button"
          class="ph-tag ph-quick-chip"
          class:active={someTag(item.tag)}
          data-testid="quick-tag-chip"
          data-tag={item.tag}
          data-mixed={someTag(item.tag) && !hasTag(item.tag) ? 'true' : 'false'}
          aria-pressed={hasTag(item.tag)}
          style={tagCss(style)}
          onclick={() => ontoggle(item.tag)}
        >{tagText(style, item.tag)}</button>
      {/each}
    </div>
  {/if}
  <div class="ph-quick-input">
    <input
      bind:this={inputEl}
      bind:value={draft}
      class="ph-input"
      data-testid="quick-tag-input"
      placeholder={t('tags.quickPlaceholder')}
      spellcheck="false"
      maxlength="24"
    />
    <button type="button" class="ph-btn" data-testid="quick-tag-add" onclick={add}>
      {t('tags.add')}
    </button>
  </div>
</div>

<style>
  .ph-quick-tag {
    position: fixed;
    z-index: 2147482000;
    height: auto;
    max-height: min(320px, 60vh);
    overflow-y: auto;
    padding: 10px;
    background: var(--ph-bg-alt);
    box-shadow: 6px 6px 0 var(--ph-shadow);
  }

  .ph-quick-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 8px;
  }

  .ph-quick-close {
    border: 0;
    background: transparent;
    color: var(--ph-text-muted);
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
  }

  .ph-quick-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 8px;
  }

  .ph-quick-chip.active {
    border-color: var(--ph-accent);
    color: var(--ph-accent);
  }

  .ph-quick-input {
    display: flex;
    gap: 6px;
  }

  .ph-quick-input .ph-input {
    min-width: 0;
  }
</style>
