import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright E2E configuration for TTL-Legacy dashboard vault management.
 *
 * See https://playwright.dev/docs/test-configuration for all options.
 */
export default defineConfig({
  // Directory where test files live.
  testDir: "./tests",

  // Retry failed tests twice in CI so transient flakiness doesn't block PRs.
  retries: process.env.CI ? 2 : 0,

  // Run tests in parallel by default; reduce to 1 if tests share state.
  workers: process.env.CI ? 1 : undefined,

  // Reporter: use GitHub-Actions-friendly reporting in CI, list otherwise.
  reporter: process.env.CI ? "github" : "list",

  use: {
    // Base URL so tests can use relative paths like `await page.goto('/')`.
    baseURL: "http://localhost:3000",

    // Retain a trace on failure to aid debugging.
    trace: "on-first-retry",
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
