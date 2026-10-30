import { expect, test, type Page } from '@playwright/test'

// Multi-select + batch tagging: Ctrl/⌘ click, Shift range, Ctrl+A, then one
// quick-tag action applies to every selected app.

let pageErrors: Error[] = []

test.beforeEach(async ({ page }) => {
  pageErrors = []
  page.on('pageerror', (error) => pageErrors.push(error))
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

function card(page: Page, name: string) {
  return page.locator(`[data-testid="app-card"][data-app-name="${name}"]`)
}

test('ctrl-click and shift-click select several apps, and batch tag writes to all', async ({
  page
}) => {
  await card(page, 'Alpha').click()
  await card(page, 'Beta').click({ modifiers: ['Control'] })
  await expect(page.getByTestId('selection-count')).toHaveText('已选 2 项')

  await page.getByTestId('batch-tag').click()
  const quick = page.getByTestId('quick-tag')
  await expect(quick).toBeVisible()
  await expect(quick).toContainText('批量标记 2 个应用')
  await page.getByTestId('quick-tag-input').fill('已玩')
  await page.getByTestId('quick-tag-input').press('Enter')

  await expect(card(page, 'Alpha').getByTestId('card-tag')).toHaveText('已玩')
  await expect(card(page, 'Beta').getByTestId('card-tag')).toHaveText('已玩')
  await expect(card(page, 'Gamma').getByTestId('card-tag')).toHaveCount(0)
  const tags = await page.evaluate(() =>
    (window as any).__PH_MOCK__.apps.map((a: any) => ({ name: a.name, tags: a.tags }))
  )
  expect(tags).toEqual([
    { name: 'Alpha', tags: ['已玩'] },
    { name: 'Beta', tags: ['已玩'] },
    { name: 'Gamma', tags: [] }
  ])
  expect(pageErrors.map((e) => e.message)).toEqual([])
})

test('shift-click extends a range, Ctrl+A selects everything and clear resets', async ({ page }) => {
  await card(page, 'Alpha').click()
  await card(page, 'Gamma').click({ modifiers: ['Shift'] })
  await expect(page.getByTestId('selection-count')).toHaveText('已选 3 项')

  await page.getByTestId('selection-clear').click()
  await expect(page.getByTestId('selection-bar')).toHaveCount(0)
  await expect(page.locator('.ph-card.selected')).toHaveCount(0)

  // Ctrl+A selects the visible set without touching inputs.
  await page.keyboard.press('Control+a')
  await expect(page.getByTestId('selection-count')).toHaveText('已选 3 项')
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('selection-bar')).toHaveCount(0)

  expect(pageErrors.map((e) => e.message)).toEqual([])
})
