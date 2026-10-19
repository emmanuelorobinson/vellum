import { copyFile, mkdir, readdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const root = fileURLToPath(new URL(".", import.meta.url));
const output = resolve(root, "dist");
const bundles = [];
const arg = (name) => {
  const at = process.argv.indexOf(name);
  return at < 0 ? undefined : process.argv[at + 1];
};
const target = arg("--target");
const daemon = arg("--daemon");
await rm(output, { recursive: true, force: true });
for (const [entry, file, platform] of [
  ["src/main/main.ts", "main/index.cjs", "node"],
  ["src/preload/index.ts", "preload/index.cjs", "node"],
  ["src/renderer/main.tsx", "renderer/editor.js", "browser"],
]) {
  bundles.push(
    await build({
      metafile: true,
      mainFields: ["module", "main"],
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
    }),
  );
}
await writeFile(
  resolve(output, "renderer/index.html"),
  '<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><link rel="stylesheet" href="editor.css"><title>Desktop</title></head><body><div id="root"></div><script src="editor.js"></script></body></html>',
);
if (process.argv.includes("--with-daemon") || daemon) {
  await mkdir(resolve(output, "bin"), { recursive: true });
  const name = (target ?? process.platform).startsWith("win32") ? "vellum.exe" : "vellum";
  const temporary = resolve(output, "bin", `${name}.new`);
  await copyFile(daemon ? resolve(daemon) : resolve(root, "../../target/debug", name), temporary);
  await rename(temporary, resolve(output, "bin", name));
}

for (const file of ["LICENSE", "THIRD_PARTY_NOTICES.md"])
  await copyFile(resolve(root, "../..", file), resolve(output, file));
const notices = new Map();
for (const bundle of bundles)
  for (const input of Object.keys(bundle.metafile.inputs)) {
    if (!input.includes("node_modules/")) continue;
    let folder = dirname(resolve(root, input));
    while (folder !== dirname(folder)) {
      let pkg;
      try {
        pkg = JSON.parse(await readFile(resolve(folder, "package.json"), "utf8"));
      } catch {
        folder = dirname(folder);
        continue;
      }
      const id = `${pkg.name}@${pkg.version}`;
      if (!notices.has(id)) {
        const licences = (await readdir(folder)).filter((name) =>
          /^(licen[sc]e|copying)(\.|$)/i.test(name),
        );
        if (!licences.length) {
          if (id !== "lazy-val@1.0.5" || pkg.license !== "MIT")
            throw Error(`Missing bundled dependency licence: ${id}`);
          notices.set(id, await readFile(resolve(root, "licenses/lazy-val.txt"), "utf8"));
          break;
        }
        notices.set(
          id,
          (await Promise.all(licences.map((name) => readFile(resolve(folder, name), "utf8")))).join(
            "\n",
          ),
        );
      }
      break;
    }
  }
await writeFile(
  resolve(output, "THIRD_PARTY_LICENSES.txt"),
  [...notices]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([id, license]) => `${id}\n${license}`)
    .join("\n\n"),
);
