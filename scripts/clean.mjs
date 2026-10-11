#!/usr/bin/env node
// Removes build output. Never touches node_modules, refs/, .design-cache/ or anything under docs/.
//   node scripts/clean.mjs [--dry-run] [--root <dir>]
import { existsSync, readdirSync, rmSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const dry = args.includes("--dry-run");
const root = args.includes("--root")
  ? args[args.indexOf("--root") + 1]
  : join(dirname(fileURLToPath(import.meta.url)), "..");

const OUTPUT = ["target", ".turbo", "dist", ".next", "coverage"];
const places = [root];
for (const group of ["packages", "apps"]) {
  const dir = join(root, group);
  if (existsSync(dir))
    for (const e of readdirSync(dir, { withFileTypes: true }))
      if (e.isDirectory()) places.push(join(dir, e.name));
}

let removed = 0;
for (const place of places) {
  for (const name of OUTPUT) {
    const path = join(place, name);
    if (!existsSync(path)) continue;
    console.log(`${dry ? "would remove" : "removed"} ${relative(root, path)}`);
    if (!dry) rmSync(path, { recursive: true, force: true });
    removed++;
  }
}
console.log(
  `${dry ? "would remove" : "removed"} ${removed} build output ${removed === 1 ? "directory" : "directories"}`,
);
