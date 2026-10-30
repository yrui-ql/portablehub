import { expect, test } from '@playwright/test'

// BUG-0015 · accessibility basics: visible focus, announced status, accessible
// names, reduced-motion support. Browser-only (mock API), like the other specs.

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

async function focusStyles(page: import('@playwright/test').Page) {
  return page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null
    if (!el) return null
    const s = getComputedStyle(el)
    return {
      tag: el.tagName.toLowerCase(),
      testid: el.getAttribute('data-testid'),
      outlineStyle: s.outlineStyle,
      outlineWidth: Number.parseFloat(s.outlineWidth) || 0
    }
  })
}

test('AC-B15-1: keyboard focus is visible on interactive elements', async ({ page }) => {
  await page.locator('body').click()
  // The search input is the first focusable element in the main view.
  await page.keyboard.press('Tab')
  const first = await focusStyles(page)
  expect(first?.outlineStyle).not.toBe('none')
  expect(first?.outlineWidth).toBeGreaterThanOrEqual(2)

  // Tab to the first toolbar button and check it too.
  await page.keyboard.press('Tab')
  const second = await focusStyles(page)
  expect(second?.outlineStyle).not.toBe('none')
  expect(second?.outlineWidth).toBeGreaterThanOrEqual(2)
})

test('AC-B15-2: toasts are announced as status', async ({ page }) => {
  // Trigger a normal (polite) toast: scan returns candidates, then add them.
  await page.getByTestId('scan-btn').click()
  await page.getByTestId('add-all').click()
  const toast = page.locator('.ph-toast')
  await expect(toast).toBeVisible()
  await expect(toast).toHaveAttribute('role', 'status')
  await expect(toast).toHaveAttribute('aria-live', 'polite')

  // Trigger an error toast: make the saved hotkey conflict, then save it.
  await page.evaluate(() => (window as any).__PH_MOCK__.setHotkeyConflict('Ctrl+Alt+P'))
  await page.getByTestId('settings-btn').click()
  // Save the hotkey form specifically: the settings page now has more save
  // buttons (e.g. AI settings), so scope the lookup instead of matching by name.
  await page
    .getByTestId('settings-section-hotkey')
    .getByRole('button', { name: '保存', exact: true })
    .click()
  const errorToast = page.locator('.ph-toast.error')
  await expect(errorToast).toBeVisible()
  await expect(errorToast).toHaveAttribute('aria-live', 'assertive')
})

test('AC-B15-3: the main search box has an accessible name', async ({ page }) => {
  await expect(page.locator('.ph-search').first()).toHaveAttribute('aria-label', '搜索应用')
})

test('AC-B15-4: reduced motion disables transitions', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  const duration = await page.evaluate(() => {
    const el = document.querySelector('.ph-btn') as HTMLElement
    const s = getComputedStyle(el)
    return s.transitionDuration
  })
  // 0.01ms is the intentional near-zero value.
  expect(duration.split(',').every((d) => Number.parseFloat(d) <= 0.01)).toBe(true)
})
