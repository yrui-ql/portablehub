import { expect, test } from '@playwright/test'

// REQ-0030 · settings help text must not expose implementation details
// (config.json / registry) when the control is self-explanatory.

test('language and startup hints do not leak implementation details', async ({ page }) => {
  await page.goto('/')
  await page.getByTestId('settings-btn').click()

  const language = page.getByTestId('settings-section-language')
  await expect(language).toBeVisible()
  await expect(language.locator('.ph-hint')).toHaveCount(0)

  const startup = page.getByTestId('settings-section-startup')
  await expect(startup).toBeVisible()
  await expect(startup).not.toContainText('注册表')
  await expect(startup).not.toContainText('config.json')
})

test('the import-mode summary stays short and details live in the info tip', async ({ page }) => {
  await page.goto('/')
  await page.getByTestId('settings-btn').click()

  const section = page.getByTestId('settings-section-folders')
  const summary = section.getByTestId('settings-import-summary')
  await expect(summary).toBeVisible()
  await expect(summary).toHaveText('原文件不动，只建立链接')
  const text = (await summary.innerText()).trim()
  // One short consequence, not a manual.
  expect(text.length).toBeLessThan(100)

  await section.getByTestId('settings-import-info').hover()
  const tip = page.getByRole('tooltip')
  await expect(tip).toBeVisible()
  await expect(tip).toContainText('硬链接')
  await expect(tip).not.toContainText('config.json')

  // Selecting another mode updates both the summary and the tip content.
  await section.locator('#importmode').selectOption('register')
  await expect(summary).toHaveText('只记录原路径，不移动文件')
  await section.getByTestId('settings-import-info').hover()
  await expect(page.getByRole('tooltip')).toContainText('迁移')
})
