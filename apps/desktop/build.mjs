import { copyFile, mkdir, rename, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const root = fileURLToPath(new URL(".", import.meta.url));
const output = resolve(root, "dist");
await rm(output, { recursive: true, force: true });
for (const [entry, file, platform] of [
  ["src/main/main.ts", "main/index.cjs", "node"],
  ["src/preload/index.ts", "preload/index.cjs", "node"],
  ["src/renderer/main.tsx", "renderer/editor.js", "browser"],
]) {
  await build({
    absWorkingDir: root,
    entryPoints: [entry],
    outfile: resolve(output, file),
    bundle: true,
    platform,
    format: platform === "node" ? "cjs" : "iife",
    external: platform === "node" ? ["electron"] : [],
    jsx: "automatic",
    target: "es2023",
    loader: { ".woff2": "file", ".ttf": "file" },
    assetNames: "assets/[name]-[hash]",
    define: { "process.env.NODE_ENV": '"production"' },
  });
}
await writeFile(
  resolve(output, "renderer/index.html"),
  '<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><link rel="stylesheet" href="editor.css"><title>Desktop</title></head><body><div id="root"></div><script src="editor.js"></script></body></html>',
);
if (process.argv.includes("--with-daemon")) {
  await mkdir(resolve(output, "bin"), { recursive: true });
  const name = process.platform === "win32" ? "vellum.exe" : "vellum";
  const temporary = resolve(output, "bin", `${name}.new`);
  await copyFile(resolve(root, "../../target/debug", name), temporary);
  await rename(temporary, resolve(output, "bin", name));
}
