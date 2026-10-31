<script lang="ts">
  import { onMount } from 'svelte'
  import type {
    AiCliCandidate,
    AiConfigView,
    AiProvider,
    AiSavePayload,
    Bootstrap,
    StartupStatus,
    ThemeSummary
  } from '../lib/types'
  import type { Translator } from '../lib/i18n'
  import { locales } from '../lib/i18n'
  import { displayPath } from '../lib/path'
  import { pickDirectory } from '../lib/desktop'
  import { api } from '../lib/api'
  import MigrationModal from './MigrationModal.svelte'
  import HotkeyInput from './HotkeyInput.svelte'
  import InfoTip from './InfoTip.svelte'

  type SectionId = 'appearance' | 'language' | 'startup' | 'hotkey' | 'folders' | 'ai' | 'about'

  // Sidebar entries, in the same order as the sections below (REQ-0012).
  const sectionIds: SectionId[] = [
    'appearance',
    'language',
    'startup',
    'hotkey',
    'folders',
    'ai',
    'about'
  ]

  // REQ-0035 · provider presets. Switching provider keeps custom values, but
  // replaces an empty field or one that still holds the previous preset.
  // REQ-0038 adds two CLI providers that have no URL/model preset at all.
  type ApiProvider = Extract<AiProvider, 'anthropic' | 'openai'>

  function isCliProvider(provider: AiProvider): provider is 'claude-cli' | 'codex-cli' {
    return provider === 'claude-cli' || provider === 'codex-cli'
  }

  const AI_PRESETS: Record<ApiProvider, { baseUrl: string; model: string }> = {
    anthropic: { baseUrl: 'https://api.anthropic.com', model: 'claude-sonnet-4-5' },
    openai: { baseUrl: 'https://api.openai.com/v1', model: 'gpt-5-codex' }
  }
  const DEEPSEEK_PRESET = { baseUrl: 'https://api.deepseek.com/v1', model: 'deepseek-chat' }
  // Static sample only: synthetic names, no filesystem paths from this machine.
  const AI_PREVIEW_JSON = `{
  "apps": [
    { "name": "Alpha", "path": "Apps/Alpha/Alpha.exe", "size": 12582912 },
    { "name": "Beta", "path": "Apps/Beta/Beta.exe", "size": 4194304 }
  ]
}`

  let {
    boot,
    themes,
    startup,
    closeToTray,
    pixelIcons,
    pixelFont,
    locale,
    importMode,
    t,
    hotkeyDraft = $bindable(''),
    onclose,
    ontheme,
    onpixelicons,
    onpixelfont,
    onlocale,
    onappsdir,
    onimportmode,
    onhotkey,
    onrecordstart = () => {},
    onrecordstop = () => {},
    onautostart,
    onclosetotray,
    startHiddenAtLogin,
    onstarthidden,
    onrunautostart,
    onquit
  }: {
    boot: Bootstrap
    themes: ThemeSummary[]
    startup: StartupStatus
    closeToTray: boolean
    pixelIcons: boolean
    pixelFont: boolean
    startHiddenAtLogin: boolean
    locale: string
    importMode: string
    t: Translator
    hotkeyDraft?: string
    onclose: () => void
    ontheme: (id: string) => void
    onpixelicons: (enabled: boolean) => void
    onpixelfont: (enabled: boolean) => void
    onlocale: (id: string) => void
    onappsdir: (path: string) => void
    onimportmode: (mode: string) => void
    onhotkey: () => void
    onrecordstart?: () => void
    onrecordstop?: () => void
    onautostart: (enabled: boolean) => void
    onclosetotray: (enabled: boolean) => void
    onstarthidden: (enabled: boolean) => void
    onrunautostart: () => void
    onquit: () => void
  } = $props()

  let appsDirDraft = $state('')
  let showMigrate = $state(false)
  let activeSection = $state<SectionId>('appearance')
  let contentEl = $state<HTMLDivElement>()
  // While a navigation click is scrolling, keep the clicked entry highlighted;
  // the scroll handler would otherwise re-derive the active entry from the
  // (possibly bottom-clamped) scroll position. The lock is released on the
  // next manual wheel scroll, or after a safety timeout.
  let navigationLock = false
  let navigationTimer: number | undefined

  function releaseNavigationLock() {
    navigationLock = false
    clearTimeout(navigationTimer)
    navigationTimer = undefined
  }

  $effect(() => {
    appsDirDraft = displayPath(boot.root, boot.config.appsDir)
  })

  function sectionElement(id: SectionId): HTMLElement | null {
    const el = contentEl
    if (!el) return null
    return el.querySelector<HTMLElement>(`#settings-section-${id}`)
  }

  function scrollToSection(id: SectionId) {
    activeSection = id
    const el = contentEl
    const section = sectionElement(id)
    if (!el || !section) return
    const delta = section.getBoundingClientRect().top - el.getBoundingClientRect().top
    const max = Math.max(0, el.scrollHeight - el.clientHeight)
    const top = Math.max(0, Math.min(el.scrollTop + delta, max))
    navigationLock = true
    clearTimeout(navigationTimer)
    navigationTimer = window.setTimeout(releaseNavigationLock, 1200)
    el.scrollTo({ top, behavior: 'auto' })
  }

  function trackActiveSection() {
    const el = contentEl
    if (!el || navigationLock) return
    const max = el.scrollHeight - el.clientHeight
    if (max <= 0) return
    if (el.scrollTop >= max - 2) {
      activeSection = sectionIds[sectionIds.length - 1]
      return
    }
    const contentTop = el.getBoundingClientRect().top
    let current: SectionId = sectionIds[0]
    for (const id of sectionIds) {
      const section = sectionElement(id)
      if (section && section.getBoundingClientRect().top - contentTop <= 40) current = id
    }
    activeSection = current
  }

  async function browseAppsDir() {
    const picked = await pickDirectory(displayPath(boot.root, boot.config.appsDir))
    if (picked) appsDirDraft = picked
  }

  // ---- REQ-0035 · AI provider settings -------------------------------------
  type AiStatus = { text: string; ok: boolean }

  let aiConfig = $state<AiConfigView>({
    provider: 'openai',
    baseUrl: '',
    model: '',
    hasKey: false,
    keyHint: '',
    consent: false,
    cliPath: '',
    cliVersion: ''
  })
  let aiLoaded = $state(false)
  let aiKeyDraft = $state('')
  let aiStatus = $state<AiStatus | null>(null)
  // Which long-running action owns the busy state, so each button shows its own
  // in-progress label instead of a generic "testing" flash on save.
  let aiAction = $state<'save' | 'test' | 'detect' | 'bind' | null>(null)
  let aiBusy = $derived(aiAction !== null)
  // REQ-0038 · local CLI detection/binding state.
  let aiCliCandidates = $state<AiCliCandidate[]>([])
  let aiCliDetected = $state(false)
  let aiCliPathDraft = $state('')
  let aiIsCli = $derived(isCliProvider(aiConfig.provider))

  // Only the masked hint is ever shown; a blank draft keeps the stored key.
  let aiKeyPlaceholder = $derived(
    aiConfig.hasKey ? t('settings.aiKeyPlaceholder', { hint: aiConfig.keyHint }) : ''
  )

  onMount(() => {
    api
      .aiGetConfig()
      .then((config) => {
        aiConfig = config
      })
      .catch((error) => {
        aiStatus = { text: String(error), ok: false }
      })
      .finally(() => {
        aiLoaded = true
      })
  })

  function setAiStatus(text: string, ok: boolean) {
    aiStatus = { text, ok }
  }

  function changeAiProvider(next: AiProvider) {
    aiStatus = null
    const previousProvider = aiConfig.provider
    // Entering/leaving CLI mode has no URL/model preset side effects.
    if (isCliProvider(next) || isCliProvider(previousProvider)) {
      aiConfig = { ...aiConfig, provider: next }
      return
    }
    const previous = AI_PRESETS[previousProvider]
    const target = AI_PRESETS[next]
    const baseUrl =
      !aiConfig.baseUrl.trim() || aiConfig.baseUrl === previous.baseUrl
        ? target.baseUrl
        : aiConfig.baseUrl
    const model =
      !aiConfig.model.trim() || aiConfig.model === previous.model ? target.model : aiConfig.model
    aiConfig = { ...aiConfig, provider: next, baseUrl, model }
  }

  function applyDeepseekPreset() {
    aiConfig = { ...aiConfig, provider: 'openai', ...DEEPSEEK_PRESET }
    aiStatus = null
  }

  async function saveAiConfig() {
    aiAction = 'save'
    aiStatus = null
    try {
      const payload: AiSavePayload = {
        provider: aiConfig.provider,
        baseUrl: aiConfig.baseUrl,
        model: aiConfig.model
      }
      const key = aiKeyDraft.trim()
      if (key) payload.apiKey = key
      aiConfig = await api.aiSaveConfig(payload)
      aiKeyDraft = ''
      setAiStatus(t('settings.aiSaved'), true)
    } catch (error) {
      setAiStatus(String(error), false)
    } finally {
      aiAction = null
    }
  }

  async function testAiConnection() {
    aiAction = 'test'
    aiStatus = null
    try {
      const result = await api.aiTestConnection()
      setAiStatus(
        result.ok ? t('settings.aiTestOk') : t('settings.aiTestFail', { error: result.message }),
        result.ok
      )
    } catch (error) {
      setAiStatus(t('settings.aiTestFail', { error: String(error) }), false)
    } finally {
      aiAction = null
    }
  }

  async function changeAiConsent(enabled: boolean) {
    try {
      aiConfig = await api.aiSetConsent(enabled)
    } catch (error) {
      setAiStatus(String(error), false)
    }
  }

  // ---- REQ-0038 · local CLI detection & binding ----------------------------
  async function detectAiClis() {
    aiAction = 'detect'
    aiStatus = null
    try {
      aiCliCandidates = await api.aiDetectClis()
      aiCliDetected = true
    } catch (error) {
      setAiStatus(t('settings.aiCliBindFailed', { error: String(error) }), false)
    } finally {
      aiAction = null
    }
  }

  async function bindAiCli(candidate: AiCliCandidate) {
    aiAction = 'bind'
    aiStatus = null
    try {
      aiConfig = await api.aiBindCli(candidate.provider, candidate.path)
      aiCliPathDraft = ''
      setAiStatus(t('settings.aiCliBoundOk'), true)
    } catch (error) {
      setAiStatus(t('settings.aiCliBindFailed', { error: String(error) }), false)
    } finally {
      aiAction = null
    }
  }

  async function bindAiCliManual() {
    const path = aiCliPathDraft.trim()
    if (!path) return
    const provider: 'claude-cli' | 'codex-cli' =
      aiConfig.provider === 'codex-cli' ? 'codex-cli' : 'claude-cli'
    aiAction = 'bind'
    aiStatus = null
    try {
      aiConfig = await api.aiBindCli(provider, path)
      aiCliPathDraft = ''
      setAiStatus(t('settings.aiCliBoundOk'), true)
    } catch (error) {
      setAiStatus(t('settings.aiCliBindFailed', { error: String(error) }), false)
    } finally {
      aiAction = null
    }
  }
</script>

<div class="ph-overlay" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) onclose() }}>
  <div class="ph-settings" role="dialog" aria-modal="true" aria-label={t('settings.title')}>
    <div class="ph-settings-head">
      <strong>{t('settings.title')}</strong>
      <button class="ph-btn" data-testid="settings-close" onclick={onclose}>{t('settings.close')}</button>
    </div>

    <div class="ph-settings-body">
      <nav class="ph-settings-nav" data-testid="settings-nav" aria-label={t('settings.nav.aria')}>
        {#each sectionIds as id (id)}
          <button
            type="button"
            class="ph-settings-nav-item"
            class:active={activeSection === id}
            data-testid={`settings-nav-${id}`}
            aria-current={activeSection === id ? 'true' : undefined}
            onclick={() => scrollToSection(id)}
          >
            {t(`settings.nav.${id}`)}
          </button>
        {/each}
      </nav>

      <div
        class="ph-settings-content"
        data-testid="settings-content"
        bind:this={contentEl}
        onscroll={trackActiveSection}
        onwheel={releaseNavigationLock}
      >
        <section id="settings-section-appearance" class="ph-section" data-testid="settings-section-appearance">
          <h3>{t('settings.appearance')}</h3>
          <div class="ph-row">
            <label for="theme">{t('settings.theme')}</label>
            <select
              id="theme"
              class="ph-select"
              value={boot.config.theme}
              onchange={(e) => ontheme(e.currentTarget.value)}
            >
              {#each themes as item (item.id)}
                <option value={item.id}>
                  {item.name}{item.builtin ? '' : t('settings.customSuffix')}{item.error ? ' ⚠' : ''}
                </option>
              {/each}
            </select>
            <button
              class="ph-btn"
              data-testid="open-themes"
              onclick={() => api.openThemeDir().catch(() => {})}
            >
              {t('settings.openThemes')}
            </button>
            <InfoTip
              testid="theme-info"
              label={t('settings.themeHintLabel')}
              text={t('settings.themeHint')}
            />
          </div>
          <div class="ph-row">
            <label for="pixel-font">{t('settings.pixelFont')}</label>
            <input
              id="pixel-font"
              class="ph-check"
              type="checkbox"
              data-testid="pixel-font"
              checked={pixelFont}
              onchange={(e) => onpixelfont(e.currentTarget.checked)}
            />
          </div>
          <div class="ph-row">
            <label for="pixel-icons">{t('settings.pixelIcons')}</label>
            <input
              id="pixel-icons"
              class="ph-check"
              type="checkbox"
              data-testid="pixel-icons"
              checked={pixelIcons}
              onchange={(e) => onpixelicons(e.currentTarget.checked)}
            />
          </div>
        </section>

        <section id="settings-section-language" class="ph-section" data-testid="settings-section-language">
          <h3>{t('settings.language')}</h3>
          <div class="ph-row">
            <label for="locale">{t('settings.language')}</label>
            <select
              id="locale"
              class="ph-select"
              value={locale}
              onchange={(e) => onlocale(e.currentTarget.value)}
            >
              {#each locales as item (item.id)}
                <option value={item.id}>{item.label}</option>
              {/each}
            </select>
          </div>
        </section>

        <section id="settings-section-startup" class="ph-section" data-testid="settings-section-startup">
          <h3>{t('settings.startup')}</h3>
          <div class="ph-row">
            <label for="login">{t('settings.launchAtLogin')}</label>
            <input
              id="login"
              class="ph-check"
              type="checkbox"
              checked={startup.enabled}
              onchange={(e) => onautostart(e.currentTarget.checked)}
            />
            <span class="ph-hint">{startup.detail}</span>
          </div>
          <div class="ph-row">
            <label for="tray">{t('settings.closeToTray')}</label>
            <input
              id="tray"
              class="ph-check"
              type="checkbox"
              checked={closeToTray}
              onchange={(e) => onclosetotray(e.currentTarget.checked)}
            />
          </div>
          <div class="ph-row">
            <label for="starthidden">{t('settings.startHiddenAtLogin')}</label>
            <input
              id="starthidden"
              class="ph-check"
              type="checkbox"
              data-testid="start-hidden"
              checked={startHiddenAtLogin}
              onchange={(e) => onstarthidden(e.currentTarget.checked)}
            />
            <InfoTip
              testid="start-hidden-info"
              label={t('settings.startHiddenHintLabel')}
              text={t('settings.startHiddenHint')}
            />
          </div>
          <div class="ph-row">
            <button class="ph-btn" onclick={onrunautostart}>{t('settings.runAutostart')}</button>
            <span class="ph-hint">{t('settings.runAutostartHint')}</span>
          </div>
        </section>

        <section id="settings-section-hotkey" class="ph-section" data-testid="settings-section-hotkey">
          <h3>{t('settings.hotkey')}</h3>
          <div class="ph-row">
            <span class="ph-label">{t('settings.hotkeyLabel')}</span>
            <HotkeyInput
              value={hotkeyDraft}
              {t}
              onrecordstart={onrecordstart}
              onrecordstop={onrecordstop}
              onchange={(accel) => (hotkeyDraft = accel)}
            />
            <button class="ph-btn" onclick={onhotkey}>{t('settings.save')}</button>
            <InfoTip
              testid="hotkey-info"
              label={t('settings.hotkeyHintLabel')}
              text={`${t('settings.hotkeyHint')} ${t('settings.hotkeyHint2')}`}
            />
          </div>
        </section>

        <section id="settings-section-folders" class="ph-section" data-testid="settings-section-folders">
          <h3>{t('settings.paths')}</h3>
          <div class="ph-row">
            <span class="ph-label">{t('settings.programDir')}</span>
            <span>{boot.root}</span>
            <InfoTip
              testid="program-dir-info"
              label={t('settings.programDirHintLabel')}
              text={t('settings.programDirHint')}
            />
          </div>
          <div class="ph-row">
            <label for="appsdir">{t('settings.appsDir')}</label>
            <input id="appsdir" class="ph-input" bind:value={appsDirDraft} spellcheck="false" />
            <button class="ph-btn" onclick={browseAppsDir}>{t('settings.browse')}</button>
            <button class="ph-btn accent" onclick={() => onappsdir(appsDirDraft)}>
              {t('settings.apply')}
            </button>
            <InfoTip
              testid="apps-dir-info"
              label={t('settings.appsDirHintLabel')}
              text={t('settings.appsDirHint')}
            />
          </div>
          <div class="ph-row">
            <label for="importmode">{t('settings.importMode')}</label>
            <select
              id="importmode"
              class="ph-select"
              value={importMode}
              onchange={(e) => onimportmode(e.currentTarget.value)}
            >
              <option value="link">{t('discover.modeLink')}</option>
              <option value="move">{t('discover.modeMove')}</option>
              <option value="register">{t('discover.modeRegister')}</option>
            </select>
            <span class="ph-hint" data-testid="settings-import-summary">
              {t(`discover.modeShort.${importMode}`)}
            </span>
            <InfoTip
              testid="settings-import-info"
              label={t('discover.modeInfo')}
              text={t(`discover.modeDetail.${importMode}`)}
            />
          </div>
          <div class="ph-row" style="margin-top:10px">
            <button class="ph-btn" data-testid="migrate-open" onclick={() => (showMigrate = true)}>
              {t('migrate.open')}
            </button>
          </div>
          <p class="ph-hint" data-testid="migrate-hint-short">
            {t('migrate.hintShort')}
            <InfoTip
              testid="migrate-info"
              align="left"
              label={t('migrate.hintLabel')}
              text={t('migrate.hint')}
            />
          </p>
        </section>

        <section id="settings-section-ai" class="ph-section" data-testid="settings-section-ai">
          <h3>{t('settings.ai')}</h3>
          {#if aiLoaded}
            <div class="ph-row">
              <label for="ai-provider">{t('settings.aiProvider')}</label>
              <select
                id="ai-provider"
                class="ph-select"
                data-testid="ai-provider"
                value={aiConfig.provider}
                onchange={(e) => changeAiProvider(e.currentTarget.value as AiProvider)}
              >
                <option value="anthropic">{t('settings.aiProviderClaude')}</option>
                <option value="openai">{t('settings.aiProviderOpenai')}</option>
                <option value="claude-cli">{t('settings.aiProviderClaudeCli')}</option>
                <option value="codex-cli">{t('settings.aiProviderCodexCli')}</option>
              </select>
              {#if !aiIsCli}
                <button class="ph-btn" data-testid="ai-preset-deepseek" onclick={applyDeepseekPreset}>
                  {t('settings.aiPresetDeepseek')}
                </button>
              {/if}
              <InfoTip
                testid="ai-provider-info"
                label={t('settings.aiHintLabel')}
                text={aiIsCli ? t('settings.aiCliHint') : t('settings.aiHint')}
              />
            </div>

            {#if aiIsCli}
              {#if aiConfig.cliPath}
                <p class="ph-hint ai-cli-bound" data-testid="ai-cli-bound">
                  {t('settings.aiCliBound')}：{aiConfig.cliPath}{aiConfig.cliVersion
                    ? ` · ${aiConfig.cliVersion}`
                    : ''}
                </p>
              {:else}
                <p class="ph-hint" data-testid="ai-cli-unbound">{t('settings.aiCliUnbound')}</p>
              {/if}

              <div class="ph-row">
                <button
                  class="ph-btn"
                  data-testid="ai-detect-cli"
                  disabled={aiBusy}
                  onclick={detectAiClis}
                >
                  {aiAction === 'detect' ? t('settings.aiCliDetecting') : t('settings.aiCliDetect')}
                </button>
              </div>

              {#if aiCliDetected}
                {#if aiCliCandidates.length}
                  <ul class="ai-cli-list">
                    {#each aiCliCandidates as candidate, index (candidate.provider + candidate.path + index)}
                      <li class="ai-cli-candidate" data-testid="ai-cli-candidate">
                        <span class="ai-cli-provider">
                          {candidate.provider === 'codex-cli'
                            ? t('settings.aiProviderCodexCli')
                            : t('settings.aiProviderClaudeCli')}
                        </span>
                        <span class="ai-cli-path" title={candidate.path}>{candidate.path}</span>
                        <span class="ph-hint">{candidate.version} · {candidate.source}</span>
                        <button
                          class="ph-btn"
                          data-testid="ai-cli-bind"
                          disabled={aiBusy}
                          onclick={() => bindAiCli(candidate)}
                        >
                          {t('settings.aiCliBind')}
                        </button>
                      </li>
                    {/each}
                  </ul>
                {:else}
                  <p class="ph-hint" data-testid="ai-cli-none">{t('settings.aiCliNone')}</p>
                {/if}
              {/if}

              <div class="ph-row">
                <label for="ai-cli-path">{t('settings.aiCliPath')}</label>
                <input
                  id="ai-cli-path"
                  class="ph-input"
                  data-testid="ai-cli-path"
                  bind:value={aiCliPathDraft}
                  placeholder={t('settings.aiCliPathPlaceholder')}
                  spellcheck="false"
                />
                <button
                  class="ph-btn"
                  data-testid="ai-cli-bind-manual"
                  disabled={aiBusy || !aiCliPathDraft.trim()}
                  onclick={bindAiCliManual}
                >
                  {t('settings.aiCliBindManual')}
                </button>
              </div>
            {:else}
              <div class="ph-row">
                <label for="ai-base">{t('settings.aiBaseUrl')}</label>
                <input
                  id="ai-base"
                  class="ph-input"
                  data-testid="ai-base"
                  bind:value={aiConfig.baseUrl}
                  spellcheck="false"
                />
              </div>
              <div class="ph-row">
                <label for="ai-model">{t('settings.aiModel')}</label>
                <input
                  id="ai-model"
                  class="ph-input"
                  data-testid="ai-model"
                  bind:value={aiConfig.model}
                  spellcheck="false"
                />
              </div>
              <div class="ph-row">
                <label for="ai-key">{t('settings.aiKey')}</label>
                <input
                  id="ai-key"
                  class="ph-input"
                  data-testid="ai-key"
                  type="password"
                  autocomplete="off"
                  placeholder={aiKeyPlaceholder}
                  bind:value={aiKeyDraft}
                />
                <span class="ph-hint" data-testid="ai-key-short">{t('settings.aiKeyShort')}</span>
                <InfoTip
                  testid="ai-key-info"
                  label={t('settings.aiKeyHintLabel')}
                  text={t('settings.aiPrivacyNote')}
                />
              </div>
            {/if}

            <div class="ph-row">
              {#if !aiIsCli}
                <button
                  class="ph-btn accent"
                  data-testid="ai-save"
                  disabled={aiBusy}
                  onclick={saveAiConfig}
                >
                  {aiAction === 'save' ? t('settings.aiSaving') : t('settings.aiSave')}
                </button>
              {/if}
              <button
                class="ph-btn"
                data-testid="ai-test"
                disabled={aiBusy}
                onclick={testAiConnection}
              >
                {aiAction === 'test' ? t('settings.aiTesting') : t('settings.aiTest')}
              </button>
            </div>
            {#if aiStatus}
              <p
                class="ph-hint"
                class:ai-ok={aiStatus.ok}
                class:ph-danger={!aiStatus.ok}
                data-testid="ai-status"
                data-state={aiStatus.ok ? 'ok' : 'error'}
                role="status"
              >
                {aiStatus.text}
              </p>
            {/if}

            <div class="ph-row">
              <label for="ai-consent">{t('settings.aiConsent')}</label>
              <input
                id="ai-consent"
                class="ph-check"
                type="checkbox"
                data-testid="ai-consent"
                checked={aiConfig.consent}
                onchange={(e) => changeAiConsent(e.currentTarget.checked)}
              />
              <span class="ph-hint" data-testid="ai-consent-short">{t('settings.aiConsentShort')}</span>
              <InfoTip
                testid="ai-consent-info"
                label={t('settings.aiConsentHintLabel')}
                text={t('settings.aiConsentHint')}
              />
            </div>

            <details class="ai-preview" data-testid="ai-preview">
              <summary>{t('settings.aiPreview')}</summary>
              <pre class="ai-preview-code" data-testid="ai-preview-json">{AI_PREVIEW_JSON}</pre>
            </details>
            <p class="ph-hint">{t('settings.aiPreviewHint')}</p>
          {:else}
            <p class="ph-hint">{t('settings.aiLoading')}</p>
          {/if}
        </section>

        <section id="settings-section-about" class="ph-section" data-testid="settings-section-about">
          <h3>{t('settings.about')}</h3>
          <p class="ph-hint">
            {t('settings.version', {
              version: boot.version,
              webview2: boot.webview2 ?? t('settings.webviewMissing')
            })}
          </p>
          {#if boot.warning}
            <p class="ph-hint ph-danger">{boot.warning}</p>
          {/if}
          <div class="ph-row" style="margin-top:10px">
            <button class="ph-btn ph-danger" onclick={onquit}>
              {t('settings.quit', { app: boot.appName })}
            </button>
          </div>
        </section>
      </div>
    </div>
  </div>
</div>

{#if showMigrate}
  <MigrationModal {t} onclose={() => (showMigrate = false)} />
{/if}

<style>
  .ai-preview {
    margin: 10px 0 0;
    padding: 6px 8px;
    border: var(--ph-border-width) solid var(--ph-border);
    background: var(--ph-bg-alt);
  }

  .ai-preview summary {
    color: var(--ph-text-muted);
    cursor: pointer;
  }

  .ai-preview-code {
    max-height: 180px;
    margin: 6px 0 0;
    overflow: auto;
    color: var(--ph-text-muted);
    font-size: 11px;
    line-height: 1.5;
    white-space: pre;
  }

  .ai-ok {
    color: var(--ph-accent-2);
  }

  .ai-cli-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 8px 0;
    padding: 0;
    list-style: none;
  }

  .ai-cli-candidate {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border: var(--ph-border-width) solid var(--ph-border);
    background: var(--ph-panel);
  }

  .ai-cli-provider {
    color: var(--ph-accent);
  }

  .ai-cli-path {
    flex: 1;
    min-width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ai-cli-bound {
    color: var(--ph-accent-2);
  }
</style>
