import { expect, test } from '@playwright/test'

// BUG-0016: the old "Move (with backup)" label promised a backup the GUI never
// made. Import mode names must describe what actually happens, and must not be
// confused with the separate Migration/export feature.

test('settings import modes describe the real behaviour', async ({ page }) => {
  await page.goto('/')
  await page.getByTestId('settings-btn').click()

  const section = page.getByTestId('settings-section-folders')
  await expect(section).toBeVisible()

  const options = section.locator('#importmode option')
  await expect(options).toHaveCount(3)
  await expect(options.nth(0)).toHaveText('链接（不移动）')
  await expect(options.nth(1)).toHaveText('移动（搬到应用目录）')
  await expect(options.nth(2)).toHaveText('仅登记（路径不变）')

  const summary = section.getByTestId('settings-import-summary')
  await expect(summary).toHaveText('原文件不动，只建立链接')
  await section.getByTestId('settings-import-info').hover()
  await expect(page.getByRole('tooltip')).toContainText('硬链接')
  await expect(section).not.toContainText('带备份')

  await section.locator('#importmode').selectOption('move')
  await expect(summary).toHaveText('搬到应用目录，原位置删除')
})

test('discover dialog explains move vs migration through progressive disclosure', async ({ page }) => {
  await page.goto('/')
  await page.getByTestId('discover-btn').click()
  await expect(page.getByTestId('discover-modal')).toBeVisible()

  // The import-mode row only appears once there are discover results.
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('discover-row').first()).toBeVisible()

  const options = page.locator('#mode option')
  await expect(options.nth(1)).toHaveText('移动（搬到应用目录）')

  // Default: one short consequence line, no wall of text.
  const summary = page.getByTestId('mode-summary')
  await expect(summary).toHaveText('原文件不动，只建立链接')
  await expect(page.getByTestId('mode-hint')).toHaveCount(0)

  await page.selectOption('#mode', 'move')
  await expect(summary).toHaveText('搬到应用目录，原位置删除')
  await page.selectOption('#mode', 'register')
  await expect(summary).toHaveText('只记录原路径，不移动文件')

  // Details are tucked behind the info affordance.
  await expect(page.getByRole('tooltip')).toHaveCount(0)
  await page.getByTestId('mode-info').hover()
  const tip = page.getByRole('tooltip')
  await expect(tip).toBeVisible()
  await expect(tip).toContainText('迁移')
  await expect(tip).not.toContainText('改名备份')

  await page.getByTestId('discover-close').click()
  await expect(page.getByTestId('discover-modal')).toHaveCount(0)
})
