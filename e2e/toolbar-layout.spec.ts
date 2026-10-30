import { expect, test } from '@playwright/test'

// BUG-0022 · when the toolbar runs out of room, buttons must keep their labels
// horizontal (they used to shrink until Chinese characters stacked vertically).
// The toolbar should wrap to a second row instead.

async function openApp(page: import('@playwright/test').Page): Promise<void> {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
}

async function expectHorizontalButtons(page: import('@playwright/test').Page): Promise<void> {
  const toolbar = page.locator('.ph-toolbar').first()
  const wrap = await toolbar.evaluate((el) => getComputedStyle(el).flexWrap)
  expect(wrap).toBe('wrap')

  const buttons = toolbar.locator('.ph-btn')
  const count = await buttons.count()
  expect(count).toBeGreaterThan(3)
  for (let i = 0; i < count; i++) {
    const button = buttons.nth(i)
    const box = await button.boundingBox()
    expect(box).not.toBeNull()
    expect(box!.height).toBeLessThanOrEqual(34)
    const style = await button.evaluate((el) => getComputedStyle(el).whiteSpace)
    expect(style).toBe('nowrap')
    const clipped = await button.evaluate(
      (el) => el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 1
    )
    expect(clipped, `button ${i} clips its label`).toBe(false)
  }
}

test('BUG-0022: toolbar labels stay horizontal at normal and minimum widths', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))

  await openApp(page)
  await expectHorizontalButtons(page)

  await page.setViewportSize({ width: 760, height: 600 })
  await openApp(page)
  await expectHorizontalButtons(page)

  // Wrapping keeps the page from growing a horizontal scrollbar.
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1)
  expect(overflow).toBe(false)
  expect(errors).toEqual([])
})
