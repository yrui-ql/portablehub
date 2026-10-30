import { defineConfig } from 'vitest/config'

// Unit tests only. Playwright E2E specs live in e2e/ and must never be
// collected by Vitest, or `npm test` fails with "test.beforeEach() called here".
export default defineConfig({
  test: {
    include: ['tests/**/*.test.ts'],
    exclude: ['e2e/**', 'node_modules/**']
  }
})
