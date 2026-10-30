import { expect, test, type Page } from '@playwright/test'

// REQ-0014 · import result panel.
// The in-memory mock records clipboard writes so the copy button can be checked
// without relying on the browser's real clipboard permission.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const store: { text: string } = { text: '' }
    ;(window as unknown as Record<string, unknown>).__PH_CLIP__ = store
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          store.text = text
        }
      }
    })
  })
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

async function startScan(page: Page) {
  await page.getByTestId('discover-btn').click()
  await expect(page.getByTestId('discover-modal')).toBeVisible()
  await page.getByTestId('start-scan').click()
}

async function importFirstNew(page: Page) {
  await startScan(page)
  await expect(page.getByTestId('discover-row')).toHaveCount(1)
  await page.getByTestId('discover-row').first().locator('input[type=checkbox]').check()
  await page.getByTestId('import-btn').click()
  await expect(page.getByTestId('import-report')).toBeVisible()
}

/** Force the mock import of `name` to fail (see api-mock's `__PH_MOCK__.failOn`). */
async function failNextImport(page: Page, name: string) {
  await page.evaluate((failName) => {
    const mock = (
      window as unknown as { __PH_MOCK__: { failOn: (value: string) => void } }
    ).__PH_MOCK__
    mock.failOn(failName)
  }, name)
}

test('AC-14-1/2/3: summary, details, copy and close-refresh', async ({ page }) => {
  await importFirstNew(page)

  // AC-14-1: aggregated numbers + one row per imported item.
  await expect(page.getByTestId('report-summary')).toHaveText('新增 1，跳过 0，失败 0')
  await expect(page.getByTestId('report-row')).toHaveCount(1)
  const row = page.getByTestId('report-row').first()
  await expect(row).toHaveAttribute('data-state', 'added')
  await expect(row).toContainText('Delta')
  await expect(row).toContainText('已新增')
  await expect(row).toContainText('link')

  // AC-14-2: plain-text copy of the whole report.
  await expect(page.getByTestId('report-copy')).toBeVisible()
  await page.getByTestId('report-copy').click()
  await expect(page.getByTestId('report-copy-hint')).toHaveText('已复制到剪贴板')
  const copied = await page.evaluate(
    () => (window as unknown as { __PH_CLIP__: { text: string } }).__PH_CLIP__.text
  )
  expect(copied).toContain('导入结果：新增 1，跳过 0，失败 0')
  expect(copied).toContain('Delta')
  expect(copied).toContain('已新增')

  // AC-14-3: closing the panel closes the dialog and the grid has refreshed.
  await page.getByTestId('report-close').click()
  await expect(page.getByTestId('discover-modal')).toBeHidden()
  await expect(page.getByTestId('app-card')).toHaveCount(4)
})

test('AC-14-2: a rescan hides already-added apps entirely', async ({ page }) => {
  // First round: add Delta so it is on disk when the second window opens.
  await importFirstNew(page)
  await page.getByTestId('report-close').click()
  await expect(page.getByTestId('discover-modal')).toBeHidden()
  await expect(page.getByTestId('app-card')).toHaveCount(4)

  // Second round: existing rows are not rendered at all; only a count remains.
  await startScan(page)
  await expect(page.getByTestId('only-new')).toHaveCount(0)
  await expect(page.getByTestId('discover-row')).toHaveCount(0)
  await expect(page.getByTestId('existing-hidden')).toContainText('已隐藏 2 个已存在')

  await page.getByTestId('discover-close').click()
  await expect(page.getByTestId('discover-modal')).toBeHidden()
  await expect(page.getByTestId('app-card')).toHaveCount(4)
})

test('AC-14-2: copy degrades to selectable text when the clipboard is blocked', async ({ page }) => {
  await importFirstNew(page)

  // Simulate a denied clipboard permission.
  await page.evaluate(() => {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      get: () => ({
        writeText: async () => {
          throw new Error('denied')
        }
      })
    })
  })

  await page.getByTestId('report-copy').click()
  await expect(page.getByTestId('report-copy-hint')).toContainText('剪贴板不可用')
  const fallback = page.locator('textarea.ph-report-fallback')
  await expect(fallback).toBeVisible()
  await expect(fallback).toHaveValue(/导入结果：新增 1，跳过 0，失败 0/)
  await expect(fallback).toHaveValue(/Delta/)

  await page.getByTestId('report-close').click()
  await expect(page.getByTestId('discover-modal')).toBeHidden()
  await expect(page.getByTestId('app-card')).toHaveCount(4)
})

test('AC-14-2: failed imports are counted and keep a readable message', async ({ page }) => {
  await failNextImport(page, 'Delta')
  await startScan(page)
  await expect(page.getByTestId('discover-row')).toHaveCount(1)
  await page.getByTestId('discover-row').first().locator('input[type=checkbox]').check()
  await page.getByTestId('import-btn').click()

  await expect(page.getByTestId('import-report')).toBeVisible()
  await expect(page.getByTestId('report-summary')).toHaveText('新增 0，跳过 0，失败 1')
  const row = page.getByTestId('report-row').first()
  await expect(row).toHaveAttribute('data-state', 'failed')
  await expect(row).toContainText('Delta')
  await expect(row).toContainText('失败')
  await expect(row).toContainText('mock import failed')

  // Failed rows are visually distinct and the full message is reachable on hover.
  await expect(row.locator('.ph-discover-path')).toHaveAttribute('title', 'mock import failed')
  await expect(row.locator('.ph-discover-path')).toHaveClass(/ph-danger/)
  await expect(row.locator('.ph-badge')).toHaveClass(/missing/)

  // Copying still includes failed items.
  await page.getByTestId('report-copy').click()
  const copied = await page.evaluate(
    () => (window as unknown as { __PH_CLIP__: { text: string } }).__PH_CLIP__.text
  )
  expect(copied).toContain('导入结果：新增 0，跳过 0，失败 1')
  expect(copied).toContain('Delta')
  expect(copied).toContain('失败')
  expect(copied).toContain('mock import failed')

  // A failure is not added to the grid.
  await page.getByTestId('report-close').click()
  await expect(page.getByTestId('discover-modal')).toBeHidden()
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})
