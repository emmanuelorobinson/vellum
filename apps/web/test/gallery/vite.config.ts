import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  publicDir: fileURLToPath(new URL("../../../account/public", import.meta.url)),
  server: {
    host: "127.0.0.1",
    port: 3113,
    strictPort: true,
    fs: { allow: [fileURLToPath(new URL("../../../..", import.meta.url))] },
  },
});
