// Browser tests for the editor chrome. Chrome lane: needs Playwright's Chromium (pnpm exec playwright install chromium).
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  fullyParallel: true,
  reporter: [["list"]],
  use: {
    baseURL: "http://127.0.0.1:5174",
    ...devices["Desktop Chrome"],
    viewport: { width: 1280, height: 800 },
    reducedMotion: "reduce",
  },
  webServer: {
    command: "pnpm exec vite --config vite.config.ts --host 127.0.0.1",
    cwd: ".",
    url: "http://127.0.0.1:5174",
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
