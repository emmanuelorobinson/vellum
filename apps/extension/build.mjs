import {
  copyFile,
  mkdir,
  readdir,
  readFile,
  rename,
  rm,
  unlink,
  writeFile,
} from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";
import { targetInfo, validateBinary } from "./packaging.mjs";

const root = fileURLToPath(new URL(".", import.meta.url));
const output = resolve(root, "dist/package");
await rm(output, { recursive: true, force: true });
const bundles = [];
bundles.push(
  await build({
    absWorkingDir: root,
    entryPoints: ["src/extension.ts"],
    bundle: true,
    platform: "node",
    format: "cjs",
    target: "node22",
    external: ["vscode"],
    mainFields: ["module", "main"],
    metafile: true,
    outfile: resolve(output, "dist/extension.cjs"),
  }),
);
bundles.push(
  await build({
    absWorkingDir: root,
    entryPoints: ["src/webview/connect.tsx"],
    bundle: true,
    platform: "browser",
    format: "iife",
    target: "chrome140",
    outfile: resolve(output, "dist/webview/connect.js"),
    jsx: "automatic",
    loader: { ".woff2": "file" },
    assetNames: "assets/[name]-[hash]",
    define: { "process.env.NODE_ENV": '"production"' },
    metafile: true,
  }),
);
bundles.push(
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
    metafile: true,
  }),
);
await copyFile(
  resolve(root, "../../fixtures/documents/shapes.vellum"),
  resolve(output, "dist/welcome.vellum"),
);
const arg = (name) => {
  const at = process.argv.indexOf(name);
  return at < 0 ? undefined : process.argv[at + 1];
};
const target = arg("--target");
const daemon = arg("--daemon");
if (target && !daemon) throw Error("A targeted build requires --daemon <release-binary>");
if (process.argv.includes("--with-daemon") || daemon) {
  await mkdir(resolve(output, "bin"), { recursive: true });
  const binary = target
    ? targetInfo(target).binary
    : process.platform === "win32"
      ? "vellum.exe"
      : "vellum";
  const source = daemon ? resolve(daemon) : resolve(root, "../../target/debug", binary);
  if (target) validateBinary(await readFile(source), target);
  const temporary = resolve(output, "bin", `${binary}.new`);
  await unlink(temporary).catch((error) => {
    if (error.code !== "ENOENT") throw error;
  });
  await copyFile(source, temporary);
  // Replace the inode: overwriting an executable in place leaves stale macOS code-signing cache entries.
  await rename(temporary, resolve(output, "bin", binary));
}
await mkdir(resolve(output, "media"), { recursive: true });
// Reuse our bundled Lucide frame glyph; this is an Activity Bar action, not a new brand logo.
const icons = await readFile(
  resolve(root, "../../packages/editor-react/src/icons/lucide.generated.ts"),
  "utf8",
);
const path = JSON.parse(icons.match(/"frame": (".*")/)[1]);
await writeFile(
  resolve(output, "media/canvas.svg"),
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="${path}" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>`,
);
for (const [source, destination] of [
  ["README.md", "README.md"],
  ["../../THIRD_PARTY_NOTICES.md", "THIRD_PARTY_NOTICES.md"],
])
  await copyFile(resolve(root, source), resolve(output, destination));
await writeFile(resolve(output, ".vscodeignore"), "**/*.map\n");
// Bundle licences as well as the repository's asset/port notices. Tool-only dependencies never ship.
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
        if (!licences.length) throw Error(`Missing bundled dependency licence: ${id}`);
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

const manifest = JSON.parse(await readFile(resolve(root, "package.json"), "utf8"));
manifest.name = manifest.displayName.toLowerCase();
manifest.type = "commonjs";
for (const key of ["private", "scripts", "dependencies", "devDependencies"]) delete manifest[key];
await writeFile(resolve(output, "package.json"), `${JSON.stringify(manifest, null, 2)}\n`);
