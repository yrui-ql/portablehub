import { expect, test, type Page } from '@playwright/test'

// REQ-0035 · AI provider settings in the settings dialog.
// REQ-0038 · local Claude Code / Codex CLI detection and binding.
//
// The browser E2E suite runs against the in-memory mock: aiGetConfig / aiSaveConfig
// / aiTestConnection / aiSetConsent / aiDetectClis / aiBindCli are mocked and
// `__PH_MOCK__.aiConfig` exposes the stored (masked) state. No network request is
// ever made by these tests.

interface AiMockConfig {
  provider: string
  baseUrl: string
  model: string
  hasKey: boolean
  keyHint: string
  consent: boolean
  cliPath: string
  cliVersion: string
}

interface MockWindow {
  __PH_MOCK__: {
    aiConfig: AiMockConfig
    setAiTestResult: (ok: boolean, message: string) => void
  }
}

async function aiConfig(page: Page): Promise<AiMockConfig> {
  return page.evaluate(() => (window as unknown as MockWindow).__PH_MOCK__.aiConfig)
}

async function openAiSection(page: Page) {
  await page.getByTestId('settings-btn').click()
  await expect(page.locator('.ph-settings')).toBeVisible()
  await page.getByTestId('settings-nav-ai').click()
  await expect(page.getByTestId('settings-nav-ai')).toHaveAttribute('aria-current', 'true')
  // Wait for the async aiGetConfig() load to land (mock fixture).
  await expect(page.getByTestId('ai-provider')).toBeVisible()
  await expect(page.getByTestId('ai-base')).toHaveValue('https://api.deepseek.com/v1')
}

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

test('AI settings load from the mock and are reachable from the sidebar', async ({ page }) => {
  await openAiSection(page)

  await expect(page.getByTestId('settings-section-ai')).toBeVisible()
  await expect(page.getByTestId('settings-nav-ai')).toHaveText('AI')
  await expect(page.getByTestId('ai-provider')).toHaveValue('openai')
  await expect(page.getByTestId('ai-model')).toHaveValue('deepseek-chat')
  await expect(page.getByTestId('ai-key')).toHaveValue('')

  // Privacy copy is present and the preview stays a static sample. The long
  // explanation lives behind the info affordance; only the consequence stays inline.
  await expect(page.getByTestId('ai-consent-short')).toHaveText('只发送名称、路径和大小')
  await expect(page.getByTestId('settings-section-ai')).not.toContainText('不会发送文件内容')
  await page.getByTestId('ai-consent-info').hover()
  await expect(page.getByRole('tooltip')).toContainText('不会发送文件内容')
  await page.getByTestId('ai-key-info').hover()
  await expect(page.getByRole('tooltip')).toContainText('_private/ai.json')
  await page.getByTestId('ai-preview').locator('summary').click()
  await expect(page.getByTestId('ai-preview-json')).toContainText('Alpha')
  await expect(page.getByTestId('ai-preview-json')).toContainText('Apps/Beta/Beta.exe')
})

test('saving the AI config stores base/model and keeps the key masked', async ({ page }) => {
  const pageErrors: string[] = []
  page.on('pageerror', (error) => pageErrors.push(String(error)))

  await openAiSection(page)
  await page.getByTestId('ai-base').fill('https://example.test/v1')
  await page.getByTestId('ai-model').fill('test-model-a')
  await page.getByTestId('ai-key').fill('sk-test-1234567890')
  await page.getByTestId('ai-save').click()

  await expect(page.getByTestId('ai-status')).toHaveAttribute('data-state', 'ok')
  await expect(page.getByTestId('ai-status')).toContainText('AI 设置已保存')

  const config = await aiConfig(page)
  expect(config.provider).toBe('openai')
  expect(config.baseUrl).toBe('https://example.test/v1')
  expect(config.model).toBe('test-model-a')
  expect(config.hasKey).toBe(true)
  // The raw key is never returned: only a masked hint.
  expect(config.keyHint).not.toBe('sk-test-1234567890')
  expect(config.keyHint).not.toContain('sk-test-1234567890')
  expect(config.keyHint).toMatch(/^sk-t.+7890$/)

  // The draft is cleared and the placeholder offers the masked hint, so saving
  // again without typing keeps the stored key.
  await expect(page.getByTestId('ai-key')).toHaveValue('')
  await expect(page.getByTestId('ai-key')).toHaveAttribute('placeholder', /已保存 sk-t…7890/)
  await expect(page.getByTestId('ai-key')).toHaveAttribute('placeholder', /留空沿用/)
  expect(pageErrors).toEqual([])
})

test('the connection test reports the mock success and failure states', async ({ page }) => {
  await openAiSection(page)

  await page.getByTestId('ai-test').click()
  await expect(page.getByTestId('ai-status')).toHaveAttribute('data-state', 'ok')
  await expect(page.getByTestId('ai-status')).toContainText('连接成功')
  await expect(page.getByTestId('ai-test')).toBeEnabled()

  await page.evaluate(() => {
    ;(window as unknown as MockWindow).__PH_MOCK__.setAiTestResult(false, 'mock error')
  })
  await page.getByTestId('ai-test').click()
  await expect(page.getByTestId('ai-status')).toHaveAttribute('data-state', 'error')
  await expect(page.getByTestId('ai-status')).toContainText('mock error')
  await expect(page.getByTestId('ai-status')).toHaveClass(/ph-danger/)
})

test('the privacy consent checkbox is persisted through the mock', async ({ page }) => {
  await openAiSection(page)
  expect((await aiConfig(page)).consent).toBe(false)

  await page.getByTestId('ai-consent').check()
  await expect.poll(async () => (await aiConfig(page)).consent).toBe(true)
  await expect(page.getByTestId('ai-consent')).toBeChecked()

  await page.getByTestId('ai-consent').uncheck()
  await expect.poll(async () => (await aiConfig(page)).consent).toBe(false)
  await expect(page.getByTestId('ai-consent')).not.toBeChecked()
})

test('switching providers fills presets while custom values are kept', async ({ page }) => {
  await openAiSection(page)

  // Empty fields get the new provider's preset.
  await page.getByTestId('ai-base').fill('')
  await page.getByTestId('ai-model').fill('')
  await page.getByTestId('ai-provider').selectOption('anthropic')
  await expect(page.getByTestId('ai-base')).toHaveValue('https://api.anthropic.com')
  await expect(page.getByTestId('ai-model')).toHaveValue('claude-sonnet-4-5')

  // Values that still equal the previous preset are replaced, too.
  await page.getByTestId('ai-provider').selectOption('openai')
  await expect(page.getByTestId('ai-base')).toHaveValue('https://api.openai.com/v1')
  await expect(page.getByTestId('ai-model')).toHaveValue('gpt-5-codex')

  // The DeepSeek preset is one click away.
  await page.getByTestId('ai-preset-deepseek').click()
  await expect(page.getByTestId('ai-provider')).toHaveValue('openai')
  await expect(page.getByTestId('ai-base')).toHaveValue('https://api.deepseek.com/v1')
  await expect(page.getByTestId('ai-model')).toHaveValue('deepseek-chat')

  // Custom endpoints are not overwritten by a provider switch.
  await page.getByTestId('ai-base').fill('https://custom.example.test/v1')
  await page.getByTestId('ai-model').fill('custom-model')
  await page.getByTestId('ai-provider').selectOption('anthropic')
  await expect(page.getByTestId('ai-base')).toHaveValue('https://custom.example.test/v1')
  await expect(page.getByTestId('ai-model')).toHaveValue('custom-model')
})

test('binding a detected Claude Code CLI persists the CLI config', async ({ page }) => {
  const pageErrors: string[] = []
  page.on('pageerror', (error) => pageErrors.push(String(error)))

  await openAiSection(page)
  await expect(page.getByTestId('ai-provider').locator('option')).toHaveCount(4)

  await page.getByTestId('ai-provider').selectOption('claude-cli')

  // CLI mode hides the API credential fields, the DeepSeek preset and the save
  // button (binding is the save action), but keeps consent and preview.
  await expect(page.getByTestId('ai-base')).toHaveCount(0)
  await expect(page.getByTestId('ai-model')).toHaveCount(0)
  await expect(page.getByTestId('ai-key')).toHaveCount(0)
  await expect(page.getByTestId('ai-preset-deepseek')).toHaveCount(0)
  await expect(page.getByTestId('ai-save')).toHaveCount(0)
  await expect(page.getByTestId('ai-consent')).toBeVisible()
  await expect(page.getByTestId('ai-preview')).toBeVisible()
  await expect(page.getByTestId('ai-cli-unbound')).toBeVisible()

  await page.getByTestId('ai-detect-cli').click()
  const candidates = page.getByTestId('ai-cli-candidate')
  await expect(candidates).toHaveCount(1)
  await expect(candidates.first()).toContainText('C:/mock/claude.cmd')
  await expect(candidates.first()).toContainText('2.1.296')

  await candidates.first().getByTestId('ai-cli-bind').click()
  await expect(page.getByTestId('ai-status')).toHaveAttribute('data-state', 'ok')
  await expect(page.getByTestId('ai-status')).toContainText('CLI 已绑定')

  const bound = page.getByTestId('ai-cli-bound')
  await expect(bound).toBeVisible()
  await expect(bound).toContainText('C:/mock/claude.cmd')
  await expect(bound).toContainText('2.1.296')

  const config = await aiConfig(page)
  expect(config.provider).toBe('claude-cli')
  expect(config.cliPath).toBe('C:/mock/claude.cmd')
  expect(config.cliVersion).toContain('2.1.296')

  // The connection test still works in CLI mode.
  await page.getByTestId('ai-test').click()
  await expect(page.getByTestId('ai-status')).toHaveAttribute('data-state', 'ok')
  await expect(page.getByTestId('ai-status')).toContainText('连接成功')
  expect(pageErrors).toEqual([])
})

test('a manual CLI path can be bound and invalid paths report an error', async ({ page }) => {
  const pageErrors: string[] = []
  page.on('pageerror', (error) => pageErrors.push(String(error)))

  await openAiSection(page)
  await page.getByTestId('ai-provider').selectOption('claude-cli')

  await page.getByTestId('ai-cli-path').fill('C:/mock/claude.cmd')
  await page.getByTestId('ai-cli-bind-manual').click()
  await expect(page.getByTestId('ai-cli-bound')).toContainText('C:/mock/claude.cmd')
  await expect(page.getByTestId('ai-status')).toHaveAttribute('data-state', 'ok')

  const boundConfig = await aiConfig(page)
  expect(boundConfig.provider).toBe('claude-cli')
  expect(boundConfig.cliPath).toBe('C:/mock/claude.cmd')
  expect(boundConfig.cliVersion).toContain('2.1.296')

  // The mock rejects paths that were not detected; the bound CLI must survive.
  await page.getByTestId('ai-cli-path').fill('C:/nowhere/missing.cmd')
  await page.getByTestId('ai-cli-bind-manual').click()
  await expect(page.getByTestId('ai-status')).toHaveAttribute('data-state', 'error')
  await expect(page.getByTestId('ai-status')).toContainText('CLI 绑定失败')
  expect((await aiConfig(page)).cliPath).toBe('C:/mock/claude.cmd')
  await expect(page.getByTestId('ai-cli-bound')).toContainText('C:/mock/claude.cmd')
  expect(pageErrors).toEqual([])
})
