import { expect, test, type Locator, type Page } from '@playwright/test'

// BUG-0008 · badges reused by the candidate bar / discover rows / import report
// must stay inside their row instead of flying to the window's top-right corner;
// card status badges must keep their corner treatment; many/long candidates
// must not push the add/ignore actions out of the window.
// BUG-0009 · "add all" must report the real added/skipped counts.
// REQ-0019 · a discover scan is a background task: closing the dialog does not
// cancel it, the main screen shows the running state and stays usable, and the
// result survives a reopen (no re-scan, no loss). A real cancel keeps the
// partial results and marks them as incomplete.

interface Box {
  x: number
  y: number
  width: number
  height: number
}

async function boxOf(locator: Locator): Promise<Box> {
  const box = await locator.boundingBox()
  expect(box, `no bounding box for ${String(locator)}`).not.toBeNull()
  return box as Box
}

function expectInside(inner: Box, outer: Box, tolerance = 1): void {
  expect(inner.x, 'badge left edge').toBeGreaterThanOrEqual(outer.x - tolerance)
  expect(inner.y, 'badge top edge').toBeGreaterThanOrEqual(outer.y - tolerance)
  expect(inner.x + inner.width, 'badge right edge').toBeLessThanOrEqual(
    outer.x + outer.width + tolerance
  )
  expect(inner.y + inner.height, 'badge bottom edge').toBeLessThanOrEqual(
    outer.y + outer.height + tolerance
  )
}

interface CandidateLike {
  name: string
  relPath: string
  score: number
  hasManifest: boolean
}

interface MockWindow {
  __PH_MOCK__: {
    apps: Array<Record<string, unknown>>
    discoverDelayMs: number
    discoverCalls: number
    launches: number
    setScanCandidates: (list: CandidateLike[]) => void
  }
}

async function setDiscoverDelay(page: Page, ms: number): Promise<void> {
  await page.evaluate((delay) => {
    ;(window as unknown as MockWindow).__PH_MOCK__.discoverDelayMs = delay
  }, ms)
}

/** Inject the candidates the toolbar "Scan" action will return (BUG-0008 M1). */
async function setScanCandidates(page: Page, names: string[]): Promise<void> {
  await page.evaluate((list) => {
    ;(window as unknown as MockWindow).__PH_MOCK__.setScanCandidates(
      list.map((name, index) => ({
        name,
        relPath: `Apps/${name}/${name}.exe`,
        score: 900 - index,
        hasManifest: false
      }))
    )
  }, names)
}

async function discoverCalls(page: Page): Promise<number> {
  return page.evaluate(
    () => (window as unknown as MockWindow).__PH_MOCK__.discoverCalls
  )
}

async function launchCount(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as MockWindow).__PH_MOCK__.launches)
}

async function openDiscover(page: Page): Promise<void> {
  await page.getByTestId('discover-btn').click()
  await expect(page.getByTestId('discover-modal')).toBeVisible()
}

const LONG_CANDIDATE_NAMES = Array.from(
  { length: 24 },
  (_, index) => `SyntheticCandidate-${String(index + 1).padStart(2, '0')}`
)

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

test('folder scan opens the unified review list inside the add-apps modal', async ({ page }) => {
  await page.getByTestId('discover-btn').click()
  await expect(page.getByTestId('discover-modal')).toBeVisible()
  await page.getByTestId('scan-folder').click()

  const rows = page.getByTestId('discover-row')
  await expect(rows).toHaveCount(2)
  await expect(rows).toContainText(['Omega', 'Sigma'])

  for (let i = 0; i < (await rows.count()); i++) {
    const row = rows.nth(i)
    const rowBox = await boxOf(row)
    const badgeBox = await boxOf(row.locator('.ph-badge'))
    expectInside(badgeBox, rowBox)
    // The old global rule pinned .ph-badge to the window (top: 4px / right: 4px).
    expect(badgeBox.y, 'badge must stay inside its row').toBeGreaterThan(rowBox.y)
    expect(badgeBox.x + badgeBox.width).toBeLessThan(rowBox.x + rowBox.width)
  }

  // Scanning never changes the card grid.
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

test('many long candidates keep the review actions visible and clickable', async ({ page }) => {
  await setScanCandidates(page, LONG_CANDIDATE_NAMES)
  await page.getByTestId('discover-btn').click()
  await page.getByTestId('scan-folder').click()
  await expect(page.getByTestId('discover-row')).toHaveCount(24)

  const viewport = page.viewportSize()
  expect(viewport).not.toBeNull()
  const { width, height } = viewport as { width: number; height: number }

  // The actions stay inside the modal/viewport instead of being pushed off-screen.
  for (const locator of [page.getByRole('button', { name: '全选', exact: true }), page.getByTestId('import-btn')]) {
    await expect(locator).toBeVisible()
    const box = await boxOf(locator)
    expect(box.x).toBeGreaterThanOrEqual(0)
    expect(box.y).toBeGreaterThanOrEqual(0)
    expect(box.x + box.width).toBeLessThanOrEqual(width)
    expect(box.y + box.height).toBeLessThanOrEqual(height)
  }

  await page.getByRole('button', { name: '全选', exact: true }).click()
  await expect(page.getByTestId('selected-count')).toContainText('24')
  await page.getByTestId('import-btn').click()
  await expect(page.getByTestId('import-report')).toBeVisible()
  await page.getByTestId('report-close').click()
  await expect(page.getByTestId('app-card')).toHaveCount(27)
})

test('same-name scan hits are merged and existing apps are hidden', async ({ page }) => {
  // One already-added candidate (Alpha), one duplicated name (Omega), one new.
  await setScanCandidates(page, ['Alpha', 'Omega', 'Omega', 'Sigma'])
  await page.getByTestId('discover-btn').click()
  await page.getByTestId('scan-folder').click()

  const rows = page.getByTestId('discover-row')
  await expect(rows).toHaveCount(2)
  await expect(page.getByTestId('merged-count')).toContainText('已合并 1 个同名位置')
  await expect(page.getByTestId('merged-badge')).toHaveText('+1')
  await expect(page.getByTestId('existing-hidden')).toContainText('已隐藏 1 个已存在')

  await page.getByRole('button', { name: '全选', exact: true }).click()
  await page.getByTestId('import-btn').click()
  await expect(page.getByTestId('import-report')).toBeVisible()
  await page.getByTestId('report-close').click()
  await expect(page.getByTestId('app-card')).toHaveCount(5)
})

test('AC-B8-2: discover result and import report badges stay inside their rows', async ({ page }) => {
  await openDiscover(page)
  await page.getByTestId('start-scan').click()

  // Existing apps are always hidden: only Delta (new) remains.
  const rows = page.getByTestId('discover-row')
  await expect(rows).toHaveCount(1)

  for (let i = 0; i < (await rows.count()); i++) {
    const row = rows.nth(i)
    const rowBox = await boxOf(row)
    const badgeBox = await boxOf(row.locator('.ph-badge'))
    expectInside(badgeBox, rowBox)
    // The kind badge sits after the name column, never in the window corner.
    expect(badgeBox.x, 'badge must follow the name cell').toBeGreaterThan(rowBox.x + 18)
  }

  // The import report rows use the same global .ph-badge class.
  const delta = rows.filter({ hasText: 'Delta' })
  await delta.locator('input[type=checkbox]').check()
  await page.getByTestId('import-btn').click()
  await expect(page.getByTestId('import-report')).toBeVisible()

  const reportRow = page.getByTestId('report-row').first()
  const reportRowBox = await boxOf(reportRow)
  const reportBadgeBox = await boxOf(reportRow.locator('.ph-badge'))
  expectInside(reportBadgeBox, reportRowBox)
  // Same window-corner guard as above.
  expect(reportBadgeBox.y).toBeGreaterThan(reportRowBox.y)
})

test('AC-B8-3: card autostart/missing badges keep the top-right card corner', async ({ page }) => {
  await page.evaluate(() => {
    const mock = (window as unknown as MockWindow).__PH_MOCK__
    mock.apps[0].autostart = true
    mock.apps[1].exists = false
  })

  // Toggling autostart from the context menu refreshes the grid, which is the
  // only public way to make the mutated fixture visible.
  const alpha = page.locator('[data-testid="app-card"][data-app-name="Alpha"]')
  await alpha.click({ button: 'right' })
  await page.locator('.ph-menu').getByRole('button', { name: '设为开机自启' }).click()

  const alphaBadge = alpha.locator('.ph-badge')
  await expect(alphaBadge).toHaveClass(/on/)
  await expect(alphaBadge).toHaveText('自启')

  const beta = page.locator('[data-testid="app-card"][data-app-name="Beta"]')
  const betaBadge = beta.locator('.ph-badge')
  await expect(betaBadge).toHaveText('丢失')

  for (const [card, badge] of [
    [alpha, alphaBadge],
    [beta, betaBadge]
  ] as const) {
    const cardBox = await boxOf(card)
    const badgeBox = await boxOf(badge)
    expectInside(badgeBox, cardBox)
    // Top-right corner of the card: past the middle horizontally, near the top.
    expect(badgeBox.x, 'card badge must hug the right edge').toBeGreaterThan(
      cardBox.x + cardBox.width / 2
    )
    expect(badgeBox.y - cardBox.y, 'card badge must hug the top edge').toBeLessThan(20)
  }
})

test('AC-19-1/2: a closed dialog does not stop the scan and reopens in progress', async ({ page }) => {
  await setDiscoverDelay(page, 6000)
  await openDiscover(page)
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('scan-progress')).toBeVisible()

  // Closing is explicit and is NOT a cancel.
  await expect(page.getByTestId('discover-close')).toHaveText('关闭')
  await page.getByTestId('discover-close').click()
  await expect(page.getByTestId('discover-modal')).toBeHidden()

  // AC-19-1: the main screen owns the scan state and stays operable.
  await expect(page.getByTestId('scan-status')).toHaveText('正在扫描…')
  await expect(page.getByTestId('scan-cancel-top')).toBeVisible()

  const before = await launchCount(page)
  await page.locator('.ph-search').first().fill('bet')
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await page.getByTestId('app-card').first().dblclick()
  expect(await launchCount(page)).toBe(before + 1)
  await page.locator('.ph-search').first().fill('')
  await expect(page.getByTestId('app-card')).toHaveCount(3)

  await page.getByTestId('settings-btn').click()
  await expect(page.locator('.ph-settings')).toBeVisible()
  await page.locator('.ph-settings-head').getByRole('button', { name: '关闭' }).click()
  await expect(page.getByTestId('scan-status')).toBeVisible()

  // AC-19-2: reopening shows the running scan and must not start another one.
  await page.getByTestId('discover-btn').click()
  await expect(page.getByTestId('discover-modal')).toBeVisible()
  await expect(page.getByTestId('scan-progress')).toBeVisible()
  expect(await discoverCalls(page)).toBe(1)

  // It finishes while the dialog is open: results appear, still one scan only.
  await expect(page.getByTestId('discover-row')).toHaveCount(1, { timeout: 15_000 })
  expect(await discoverCalls(page)).toBe(1)
  await expect(page.getByTestId('scan-status')).toBeHidden()
})

test('AC-19-3: a scan finishing while closed toasts the count and keeps its results', async ({
  page
}) => {
  await setDiscoverDelay(page, 1500)
  await openDiscover(page)
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('scan-progress')).toBeVisible()
  await page.getByTestId('discover-close').click()
  await expect(page.getByTestId('discover-modal')).toBeHidden()

  await expect(page.locator('.ph-toast')).toContainText('发现 2 项')
  await expect(page.getByTestId('scan-status')).toBeHidden()
  expect(await discoverCalls(page)).toBe(1)

  // Reopening shows the finished results and does not scan again.
  await page.getByTestId('discover-btn').click()
  await expect(page.getByTestId('discover-row')).toHaveCount(1)
  expect(await discoverCalls(page)).toBe(1)
})

test('AC-19-4: cancelling keeps partial results, marks them incomplete, allows a rescan', async ({
  page
}) => {
  await setDiscoverDelay(page, 3000)
  await openDiscover(page)
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('scan-progress')).toBeVisible()
  await page.getByTestId('discover-close').click()
  await expect(page.getByTestId('scan-status')).toBeVisible()

  await page.getByTestId('scan-cancel-top').click()
  // The mock stops immediately and reports the partial result set (M4).
  await expect(page.locator('.ph-toast')).toContainText('已保留 1 项结果（可能不完整）')
  await expect(page.getByTestId('scan-status')).toBeHidden()
  expect(await discoverCalls(page)).toBe(1)

  // The partial results are kept but flagged as incomplete.
  await page.getByTestId('discover-btn').click()
  await expect(page.getByTestId('scan-incomplete')).toBeVisible()
  await expect(page.getByTestId('scan-incomplete')).toHaveText('扫描已取消，以下结果可能不完整')
  await expect(page.getByTestId('discover-row')).toHaveCount(1)

  // Rescan clears the cancelled state and starts from the scope step again.
  await page.getByTestId('rescan-btn').click()
  await expect(page.getByTestId('scan-incomplete')).toHaveCount(0)
  await expect(page.getByTestId('start-scan')).toBeVisible()
  await setDiscoverDelay(page, 0)
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('discover-row')).toHaveCount(1)
  await expect(page.getByTestId('scan-incomplete')).toHaveCount(0)
  expect(await discoverCalls(page)).toBe(2)
})

test('AC-19-4: the dialog scan-cancel button also keeps partial results', async ({ page }) => {
  await setDiscoverDelay(page, 3000)
  await openDiscover(page)
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('scan-progress')).toBeVisible()

  await page.getByTestId('scan-cancel').click()
  await expect(page.locator('.ph-toast')).toContainText('已保留 1 项结果（可能不完整）')
  await expect(page.getByTestId('scan-incomplete')).toBeVisible()
  await expect(page.getByTestId('discover-row')).toHaveCount(1)
  await expect(page.getByTestId('scan-status')).toBeHidden()
})
