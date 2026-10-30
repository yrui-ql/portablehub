import { expect, test, type Page } from '@playwright/test'

// BUG-0012 · "Change scope" must be view navigation, not a destructive rescan.
// REQ-0021 · discovered results are grouped by source directory.
// The E2E mock can swap the discover fixture so multi-directory grouping is
// covered without changing the default fixture other specs depend on.

interface SyntheticItem {
  name: string
  path: string
  kind?: 'folder' | 'file'
  size?: number
}

interface MockWindow {
  __PH_MOCK__: {
    discoverCalls: number
    discoverDelayMs: number
    setDiscoverItems: (list: unknown[]) => void
  }
}

async function setDiscoverItems(page: Page, list: SyntheticItem[]): Promise<void> {
  await page.evaluate((items) => {
    const mock = (window as unknown as MockWindow).__PH_MOCK__
    mock.setDiscoverItems(
      items.map((item) => ({
        name: item.name,
        path: item.path,
        exe: item.kind === 'file' ? item.path : `${item.path}/${item.name}.exe`,
        size: item.size ?? 1024 * 1024,
        kind: item.kind ?? 'folder',
        score: 500
      }))
    )
  }, list)
}

async function setDiscoverDelay(page: Page, ms: number): Promise<void> {
  await page.evaluate((delay) => {
    ;(window as unknown as MockWindow).__PH_MOCK__.discoverDelayMs = delay
  }, ms)
}

async function discoverCalls(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as MockWindow).__PH_MOCK__.discoverCalls)
}

async function openDiscover(page: Page): Promise<void> {
  await page.getByTestId('discover-btn').click()
  await expect(page.getByTestId('discover-modal')).toBeVisible()
}

function group(page: Page, key: string) {
  return page.locator(`[data-testid="discover-group"][data-key="${key}"]`)
}

const MULTI_DIR: SyntheticItem[] = [
  { name: 'Zulu', path: 'E:/apps/one/Zulu' },
  { name: 'Yankee', path: 'D:/games/two/Yankee' },
  { name: 'Xray', path: 'D:/games/two/Xray.exe', kind: 'file' },
  { name: 'Whiskey', path: 'E:/apps/one/deep/Whiskey' }
]

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

test('AC-21-1: results are grouped by source directory and sorted by path', async ({ page }) => {
  await setDiscoverItems(page, MULTI_DIR)
  await openDiscover(page)
  await page.getByTestId('start-scan').click()

  await expect(page.getByTestId('discover-group')).toHaveCount(3)
  const keys = await page
    .getByTestId('discover-group')
    .evaluateAll((nodes) => nodes.map((node) => node.getAttribute('data-key')))
  expect(keys).toEqual(['D:/games/two', 'E:/apps/one', 'E:/apps/one/deep'])
  await expect(page.getByTestId('group-hint')).toHaveText('按来源目录分组')

  const games = group(page, 'D:/games/two')
  await expect(games.getByTestId('discover-group-label')).toHaveText('D:\\games\\two')
  await expect(games.getByTestId('discover-group-count')).toHaveText('2 项')
  await expect(games.getByTestId('discover-row')).toHaveCount(2)

  const one = group(page, 'E:/apps/one')
  await expect(one.getByTestId('discover-group-label')).toHaveText('E:\\apps\\one')
  await expect(one.getByTestId('discover-row')).toHaveCount(1)

  const deep = group(page, 'E:/apps/one/deep')
  await expect(deep.getByTestId('discover-group-label')).toHaveText('E:\\…\\one\\deep')
  await expect(deep.getByTestId('discover-group-label')).toHaveAttribute(
    'title',
    'E:\\apps\\one\\deep'
  )
  await expect(deep.getByTestId('discover-row')).toHaveCount(1)

  await expect(page.getByTestId('discover-row')).toHaveCount(4)
})

test('AC-21-2: collapsing a group hides only that group', async ({ page }) => {
  await setDiscoverItems(page, MULTI_DIR)
  await openDiscover(page)
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('discover-row')).toHaveCount(4)

  const games = group(page, 'D:/games/two')
  const toggle = games.getByTestId('discover-group-toggle')
  await expect(toggle).toHaveText('▾')
  await toggle.click()

  await expect(games).toHaveAttribute('data-collapsed', 'true')
  await expect(toggle).toHaveText('▸')
  await expect(games.getByTestId('discover-row')).toHaveCount(0)
  // Other groups are unaffected.
  await expect(group(page, 'E:/apps/one').getByTestId('discover-row')).toHaveCount(1)
  await expect(group(page, 'E:/apps/one/deep').getByTestId('discover-row')).toHaveCount(1)
  await expect(page.getByTestId('discover-row')).toHaveCount(2)

  await toggle.click()
  await expect(games).toHaveAttribute('data-collapsed', 'false')
  await expect(games.getByTestId('discover-row')).toHaveCount(2)
  await expect(page.getByTestId('discover-row')).toHaveCount(4)
})

test('AC-21-3: "select group" only selects that group and only visible items', async ({ page }) => {
  await setDiscoverItems(page, MULTI_DIR)
  await openDiscover(page)
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('discover-row')).toHaveCount(4)
  await expect(page.getByTestId('selected-count')).toHaveText('已选 0 项')

  const games = group(page, 'D:/games/two')
  await games.getByTestId('discover-group-select').click()
  await expect(page.getByTestId('selected-count')).toHaveText('已选 2 项')
  const gamesBoxes = games.getByTestId('discover-row').locator('input[type=checkbox]')
  await expect(gamesBoxes.nth(0)).toBeChecked()
  await expect(gamesBoxes.nth(1)).toBeChecked()
  await expect(
    group(page, 'E:/apps/one').locator('input[type=checkbox]')
  ).not.toBeChecked()

  // Clicking again clears the group (toggle semantics).
  await games.getByTestId('discover-group-select').click()
  await expect(page.getByTestId('selected-count')).toHaveText('已选 0 项')

  // Filtered: only the visible item of the group is selected.
  const search = page.getByTestId('discover-modal').locator('input.ph-search')
  await search.fill('yank')
  await expect(page.getByTestId('discover-row')).toHaveCount(1)
  await games.getByTestId('discover-group-select').click()
  await expect(page.getByTestId('selected-count')).toHaveText('已选 1 项')
  await search.fill('')
  await expect(page.getByTestId('discover-row')).toHaveCount(4)
  // Rows inside the group are sorted by name: Xray then Yankee.
  await expect(gamesBoxes.nth(0)).not.toBeChecked()
  await expect(gamesBoxes.nth(1)).toBeChecked()
})

test('AC-21-4: search filters inside groups, hides empty groups and existing apps', async ({
  page
}) => {
  const items: SyntheticItem[] = [
    ...MULTI_DIR,
    { name: 'Alpha', path: 'E:/apps/one/Alpha' } // already registered app
  ]
  await setDiscoverItems(page, items)
  await openDiscover(page)
  await page.getByTestId('start-scan').click()

  // The existing Alpha row is always hidden.
  await expect(page.getByTestId('discover-row')).toHaveCount(4)
  await expect(group(page, 'E:/apps/one').getByTestId('discover-row')).toHaveCount(1)
  await expect(page.getByTestId('only-new')).toHaveCount(0)
  await expect(page.getByTestId('existing-hidden')).toContainText('已隐藏 1 个已存在')

  const search = page.getByTestId('discover-modal').locator('input.ph-search')
  await search.fill('whis')
  await expect(page.getByTestId('discover-group')).toHaveCount(1)
  await expect(group(page, 'E:/apps/one/deep').getByTestId('discover-row')).toHaveCount(1)
  await expect(page.getByTestId('discover-row')).toHaveCount(1)

  await search.fill('zzz')
  await expect(page.getByTestId('discover-group')).toHaveCount(0)
  await expect(page.getByTestId('discover-modal').getByText('没有发现可用应用')).toBeVisible()
})

test('AC-B12-1: "Change scope" keeps the scanned results', async ({ page }) => {
  await setDiscoverItems(page, [
    { name: 'Zulu', path: 'E:/apps/one/Zulu' },
    { name: 'Yankee', path: 'D:/games/two/Yankee' }
  ])
  await openDiscover(page)
  await expect(page.getByTestId('kept-results')).toHaveCount(0)
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('discover-row')).toHaveCount(2)
  expect(await discoverCalls(page)).toBe(1)

  await page.getByTestId('rescan-btn').click()
  // View switches to the scope step, but the results survive and no scan starts.
  await expect(page.getByTestId('start-scan')).toBeVisible()
  await expect(page.getByTestId('kept-results')).toHaveText('已有 2 项扫描结果')
  await expect(page.getByTestId('retry-scan')).toBeVisible()
  await expect(page.getByTestId('back-to-results')).toBeVisible()
  await expect(page.getByTestId('discover-row')).toHaveCount(0)
  expect(await discoverCalls(page)).toBe(1)

  // Back to the results and continue: select + import still works.
  await page.getByTestId('back-to-results').click()
  await expect(page.getByTestId('discover-row')).toHaveCount(2)
  await page.getByTestId('discover-row').first().locator('input[type=checkbox]').check()
  await expect(page.getByTestId('import-btn')).toBeEnabled()
  await page.getByTestId('import-btn').click()
  await expect(page.getByTestId('import-report')).toBeVisible()
  await page.getByTestId('report-close').click()
  await expect(page.getByTestId('discover-modal')).toBeHidden()
  await expect(page.getByTestId('app-card')).toHaveCount(4)
})

test('AC-B12-2: "Rescan" replaces the old results and starts a scan', async ({ page }) => {
  await setDiscoverItems(page, [{ name: 'Zulu', path: 'E:/apps/one/Zulu' }])
  await openDiscover(page)
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('discover-row')).toHaveCount(1)
  await expect(page.getByTestId('discover-row')).toContainText('Zulu')
  expect(await discoverCalls(page)).toBe(1)

  await page.getByTestId('rescan-btn').click()
  await expect(page.getByTestId('kept-results')).toHaveText('已有 1 项扫描结果')

  // The second scan returns a different fixture: the old result must be replaced.
  await setDiscoverItems(page, [{ name: 'Whiskey', path: 'E:/apps/three/Whiskey' }])
  await page.getByTestId('retry-scan').click()
  await expect(page.getByTestId('discover-row')).toHaveCount(1)
  await expect(page.getByTestId('discover-row')).toContainText('Whiskey')
  await expect(page.getByTestId('discover-row')).not.toContainText('Zulu')
  await expect(page.getByTestId('kept-results')).toHaveCount(0)
  await expect(page.getByTestId('start-scan')).toHaveCount(0)
  expect(await discoverCalls(page)).toBe(2)
})

test('AC-B12-3: closing and reopening during a scan keeps showing progress', async ({ page }) => {
  await setDiscoverDelay(page, 1500)
  await setDiscoverItems(page, [
    { name: 'Zulu', path: 'E:/apps/one/Zulu' },
    { name: 'Yankee', path: 'D:/games/two/Yankee' }
  ])
  await openDiscover(page)
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('scan-progress')).toBeVisible()

  await page.getByTestId('discover-close').click()
  await expect(page.getByTestId('discover-modal')).toBeHidden()
  await page.getByTestId('discover-btn').click()
  await expect(page.getByTestId('scan-progress')).toBeVisible()
  expect(await discoverCalls(page)).toBe(1)

  // It finishes while the dialog is open: grouped results appear, still one scan.
  await expect(page.getByTestId('discover-row')).toHaveCount(2, { timeout: 15_000 })
  await expect(page.getByTestId('discover-group')).toHaveCount(2)
  expect(await discoverCalls(page)).toBe(1)
})

test('BUG-0020: selecting a group exposes the import action in a fixed footer', async ({
  page
}) => {
  const many = Array.from({ length: 40 }, (_, index) => ({
    name: `Item-${String(index + 1).padStart(2, '0')}`,
    path: `E:/apps/many/Item-${String(index + 1).padStart(2, '0')}`
  }))
  await setDiscoverItems(page, many)
  await openDiscover(page)
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('discover-row')).toHaveCount(40)

  // "Select group" must not leave the user without a visible next action: the
  // footer with the selected count + import button stays pinned to the dialog.
  await page.getByTestId('discover-group-select').first().click()
  await expect(page.getByTestId('selected-count')).toHaveText('已选 40 项')
  const actions = page.getByTestId('discover-actions')
  await expect(actions).toBeVisible()
  const box = await actions.boundingBox()
  const viewport = page.viewportSize()
  expect(box).not.toBeNull()
  expect(viewport).not.toBeNull()
  expect(box!.y + box!.height).toBeLessThanOrEqual(viewport!.height + 1)

  const importBtn = page.getByTestId('import-btn')
  await expect(importBtn).toBeEnabled()
  await expect(importBtn).toContainText('导入所选 (40)')
  await importBtn.click()
  await expect(page.getByTestId('import-report')).toBeVisible()
})
