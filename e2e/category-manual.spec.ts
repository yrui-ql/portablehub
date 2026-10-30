import { expect, test, type Page } from '@playwright/test'

// Categories are user-owned: AI may propose them, but the user can always pick
// or clear a category manually, and importing never writes AI categories
// unless the user explicitly opts in.

function card(page: Page, name: string) {
  return page.locator(`[data-testid="app-card"][data-app-name="${name}"]`)
}

test('category can be chosen manually for one app and for a selection', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)

  await card(page, 'Alpha').click({ button: 'right' })
  await page.getByTestId('menu-category').click()
  await expect(page.getByTestId('category-picker')).toBeVisible()
  await page.locator('[data-testid="category-option"][data-category="game"]').click()
  await expect(card(page, 'Alpha').getByTestId('card-category')).toHaveText('游戏')

  await card(page, 'Beta').click()
  await card(page, 'Gamma').click({ modifiers: ['Control'] })
  await page.getByTestId('selection-category').click()
  await page.locator('[data-testid="category-option"][data-category="dev"]').click()
  await expect(card(page, 'Beta').getByTestId('card-category')).toHaveText('开发')
  await expect(card(page, 'Gamma').getByTestId('card-category')).toHaveText('开发')

  const categories = await page.evaluate(() =>
    (window as any).__PH_MOCK__.apps.map((a: any) => ({ name: a.name, category: a.category }))
  )
  expect(categories).toEqual([
    { name: 'Alpha', category: 'game' },
    { name: 'Beta', category: 'dev' },
    { name: 'Gamma', category: 'dev' }
  ])
})

test('AI categories are not written on import unless enabled', async ({ page }) => {
  await page.goto('/')
  await page.evaluate(() =>
    (window as any).__PH_MOCK__.setDiscoverItems([
      {
        name: 'ZetaTool',
        path: 'E:/tools/ZetaTool',
        exe: 'E:/tools/ZetaTool/ZetaTool.exe',
        size: 1024,
        kind: 'folder',
        score: 900
      }
    ])
  )
  await page.getByTestId('discover-btn').click()
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('discover-row')).toHaveCount(1)
  await page.getByTestId('discover-classify').click()
  await expect(page.getByTestId('discover-category')).toHaveCount(1)
  await page.getByTestId('discover-row').locator('input[type=checkbox]').check()
  await expect(page.getByTestId('import-apply-categories')).not.toBeChecked()
  await page.getByTestId('import-btn').click()
  await expect(page.getByTestId('import-report')).toBeVisible()
  await page.getByTestId('report-close').click()

  const zeta = await page.evaluate(() =>
    (window as any).__PH_MOCK__.apps.find((a: any) => a.name === 'ZetaTool')
  )
  expect(zeta).toBeTruthy()
  expect(zeta.category).toBe('')
})
