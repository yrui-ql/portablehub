<script lang="ts">
  import type { Translator } from '../lib/i18n'
  import { closeWindow, minimizeWindow, toggleMaximizeWindow } from '../lib/desktop'
  import Icon from './Icon.svelte'

  let { t }: { t: Translator } = $props()
</script>

<div class="ph-titlebar" data-tauri-drag-region>
  <span class="ph-title" data-tauri-drag-region>{t('app.title')}</span>
  <button class="ph-wbtn" title={t('titlebar.minimize')} aria-label={t('titlebar.minimize')} onclick={() => minimizeWindow()}>
    <Icon name="minimize" size={14} />
  </button>
  <button class="ph-wbtn" title={t('titlebar.maximize')} aria-label={t('titlebar.maximize')} onclick={() => toggleMaximizeWindow()}>
    <Icon name="maximize" size={12} />
  </button>
  <button class="ph-wbtn close" title={t('titlebar.close')} aria-label={t('titlebar.close')} onclick={() => closeWindow()}>
    <Icon name="close" size={14} />
  </button>
</div>
