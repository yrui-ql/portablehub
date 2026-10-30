import { expect, test, type Page } from '@playwright/test'

// Quick tagging: right-click → click an existing tag, or "New tag…" → type + Enter.
// No full editor dialog and no Save button.

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

async function openMenu(page: Page, name: string) {
  await card(page, name).click({ button: 'right' })
  await expect(page.locator('.ph-menu')).toBeVisible()
}

test('new tag via the quick popover is written immediately', async ({ page }) => {
  await openMenu(page, 'Alpha')
  await page.getByTestId('menu-quick-tags-new').click()

  const quick = page.getByTestId('quick-tag')
  await expect(quick).toBeVisible()
  await page.getByTestId('quick-tag-input').fill('已玩')
  await page.getByTestId('quick-tag-input').press('Enter')

  await expect(card(page, 'Alpha').getByTestId('card-tag')).toHaveText('已玩')
  await expect(page.getByTestId('quick-tag-chip').filter({ hasText: '已玩' })).toBeVisible()
  const tags = await page.evaluate(
    () => (window as any).__PH_MOCK__.apps.find((a: any) => a.name === 'Alpha').tags
  )
  expect(tags).toEqual(['已玩'])

  await page.getByTestId('quick-tag').getByRole('button', { name: '取消' }).click()
  await expect(quick).toHaveCount(0)
  expect(pageErrors.map((e) => e.message)).toEqual([])
})

test('existing tag is one right-click + one click, and toggles off', async ({ page }) => {
  // Seed the tag once.
  await openMenu(page, 'Alpha')
  await page.getByTestId('menu-quick-tags-new').click()
  await page.getByTestId('quick-tag-input').fill('已玩')
  await page.getByTestId('quick-tag-input').press('Enter')
  await page.getByTestId('quick-tag').getByRole('button', { name: '取消' }).click()

  // Second tag on Beta with a single menu click.
  await openMenu(page, 'Beta')
  const quick = page.getByTestId('menu-quick-tag').filter({ hasText: '已玩' })
  await expect(quick).toHaveText(/\+ 已玩/)
  await quick.click()
  await expect(card(page, 'Beta').getByTestId('card-tag')).toHaveText('已玩')

  // Toggling it again removes it (check mark shown first).
  await openMenu(page, 'Beta')
  await expect(page.getByTestId('menu-quick-tag').filter({ hasText: '已玩' })).toHaveText(/✓ 已玩/)
  await page.getByTestId('menu-quick-tag').filter({ hasText: '已玩' }).click()
  await expect(card(page, 'Beta').getByTestId('card-tag')).toHaveCount(0)

  expect(pageErrors.map((e) => e.message)).toEqual([])
})
