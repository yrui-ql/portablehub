import { expect, test, type Page } from '@playwright/test'

// BUG-0013 · the settings hotkey field records a key combination instead of
// requiring the user to type it, and recording temporarily suspends the global
// hotkey so the old combination cannot hide the window.
//
// The browser E2E suite runs against the in-memory mock, which records
// suspend/resume calls and can simulate a registration conflict.

interface MockWindow {
  __PH_MOCK__: {
    config: { hotkey: string }
    hotkeySuspended: boolean
    suspendCalls: number
    resumeCalls: number
    setHotkeyConflict: (accel: string | null) => void
  }
}

interface HotkeyState {
  hotkey: string
  suspended: boolean
  suspendCalls: number
  resumeCalls: number
}

async function hotkeyState(page: Page): Promise<HotkeyState> {
  return page.evaluate(() => {
    const mock = (window as unknown as MockWindow).__PH_MOCK__
    return {
      hotkey: mock.config.hotkey,
      suspended: mock.hotkeySuspended,
      suspendCalls: mock.suspendCalls,
      resumeCalls: mock.resumeCalls
    }
  })
}

function hotkeyBox(page: Page) {
  return page.getByTestId('hotkey-input')
}

function hotkeySection(page: Page) {
  return page.locator('#settings-section-hotkey')
}

async function openSettings(page: Page) {
  await page.getByTestId('settings-btn').click()
  await expect(page.locator('.ph-settings')).toBeVisible()
}

async function startRecording(page: Page) {
  await hotkeyBox(page).click()
  await expect(hotkeyBox(page)).toHaveAttribute('data-recording', 'true')
}

async function expectSuspended(page: Page, suspended: boolean) {
  await expect
    .poll(async () => (await hotkeyState(page)).suspended, {
      message: `global hotkey should be ${suspended ? 'suspended' : 'active'}`
    })
    .toBe(suspended)
}

async function saveHotkey(page: Page) {
  await hotkeySection(page).getByRole('button', { name: '保存' }).click()
}

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await expect(page.getByTestId('app-card')).toHaveCount(3)
  await openSettings(page)
})

test('AC-B13-1: pressing Ctrl+Alt+K records and saves the accelerator', async ({ page }) => {
  const pageErrors: string[] = []
  page.on('pageerror', (error) => pageErrors.push(String(error)))

  await expect(hotkeyBox(page)).toContainText('Ctrl+Alt+P')

  await startRecording(page)
  await expect(hotkeyBox(page)).toContainText('请按下新快捷键')
  await page.keyboard.press('Control+Alt+K')

  await expect(hotkeyBox(page)).toHaveAttribute('data-recording', 'false')
  await expect(hotkeyBox(page)).toContainText('Ctrl+Alt+K')

  await saveHotkey(page)
  await expect(page.locator('.ph-toast')).toContainText('快捷键已保存')
  expect((await hotkeyState(page)).hotkey).toBe('Ctrl+Alt+K')
  expect(pageErrors).toEqual([])
})

test('AC-B13-2: modifier-only and unmodified keys never write a value', async ({ page }) => {
  await startRecording(page)

  // A lone modifier keeps the recorder waiting, with no error.
  await page.keyboard.press('Control')
  await expect(hotkeyBox(page)).toHaveAttribute('data-recording', 'true')
  await expect(page.getByTestId('hotkey-error')).toHaveCount(0)

  // A regular key without modifiers asks for at least one modifier.
  await page.keyboard.press('K')
  await expect(page.getByTestId('hotkey-error')).toBeVisible()
  await expect(page.getByTestId('hotkey-error')).toContainText('需要至少一个修饰键')

  // Cancel the recording: the stored value was never touched.
  await page.keyboard.press('Escape')
  await expect(hotkeyBox(page)).toHaveAttribute('data-recording', 'false')
  await expect(hotkeyBox(page)).toContainText('Ctrl+Alt+P')
  expect((await hotkeyState(page)).hotkey).toBe('Ctrl+Alt+P')
})

test('AC-B13-3: Esc keeps the old value, Backspace clears the field', async ({ page }) => {
  // Esc cancels and restores/finishes without writing.
  await startRecording(page)
  await expectSuspended(page, true)
  await page.keyboard.press('Escape')
  await expect(hotkeyBox(page)).toHaveAttribute('data-recording', 'false')
  await expect(hotkeyBox(page)).toContainText('Ctrl+Alt+P')
  await expectSuspended(page, false)
  expect((await hotkeyState(page)).hotkey).toBe('Ctrl+Alt+P')

  // Backspace clears the recorded value.
  await startRecording(page)
  await page.keyboard.press('Backspace')
  await expect(hotkeyBox(page)).toHaveAttribute('data-recording', 'false')
  await expect(hotkeyBox(page)).toContainText('点击后按下新快捷键')
  await expectSuspended(page, false)

  await saveHotkey(page)
  await expect(page.locator('.ph-toast')).toContainText('快捷键已保存')
  expect((await hotkeyState(page)).hotkey).toBe('')
})

test('AC-B13-4: recording suspends the old global hotkey and keeps focus in the field', async ({
  page
}) => {
  const before = await hotkeyState(page)
  expect(before.suspended).toBe(false)

  await hotkeyBox(page).click()
  await expectSuspended(page, true)

  // The old combination is captured by the recorder instead of hiding the
  // window; focus must not jump to the main search box.
  await page.keyboard.press('Control+Alt+P')
  await expect(hotkeyBox(page)).toHaveAttribute('data-recording', 'false')
  await expectSuspended(page, false)

  const after = await hotkeyState(page)
  expect(after.suspendCalls).toBeGreaterThan(before.suspendCalls)
  expect(after.resumeCalls).toBeGreaterThan(before.resumeCalls)
  expect(await page.evaluate(() => document.activeElement?.getAttribute('data-testid'))).toBe(
    'hotkey-input'
  )
  await expect(page.locator('.ph-search').first()).not.toBeFocused()
  await expect(page.locator('.ph-settings')).toBeVisible()
  await expect(hotkeyBox(page)).toContainText('Ctrl+Alt+P')
})

test('AC-B13-5: a registration conflict keeps the previous value and reports an error', async ({
  page
}) => {
  await page.evaluate(() => {
    ;(window as unknown as MockWindow).__PH_MOCK__.setHotkeyConflict('Ctrl+Alt+K')
  })

  await startRecording(page)
  await page.keyboard.press('Control+Alt+K')
  await expect(hotkeyBox(page)).toHaveAttribute('data-recording', 'false')
  await expect(hotkeyBox(page)).toContainText('Ctrl+Alt+K')

  await saveHotkey(page)
  const toast = page.locator('.ph-toast')
  await expect(toast).toBeVisible()
  await expect(toast).toHaveClass(/error/)

  // The app restores the previously registered accelerator.
  await expect(hotkeyBox(page)).toContainText('Ctrl+Alt+P')
  expect((await hotkeyState(page)).hotkey).toBe('Ctrl+Alt+P')
})

test('AC-B13-6: recording and saving work in a plain browser without throwing', async ({ page }) => {
  const pageErrors: string[] = []
  page.on('pageerror', (error) => pageErrors.push(String(error)))

  await startRecording(page)
  await page.keyboard.press('Control+Shift+F9')
  await expect(hotkeyBox(page)).toHaveAttribute('data-recording', 'false')
  await expect(hotkeyBox(page)).toContainText('Ctrl+Shift+F9')

  await saveHotkey(page)
  await expect(page.locator('.ph-toast')).toContainText('快捷键已保存')
  expect((await hotkeyState(page)).hotkey).toBe('Ctrl+Shift+F9')
  expect((await hotkeyState(page)).suspended).toBe(false)
  expect(pageErrors).toEqual([])
})
