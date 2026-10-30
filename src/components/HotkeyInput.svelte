<script lang="ts">
  // BUG-0013 · hotkey recording field.
  // Controlled component: it never stores the value itself, it reports the
  // recorded accelerator through `onchange`. Recording brackets the session
  // with `onrecordstart` / `onrecordstop` so the app can temporarily suspend the
  // global hotkey (otherwise pressing the old combination would hide the window).
  import type { Translator } from '../lib/i18n'
  import { acceleratorFromEvent } from '../lib/hotkey'

  let {
    value,
    disabled = false,
    t,
    onrecordstart = () => {},
    onrecordstop = () => {},
    onchange
  }: {
    value: string
    disabled?: boolean
    t: Translator
    onrecordstart?: () => void
    onrecordstop?: () => void
    onchange: (accel: string) => void
  } = $props()

  let recording = $state(false)
  let error = $state('')
  let boxEl = $state<HTMLDivElement>()

  function startRecording() {
    if (disabled || recording) return
    recording = true
    error = ''
    onrecordstart()
    // Keep the focus on the field so global shortcuts never see these keys.
    boxEl?.focus()
  }

  function stopRecording() {
    if (!recording) return
    recording = false
    onrecordstop()
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!recording) return
    // Nothing may leak to the window-level shortcuts while recording.
    event.preventDefault()
    event.stopPropagation()

    // Esc cancels: the controlled value was never touched, so it stays as it was.
    if (event.key === 'Escape' || event.key === 'Esc') {
      error = ''
      stopRecording()
      return
    }

    // Backspace (and bare Delete) clear the recorded shortcut.
    const bareDelete =
      event.key === 'Delete' &&
      !event.ctrlKey &&
      !event.altKey &&
      !event.shiftKey &&
      !event.metaKey
    if (event.key === 'Backspace' || bareDelete) {
      error = ''
      onchange('')
      stopRecording()
      return
    }

    const result = acceleratorFromEvent(event)
    if (result.accel !== null) {
      error = ''
      onchange(result.accel)
      stopRecording()
      return
    }
    // A lone modifier is expected mid-combination: keep waiting silently.
    if (result.reason === 'modifier-only') return
    error =
      result.reason === 'needs-modifier'
        ? t('settings.hotkeyNeedsModifier')
        : t('settings.hotkeyReserved')
  }
</script>

<div class="hotkey-field">
  <div
    class="hotkey-box"
    class:recording
    class:disabled
    class:empty={!value && !recording}
    class:has-error={!!error}
    role="button"
    tabindex={disabled ? -1 : 0}
    aria-label={t('settings.hotkeyLabel')}
    aria-disabled={disabled ? 'true' : undefined}
    data-testid="hotkey-input"
    data-recording={recording ? 'true' : 'false'}
    bind:this={boxEl}
    onclick={startRecording}
    onfocus={startRecording}
    onblur={stopRecording}
    onkeydown={handleKeydown}
  >
    {#if recording}
      <span class="hotkey-recording">{t('settings.hotkeyRecording')}</span>
    {:else if value}
      <span class="hotkey-value">{value}</span>
    {:else}
      <span class="hotkey-placeholder">{t('settings.hotkeyRecord')}</span>
    {/if}
  </div>
  {#if error}
    <p class="ph-hint ph-danger hotkey-error" data-testid="hotkey-error">{error}</p>
  {/if}
</div>

<style>
  .hotkey-field {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 4px;
    min-width: 180px;
  }

  .hotkey-box {
    display: flex;
    align-items: center;
    height: 28px;
    padding: 0 8px;
    overflow: hidden;
    background: var(--ph-panel);
    border: var(--ph-border-width) solid var(--ph-border);
    color: var(--ph-text);
    cursor: pointer;
    outline: none;
    white-space: nowrap;
  }

  .hotkey-box:hover {
    background: var(--ph-panel-hover);
  }

  .hotkey-box:focus-visible {
    border-color: var(--ph-border-light);
  }

  .hotkey-box.recording {
    border-color: var(--ph-accent);
    color: var(--ph-accent);
  }

  .hotkey-box.has-error {
    border-color: var(--ph-danger);
  }

  .hotkey-box.disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .hotkey-value,
  .hotkey-recording {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .hotkey-placeholder {
    color: var(--ph-text-muted);
  }

  .hotkey-error {
    margin: 0;
  }
</style>
