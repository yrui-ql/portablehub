import { expect, test, type Page } from '@playwright/test'

// REQ-0023 · migration dialog (source audit + self-contained bundle export).
// Fixtures come from src/lib/api-mock.ts: Alpha / Beta / Gamma under Apps/.
// The dialog is reachable through Settings; the target path is a plain input
// because pickDirectory is inert in a plain browser.

type MockHandle = {
  setAudit: (list: unknown[]) => void
  setExportFailures: (names: string[]) => void
  setTargetNotEmpty: (value: boolean) => void
}

test.beforeEach(async ({ page }) => {
  // The copy button is checked against a recorded clipboard write, the same
  // way e2e/import-report.spec.ts does it.
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

async function openMigration(page: Page) {
  await page.getByTestId('settings-btn').click()
  await expect(page.getByTestId('settings-nav')).toBeVisible()
  await page.getByTestId('migrate-open').click()
  await expect(page.getByTestId('migration-modal')).toBeVisible()
  // The audit resolves asynchronously; wait for the summary line to settle.
  await expect(page.getByTestId('audit-summary')).toBeVisible()
}

async function clipboardText(page: Page): Promise<string> {
  return page.evaluate(
    () => (window as unknown as { __PH_CLIP__: { text: string } }).__PH_CLIP__.text
  )
}

test('AC-23-1: audit lists every app with its source kind', async ({ page }) => {
  await openMigration(page)

  await expect(page.getByTestId('audit-row')).toHaveCount(3)
  await expect(page.getByTestId('audit-summary')).toHaveText('本目录 3 · 链接 0 · 外部 0 · 断链 0')

  for (const name of ['Alpha', 'Beta', 'Gamma']) {
    const row = page.getByTestId('audit-row').filter({ hasText: name })
    await expect(row).toHaveCount(1)
    await expect(row).toHaveAttribute('data-kind', 'local')
    await expect(row.getByTestId('audit-kind')).toHaveAttribute('data-kind', 'local')
    await expect(row.getByTestId('audit-kind')).toHaveText('本目录')
  }

  const alpha = page.getByTestId('audit-row').filter({ hasText: 'Alpha' })
  await expect(alpha.getByTestId('audit-path')).toHaveText('Apps/Alpha/Alpha.exe')
  await expect(alpha.getByTestId('audit-target')).toHaveText('Apps/Alpha/Alpha.exe')

  // Closing the migration dialog returns to Settings without closing it.
  await page.getByTestId('migrate-close').click()
  await expect(page.getByTestId('migration-modal')).toBeHidden()
  await expect(page.getByTestId('settings-nav')).toBeVisible()
})

test('AC-23-1: audit shows the four source kinds and resolved targets', async ({ page }) => {
  await page.evaluate(() => {
    const mock = (window as unknown as { __PH_MOCK__: MockHandle }).__PH_MOCK__
    mock.setAudit([
      {
        id: 'a1',
        name: 'Alpha',
        kind: 'local',
        path: 'Apps/Alpha/Alpha.exe',
        target: 'Apps/Alpha/Alpha.exe'
      },
      {
        id: 'a2',
        name: 'Beta',
        kind: 'linked',
        path: 'Apps/Beta/Beta.exe',
        target: 'D:/real/Beta/Beta.exe'
      },
      {
        id: 'a3',
        name: 'Gamma',
        kind: 'external',
        path: 'E:/tools/Gamma/Gamma.exe',
        target: 'E:/tools/Gamma/Gamma.exe'
      },
      { id: 'a4', name: 'Delta', kind: 'missing', path: 'Apps/Delta/Delta.exe', target: '' }
    ])
  })

  await openMigration(page)

  await expect(page.getByTestId('audit-row')).toHaveCount(4)
  await expect(page.getByTestId('audit-summary')).toHaveText(
    '本目录 1 · 链接 1 · 外部 1 · 断链 1'
  )

  const expected: Record<string, [string, string]> = {
    Alpha: ['local', '本目录'],
    Beta: ['linked', '链接'],
    Gamma: ['external', '外部路径'],
    Delta: ['missing', '断链']
  }
  for (const [name, [kind, label]] of Object.entries(expected)) {
    const row = page.getByTestId('audit-row').filter({ hasText: name })
    await expect(row).toHaveAttribute('data-kind', kind)
    await expect(row.getByTestId('audit-kind')).toHaveAttribute('data-kind', kind)
    await expect(row.getByTestId('audit-kind')).toHaveText(label)
  }

  // The resolved real path of a linked app is exposed in full via the title.
  const linked = page.getByTestId('audit-row').filter({ hasText: 'Beta' })
  await expect(linked.getByTestId('audit-target')).toHaveAttribute('title', 'D:/real/Beta/Beta.exe')
})

test('AC-23-2: export requires a target and reports copied apps plus the report path', async ({
  page
}) => {
  await openMigration(page)
  await expect(page.getByTestId('export-report')).toHaveCount(0)

  // Empty target is refused with the dialog's own message.
  await page.getByTestId('export-btn').click()
  await expect(page.getByTestId('migrate-error')).toHaveText('请先选择导出目标')
  await expect(page.getByTestId('export-report')).toHaveCount(0)

  await page.getByTestId('export-target').fill('D:/MigrateOut')
  await page.getByTestId('export-btn').click()

  await expect(page.getByTestId('export-report')).toBeVisible()
  await expect(page.getByTestId('export-done')).toHaveText('已导出 3 个应用')
  await expect(page.getByTestId('export-failures')).toHaveCount(0)
  await expect(page.getByTestId('export-report-path')).toHaveText(
    'D:/MigrateOut/migration-report.json'
  )
  await expect(page.getByTestId('migrate-error')).toHaveCount(0)

  await expect(page.getByTestId('export-row')).toHaveCount(3)
  for (const name of ['Alpha', 'Beta', 'Gamma']) {
    const row = page.getByTestId('export-row').filter({ hasText: name })
    await expect(row).toHaveAttribute('data-ok', 'true')
    await expect(row).toContainText('copied')
  }

  // The report can be copied as plain text.
  await page.getByTestId('copy-report').click()
  await expect(page.getByTestId('copy-report-hint')).toHaveText('已复制到剪贴板')
  const copied = await clipboardText(page)
  expect(copied).toContain('迁移报告')
  expect(copied).toContain('Alpha')
  expect(copied).toContain('migration-report.json')
})

test('AC-23-2/3: a failed app is listed while the rest still export', async ({ page }) => {
  await page.evaluate(() => {
    const mock = (window as unknown as { __PH_MOCK__: MockHandle }).__PH_MOCK__
    mock.setExportFailures(['Alpha'])
  })
  await openMigration(page)

  await page.getByTestId('export-target').fill('D:/MigrateOut2')
  await page.getByTestId('export-btn').click()

  await expect(page.getByTestId('export-report')).toBeVisible()
  await expect(page.getByTestId('export-done')).toHaveText('已导出 2 个应用')
  await expect(page.getByTestId('export-failures')).toHaveText('1 个失败')
  await expect(page.getByTestId('export-row')).toHaveCount(3)

  const failed = page.getByTestId('export-row').filter({ hasText: 'Alpha' })
  await expect(failed).toHaveAttribute('data-ok', 'false')
  await expect(failed).toContainText('mock export failed')
  await expect(page.getByTestId('export-failure-names')).toContainText('Alpha')

  const ok = page.getByTestId('export-row').filter({ hasText: 'Beta' })
  await expect(ok).toHaveAttribute('data-ok', 'true')
  await expect(page.getByTestId('export-report-path')).toHaveText(
    'D:/MigrateOut2/migration-report.json'
  )
})

test('AC-23-4: non-empty and inside-program targets are refused with a readable reason', async ({
  page
}) => {
  await openMigration(page)

  await page.evaluate(() => {
    const mock = (window as unknown as { __PH_MOCK__: MockHandle }).__PH_MOCK__
    mock.setTargetNotEmpty(true)
  })
  await page.getByTestId('export-target').fill('D:/NotEmpty')
  await page.getByTestId('export-btn').click()
  await expect(page.getByTestId('migrate-error')).toHaveText('目标目录必须是空目录（或尚不存在）')
  await expect(page.getByTestId('export-report')).toHaveCount(0)

  await page.evaluate(() => {
    const mock = (window as unknown as { __PH_MOCK__: MockHandle }).__PH_MOCK__
    mock.setTargetNotEmpty(false)
  })
  await page.getByTestId('export-target').fill('C:/PortableHub/Sub')
  await page.getByTestId('export-btn').click()
  await expect(page.getByTestId('migrate-error')).toHaveText('目标目录不能在程序目录内部')
  await expect(page.getByTestId('export-report')).toHaveCount(0)

  // The dialog recovers once a valid target is supplied; the error clears.
  await page.getByTestId('export-target').fill('D:/GoodTarget')
  await page.getByTestId('export-btn').click()
  await expect(page.getByTestId('export-report')).toBeVisible()
  await expect(page.getByTestId('migrate-error')).toHaveCount(0)
})
