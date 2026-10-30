import { defineConfig } from '@playwright/test'

// Uses the locally installed Microsoft Edge (channel: msedge), so no browser
// download is needed. The frontend runs against the in-memory mock API.
export default defineConfig({
  testDir: './e2e',
  timeout: 30_000,
  fullyParallel: false,
  workers: 1,
  reporter: [['list']],
  use: {
    baseURL: 'http://localhost:1420',
    channel: 'msedge',
    headless: true,
    viewport: { width: 1100, height: 800 }
  },
  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:1420',
    reuseExistingServer: true,
    timeout: 60_000,
    env: { VITE_E2E: '1' }
  }
})
