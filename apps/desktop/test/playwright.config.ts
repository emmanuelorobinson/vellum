import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: ".",
  testMatch: "electron.spec.ts",
  workers: 1,
  timeout: 60_000,
  outputDir: "../../../target/desktop-test-results",
  reporter: "list",
});
