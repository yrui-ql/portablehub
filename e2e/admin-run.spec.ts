import { expect, test, type Page } from '@playwright/test'

// BUG-0023 · "run as administrator" flag: edit dialog checkbox, card badge and
// persistence through the mock updateApp.

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

async function openEdit(page: Page, name: string) {
  await card(page, name).click({ button: 'right' })
  await page.getByTestId('menu-edit').click()
  await expect(page.getByTestId('edit-modal')).toBeVisible()
}

async function mockRunAsAdmin(page: Page, name: string): Promise<boolean | undefined> {
  return page.evaluate(
    (appName) =>
      (window as any).__PH_MOCK__.apps.find((app: { name: string }) => app.name === appName)
        .runAsAdmin,
    name
  )
}

test('enabling run-as-admin persists the flag and shows a card badge', async ({ page }) => {
  await openEdit(page, 'Alpha')
  const checkbox = page.getByTestId('edit-run-as-admin')
  await expect(checkbox).not.toBeChecked()
  await expect(page.getByTestId('edit-modal')).toContainText('以管理员身份运行')

  await checkbox.check()
  await page.getByTestId('edit-save').click()
  await expect(page.getByTestId('edit-modal')).toBeHidden()

  const badge = card(page, 'Alpha').getByTestId('card-admin')
  await expect(badge).toBeVisible()
  await expect(badge).toHaveText('管理员')
  await expect(badge).toHaveAttribute('title', '以管理员身份运行')
  expect(await mockRunAsAdmin(page, 'Alpha')).toBe(true)
  // Other apps are untouched.
  expect(await mockRunAsAdmin(page, 'Beta')).toBe(false)

  // Reopening the editor shows the saved state.
  await openEdit(page, 'Alpha')
  await expect(page.getByTestId('edit-run-as-admin')).toBeChecked()
  await page.getByTestId('edit-cancel').click()
  await expect(page.getByTestId('edit-modal')).toBeHidden()

  expectNoPageErrors()
})

test('unchecking run-as-admin clears the flag and the badge', async ({ page }) => {
  await openEdit(page, 'Alpha')
  await page.getByTestId('edit-run-as-admin').check()
  await page.getByTestId('edit-save').click()
  await expect(card(page, 'Alpha').getByTestId('card-admin')).toBeVisible()

  await openEdit(page, 'Alpha')
  await expect(page.getByTestId('edit-run-as-admin')).toBeChecked()
  await page.getByTestId('edit-run-as-admin').uncheck()
  await page.getByTestId('edit-save').click()
  await expect(page.getByTestId('edit-modal')).toBeHidden()

  await expect(card(page, 'Alpha').getByTestId('card-admin')).toHaveCount(0)
  expect(await mockRunAsAdmin(page, 'Alpha')).toBe(false)

  expectNoPageErrors()
})

test('the admin badge stacks with the autostart badge instead of overlapping', async ({ page }) => {
  // Turn on autostart through the public context-menu path first.
  const alpha = card(page, 'Alpha')
  await alpha.click({ button: 'right' })
  await page.locator('.ph-menu').getByRole('button', { name: '设为开机自启' }).click()
  await expect(alpha.locator('.ph-badge.on')).toBeVisible()

  await openEdit(page, 'Alpha')
  await page.getByTestId('edit-run-as-admin').check()
  await page.getByTestId('edit-save').click()
  await expect(page.getByTestId('edit-modal')).toBeHidden()

  const admin = alpha.getByTestId('card-admin')
  const auto = alpha.locator('.ph-badge.on')
  await expect(admin).toBeVisible()
  await expect(auto).toBeVisible()

  const adminBox = await admin.boundingBox()
  const autoBox = await auto.boundingBox()
  const cardBox = await alpha.boundingBox()
  expect(adminBox).not.toBeNull()
  expect(autoBox).not.toBeNull()
  expect(cardBox).not.toBeNull()

  // Badges stack vertically (no overlap) and both stay inside the card.
  expect(autoBox!.y).toBeGreaterThanOrEqual(adminBox!.y + adminBox!.height)
  for (const box of [adminBox!, autoBox!]) {
    expect(box.x).toBeGreaterThanOrEqual(cardBox!.x)
    expect(box.y).toBeGreaterThanOrEqual(cardBox!.y)
    expect(box.x + box.width).toBeLessThanOrEqual(cardBox!.x + cardBox!.width + 1)
  }

  expectNoPageErrors()
})
