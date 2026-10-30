import { expect, test, type Page } from '@playwright/test'

// REQ-0013 · 应用编辑对话框（AC-13-1..4）。
// 数据来自 src/lib/api-mock.ts：Alpha / Beta / Gamma，Alpha.path = Apps/Alpha/Alpha.exe。

/** 右键卡片 →「编辑」，等待对话框打开。 */
async function openEditFor(page: Page, appName: string) {
  const card = page.locator('.ph-card', { hasText: appName }).first()
  await card.click({ button: 'right' })
  await page.getByTestId('menu-edit').click()
  await expect(page.getByTestId('edit-modal')).toBeVisible()
}

async function mockApp(page: Page, id: string) {
  return page.evaluate(
    (appId) => (window as any).__PH_MOCK__.apps.find((a: any) => a.id === appId),
    id
  )
}

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

test('AC-13-1 open edit shows name, path, args and working directory', async ({ page }) => {
  await openEditFor(page, 'Alpha')

  await expect(page.getByTestId('edit-modal')).toBeVisible()
  await expect(page.getByTestId('edit-name')).toHaveValue('Alpha')
  await expect(page.getByTestId('edit-path')).toHaveValue('Apps/Alpha/Alpha.exe')
  await expect(page.getByTestId('edit-args')).toHaveValue('')
  await expect(page.getByTestId('edit-workdir')).toHaveValue('')
  // No validation error before the first save attempt.
  await expect(page.getByTestId('edit-error')).toHaveCount(0)
})

test('AC-13-2 saving a new name updates the card and reopens with the new value', async ({
  page
}) => {
  await openEditFor(page, 'Alpha')

  await page.getByTestId('edit-name').fill('Alpha Prime')
  // Whitespace-separated arguments are split into string[] on save.
  await page.getByTestId('edit-args').fill('  --flag   value  ')
  await page.getByTestId('edit-save').click()

  // Parent unmounts the dialog after a successful save.
  await expect(page.getByTestId('edit-modal')).toBeHidden()

  const renamed = page.getByTestId('app-card').filter({ hasText: 'Alpha Prime' })
  await expect(renamed).toHaveCount(1)
  await expect(renamed).toHaveAttribute('data-app-name', 'Alpha Prime')
  await expect(page.getByTestId('app-card')).toHaveCount(3)

  const saved = await mockApp(page, 'id-0')
  expect(saved.name).toBe('Alpha Prime')
  expect(saved.args).toEqual(['--flag', 'value'])

  // Reopening shows the persisted values.
  await openEditFor(page, 'Alpha Prime')
  await expect(page.getByTestId('edit-name')).toHaveValue('Alpha Prime')
  await expect(page.getByTestId('edit-args')).toHaveValue('--flag value')
})

test('AC-13-3 empty path (and empty name) blocks saving with a readable error', async ({
  page
}) => {
  await openEditFor(page, 'Alpha')

  await page.getByTestId('edit-path').fill('')
  await page.getByTestId('edit-save').click()

  await expect(page.getByTestId('edit-error')).toBeVisible()
  await expect(page.getByTestId('edit-error')).toHaveText('主程序路径不能为空')
  await expect(page.getByTestId('edit-modal')).toBeVisible()
  // Nothing was persisted.
  expect((await mockApp(page, 'id-0')).path).toBe('Apps/Alpha/Alpha.exe')
  await expect(page.getByTestId('app-card').first()).toHaveAttribute('data-app-name', 'Alpha')

  // A blank name is rejected the same way.
  await page.getByTestId('edit-path').fill('Apps/Alpha/Alpha.exe')
  await page.getByTestId('edit-name').fill('   ')
  await page.getByTestId('edit-save').click()
  await expect(page.getByTestId('edit-error')).toBeVisible()
  await expect(page.getByTestId('edit-error')).toHaveText('名称不能为空')
  await expect(page.getByTestId('edit-modal')).toBeVisible()
  expect((await mockApp(page, 'id-0')).name).toBe('Alpha')
})

test('AC-13-4 cancel closes the dialog and changes nothing', async ({ page }) => {
  await openEditFor(page, 'Alpha')
  await page.getByTestId('edit-name').fill('Should Not Save')
  await page.getByTestId('edit-workdir').fill('C:/elsewhere')
  await page.getByTestId('edit-cancel').click()

  await expect(page.getByTestId('edit-modal')).toBeHidden()
  await expect(page.getByTestId('app-card')).toHaveCount(3)
  await expect(
    page.getByTestId('app-card').filter({ hasText: 'Should Not Save' })
  ).toHaveCount(0)

  const untouched = await mockApp(page, 'id-0')
  expect(untouched.name).toBe('Alpha')
  expect(untouched.workDir).toBe('')
})

test('keyboard shortcuts do not leak past the edit dialog', async ({ page }) => {
  await openEditFor(page, 'Alpha')
  const before = await page.evaluate(() => (window as any).__PH_MOCK__.launches)

  // Enter submits the form instead of launching the selected app behind it.
  await page.getByTestId('edit-name').fill('Alpha Renamed')
  await page.getByTestId('edit-name').press('Enter')
  await expect(page.getByTestId('edit-modal')).toBeHidden()
  await expect(page.getByTestId('app-card').first()).toHaveAttribute(
    'data-app-name',
    'Alpha Renamed'
  )
  const after = await page.evaluate(() => (window as any).__PH_MOCK__.launches)
  expect(after).toBe(before)

  // `/` must stay in the input, and Ctrl+, must not stack a settings dialog.
  await openEditFor(page, 'Alpha Renamed')
  await page.getByTestId('edit-args').click()
  await page.keyboard.type('/')
  await expect(page.getByTestId('edit-args')).toHaveValue('/')
  await page.keyboard.press('Control+,')
  await expect(page.getByRole('dialog', { name: '设置' })).toHaveCount(0)
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('edit-modal')).toBeHidden()
})
