import { expect, test } from '@playwright/test'

// The Discover scope must remember the last selected directories; a later
// scan should reuse them instead of resetting to every non-C drive.

test('discover remembers the last selected source directories', async ({ page }) => {
  await page.goto('/')
  await page.getByTestId('discover-btn').click()

  const drive = (name: string) =>
    page.locator(`[data-testid="scope-drive"][data-drive="${name}"]`)

  // Defaults: C off, D/E on.
  await expect(drive('C:')).not.toBeChecked()
  await expect(drive('D:')).toBeChecked()
  await expect(drive('E:')).toBeChecked()

  // Scan only D:, then close and reopen the dialog.
  await drive('E:').uncheck()
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('rescan-btn')).toBeVisible()
  await page.getByTestId('discover-close').click()

  await page.getByTestId('discover-btn').click()
  await page.getByTestId('rescan-btn').click()
  await expect(drive('D:')).toBeChecked()
  await expect(drive('E:')).not.toBeChecked()

  const config = await page.evaluate(() => (window as any).__PH_MOCK__.config)
  expect(config.discoverRoots).toEqual(['D:'])
})
