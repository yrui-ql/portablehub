import { expect, test } from '@playwright/test'

// App icon pixelation is a user preference, not part of the theme.

test('pixelated app icons can be toggled from appearance settings', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)

  await page.getByTestId('settings-btn').click()
  const toggle = page.getByTestId('pixel-icons')
  const fontToggle = page.getByTestId('pixel-font')
  await expect(toggle).toBeVisible()
  await expect(toggle).not.toBeChecked()
  await expect(fontToggle).toBeChecked()
  await expect(page.locator('html')).toHaveAttribute('data-pixel-icons', 'false')
  await expect(page.locator('html')).toHaveAttribute('data-pixel-font', 'true')

  await fontToggle.uncheck()
  await expect(page.locator('html')).toHaveAttribute('data-pixel-font', 'false')
  await fontToggle.check()
  await expect(page.locator('html')).toHaveAttribute('data-pixel-font', 'true')

  await toggle.check()
  await expect(page.locator('html')).toHaveAttribute('data-pixel-icons', 'true')
  const config = await page.evaluate(() => (window as any).__PH_MOCK__.config)
  expect(config.pixelIcons).toBe(true)

  await toggle.uncheck()
  await expect(page.locator('html')).toHaveAttribute('data-pixel-icons', 'false')
})
