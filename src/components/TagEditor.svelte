<script lang="ts">
  // REQ-0032 · tag editor dialog.
  //
  // The parent owns persistence: onsave(tags) calls api.setAppTags and closes
  // this dialog. Editing here is local until Save, so Cancel/Esc discard.
  import { onMount, untrack } from 'svelte'
  import type { AppView, TagStyle } from '../lib/types'
  import type { Translator } from '../lib/i18n'
  import { normalizeTags, type TagCount } from '../lib/tags'
  import { tagCss, tagText } from '../lib/tag-style'
  import InfoTip from './InfoTip.svelte'

  let {
    app,
    allTags,
    tagStyles,
    t,
    onsave,
    onclose
  }: {
    app: AppView
    allTags: TagCount[]
    tagStyles: Record<string, TagStyle>
    t: Translator
    onsave: (tags: string[]) => void
    onclose: () => void
  } = $props()

  // Snapshot once per open dialog; the parent mounts a fresh instance per app.
  let selected = $state<string[]>(untrack(() => normalizeTags([...(app.tags ?? [])])))
  let draft = $state('')
  let inputEl: HTMLInputElement | undefined = $state()

  onMount(() => {
    inputEl?.focus()
    // Esc closes this dialog only; capture so the main screen's global Esc
    // handler (clear search / selection) never fires while the dialog is open.
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      onclose()
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  })

  function hasTag(tag: string): boolean {
    const needle = tag.toLowerCase()
    return selected.some((item) => item.toLowerCase() === needle)
  }

  function addTag() {
    const tag = draft.trim()
    if (!tag) return
    selected = normalizeTags([...selected, tag])
    draft = ''
    inputEl?.focus()
  }

  function toggleTag(tag: string) {
    if (hasTag(tag)) {
      const needle = tag.toLowerCase()
      selected = selected.filter((item) => item.toLowerCase() !== needle)
    } else {
      selected = normalizeTags([...selected, tag])
    }
  }

  function removeTag(tag: string) {
    selected = selected.filter((item) => item !== tag)
  }

  function onInputKeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter') return
    event.preventDefault()
    addTag()
  }

  function save() {
    onsave(normalizeTags(selected))
  }
</script>

<div
  class="ph-overlay"
  role="presentation"
  onclick={(event) => {
    if (event.target === event.currentTarget) onclose()
  }}
>
  <div
    class="ph-settings ph-tag-editor"
    role="dialog"
    aria-modal="true"
    aria-label={t('tags.title', { name: app.name })}
    data-testid="tag-editor"
  >
    <div class="ph-settings-head">
      <strong>{t('tags.title', { name: app.name })}</strong>
      <InfoTip testid="tags-hint-info" label={t('tags.title', { name: app.name })} text={t('tags.hint')} />
      <button type="button" class="ph-btn" data-testid="tag-cancel" onclick={onclose}>
        {t('tags.cancel')}
      </button>
    </div>

    <div class="ph-tag-editor-body">
      <section class="ph-section">
        <div class="ph-tag-chip-row" data-testid="tag-current-list">
          {#if selected.length}
            {#each selected as tag (tag)}
              {@const style = tagStyles[tag]}
              <span class="ph-tag" data-testid="tag-current" style={tagCss(style)}>
                {tagText(style, tag)}
                <button
                  type="button"
                  class="ph-tag-remove"
                  data-testid="tag-remove"
                  title={t('tags.remove')}
                  aria-label={`${t('tags.remove')}: ${tag}`}
                  onclick={() => removeTag(tag)}
                >×</button>
              </span>
            {/each}
          {:else}
            <span class="ph-hint" data-testid="tag-current-empty">{t('app.tagNone')}</span>
          {/if}
        </div>

        <div class="ph-row">
          <input
            class="ph-input"
            data-testid="tag-input"
            bind:this={inputEl}
            bind:value={draft}
            placeholder={t('tags.placeholder')}
            spellcheck="false"
            autocomplete="off"
            onkeydown={onInputKeydown}
          />
          <button type="button" class="ph-btn" data-testid="tag-add" onclick={addTag}>
            {t('tags.add')}
          </button>
        </div>
      </section>

      <section class="ph-section">
        <h3>{t('tags.existing')}</h3>
        {#if allTags.length}
          <div class="ph-tag-chip-row" data-testid="tag-existing-list">
            {#each allTags as item (item.tag)}
              {@const style = tagStyles[item.tag]}
              <button
                type="button"
                class="ph-tag ph-tag-toggle"
                class:active={hasTag(item.tag)}
                data-testid="tag-existing"
                data-tag={item.tag}
                aria-pressed={hasTag(item.tag)}
                style={tagCss(style)}
                onclick={() => toggleTag(item.tag)}
              >
                {tagText(style, item.tag)}
                <span class="ph-tag-count">{item.count}</span>
              </button>
            {/each}
          </div>
        {:else}
          <p class="ph-hint" data-testid="tag-existing-empty">{t('tags.empty')}</p>
        {/if}
      </section>
    </div>

    <div class="ph-tag-editor-foot">
      <button type="button" class="ph-btn" data-testid="tag-cancel-foot" onclick={onclose}>
        {t('tags.cancel')}
      </button>
      <button type="button" class="ph-btn accent" data-testid="tag-save" onclick={save}>
        {t('tags.save')}
      </button>
    </div>
  </div>
</div>

<style>
  .ph-tag-editor {
    width: min(560px, 94vw);
  }

  .ph-tag-editor-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .ph-tag-chip-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-bottom: 10px;
  }

  .ph-tag {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    max-width: 100%;
    padding: 1px 6px;
    background: var(--ph-panel);
    border: 1px solid var(--ph-border);
    color: var(--ph-text);
    font-family: var(--ph-font);
    font-size: 11px;
    line-height: 1.7;
  }

  .ph-tag-toggle {
    cursor: pointer;
  }

  .ph-tag-toggle:hover {
    background: var(--ph-panel-hover);
    border-color: var(--ph-border-light);
  }

  .ph-tag-toggle.active {
    border-color: var(--ph-accent);
    color: var(--ph-accent);
  }

  .ph-tag-count {
    color: var(--ph-text-muted);
    font-size: 10px;
  }

  .ph-tag-toggle.active .ph-tag-count {
    color: inherit;
  }

  .ph-tag-remove {
    padding: 0 2px;
    border: 0;
    background: transparent;
    color: var(--ph-text-muted);
    line-height: 1;
    cursor: pointer;
  }

  .ph-tag-remove:hover {
    color: var(--ph-danger);
  }

  .ph-tag-editor-foot {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding: 12px 14px;
    border-top: var(--ph-border-width) solid var(--ph-border);
  }
</style>
