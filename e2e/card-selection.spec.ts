import { expect, test } from '@playwright/test'

// BUG-0030: the card's data-selected flag used to mirror autostart. Autostart
// apps therefore looked selected even when the user had not clicked them.

test('autostart cards are not highlighted unless actually selected', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)

  for (const card of await page.getByTestId('app-card').all()) {
    await expect(card).toHaveAttribute('data-selected', 'false')
    await expect(card).not.toHaveClass(/selected/)
    await expect(card).toHaveCSS('outline-style', 'none')
  }

  // Turn on autostart for Beta, then clear the incidental right-click selection.
  const beta = page.locator('[data-testid="app-card"][data-app-name="Beta"]')
  await beta.click({ button: 'right' })
  await page.locator('.ph-menu-item').filter({ hasText: '开机自启' }).click()
  await page.keyboard.press('Escape')
  await expect(beta).toHaveAttribute('data-autostart', 'true')
  await expect(beta).toHaveAttribute('data-selected', 'false')
  await expect(beta).toHaveCSS('outline-style', 'none')

  // Selecting a different card must not visually select Beta.
  const alpha = page.locator('[data-testid="app-card"][data-app-name="Alpha"]')
  await alpha.click()
  await expect(alpha).toHaveAttribute('data-selected', 'true')
  await expect(alpha).toHaveCSS('outline-style', 'solid')
  await expect(beta).toHaveAttribute('data-selected', 'false')
  await expect(beta).toHaveCSS('outline-style', 'none')
})
