import { expect, test, type Page } from '@playwright/test'

// Autostart can hide an app's window. The global switch lives in Settings; an
// app can override it in the edit dialog with a three-state choice:
// follow global / always hidden / always visible.

async function openEdit(page: Page, name: string) {
  const card = page.locator(`[data-testid="app-card"][data-app-name="${name}"]`)
  await card.click({ button: 'right' })
  await page.getByTestId('menu-edit').click()
  await expect(page.getByTestId('edit-modal')).toBeVisible()
}

async function mockQuietStart(page: Page, name: string): Promise<boolean | null | undefined> {
  return page.evaluate(
    (appName) =>
      (window as any).__PH_MOCK__.apps.find((app: { name: string }) => app.name === appName)
        .quietStart,
    name
  )
}

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

test('the editor defaults to following the global setting', async ({ page }) => {
  await openEdit(page, 'Alpha')
  await expect(page.getByTestId('edit-quiet-start')).toHaveValue('inherit')
  await expect(page.getByTestId('edit-modal')).toContainText('自启时的窗口')
  await page.getByTestId('edit-cancel').click()
})

test('choosing "start hidden" persists a per-app override', async ({ page }) => {
  await openEdit(page, 'Alpha')
  await page.getByTestId('edit-quiet-start').selectOption('quiet')
  await page.getByTestId('edit-save').click()
  await expect(page.getByTestId('edit-modal')).toBeHidden()

  expect(await mockQuietStart(page, 'Alpha')).toBe(true)

  await openEdit(page, 'Alpha')
  await expect(page.getByTestId('edit-quiet-start')).toHaveValue('quiet')
  await page.getByTestId('edit-cancel').click()
})

test('choosing "show the window" persists false and switching back clears it', async ({
  page
}) => {
  await openEdit(page, 'Beta')
  await page.getByTestId('edit-quiet-start').selectOption('visible')
  await page.getByTestId('edit-save').click()
  await expect(page.getByTestId('edit-modal')).toBeHidden()
  expect(await mockQuietStart(page, 'Beta')).toBe(false)

  await openEdit(page, 'Beta')
  await page.getByTestId('edit-quiet-start').selectOption('inherit')
  await page.getByTestId('edit-save').click()
  await expect(page.getByTestId('edit-modal')).toBeHidden()
  expect(await mockQuietStart(page, 'Beta')).toBeNull()
})
