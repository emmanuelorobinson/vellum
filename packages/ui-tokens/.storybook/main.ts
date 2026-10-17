// Storybook for @vellum/ui-tokens (html renderer, Vite). Set up with `npm create storybook@latest -- --type html`
// (see reports.md, Prompt 43); the visual harness for every component comes with Prompt 45.
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import type { StorybookConfig } from "@storybook/html-vite";

/** The absolute path of a package, which a pnpm workspace needs to resolve addons. */
function getAbsolutePath(value: string) {
  return dirname(fileURLToPath(import.meta.resolve(`${value}/package.json`)));
}

const config: StorybookConfig = {
  stories: ["../src/**/*.mdx", "../src/**/*.stories.@(js|jsx|mjs|ts|tsx)"],
  addons: [
    getAbsolutePath("@chromatic-com/storybook"),
    getAbsolutePath("@storybook/addon-vitest"),
    getAbsolutePath("@storybook/addon-a11y"),
    getAbsolutePath("@storybook/addon-docs"),
  ],
  framework: getAbsolutePath("@storybook/html-vite"),
  // The UI faces (DESIGN.md "Typography"): JetBrains Mono from this package, Inter from the repo's one copy.
  staticDirs: [
    { from: "../fonts", to: "/fonts/jetbrains-mono" },
    { from: "../../../crates/adapters/text-parley/fonts", to: "/fonts/inter" },
  ],
  core: { disableTelemetry: true },
};

export default config;
