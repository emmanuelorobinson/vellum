// Browser tests are opt-in: pnpm test:storybook needs Playwright's Chromium (chrome lane).
import { fileURLToPath } from "node:url";
import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { playwright } from "@vitest/browser-playwright";
import { defineConfig } from "vitest/config";
export default defineConfig({
  test: {
    projects: [
      {
        extends: true,
        test: {
          name: "unit",
          include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
          environment: "node",
        },
      },
      {
        extends: true,
        // Avoid a late optimizer reload when the browser imports the new resource dialogs.
        optimizeDeps: { include: ["@base-ui/react/dialog"] },
        plugins: [
          storybookTest({ configDir: fileURLToPath(new URL(".storybook", import.meta.url)) }),
        ],
        test: {
          name: "storybook",
          browser: {
            enabled: true,
            headless: true,
            provider: playwright({}),
            instances: [{ browser: "chromium" }],
          },
        },
      },
    ],
  },
});
