import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import type { StorybookConfig } from "@storybook/react-vite";

const packagePath = (name: string) =>
  dirname(fileURLToPath(import.meta.resolve(`${name}/package.json`)));
const config: StorybookConfig = {
  framework: packagePath("@storybook/react-vite"),
  stories: ["../src/**/*.stories.tsx", "../fixtures/**/*.stories.tsx"],
  addons: [packagePath("@storybook/addon-vitest")],
  // Stories supply their args. Dependencies remain intact because the React preset imports docgen
  // even on this path; C25 records the owner's exact transitive prerelease exception.
  typescript: { reactDocgen: false },
  staticDirs: [
    { from: "../../ui-tokens/fonts", to: "/fonts/jetbrains-mono" },
    { from: "../../../crates/adapters/text-parley/fonts", to: "/fonts/inter" },
  ],
  core: { disableTelemetry: true },
};
export default config;
