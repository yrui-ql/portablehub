import { expect, test, type Page } from '@playwright/test'

// Tag styles are global: colour + optional emoji, persisted in config and shown
// on card chips, the filter bar, quick tag and the editor.

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

async function seedTag(page: Page) {
  await card(page, 'Alpha').click({ button: 'right' })
  await page.getByTestId('menu-quick-tags-new').click()
  await page.getByTestId('quick-tag-input').fill('已玩')
  await page.getByTestId('quick-tag-input').press('Enter')
  await page.getByTestId('quick-tag').getByRole('button', { name: '取消' }).click()
}

async function openManager(page: Page) {
  await page.getByTestId('tag-filter-btn').click()
  await page.getByTestId('tag-manage').click()
  await expect(page.getByTestId('tag-manager')).toBeVisible()
}

test('tag colour and icon are saved and rendered everywhere', async ({ page }) => {
  await seedTag(page)
  await openManager(page)

  const row = page.getByTestId('tag-style-row').filter({ hasText: '已玩' })
  await row.getByTestId('tag-color').filter({ has: page.locator('[data-color="#4ade80"]') }).count()
  await page.locator('[data-testid="tag-color"][data-color="#4ade80"]').click()
  const icon = row.getByTestId('tag-icon')
  await icon.fill('🐱')
  await icon.press('Enter')

  await expect(row.locator('.ph-tag')).toHaveText('🐱 已玩')

  await page.getByTestId('tag-manager-close').click()
  const chip = card(page, 'Alpha').getByTestId('card-tag')
  await expect(chip).toHaveText('🐱 已玩')
  await expect(chip).toHaveCSS('color', 'rgb(74, 222, 128)')

  // The filter bar was opened to reach the manager and carries the same style.
  const filterChip = page.getByTestId('tag-filter-chip').filter({ hasText: '已玩' })
  await expect(filterChip).toHaveText('🐱 已玩 (1)')
  await expect(filterChip).toHaveCSS('color', 'rgb(74, 222, 128)')
  expect(pageErrors.map((e) => e.message)).toEqual([])
})

test('reset clears colour and icon', async ({ page }) => {
  await seedTag(page)
  await openManager(page)
  await page.locator('[data-testid="tag-color"][data-color="#4ade80"]').click()
  const icon = page.getByTestId('tag-icon')
  await icon.fill('🐱')
  await icon.press('Enter')
  await expect(page.locator('[data-testid="tag-style-row"] .ph-tag')).toHaveText('🐱 已玩')

  await page.getByTestId('tag-style-reset').click()
  await expect(page.locator('[data-testid="tag-style-row"] .ph-tag')).toHaveText('已玩')
  await page.getByTestId('tag-manager-close').click()
  await expect(card(page, 'Alpha').getByTestId('card-tag')).toHaveText('已玩')
  expect(pageErrors.map((e) => e.message)).toEqual([])
})
