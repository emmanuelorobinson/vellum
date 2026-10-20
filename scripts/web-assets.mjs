/** Publish only the built shared WASM renderer and licensed fonts, never repo files or design assets. */
import { copyFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";

const app = process.argv[2];
if (!["web", "account"].includes(app)) throw Error("Expected web or account");
const root = resolve(import.meta.dirname, "..");
for (const dir of ["engine", "fonts"])
  await mkdir(resolve(root, `apps/${app}/public/${dir}`), { recursive: true });
for (const file of ["vellum-editor.js", "vellum_editor.wasm"])
  await copyFile(
    resolve(root, `packages/renderer-skia/dist/${file}`),
    resolve(root, `apps/${app}/public/engine/${file}`),
  );
for (const file of ["InterVariable.ttf", "JetBrainsMono-wght.ttf", "Inter-OFL.txt", "OFL.txt"])
  await copyFile(
    resolve(root, `apps/account/public/fonts/${file}`),
    resolve(root, `apps/${app}/public/fonts/${file}`),
  );
