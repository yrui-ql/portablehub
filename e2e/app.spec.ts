import { expect, test } from '@playwright/test'

// AC-B6-1: mounting the UI without Tauri must not raise uncaught errors.
let pageErrors: Error[] = []

test.beforeEach(async ({ page }) => {
  pageErrors = []
  page.on('pageerror', (error) => pageErrors.push(error))
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
  expect(pageErrors.map((e) => e.message)).toEqual([])
})

test('renders cards and filters by search', async ({ page }) => {
  await expect(page.getByTestId('app-card')).toHaveCount(3)

  await page.locator('.ph-search').first().fill('bet')
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await expect(page.getByTestId('app-card')).toHaveAttribute('data-app-name', 'Beta')

  // Right-click on empty space must not open the WebView2 browser menu.
  const prevented = await page.evaluate(() => {
    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
    document.querySelector('.ph-grid-wrap')!.dispatchEvent(event)
    return event.defaultPrevented
  })
  expect(prevented).toBe(true)
  await expect(page.locator('.ph-menu')).toHaveCount(0)

  // Right-click on a card opens the app-owned menu, with the app name as its header.
  await page.locator('.ph-search').first().fill('')
  await page.getByTestId('app-card').first().click({ button: 'right' })
  await expect(page.locator('.ph-menu')).toBeVisible()
  await expect(page.getByTestId('menu-title')).toContainText('Alpha')
  await page.keyboard.press('Escape')

  await page.locator('.ph-search').first().fill('zzz')
  await expect(page.getByTestId('app-card')).toHaveCount(0)
  await expect(page.locator('.ph-empty')).toBeVisible()
})

test('single click selects, double click launches', async ({ page }) => {
  const card = page.getByTestId('app-card').first()
  await card.click()
  await expect(card).toHaveClass(/selected/)

  const before = await page.evaluate(() => (window as any).__PH_MOCK__.launches)
  await card.dblclick()
  const after = await page.evaluate(() => (window as any).__PH_MOCK__.launches)
  expect(after).toBe(before + 1)
})

test('arrow keys move selection and Enter launches', async ({ page }) => {
  await page.keyboard.press('ArrowRight')
  await expect(page.getByTestId('app-card').first()).toHaveClass(/selected/)

  const before = await page.evaluate(() => (window as any).__PH_MOCK__.launches)
  await page.keyboard.press('Enter')
  const after = await page.evaluate(() => (window as any).__PH_MOCK__.launches)
  expect(after).toBe(before + 1)
})

test('discover: scope -> scan -> select -> import', async ({ page }) => {
  await page.getByTestId('discover-btn').click()
  await expect(page.getByTestId('discover-modal')).toBeVisible()

  // Scope step only; nothing is selected or scanned yet.
  await expect(page.getByTestId('start-scan')).toBeVisible()
  await page.getByTestId('start-scan').click()

  await expect(page.getByTestId('discover-row')).toHaveCount(1)
  await page.getByTestId('discover-row').first().locator('input[type=checkbox]').check()
  await expect(page.getByTestId('import-btn')).toBeEnabled()
  await page.getByTestId('import-btn').click()

  // Import keeps the dialog open on the result panel (REQ-0014).
  await expect(page.getByTestId('import-report')).toBeVisible()
  await page.getByTestId('report-close').click()
  await expect(page.getByTestId('discover-modal')).toBeHidden()
  await expect(page.getByTestId('app-card')).toHaveCount(4)
})

test('settings opens and language switches', async ({ page }) => {
  await page.getByTestId('settings-btn').click()
  await expect(page.locator('.ph-settings')).toBeVisible()

  await page.locator('#locale').selectOption('en')
  await expect(page.locator('.ph-settings-head strong')).toHaveText('Settings')

  await page.getByTestId('settings-close').click()
  await expect(page.getByTestId('discover-btn')).toHaveText('Add apps')
})

test('titlebar buttons are inert in a plain browser', async ({ page }) => {
  await page.locator('.ph-wbtn.close').click()
  await page.locator('.ph-wbtn').first().click()
  await expect(page.getByTestId('app-card')).toHaveCount(3)
  expect(pageErrors.map((e) => e.message)).toEqual([])
})
