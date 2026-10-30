import { expect, test, type Page } from '@playwright/test'

// REQ-0022 · main screen groups apps by source folder.
//
// Synthetic fixture: invented names and folders only. The mock seeds it before
// the bundle loads, so the default Alpha/Beta/Gamma fixture used by the other
// specs stays untouched.
const MEDIA = 'D:\\Fixture\\Media'
const TOOLS = 'D:\\Fixture\\Tools'
const UTILITIES = 'E:\\Fixture\\Utilities'

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
  exists: boolean
  icon: string | null
}

let nextId = 0
function seedApp(name: string, path: string): SeedApp {
  nextId += 1
  return {
    id: `seed-${nextId}`,
    name,
    path,
    args: [],
    workDir: '',
    autostart: false,
    autostartDelayMs: 0,
    skipIfRunning: true,
    hidden: false,
    exists: true,
    icon: null
  }
}

// Expected groups, in sidebar order: the hub folder first, then by label.
const APPS: SeedApp[] = [
  seedApp('Aurora', `${MEDIA}\\Aurora\\Aurora.exe`),
  seedApp('Borealis', `${MEDIA}\\Borealis\\Borealis.exe`),
  seedApp('Cascade', `${TOOLS}\\Cascade\\Cascade.exe`),
  seedApp('Delta', `${UTILITIES}\\Delta\\Delta.exe`),
  seedApp('Local Tool', 'Apps/Local Tool/Local Tool.exe')
]

interface MockWindow {
  __PH_MOCK__: {
    apps: Array<{ name: string }>
    launches: number
  }
}

async function launchCount(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as MockWindow).__PH_MOCK__.launches)
}

function navItem(page: Page, label: string) {
  return page.getByTestId('group-nav-item').filter({ hasText: label })
}

function searchBox(page: Page) {
  return page.locator('.ph-search').first()
}

function card(page: Page, name: string) {
  return page.locator(`[data-testid="app-card"][data-app-name="${name}"]`)
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript((apps) => {
    ;(window as unknown as { __PH_APPS_SEED__?: unknown }).__PH_APPS_SEED__ = apps
  }, APPS)
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(APPS.length)
})

/** REQ-0029: grouping is opt-in; enable it through the toolbar toggle. */
async function enableGrouping(page: Page) {
  await page.getByTestId('group-toggle').click()
  await expect(page.getByTestId('group-sidebar')).toBeVisible()
}

test('grouping is off by default and can be toggled', async ({ page }) => {
  // The library rail is always present; group sections stay flat until grouping is enabled.
  await expect(page.getByTestId('group-sidebar')).toBeVisible()
  await expect(page.getByTestId('grid-group')).toHaveCount(0)
  await expect(page.getByTestId('group-toggle')).toHaveAttribute('aria-pressed', 'false')
  await expect(page.getByTestId('app-card')).toHaveCount(APPS.length)

  await enableGrouping(page)
  await expect(page.getByTestId('group-toggle')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByTestId('grid-group')).toHaveCount(4)

  await page.getByTestId('group-toggle').click()
  await expect(page.getByTestId('group-sidebar')).toBeVisible()
  await expect(page.getByTestId('grid-group')).toHaveCount(0)
})

test('AC-29-3: the grouping preference survives a reload', async ({ page }) => {
  await enableGrouping(page)
  await page.reload()
  await expect(page.getByTestId('group-sidebar')).toBeVisible()
  await expect(page.getByTestId('group-toggle')).toHaveAttribute('aria-pressed', 'true')

  await page.getByTestId('group-toggle').click()
  await page.reload()
  await expect(page.getByTestId('group-sidebar')).toBeVisible()
  await expect(page.getByTestId('group-toggle')).toHaveAttribute('aria-pressed', 'false')
})

test('AC-22-1: sidebar lists every source with counts and "all" is selected by default', async ({
  page
}) => {
  await enableGrouping(page)
  await expect(page.getByTestId('group-sidebar')).toContainText('分组')

  const items = page.getByTestId('group-nav-item')
  await expect(items).toHaveCount(4)

  // Local folder first, then labels alphabetically.
  await expect(items.nth(0)).toContainText('程序目录')
  await expect(items.nth(1)).toContainText('Fixture\\Media')
  await expect(items.nth(2)).toContainText('Fixture\\Tools')
  await expect(items.nth(3)).toContainText('Fixture\\Utilities')

  // Per-group counts.
  await expect(items.nth(0)).toContainText('1')
  await expect(items.nth(1)).toContainText('2')
  await expect(items.nth(2)).toContainText('1')
  await expect(items.nth(3)).toContainText('1')

  const all = page.getByTestId('group-nav-all')
  await expect(all).toBeVisible()
  await expect(all).toContainText('全部')
  await expect(all).toContainText(String(APPS.length))
  await expect(all).toHaveAttribute('aria-current', 'true')
  for (const item of await items.all()) {
    await expect(item).not.toHaveAttribute('aria-current', 'true')
  }
})

test('AC-22-2: picking a group shows only its cards and highlights the entry', async ({ page }) => {
  await enableGrouping(page)
  const media = navItem(page, 'Fixture\\Media')
  await media.click()

  await expect(media).toHaveAttribute('aria-current', 'true')
  await expect(page.getByTestId('group-nav-all')).not.toHaveAttribute('aria-current', 'true')
  for (const other of ['程序目录', 'Fixture\\Tools', 'Fixture\\Utilities']) {
    await expect(navItem(page, other)).not.toHaveAttribute('aria-current', 'true')
  }

  await expect(page.getByTestId('app-card')).toHaveCount(2)
  await expect(card(page, 'Aurora')).toBeVisible()
  await expect(card(page, 'Borealis')).toBeVisible()
  for (const hidden of ['Cascade', 'Delta', 'Local Tool']) {
    await expect(card(page, hidden)).toHaveCount(0)
  }

  // Its count stays visible while the group is selected.
  await expect(media).toContainText('2')

  await navItem(page, 'Fixture\\Tools').click()
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await expect(card(page, 'Cascade')).toBeVisible()

  await navItem(page, '程序目录').click()
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await expect(card(page, 'Local Tool')).toBeVisible()

  await page.getByTestId('group-nav-all').click()
  await expect(page.getByTestId('group-nav-all')).toHaveAttribute('aria-current', 'true')
  await expect(page.getByTestId('app-card')).toHaveCount(APPS.length)
})

test('AC-22-3: "all" view shows one section per group and keeps card interactions', async ({
  page
}) => {
  await enableGrouping(page)
  const groups = page.getByTestId('grid-group')
  await expect(groups).toHaveCount(4)

  const labels = page.getByTestId('grid-group-label')
  await expect(labels).toHaveCount(4)
  await expect(labels.nth(0)).toContainText('程序目录')
  await expect(labels.nth(1)).toContainText('Fixture\\Media')
  await expect(labels.nth(2)).toContainText('Fixture\\Tools')
  await expect(labels.nth(3)).toContainText('Fixture\\Utilities')

  // Each section title carries the group name; the count sits next to it and
  // the cards stay inside their own section.
  await expect(groups.nth(0)).toContainText('1')
  await expect(groups.nth(1)).toContainText('2')
  await expect(groups.nth(1).getByTestId('app-card')).toHaveCount(2)
  await expect(groups.nth(1)).toContainText('Aurora')
  await expect(groups.nth(1)).toContainText('Borealis')
  await expect(groups.nth(2).getByTestId('app-card')).toHaveCount(1)

  // Single click still selects; double click still launches.
  const aurora = card(page, 'Aurora')
  await aurora.click()
  await expect(aurora).toHaveClass(/selected/)
  const before = await launchCount(page)
  await aurora.dblclick()
  await expect.poll(() => launchCount(page)).toBe(before + 1)
})

test('AC-22-4: search stacks with the selected group and shows the empty state', async ({
  page
}) => {
  await enableGrouping(page)
  await navItem(page, 'Fixture\\Media').click()

  const search = searchBox(page)
  await search.fill('aur')
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await expect(card(page, 'Aurora')).toBeVisible()

  await search.fill('bor')
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await expect(card(page, 'Borealis')).toBeVisible()

  await search.fill('zzz')
  await expect(page.getByTestId('app-card')).toHaveCount(0)
  await expect(page.locator('.ph-empty')).toBeVisible()

  await search.fill('')
  await expect(page.getByTestId('app-card')).toHaveCount(2)

  // The group filter also applies to the current query instead of resetting it.
  await search.fill('aur')
  await navItem(page, 'Fixture\\Tools').click()
  await expect(page.getByTestId('app-card')).toHaveCount(0)
  await expect(page.locator('.ph-empty')).toBeVisible()

  await page.getByTestId('group-nav-all').click()
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await expect(card(page, 'Aurora')).toBeVisible()
})
