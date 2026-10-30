import { expect, test } from '@playwright/test'

// Basic desktop UE: search inputs expose a visible clear (×) affordance once
// they contain text, and clearing restores the full list.

test('main search has a clear icon that resets the query', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)

  const search = page.locator('.ph-search').first()
  await search.fill('Alpha')
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  const clear = page.getByTestId('search-clear')
  await expect(clear).toBeVisible()
  await clear.click()
  await expect(search).toHaveValue('')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

test('discover filter has a clear icon', async ({ page }) => {
  await page.goto('/')
  await page.getByTestId('discover-btn').click()
  await page.getByTestId('start-scan').click()
  const filter = page.getByTestId('discover-modal').locator('input.ph-search')
  await filter.fill('Delta')
  const clear = page.getByTestId('discover-filter-clear')
  await expect(clear).toBeVisible()
  await clear.click()
  await expect(filter).toHaveValue('')
})
