import { expect, test } from '@playwright/test'

// The theme help text must not print a machine absolute path; the row offers an
// explicit action ("open themes folder") instead. (UI/UX review follow-up.)

test.beforeEach(async ({ page }) => {
  page.on('pageerror', () => {
    /* fail explicitly below if the action throws */
  })
  await page.goto('/')
  await page.getByTestId('settings-btn').click()
  await expect(page.getByTestId('settings-section-appearance')).toBeVisible()
})

test('theme hint is short and path-free, and the folder can be opened explicitly', async ({
  page
}) => {
  const section = page.getByTestId('settings-section-appearance')
  await expect(section.locator('.ph-hint')).toHaveCount(0)
  await section.getByTestId('theme-info').hover()
  const tip = page.getByRole('tooltip')
  await expect(tip).toBeVisible()
  await expect(tip).toContainText('Themes')
  // No Windows absolute path (for example an installation folder) in the help text.
  await expect(tip).not.toContainText(/[A-Za-z]:[\\/]/)

  const openThemes = section.getByTestId('open-themes')
  await expect(openThemes).toBeVisible()
  await openThemes.click()
  const calls = await page.evaluate(() => (window as any).__PH_MOCK__.openThemeDirCalls)
  expect(calls).toBe(1)
})

test('migration help is a short paragraph below its action, details in the info tip', async ({
  page
}) => {
  const section = page.getByTestId('settings-section-folders')
  const button = section.getByTestId('migrate-open')
  await expect(button).toBeVisible()
  const row = button.locator('xpath=..')
  // The short consequence is the last help paragraph in the folders section.
  const hint = section.locator('p.ph-hint').last()
  await expect(hint).toBeVisible()
  await expect(hint).toContainText('自包含')
  // The hint must not be inside the button row.
  await expect(row).not.toContainText('自包含')

  // The long explanation lives behind the info affordance.
  await section.getByTestId('migrate-info').hover()
  const tip = page.getByRole('tooltip')
  await expect(tip).toBeVisible()
  await expect(tip).toContainText('来源')
  await expect(tip).not.toContainText(/[A-Za-z]:[\\/]/)
})
