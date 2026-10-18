import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const root = fileURLToPath(new URL("./app", import.meta.url));
export default defineConfig({
  root,
  server: {
    port: 5174,
    strictPort: true,
    fs: { allow: [fileURLToPath(new URL("../../..", import.meta.url))] },
  },
  preview: { port: 5174, strictPort: true },
});
