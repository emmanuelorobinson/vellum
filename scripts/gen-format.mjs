#!/usr/bin/env node
// Generates docs/format from crates/vellum-types (guide Prompt 8):
//   vellum.schema.json  the JSON Schema of the .vellum document (schemars, via the format-schema example)
//   README.md           the reference page, rendered from that schema (scripts/lib/format-spec.mjs)
//   files.md            the on-disk page: assets, fonts, libraries, the directory format, examples written by
//                       the real codecs (crates/vellum/examples/format-files.rs) and the migration notes
//
//   node scripts/gen-format.mjs                       regenerate docs/format
//   node scripts/gen-format.mjs --check [--target d]  fail if <d> (default: docs/format) differs
//
// All three files are GENERATED: never hand-edit them. AGENTS.md lists docs/format as read-only except by this script.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { renderFilesSpec, renderFormatSpec } from "./lib/format-spec.mjs";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const check = args.includes("--check");
const target = args.includes("--target")
  ? args[args.indexOf("--target") + 1]
  : join(repo, "docs", "format");

const run = spawnSync(
  "cargo",
  ["run", "--quiet", "-p", "vellum-types", "--example", "format-schema"],
  { cwd: repo, encoding: "utf8" },
);
if (run.status !== 0) {
  console.error(run.stdout, run.stderr);
  console.error("format-spec failed: cargo run --example format-schema did not pass");
  process.exit(1);
}

const filesRun = spawnSync(
  process.execPath,
  // This example uses only codecs at runtime, but its dev-dependency graph still includes Skia.
  // Use the shared source wrapper so offline reuse and the native build policy stay consistent.
  [
    join(repo, "scripts/skia-build.mjs"),
    "run",
    "--quiet",
    "--locked",
    "--no-default-features",
    "-p",
    "vellum",
    "--example",
    "format-files",
  ],
  {
    cwd: repo,
    encoding: "utf8",
  },
);
if (filesRun.status !== 0) {
  console.error(filesRun.stdout, filesRun.stderr);
  console.error("format-spec failed: cargo run --example format-files did not pass");
  process.exit(1);
}

const schemaText = run.stdout;
const expected = new Map([
  ["vellum.schema.json", schemaText],
  ["README.md", renderFormatSpec(JSON.parse(schemaText))],
  ["files.md", renderFilesSpec(JSON.parse(filesRun.stdout))],
]);

if (check) {
  const problems = [];
  for (const [file, content] of expected) {
    const path = join(target, file);
    if (!existsSync(path)) problems.push(`missing: ${file}`);
    else if (readFileSync(path, "utf8") !== content)
      problems.push(`differs from generated output: ${file}`);
  }
  if (problems.length) {
    for (const problem of problems) console.error(problem);
    console.error("docs/format has drifted from vellum-types. Run: pnpm format-spec:gen");
    process.exit(1);
  }
  console.log(`docs/format matches vellum-types (${expected.size} files)`);
} else {
  mkdirSync(target, { recursive: true });
  for (const [file, content] of expected) writeFileSync(join(target, file), content);
  console.log(`wrote ${expected.size} files to ${join("docs", "format")}`);
}
