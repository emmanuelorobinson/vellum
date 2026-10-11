#!/usr/bin/env node
// Guards the read-only specification: docs/product/**, docs/design/DESIGN.md, docs/design/vellum.pen.
//
//   node scripts/spec-checksums.mjs check  [--strict] [--root <dir>]
//   node scripts/spec-checksums.mjs update [--root <dir>]     (a human, after changing the spec on purpose)
//
// The manifest (scripts/spec-checksums.json) records one SHA-256 per file. `vellum.pen` is recorded as
// null until the design file is added to the repo; `check` reports that as pending, `--strict` refuses it.
import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const mode = args[0];
const flag = (name) => args.includes(name);
const option = (name) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
const root = option("--root") ?? join(dirname(fileURLToPath(import.meta.url)), "..");

const MANIFEST = "scripts/spec-checksums.json";
const PRODUCT_DIR = "docs/product";
const REQUIRED = ["docs/design/DESIGN.md"];
const OPTIONAL = ["docs/design/vellum.pen"]; // human-supplied; null in the manifest until it exists

const sha256 = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");
const abs = (rel) => join(root, ...rel.split("/"));

const walk = (dir) => {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter((e) => e.name !== ".DS_Store")
    .flatMap((e) => (e.isDirectory() ? walk(join(dir, e.name)) : [join(dir, e.name)]))
    .sort();
};
const productFiles = () =>
  walk(abs(PRODUCT_DIR)).map((p) => relative(root, p).split(sep).join("/"));

const fail = (lines) => {
  for (const l of lines) console.error(l);
  process.exit(1);
};

if (mode === "update") {
  const missing = REQUIRED.filter((f) => !existsSync(abs(f)));
  if (missing.length)
    fail(missing.map((f) => `missing: ${f} (cannot record a specification that is not there)`));
  const files = {};
  for (const f of [...productFiles(), ...REQUIRED]) files[f] = sha256(abs(f));
  for (const f of OPTIONAL) files[f] = existsSync(abs(f)) ? sha256(abs(f)) : null;
  const sorted = Object.fromEntries(Object.entries(files).sort(([a], [b]) => a.localeCompare(b)));
  writeFileSync(abs(MANIFEST), `${JSON.stringify({ version: 1, files: sorted }, null, 2)}\n`);
  console.log(`recorded ${Object.keys(sorted).length} entries in ${MANIFEST}`);
} else if (mode === "check") {
  if (!existsSync(abs(MANIFEST)))
    fail([`missing: ${MANIFEST} (run: pnpm checksum:update, after human review)`]);
  const { files } = JSON.parse(readFileSync(abs(MANIFEST), "utf8"));
  const errors = [];
  const pending = [];

  for (const [rel, recorded] of Object.entries(files)) {
    const present = existsSync(abs(rel)) && statSync(abs(rel)).isFile();
    if (recorded === null) {
      if (present)
        errors.push(
          `not recorded: ${rel} exists but the manifest has no checksum for it (a human runs: pnpm checksum:update)`,
        );
      else pending.push(rel);
    } else if (!present) {
      errors.push(`missing: ${rel}`);
    } else if (sha256(abs(rel)) !== recorded) {
      errors.push(
        `modified: ${rel} no longer matches its recorded checksum. The specification is read-only.`,
      );
    }
  }
  for (const rel of productFiles()) {
    if (!(rel in files))
      errors.push(
        `not recorded: ${rel} is new and has no checksum (a human runs: pnpm checksum:update)`,
      );
  }
  if (flag("--strict"))
    for (const rel of pending) errors.push(`pending: ${rel} has not been provided yet`);

  if (errors.length) fail(errors);
  for (const rel of pending)
    console.log(`pending: ${rel} has not been added to the repo yet (checked again once it is)`);
  console.log(
    `spec intact: ${Object.values(files).filter(Boolean).length} files match their checksums`,
  );
} else {
  console.error("usage: spec-checksums.mjs check [--strict] | update  [--root <dir>]");
  process.exit(2);
}
