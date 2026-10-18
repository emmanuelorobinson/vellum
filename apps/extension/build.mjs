import { copyFile, mkdir, readFile, rename, unlink, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const root = fileURLToPath(new URL(".", import.meta.url));
const output = resolve(root, "dist/package");
await build({
  absWorkingDir: root,
  entryPoints: ["src/extension.ts"],
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node22",
  external: ["vscode"],
  outfile: resolve(output, "dist/extension.cjs"),
});
await build({
  absWorkingDir: root,
  entryPoints: ["src/webview/main.tsx"],
  bundle: true,
  platform: "browser",
  format: "iife",
  target: "chrome140",
  outfile: resolve(output, "dist/webview/editor.js"),
  jsx: "automatic",
  loader: { ".woff2": "file" },
  assetNames: "assets/[name]-[hash]",
  define: { "process.env.NODE_ENV": '"production"' },
});
await copyFile(
  resolve(root, "../../fixtures/documents/shapes.vellum"),
  resolve(output, "dist/welcome.vellum"),
);
if (process.argv.includes("--with-daemon")) {
  await mkdir(resolve(output, "bin"), { recursive: true });
  const binary = process.platform === "win32" ? "vellum.exe" : "vellum";
  const temporary = resolve(output, "bin", `${binary}.new`);
  await unlink(temporary).catch((error) => {
    if (error.code !== "ENOENT") throw error;
  });
  await copyFile(resolve(root, "../../target/debug", binary), temporary);
  // Replace the inode: overwriting an executable in place leaves stale macOS code-signing cache entries.
  await rename(temporary, resolve(output, "bin", binary));
}

const manifest = JSON.parse(await readFile(resolve(root, "package.json"), "utf8"));
manifest.name = manifest.displayName.toLowerCase();
manifest.type = "commonjs";
for (const key of ["private", "scripts", "dependencies", "devDependencies"]) delete manifest[key];
await writeFile(resolve(output, "package.json"), `${JSON.stringify(manifest, null, 2)}\n`);
