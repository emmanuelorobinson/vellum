#!/usr/bin/env node
// The Rust registry is authoritative. --check never changes the generated artifact.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const target = args.includes("--target")
  ? args[args.indexOf("--target") + 1]
  : join(repo, "packages/skills/src/generated/vellum-doc.d.ts");
if (!target) throw new Error("--target needs a file path");
const run = spawnSync(
  "cargo",
  [
    "run",
    "--quiet",
    "--locked",
    "--offline",
    "-p",
    "vellum-core",
    "--example",
    "doc-api-declarations",
  ],
  { cwd: repo, encoding: "utf8" },
);
if (run.status !== 0) {
  process.stderr.write(run.stdout + run.stderr);
  process.exit(run.status ?? 1);
}
if (args.includes("--check")) {
  if (!existsSync(target) || readFileSync(target, "utf8") !== run.stdout) {
    console.error("vellum-doc.d.ts differs from the runtime registry. Run pnpm doc-api:gen");
    process.exit(1);
  }
  console.log("vellum-doc.d.ts matches the runtime registry");
} else {
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, run.stdout);
  console.log("Generated vellum-doc.d.ts");
}
