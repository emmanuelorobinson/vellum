import type { Preview } from "@storybook/react-vite";
import "../../ui-tokens/.storybook/fonts.css";
import "@vellum/ui-tokens/tokens.css";

const preview: Preview = { parameters: { layout: "fullscreen" } };
export default preview;
