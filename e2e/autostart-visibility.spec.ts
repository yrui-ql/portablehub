import { expect, test } from '@playwright/test'

// REQ-0031: "launch at login" silently started the app hidden in the tray, so
// users could believe it never started. The hidden-at-login behaviour is now an
// explicit setting.

test('startup section exposes "stay in tray at login" and persists the choice', async ({
  page
}) => {
  await page.goto('/')
  await page.getByTestId('settings-btn').click()

  const section = page.getByTestId('settings-section-startup')
  await expect(section).toBeVisible()
  const hidden = page.getByTestId('start-hidden')
  await expect(hidden).toBeVisible()
  await expect(hidden).not.toBeChecked()
  await expect(section).toContainText('开机启动时隐藏到托盘')

  await hidden.check()
  const saved = await page.evaluate(
    () => (window as any).__PH_MOCK__.config.startHiddenAtLogin
  )
  expect(saved).toBe(true)

  await hidden.uncheck()
  const savedAgain = await page.evaluate(
    () => (window as any).__PH_MOCK__.config.startHiddenAtLogin
  )
  expect(savedAgain).toBe(false)
})

test('autostart items can be started without showing their windows', async ({ page }) => {
  await page.goto('/')
  await page.getByTestId('settings-btn').click()

  const section = page.getByTestId('settings-section-startup')
  const quiet = page.getByTestId('quiet-autostart')
  await expect(quiet).toBeVisible()
  await expect(quiet).toBeChecked()
  await expect(section).toContainText('自启项静默启动')

  await quiet.uncheck()
  expect(
    await page.evaluate(() => (window as any).__PH_MOCK__.config.quietAutostart)
  ).toBe(false)

  await quiet.check()
  expect(
    await page.evaluate(() => (window as any).__PH_MOCK__.config.quietAutostart)
  ).toBe(true)
})
