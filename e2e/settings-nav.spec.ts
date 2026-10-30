import { expect, test } from '@playwright/test'
import type { Locator, Page } from '@playwright/test'

// REQ-0012 · settings section navigation (AC-12-1 / AC-12-2 / AC-12-3).
// REQ-0035 adds the AI section between folders and about (7 entries in total).
const SECTIONS = ['appearance', 'language', 'startup', 'hotkey', 'folders', 'ai', 'about'] as const

async function openSettings(page: Page) {
  await page.getByTestId('settings-btn').click()
  await expect(page.locator('.ph-settings')).toBeVisible()
  await expect(page.getByTestId('settings-nav')).toBeVisible()
}

function content(page: Page): Locator {
  return page.locator('.ph-settings-content')
}

function navItem(page: Page, id: string): Locator {
  return page.getByTestId(`settings-nav-${id}`)
}

// Distance between the top of a section and the top of the scroll container.
function sectionTop(page: Page, id: string): Promise<number> {
  return content(page).evaluate((el, sid) => {
    const section = el.querySelector<HTMLElement>(`#settings-section-${sid}`)
    if (!section) return Number.NaN
    return section.getBoundingClientRect().top - el.getBoundingClientRect().top
  }, id)
}

async function expectSectionInScrollView(page: Page, id: string) {
  const viewportHeight = await content(page).evaluate((el) => el.clientHeight)
  await expect
    .poll(() => sectionTop(page, id), { message: `section ${id} should be scrolled into view` })
    .toBeGreaterThanOrEqual(-1)
  await expect.poll(() => sectionTop(page, id)).toBeLessThan(viewportHeight)
}

async function expectActive(page: Page, id: string) {
  await expect(navItem(page, id)).toHaveAttribute('aria-current', 'true')
  await expect(navItem(page, id)).toHaveClass(/\bactive\b/)
  for (const other of SECTIONS) {
    if (other === id) continue
    await expect(navItem(page, other)).not.toHaveAttribute('aria-current', 'true')
  }
}

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

test('AC-12-1: seven section entries with appearance highlighted by default', async ({ page }) => {
  await openSettings(page)

  await expect(page.locator('.ph-settings-nav-item')).toHaveCount(SECTIONS.length)
  for (const id of SECTIONS) await expect(navItem(page, id)).toBeVisible()

  // The default locale is zh-CN (mock config), so the first entry reads 外观.
  await expect(navItem(page, 'appearance')).toHaveText('外观')
  await expectActive(page, 'appearance')
  expect(await content(page).evaluate((el) => el.scrollTop)).toBe(0)
})

test('AC-12-2: clicking an entry scrolls to its section and highlights it', async ({ page }) => {
  await openSettings(page)

  const scrollHeight = await content(page).evaluate((el) => el.scrollHeight)
  const clientHeight = await content(page).evaluate((el) => el.clientHeight)
  expect(scrollHeight).toBeGreaterThan(clientHeight)

  for (const id of SECTIONS) {
    await navItem(page, id).click()
    await expectActive(page, id)
    await expectSectionInScrollView(page, id)
  }

  // The pane really moves: back to the top for the first entry, down for the last.
  await navItem(page, 'appearance').click()
  await expectActive(page, 'appearance')
  await expect.poll(() => content(page).evaluate((el) => el.scrollTop)).toBe(0)

  await navItem(page, 'about').click()
  await expectActive(page, 'about')
  await expect.poll(() => content(page).evaluate((el) => el.scrollTop)).toBeGreaterThan(0)
})

test('AC-12-3: nav and content stay usable in a 640px-wide window', async ({ page }) => {
  await page.setViewportSize({ width: 640, height: 800 })
  await openSettings(page)

  const navBox = await page.getByTestId('settings-nav').boundingBox()
  const contentBox = await content(page).boundingBox()
  const dialogBox = await page.locator('.ph-settings').boundingBox()
  expect(navBox).not.toBeNull()
  expect(contentBox).not.toBeNull()
  expect(dialogBox).not.toBeNull()

  // Two columns are adjacent: the nav never overlaps the content pane.
  expect(navBox!.x + navBox!.width).toBeLessThanOrEqual(contentBox!.x + 1)
  // The dialog stays fully inside the narrow viewport (no horizontal overflow).
  expect(dialogBox!.x).toBeGreaterThanOrEqual(0)
  expect(dialogBox!.x + dialogBox!.width).toBeLessThanOrEqual(640)
  expect(navBox!.width).toBeGreaterThan(0)
  expect(contentBox!.width).toBeGreaterThan(0)

  for (const id of SECTIONS) {
    const itemBox = await navItem(page, id).boundingBox()
    expect(itemBox).not.toBeNull()
    expect(itemBox!.x + itemBox!.width).toBeLessThanOrEqual(contentBox!.x + 1)
    await expect(navItem(page, id)).toBeEnabled()
    await navItem(page, id).click()
    await expectActive(page, id)
    await expectSectionInScrollView(page, id)
  }
})
