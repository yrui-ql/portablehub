import { expect, test } from '@playwright/test'

// Manual groups: flat app collections, created from the current multi-selection.

test('create a manual group from selected apps and browse it', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)

  const alpha = page.locator('[data-testid="app-card"][data-app-name="Alpha"]')
  const beta = page.locator('[data-testid="app-card"][data-app-name="Beta"]')
  await alpha.click()
  await beta.click({ modifiers: ['Control'] })

  await page.getByTestId('selection-collection').click()
  await expect(page.getByTestId('collection-modal')).toBeVisible()
  await page.getByTestId('collection-new-name').fill('项目工程')
  await page.getByTestId('collection-create').click()
  await expect(page.getByTestId('collection-row')).toHaveCount(1)
  await page.getByTestId('collection-save').click()
  await expect(page.getByTestId('collection-modal')).toHaveCount(0)

  const stored = await page.evaluate(() => (window as any).__PH_MOCK__.config.collections)
  expect(stored).toHaveLength(1)
  expect(stored[0].name).toBe('项目工程')
  expect(stored[0].appIds).toHaveLength(2)

  // Switch the grid to manual groups and select the new group.
  await page.getByTestId('group-toggle').click()
  await page.getByTestId('group-mode').selectOption('collection')
  const group = page.getByTestId('group-nav-item').filter({ hasText: '项目工程' })
  await expect(group).toBeVisible()
  await group.click()
  await expect(page.getByTestId('app-card')).toHaveCount(2)
  await expect(page.getByTestId('app-card').filter({ hasText: 'Gamma' })).toHaveCount(0)
})
