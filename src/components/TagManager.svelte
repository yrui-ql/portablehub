<script lang="ts">
  import type { Translator } from '../lib/i18n'
  import type { TagCount } from '../lib/tags'
  import type { TagStyle } from '../lib/types'
  import { TAG_COLORS, tagCss, tagText } from '../lib/tag-style'

  let {
    tags,
    tagStyles,
    t,
    onsave,
    onclose
  }: {
    tags: TagCount[]
    tagStyles: Record<string, TagStyle>
    t: Translator
    onsave: (tag: string, color: string, icon: string) => Promise<void>
    onclose: () => void
  } = $props()

  let savingTag = $state('')
  let error = $state('')

  async function save(tag: string, color: string, icon: string) {
    savingTag = tag
    error = ''
    try {
      await onsave(tag, color, icon)
    } catch (reason) {
      error = `${t('tags.styleFailed')}: ${String(reason)}`
    } finally {
      savingTag = ''
    }
  }

  function setColor(tag: string, color: string) {
    void save(tag, color, tagStyles[tag]?.icon ?? '')
  }

  function commitIcon(tag: string, icon: string) {
    void save(tag, tagStyles[tag]?.color ?? '', icon)
  }

  function onIconKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault()
      ;(event.currentTarget as HTMLInputElement).blur()
    }
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
    class="ph-settings ph-tag-manager"
    role="dialog"
    aria-modal="true"
    aria-label={t('tags.styleTitle')}
    data-testid="tag-manager"
  >
    <div class="ph-settings-head">
      <strong>{t('tags.styleTitle')}</strong>
      <button class="ph-btn" data-testid="tag-manager-close" onclick={onclose}>
        {t('tags.done')}
      </button>
    </div>
    <div class="ph-tag-manager-body">
      {#if tags.length === 0}
        <p class="ph-hint">{t('tags.empty')}</p>
      {:else}
        {#each tags as item (item.tag)}
          {@const style = tagStyles[item.tag]}
          <div class="ph-tag-style-row" data-testid="tag-style-row" data-tag={item.tag}>
            <div class="ph-tag-style-preview">
              <span class="ph-tag" style={tagCss(style)}>{tagText(style, item.tag)}</span>
            </div>
            <div class="ph-tag-color-row" role="group" aria-label={t('tags.color')}>
              {#each TAG_COLORS as color (color || 'default')}
                <button
                  type="button"
                  class="ph-color-dot"
                  class:active={(style?.color ?? '') === color}
                  data-testid="tag-color"
                  data-color={color}
                  title={color || t('tags.colorDefault')}
                  aria-label={color || t('tags.colorDefault')}
                  style:background={color || 'var(--ph-panel-hover)'}
                  disabled={savingTag === item.tag}
                  onclick={() => setColor(item.tag, color)}
                ></button>
              {/each}
            </div>
            <label class="ph-tag-icon-field">
              <span>{t('tags.icon')}</span>
              <input
                class="ph-input"
                data-testid="tag-icon"
                maxlength="8"
                placeholder={t('tags.iconPlaceholder')}
                value={style?.icon ?? ''}
                disabled={savingTag === item.tag}
                onblur={(event) => commitIcon(item.tag, event.currentTarget.value)}
                onkeydown={onIconKeydown}
              />
            </label>
            <button
              type="button"
              class="ph-btn"
              data-testid="tag-style-reset"
              disabled={savingTag === item.tag}
              onclick={() => save(item.tag, '', '')}
            >{t('tags.reset')}</button>
          </div>
        {/each}
      {/if}
      {#if error}
        <p class="ph-hint ph-danger" role="alert" data-testid="tag-manager-error">{error}</p>
      {/if}
    </div>
  </div>
</div>

<style>
  .ph-tag-manager {
    width: min(560px, 94vw);
    height: auto;
    max-height: min(76vh, 620px);
  }

  .ph-tag-manager-body {
    overflow-y: auto;
    padding: 10px 14px;
  }

  .ph-tag-style-row {
    display: grid;
    grid-template-columns: minmax(90px, 1fr) auto;
    gap: 8px 12px;
    align-items: center;
    padding: 10px 0;
    border-bottom: var(--ph-border-width) solid var(--ph-border);
  }

  .ph-tag-style-row:last-child {
    border-bottom: none;
  }

  .ph-tag-style-preview {
    min-width: 0;
  }

  .ph-tag-color-row {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }

  .ph-color-dot {
    width: 16px;
    height: 16px;
    padding: 0;
    border: 1px solid var(--ph-border);
    border-radius: 0;
    cursor: pointer;
  }

  .ph-color-dot.active {
    outline: 2px solid var(--ph-accent);
    outline-offset: 1px;
  }

  .ph-tag-icon-field {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--ph-text-muted);
    font-size: 11px;
  }

  .ph-tag-icon-field .ph-input {
    width: 54px;
    min-width: 0;
    height: 26px;
    text-align: center;
  }

  .ph-tag-style-row > .ph-btn {
    justify-self: end;
  }
</style>
