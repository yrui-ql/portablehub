import { expect, test, type Page } from '@playwright/test'

// REQ-0032 · app tags: editor, card chips, AND tag filtering and tag search.
// Fixtures are the default mock apps (Alpha / Beta / Gamma, no tags).

let pageErrors: Error[] = []

test.beforeEach(async ({ page }) => {
  pageErrors = []
  page.on('pageerror', (error) => pageErrors.push(error))
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
})

function expectNoPageErrors() {
  expect(pageErrors.map((e) => e.message)).toEqual([])
}

function card(page: Page, name: string) {
  return page.locator(`[data-testid="app-card"][data-app-name="${name}"]`)
}

async function openTagEditor(page: Page, appName: string) {
  await card(page, appName).click({ button: 'right' })
  await page.getByTestId('menu-tags').click()
  await expect(page.getByTestId('tag-editor')).toBeVisible()
}

/** Add tags through the dialog and save; waits for the dialog to close. */
async function addTags(page: Page, appName: string, tags: string[]) {
  await openTagEditor(page, appName)
  for (const tag of tags) {
    await page.getByTestId('tag-input').fill(tag)
    await page.getByTestId('tag-add').click()
  }
  await page.getByTestId('tag-save').click()
  await expect(page.getByTestId('tag-editor')).toBeHidden()
}

test('adding a tag shows a chip on the card and persists it', async ({ page }) => {
  await expect(page.getByTestId('tag-filter-btn')).toHaveCount(0)

  await openTagEditor(page, 'Alpha')
  await expect(page.getByTestId('tag-current')).toHaveCount(0)
  await expect(page.getByTestId('tag-existing-empty')).toBeVisible()

  // Enter commits the draft tag.
  await page.getByTestId('tag-input').fill('工具')
  await page.getByTestId('tag-input').press('Enter')
  await expect(page.getByTestId('tag-current')).toHaveCount(1)
  await expect(page.getByTestId('tag-current').first()).toHaveText(/工具/)

  await page.getByTestId('tag-save').click()
  await expect(page.getByTestId('tag-editor')).toBeHidden()

  const alpha = card(page, 'Alpha')
  await expect(alpha.getByTestId('card-tag')).toHaveCount(1)
  await expect(alpha.getByTestId('card-tag').first()).toHaveText('工具')
  // The toolbar entry only appears once a tag exists.
  await expect(page.getByTestId('tag-filter-btn')).toBeVisible()

  const stored = await page.evaluate(
    () => (window as any).__PH_MOCK__.apps.find((a: any) => a.name === 'Alpha').tags
  )
  expect(stored).toEqual(['工具'])

  // Reopening shows the saved tag as selected and as an existing tag; removing
  // it and saving clears the card chip again.
  await openTagEditor(page, 'Alpha')
  await expect(page.getByTestId('tag-current')).toHaveCount(1)
  await expect(page.getByTestId('tag-existing').first()).toHaveAttribute('aria-pressed', 'true')
  await page.getByTestId('tag-remove').click()
  await expect(page.getByTestId('tag-current')).toHaveCount(0)
  await page.getByTestId('tag-save').click()
  await expect(page.getByTestId('tag-editor')).toBeHidden()
  await expect(card(page, 'Alpha').getByTestId('card-tag')).toHaveCount(0)
  await expect(page.getByTestId('tag-filter-btn')).toHaveCount(0)

  expectNoPageErrors()
})

test('cards show at most three tags plus a +N marker', async ({ page }) => {
  await addTags(page, 'Alpha', ['a', 'b', 'c', 'd'])

  const alpha = card(page, 'Alpha')
  await expect(alpha.getByTestId('card-tag')).toHaveCount(3)
  await expect(alpha.getByTestId('card-tag')).toHaveText(['a', 'b', 'c'])
  await expect(alpha.getByTestId('card-tag-more')).toHaveText('+1')

  // Untagged apps render no chips at all.
  await expect(card(page, 'Beta').getByTestId('card-tag')).toHaveCount(0)
  await expect(card(page, 'Beta').getByTestId('card-tag-more')).toHaveCount(0)

  expectNoPageErrors()
})

test('the tag filter bar narrows the grid and clear restores it', async ({ page }) => {
  await addTags(page, 'Alpha', ['工具'])

  await page.getByTestId('tag-filter-btn').click()
  const bar = page.getByTestId('tag-filter-bar')
  await expect(bar).toBeVisible()
  await expect(page.getByTestId('tag-filter-chip')).toHaveCount(1)
  await expect(page.getByTestId('tag-filter-chip')).toHaveText('工具 (1)')
  await expect(page.getByTestId('tag-clear-btn')).toBeDisabled()

  await page.getByTestId('tag-filter-chip').click()
  await expect(page.getByTestId('tag-filter-chip')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByTestId('tag-filter-btn')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await expect(card(page, 'Alpha')).toBeVisible()

  await page.getByTestId('tag-clear-btn').click()
  await expect(page.getByTestId('app-card')).toHaveCount(3)
  await expect(page.getByTestId('tag-clear-btn')).toBeDisabled()

  // The bar can be collapsed again without losing the tags.
  await page.getByTestId('tag-filter-btn').click()
  await expect(bar).toHaveCount(0)
  await expect(page.getByTestId('app-card')).toHaveCount(3)

  expectNoPageErrors()
})

test('typing a tag in the search box finds its app', async ({ page }) => {
  await addTags(page, 'Alpha', ['工具'])

  const search = page.locator('.ph-search').first()
  await search.fill('工具')
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await expect(card(page, 'Alpha')).toBeVisible()

  await search.fill('')
  await expect(page.getByTestId('app-card')).toHaveCount(3)

  expectNoPageErrors()
})

test('multiple selected tags are AND-combined', async ({ page }) => {
  await addTags(page, 'Alpha', ['工具', '系统'])
  await addTags(page, 'Beta', ['工具'])

  await page.getByTestId('tag-filter-btn').click()
  await expect(page.getByTestId('tag-filter-chip')).toHaveCount(2)

  const tools = page.getByTestId('tag-filter-chip').filter({ hasText: '工具' })
  const system = page.getByTestId('tag-filter-chip').filter({ hasText: '系统' })
  await expect(tools).toHaveText('工具 (2)')
  await expect(system).toHaveText('系统 (1)')

  await tools.click()
  await expect(page.getByTestId('app-card')).toHaveCount(2)
  await expect(card(page, 'Alpha')).toBeVisible()
  await expect(card(page, 'Beta')).toBeVisible()

  // Adding the second tag narrows the result to the app carrying both.
  await system.click()
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await expect(card(page, 'Alpha')).toBeVisible()
  await expect(card(page, 'Beta')).toHaveCount(0)

  // Third click turns the chip into an exclude filter: Alpha is hidden while
  // Beta (工具 only) stays.
  await system.click()
  await expect(system).toHaveAttribute('data-state', 'exclude')
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await expect(card(page, 'Beta')).toBeVisible()
  await expect(card(page, 'Alpha')).toHaveCount(0)

  // Fourth click clears only that chip; 工具 still includes Alpha + Beta.
  await system.click()
  await expect(system).toHaveAttribute('data-state', 'off')
  await expect(page.getByTestId('app-card')).toHaveCount(2)

  expectNoPageErrors()
})

test('exclude filter hides tagged apps and works together with include', async ({ page }) => {
  await addTags(page, 'Alpha', ['已玩'])
  await addTags(page, 'Beta', ['已玩'])
  await addTags(page, 'Gamma', ['未玩'])

  await page.getByTestId('tag-filter-btn').click()
  const played = page.getByTestId('tag-filter-chip').filter({ hasText: '已玩' })

  // include 已玩 → Alpha + Beta
  await played.click()
  await expect(played).toHaveAttribute('data-state', 'include')
  await expect(page.getByTestId('app-card')).toHaveCount(2)

  // exclude 已玩 → only Gamma
  await played.click()
  await expect(played).toHaveAttribute('data-state', 'exclude')
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await expect(card(page, 'Gamma')).toBeVisible()
  await expect(card(page, 'Alpha')).toHaveCount(0)

  await page.getByTestId('tag-clear-btn').click()
  await expect(page.getByTestId('app-card')).toHaveCount(3)
  expectNoPageErrors()
})

test('inside a category group the tag filter only offers that group tags', async ({ page }) => {
  await addTags(page, 'Alpha', ['已玩'])
  await addTags(page, 'Beta', ['未玩'])
  await addTags(page, 'Gamma', ['已玩'])

  // Mock categories appear only after Smart organize: Alpha=dev, Beta=media,
  // Gamma=game; applying switches grouping to smart automatically.
  await page.getByTestId('smart-organize').click()
  await page.getByTestId('smart-apply').click()
  await expect(page.getByTestId('smart-modal')).toHaveCount(0)
  await page.locator('[data-testid="group-nav-item"][data-group-key="dev"]').click()
  await page.getByTestId('tag-filter-btn').click()

  const chips = page.getByTestId('tag-filter-chip')
  await expect(chips).toHaveCount(1)
  await expect(chips.first()).toHaveText('已玩 (1)')
  await expect(page.getByTestId('tag-scope-hint')).toBeVisible()

  await chips.first().click()
  await expect(page.getByTestId('app-card')).toHaveCount(1)
  await expect(card(page, 'Alpha')).toBeVisible()

  // Switching to the media group updates the tag list to that group's tags.
  await page.locator('[data-testid="group-nav-item"][data-group-key="media"]').click()
  await expect(page.getByTestId('tag-filter-chip')).toHaveCount(1)
  await expect(page.getByTestId('tag-filter-chip').first()).toHaveText('未玩 (1)')

  expectNoPageErrors()
})
