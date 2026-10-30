import { expect, test, type Page } from '@playwright/test'

// REQ-0039 · smart categories: rule-based suggestions, review/apply, and the
// four grouping modes (off / source / tag / smart).
//
// The mock proposes dev / media / game in app order (Alpha / Beta / Gamma) with
// 92% confidence from local rules, and applyCategories writes the rows back.

let pageErrors: Error[] = []

test.beforeEach(async ({ page }) => {
  pageErrors = []
  page.on('pageerror', (error) => pageErrors.push(error))
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

function expectNoPageErrors() {
  expect(pageErrors.map((e) => e.message)).toEqual([])
}

function card(page: Page, name: string) {
  return page.locator(`[data-testid="app-card"][data-app-name="${name}"]`)
}

async function mockCategories(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    (window as any).__PH_MOCK__.apps.map((app: { category?: string }) => app.category ?? '')
  )
}

async function openSmartOrganize(page: Page) {
  await page.getByTestId('smart-organize').click()
  await expect(page.getByTestId('smart-modal')).toBeVisible()
  await expect(page.getByTestId('smart-row')).toHaveCount(3)
}

test('AC: suggestions are reviewed, applied and switch grouping to smart', async ({ page }) => {
  await openSmartOrganize(page)

  const alpha = page.getByTestId('smart-row').filter({ hasText: 'Alpha' })
  const beta = page.getByTestId('smart-row').filter({ hasText: 'Beta' })
  const gamma = page.getByTestId('smart-row').filter({ hasText: 'Gamma' })
  await expect(alpha.getByTestId('smart-category')).toHaveValue('dev')
  await expect(beta.getByTestId('smart-category')).toHaveValue('media')
  await expect(gamma.getByTestId('smart-category')).toHaveValue('game')
  await expect(alpha).toContainText('92%')
  await expect(alpha).toContainText('规则')

  await page.getByTestId('smart-apply').click()
  await expect(page.getByTestId('smart-modal')).toBeHidden()
  await expect(page.locator('.ph-toast')).toContainText('已为 3 个应用写入分类')

  // Cards show the semantic category chip.
  await expect(card(page, 'Alpha').getByTestId('card-category')).toHaveText('开发')
  await expect(card(page, 'Beta').getByTestId('card-category')).toHaveText('影音图像')
  await expect(card(page, 'Gamma').getByTestId('card-category')).toHaveText('游戏')

  // Applying categories turns grouping on in smart mode with three groups.
  await expect(page.getByTestId('group-toggle')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByTestId('group-mode')).toHaveValue('smart')
  await expect(page.getByTestId('group-sidebar')).toBeVisible()
  await expect(page.getByTestId('group-nav-item')).toHaveCount(3)
  await expect(page.getByTestId('grid-group')).toHaveCount(3)
  await expect(page.getByTestId('grid-group-label')).toHaveText(['开发', '游戏', '影音图像'])

  // Switching the mode re-derives the groups: every app lives under Apps/.
  await page.getByTestId('group-mode').selectOption('source')
  await expect(page.getByTestId('group-nav-item')).toHaveCount(1)
  await expect(page.getByTestId('grid-group')).toHaveCount(1)

  // No tags yet, so tag grouping is one "unclassified" group.
  await page.getByTestId('group-mode').selectOption('tag')
  await expect(page.getByTestId('group-nav-item')).toHaveCount(1)
  await expect(page.getByTestId('group-nav-item')).toContainText('未分类')
  await expect(page.getByTestId('grid-group-label')).toHaveText(['未分类'])

  // The grouping toggle still closes the grouped view.
  await page.getByTestId('group-toggle').click()
  await expect(page.getByTestId('group-toggle')).toHaveAttribute('aria-pressed', 'false')
  await expect(page.getByTestId('group-sidebar')).toBeVisible()
  await expect(page.getByTestId('group-mode')).toHaveCount(0)
  await expect(page.getByTestId('app-card')).toHaveCount(3)

  expectNoPageErrors()
})

test('AC: the reviewed category can be changed before applying', async ({ page }) => {
  await openSmartOrganize(page)

  const alpha = page.getByTestId('smart-row').filter({ hasText: 'Alpha' })
  await alpha.getByTestId('smart-category').selectOption('other')
  await page.getByTestId('smart-apply').click()
  await expect(page.getByTestId('smart-modal')).toBeHidden()

  await expect(card(page, 'Alpha').getByTestId('card-category')).toHaveText('其他')
  expect(await mockCategories(page)).toEqual(['other', 'media', 'game'])

  expectNoPageErrors()
})

test('AC: unchecked rows are not applied', async ({ page }) => {
  await openSmartOrganize(page)

  await page
    .getByTestId('smart-row')
    .filter({ hasText: 'Alpha' })
    .getByTestId('smart-row-select')
    .uncheck()
  await page.getByTestId('smart-apply').click()
  await expect(page.getByTestId('smart-modal')).toBeHidden()

  await expect(card(page, 'Alpha').getByTestId('card-category')).toHaveCount(0)
  await expect(card(page, 'Beta').getByTestId('card-category')).toHaveText('影音图像')
  expect(await mockCategories(page)).toEqual(['', 'media', 'game'])

  expectNoPageErrors()
})

test('AC: select all / clear and cancel never write categories', async ({ page }) => {
  await openSmartOrganize(page)

  // Every row starts selected, so the apply button is usable.
  await expect(page.getByTestId('smart-apply')).toBeEnabled()

  await page.getByTestId('smart-clear').click()
  await expect(page.getByTestId('smart-apply')).toBeDisabled()
  for (const box of await page.getByTestId('smart-row-select').all()) {
    await expect(box).not.toBeChecked()
  }

  await page.getByTestId('smart-select-all').click()
  for (const box of await page.getByTestId('smart-row-select').all()) {
    await expect(box).toBeChecked()
  }

  // Cancel closes and writes nothing.
  await page.getByTestId('smart-cancel').click()
  await expect(page.getByTestId('smart-modal')).toBeHidden()
  await expect(page.getByTestId('card-category')).toHaveCount(0)
  expect(await mockCategories(page)).toEqual(['', '', ''])

  // Esc does the same.
  await openSmartOrganize(page)
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('smart-modal')).toBeHidden()
  expect(await mockCategories(page)).toEqual(['', '', ''])

  expectNoPageErrors()
})
