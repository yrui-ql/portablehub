<script lang="ts">
  import type { AppView } from '../lib/types'
  import type { Translator } from '../lib/i18n'
  import { CATEGORY_ORDER } from '../lib/categories'

  let {
    apps,
    t,
    onapply,
    onclose
  }: {
    apps: AppView[]
    t: Translator
    onapply: (category: string) => void
    onclose: () => void
  } = $props()
</script>

<div
  class="ph-overlay"
  role="presentation"
  onclick={(event) => {
    if (event.target === event.currentTarget) onclose()
  }}
>
  <div
    class="ph-settings ph-category-picker"
    role="dialog"
    aria-modal="true"
    aria-label={t('categoryPicker.title')}
    data-testid="category-picker"
  >
    <div class="ph-settings-head">
      <strong>{t('categoryPicker.title')}</strong>
      <button class="ph-btn" data-testid="category-picker-close" onclick={onclose}>
        {t('settings.close')}
      </button>
    </div>
    <div class="ph-category-picker-body">
      <button
        type="button"
        class="ph-btn"
        data-testid="category-option"
        data-category=""
        onclick={() => onapply('')}
      >{t('category.unclassified')}</button>
      {#each CATEGORY_ORDER as category (category)}
        <button
          type="button"
          class="ph-btn"
          data-testid="category-option"
          data-category={category}
          onclick={() => onapply(category)}
        >{t(`category.${category}`)}</button>
      {/each}
    </div>
    <p class="ph-hint ph-category-picker-hint">
      {t('categoryPicker.hint', { n: apps.length })}
    </p>
  </div>
</div>

<style>
  .ph-category-picker {
    width: min(420px, 92vw);
    height: auto;
  }

  .ph-category-picker-body {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 16px 18px 8px;
  }

  .ph-category-picker-body .ph-btn {
    flex: 0 0 auto;
  }

  .ph-category-picker-hint {
    margin: 0;
    padding: 0 18px 16px;
  }
</style>
