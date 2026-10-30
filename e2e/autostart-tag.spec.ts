import { expect, test } from '@playwright/test'

// Built-in virtual tag: every app with "start with the hub" enabled.

test('the autostart tag filters apps in include / exclude cycles', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)

  // Enable autostart on Alpha through the existing context menu.
  const alpha = page.locator('[data-testid="app-card"][data-app-name="Alpha"]')
  await alpha.click({ button: 'right' })
  await page.locator('.ph-menu-item').filter({ hasText: '开机自启' }).click()
  await page.keyboard.press('Escape')

  await page.getByTestId('tag-filter-btn').click()
  const chip = page.getByTestId('tag-filter-autostart')
  await expect(chip).toHaveText('自启 (1)')

  await chip.click()
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await expect(alpha).toBeVisible()

  await chip.click()
  await expect(page.getByTestId('app-card')).toHaveCount(2)
  await expect(alpha).toHaveCount(0)

  await chip.click()
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})
