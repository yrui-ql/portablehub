import { expect, test } from '@playwright/test'

// REQ-0042 · render windowing for large libraries: the main grid mounts at most
// one page (200) of cards; "show more" grows the window, and the window resets
// whenever the filter/grouping context changes.
//
// A synthetic 250-app fixture is seeded before the bundle loads, so this spec
// does not touch the default Alpha/Beta/Gamma fixtures of the other specs.

const APP_COUNT = 250

interface SeedApp {
  id: string
  name: string
  path: string
  args: string[]
  workDir: string
  autostart: boolean
  autostartDelayMs: number
  skipIfRunning: boolean
  hidden: boolean
  tags: string[]
  category: string
  exists: boolean
  icon: string | null
}

function seedApp(index: number): SeedApp {
  const name = `App ${String(index).padStart(3, '0')}`
  return {
    id: `seed-${index}`,
    name,
    path: `Apps/${name}/${name}.exe`,
    args: [],
    workDir: '',
    autostart: false,
    autostartDelayMs: 0,
    skipIfRunning: true,
    hidden: false,
    tags: [],
    category: '',
    exists: true,
    icon: null
  }
}

const SEED: SeedApp[] = Array.from({ length: APP_COUNT }, (_, index) => seedApp(index))

let pageErrors: Error[] = []

test.beforeEach(async ({ page }) => {
  pageErrors = []
  page.on('pageerror', (error) => pageErrors.push(error))
  await page.addInitScript((apps) => {
    ;(window as unknown as { __PH_APPS_SEED__?: unknown }).__PH_APPS_SEED__ = apps
  }, SEED)
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(200)
})

function expectNoPageErrors() {
  expect(pageErrors.map((e) => e.message)).toEqual([])
}

test('AC: the flat grid mounts one page and show-more grows it', async ({ page }) => {
  await expect(page.getByTestId('app-card')).toHaveCount(200)
  await expect(page.getByTestId('grid-visible-count')).toHaveText('显示 200/250')
  await expect(page.getByTestId('grid-more')).toHaveText('显示更多（剩余 50）')

  await page.getByTestId('grid-more').click()
  await expect(page.getByTestId('app-card')).toHaveCount(APP_COUNT)
  await expect(page.getByTestId('grid-visible-count')).toHaveText(`显示 ${APP_COUNT}/${APP_COUNT}`)
  await expect(page.getByTestId('grid-more')).toHaveCount(0)

  // A new search starts a fresh window; clearing it returns to the first page.
  const search = page.locator('.ph-search').first()
  await search.fill('zzz')
  await expect(page.getByTestId('app-card')).toHaveCount(0)
  await expect(page.locator('.ph-empty')).toBeVisible()

  await search.fill('')
  await expect(page.getByTestId('app-card')).toHaveCount(200)
  await expect(page.getByTestId('grid-visible-count')).toHaveText('显示 200/250')

  expectNoPageErrors()
})

test('AC: a group section has its own render window and show-more', async ({ page }) => {
  await page.getByTestId('group-toggle').click()
  await expect(page.getByTestId('group-sidebar')).toBeVisible()
  await expect(page.getByTestId('grid-group')).toHaveCount(1)
  await expect(page.getByTestId('app-card')).toHaveCount(200)

  // Sidebar counts stay full even though only one page is mounted.
  await expect(
    page.getByTestId('group-nav-item').filter({ hasText: '程序目录' })
  ).toContainText(String(APP_COUNT))
  await expect(page.getByTestId('group-more')).toHaveText('显示更多（剩余 50）')

  await page.getByTestId('group-more').click()
  await expect(page.getByTestId('app-card')).toHaveCount(APP_COUNT)
  await expect(page.getByTestId('group-more')).toHaveCount(0)

  expectNoPageErrors()
})

test('AC: keyboard selection grows the window so the card stays mounted', async ({ page }) => {
  const cards = page.getByTestId('app-card')
  await expect(cards).toHaveCount(200)

  // Select the last mounted card, then move one past the page boundary.
  await cards.nth(199).click()
  await expect(cards.nth(199)).toHaveClass(/selected/)
  await page.keyboard.press('ArrowRight')

  await expect(page.getByTestId('app-card')).toHaveCount(APP_COUNT)
  await expect(page.getByTestId('grid-visible-count')).toHaveText(`显示 ${APP_COUNT}/${APP_COUNT}`)

  expectNoPageErrors()
})

test('AC: switching tag/search context keeps the 200-card DOM cap', async ({ page }) => {
  // Give one app a tag so the tag filter entry exists, then check the cap holds
  // when switching search/grouping contexts.
  const card = page.getByTestId('app-card').first()
  await card.click({ button: 'right' })
  await page.getByTestId('menu-tags').click()
  await page.getByTestId('tag-input').fill('批量')
  await page.getByTestId('tag-add').click()
  await page.getByTestId('tag-save').click()
  await expect(page.getByTestId('tag-editor')).toBeHidden()

  await page.getByTestId('tag-filter-btn').click()
  await page.getByTestId('tag-filter-chip').first().click()
  await expect(page.getByTestId('app-card')).toHaveCount(1)

  await page.getByTestId('tag-clear-btn').click()
  await expect(page.getByTestId('app-card')).toHaveCount(200)

  expectNoPageErrors()
})
