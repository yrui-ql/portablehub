import { expect, test, type Page } from '@playwright/test'

// REQ-0041 · AI classification of discover results.
// The mock returns dev/media/game/network suggestions by item order; the spec can
// additionally stretch or fail the call by patching the E2E mock's API object.
// Patching a mock method keeps api-mock.ts untouched (it is owned by the Lead).

interface SyntheticItem {
  name: string
  path: string
  kind?: 'folder' | 'file'
}

interface MockWindow {
  __PH_MOCK__: {
    setDiscoverItems: (list: unknown[]) => void
  }
}

const ITEMS: SyntheticItem[] = [
  { name: 'Zulu', path: 'E:/apps/one/Zulu' },
  { name: 'Yankee', path: 'D:/games/two/Yankee' },
  { name: 'Xray', path: 'D:/games/two/Xray.exe', kind: 'file' },
  { name: 'Whiskey', path: 'E:/apps/one/deep/Whiskey' }
]

// Mock/importer category order is by item index: dev, media, game, network.
// The panel orders groups by the fixed 11-category order: network, dev, game, media.
const CATEGORY_GROUP_KEYS = ['network', 'dev', 'game', 'media']
const CATEGORY_CHIP_LABELS = ['网络', '开发', '游戏', '影音图像']

let pageErrors: string[] = []

test.beforeEach(async ({ page }) => {
  pageErrors = []
  page.on('pageerror', (error) => pageErrors.push(String(error)))
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

async function setDiscoverItems(page: Page, list: SyntheticItem[]): Promise<void> {
  await page.evaluate((items) => {
    const mock = (window as unknown as MockWindow).__PH_MOCK__
    mock.setDiscoverItems(
      items.map((item) => ({
        name: item.name,
        path: item.path,
        exe: item.kind === 'file' ? item.path : `${item.path}/${item.name}.exe`,
        size: 1024 * 1024,
        kind: item.kind ?? 'folder',
        score: 500
      }))
    )
  }, list)
}

async function openAndScan(page: Page): Promise<void> {
  await page.getByTestId('discover-btn').click()
  await expect(page.getByTestId('discover-modal')).toBeVisible()
  await page.getByTestId('start-scan').click()
  await expect(page.getByTestId('discover-row')).toHaveCount(4)
}

/** Replace the mock classifier: `delay` keeps the in-flight state observable. */
async function patchClassify(page: Page, behaviour: 'delay' | 'throw'): Promise<void> {
  await page.evaluate(async (mode) => {
    const mod = (await import('/src/lib/api-mock.ts')) as unknown as {
      mockApi: { classifyDiscovered: (items: unknown[]) => Promise<unknown> }
    }
    const categories = ['dev', 'media', 'game', 'network']
    if (mode === 'throw') {
      mod.mockApi.classifyDiscovered = async () => {
        throw new Error('mock classify failure')
      }
      return
    }
    mod.mockApi.classifyDiscovered = async (items) => {
      await new Promise((resolve) => setTimeout(resolve, 700))
      return (items as { path: string }[]).map((item, index) => ({
        id: item.path,
        category: categories[index % categories.length],
        confidence: 0.9,
        reason: 'mock AI suggestion (slow)',
        source: 'ai'
      }))
    }
  }, behaviour)
}

test('classifies results, shows chips and groups by category', async ({ page }) => {
  await setDiscoverItems(page, ITEMS)
  await openAndScan(page)

  // Before classification the behaviour is unchanged: source-folder grouping
  // (three folders: E:/apps/one, E:/apps/one/deep and D:/games/two).
  await expect(page.getByTestId('discover-group')).toHaveCount(3)
  await expect(page.getByTestId('group-hint')).toHaveText('按来源目录分组')
  await expect(page.getByTestId('discover-category')).toHaveCount(0)
  await expect(page.getByTestId('discover-group-mode')).toHaveCount(0)

  await page.getByTestId('discover-classify').click()

  // One chip per row, category grouping is selected by default.
  await expect(page.getByTestId('discover-category')).toHaveCount(4)
  await expect(page.getByTestId('discover-category')).toHaveText(CATEGORY_CHIP_LABELS)
  await expect(page.getByTestId('discover-group-mode')).toHaveValue('category')
  await expect(page.getByTestId('group-hint')).toHaveText('按分类')
  await expect(page.getByTestId('discover-group')).toHaveCount(4)
  const keys = await page
    .getByTestId('discover-group')
    .evaluateAll((nodes) => nodes.map((node) => node.getAttribute('data-key')))
  expect(keys).toEqual(CATEGORY_GROUP_KEYS)
  await expect(page.getByTestId('discover-group-label')).toHaveText([
    '网络',
    '开发',
    '游戏',
    '影音图像'
  ])

  // Switching back to source grouping restores directory groups; chips stay.
  await page.getByTestId('discover-group-mode').selectOption('source')
  await expect(page.getByTestId('discover-group')).toHaveCount(3)
  await expect(page.getByTestId('group-hint')).toHaveText('按来源目录分组')
  await expect(page.getByTestId('discover-category')).toHaveCount(4)
  await expect(page.getByTestId('discover-row')).toHaveCount(4)

  await page.getByTestId('discover-group-mode').selectOption('category')
  await expect(page.getByTestId('discover-group')).toHaveCount(4)
  expect(pageErrors).toEqual([])
})

test('classified results can be selected and imported', async ({ page }) => {
  await setDiscoverItems(page, ITEMS)
  await openAndScan(page)
  await page.getByTestId('discover-classify').click()
  await expect(page.getByTestId('discover-category')).toHaveCount(4)

  await page.locator('.ph-discover-tools').getByRole('button', { name: '全选' }).click()
  await expect(page.getByTestId('selected-count')).toHaveText('已选 4 项')
  await page.getByTestId('import-btn').click()

  // The import command ran and the result panel reports all four rows.
  await expect(page.getByTestId('import-report')).toBeVisible()
  await expect(page.getByTestId('report-summary')).toHaveText('新增 4，跳过 0，失败 0')
  await page.getByTestId('report-close').click()
  await expect(page.getByTestId('discover-modal')).toBeHidden()
  await expect(page.getByTestId('app-card')).toHaveCount(7)
  expect(pageErrors).toEqual([])
})

test('classifying and failed classification states keep the results usable', async ({ page }) => {
  await setDiscoverItems(page, ITEMS)
  await openAndScan(page)

  // In-flight state: the button reports progress and the rows stay intact.
  await patchClassify(page, 'delay')
  await page.getByTestId('discover-classify').click()
  await expect(page.getByTestId('discover-classifying')).toHaveText('分类中…')
  await expect(page.getByTestId('discover-classify')).toBeDisabled()
  await expect(page.getByTestId('discover-row')).toHaveCount(4)
  await expect(page.getByTestId('discover-classifying')).toHaveCount(0)
  await expect(page.getByTestId('discover-category')).toHaveCount(4)

  // Failure state: readable danger text, results (and their chips) survive.
  await patchClassify(page, 'throw')
  await page.getByTestId('discover-classify').click()
  await expect(page.getByTestId('discover-classify-error')).toContainText(
    'mock classify failure'
  )
  await expect(page.getByTestId('discover-classify-error')).toHaveClass(/ph-danger/)
  await expect(page.getByTestId('discover-row')).toHaveCount(4)
  await expect(page.getByTestId('discover-category')).toHaveCount(4)

  // The results remain importable after a classification failure.
  await page.locator('.ph-discover-tools').getByRole('button', { name: '全选' }).click()
  await expect(page.getByTestId('selected-count')).toHaveText('已选 4 项')
  await expect(page.getByTestId('import-btn')).toBeEnabled()
  expect(pageErrors).toEqual([])
})
