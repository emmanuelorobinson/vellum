#!/usr/bin/env node
// Copy lint (guide Appendix B; Prompt 2 skeleton). Three rules:
//   1. @vellum/config's `copy` object holds exactly the strings of guide Appendix B, verbatim, under keys
//      derived from their labels (keyFor), and nothing else.
//   2. No copy string breaks the banned-copy rules: no exclamation mark, no emoji.
//   3. No package or app repeats a copy string. UI code imports `copy` from @vellum/config instead.
//
// Usage: node scripts/copy-lint.mjs [--root <dir>]
//   --root  repository root; defaults to this repository (the tests pass fixture directories)
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const CONFIG = join("packages", "config", "src", "index.ts");
const GUIDE = join("docs", "implementation", "vellum-implementation-guide.md");
const CODE = /\.(ts|tsx|js|jsx|mjs|cjs)$/;
const SKIP_DIRS = new Set(["node_modules", ".next", "dist", ".turbo", "generated"]);

// "Sync scope (A05)" -> "syncScopeA05". The key is derived from the label, so both sides agree on it.
export function keyFor(label) {
  const words = label.split(/[^A-Za-z0-9]+/).filter(Boolean);
  return words
    .map((word, i) => (i === 0 ? word[0].toLowerCase() : word[0].toUpperCase()) + word.slice(1))
    .join("");
}

// The `label: value` lines of the first text block under "# Appendix B".
export function appendixB(markdown) {
  const lines = markdown.split("\n");
  const heading = lines.findIndex((line) => /^# Appendix B\b/.test(line));
  if (heading < 0) throw new Error("the guide has no Appendix B");
  const open = lines.findIndex((line, i) => i > heading && line.trim() === "```text");
  const close = lines.findIndex((line, i) => i > open && line.trim() === "```");
  if (open < 0 || close < 0) throw new Error("Appendix B has no text block");
  const entries = [];
  for (const line of lines.slice(open + 1, close)) {
    const match = line.match(/^(.+?):\s+(.+)$/);
    if (match) entries.push({ label: match[1].trim(), value: match[2].trim() });
  }
  return entries;
}

// The `key: "value",` lines of `export const copy = { ... } as const;` in @vellum/config.
export function copyObject(source) {
  const start = source.indexOf("export const copy = {");
  if (start < 0)
    throw new Error("packages/config/src/index.ts has no `export const copy = {` object");
  const end = source.indexOf("} as const;", start);
  if (end < 0) throw new Error("the copy object in packages/config/src/index.ts is not closed");
  const entries = [];
  for (const line of source.slice(start, end).split("\n").slice(1)) {
    if (line.trim() === "") continue;
    const match = line.match(/^\s+(\w+): "([^"]*)",\s*$/);
    if (!match) throw new Error(`unexpected line in the copy object: ${line.trim()}`);
    entries.push({ key: match[1], value: match[2] });
  }
  return entries;
}

// Every code file under the packages and apps that may show copy: packages/*/{src,tests} and
// apps/*/{app,src,tests}. packages/config is the one place the strings may live.
function uiFiles(root) {
  const files = [];
  const walk = (dir) => {
    for (const name of readdirSync(dir)) {
      if (SKIP_DIRS.has(name)) continue;
      const path = join(dir, name);
      if (statSync(path).isDirectory()) walk(path);
      else if (CODE.test(name)) files.push(path);
    }
  };
  for (const [group, subdirs] of [
    ["packages", ["src", "tests"]],
    ["apps", ["app", "src", "tests"]],
  ]) {
    const base = join(root, group);
    if (!existsSync(base)) continue;
    for (const pkg of readdirSync(base)) {
      if (group === "packages" && pkg === "config") continue;
      for (const sub of subdirs) {
        const dir = join(base, pkg, sub);
        if (existsSync(dir) && statSync(dir).isDirectory()) walk(dir);
      }
    }
  }
  return files;
}

export function lint({ root, guide, config }) {
  const errors = [];
  const expected = appendixB(guide);
  const actual = copyObject(config);
  const actualByKey = new Map(actual.map((entry) => [entry.key, entry.value]));
  const expectedKeys = new Set();

  for (const { label, value } of expected) {
    const key = keyFor(label);
    if (expectedKeys.has(key)) errors.push(`two Appendix B labels map to copy.${key}`);
    expectedKeys.add(key);
    if (!actualByKey.has(key)) {
      errors.push(`missing: "${label}" (copy.${key}) is not exported by @vellum/config`);
    } else if (actualByKey.get(key) !== value) {
      errors.push(
        `drift: "${label}" is "${actualByKey.get(key)}" in @vellum/config, but Appendix B says "${value}"`,
      );
    }
  }
  for (const { key } of actual) {
    if (!expectedKeys.has(key))
      errors.push(`not in Appendix B: copy.${key} is exported by @vellum/config`);
  }
  for (const { key, value } of actual) {
    if (value.includes("!")) errors.push(`exclamation mark in copy.${key}: "${value}"`);
    if (/\p{Extended_Pictographic}/u.test(value)) errors.push(`emoji in copy.${key}: "${value}"`);
  }

  const repeated = actual.filter((entry) => entry.value.trim() !== "");
  for (const file of uiFiles(root)) {
    const text = readFileSync(file, "utf8");
    for (const { key, value } of repeated) {
      if (text.includes(value)) {
        errors.push(
          `${relative(root, file)} repeats copy.${key}; import it from @vellum/config instead`,
        );
      }
    }
  }
  return { errors, count: expected.length };
}

function main(argv) {
  let root = DEFAULT_ROOT;
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--root") root = resolve(argv[++i] ?? "");
    else throw new Error(`unknown argument: ${argv[i]}`);
  }
  const { errors, count } = lint({
    root,
    guide: readFileSync(join(root, GUIDE), "utf8"),
    config: readFileSync(join(root, CONFIG), "utf8"),
  });
  if (errors.length > 0) {
    for (const error of errors) console.error(`copy-lint: ${error}`);
    console.error(`copy-lint: FAILED with ${errors.length} problem(s)`);
    return 1;
  }
  console.log(
    `copy-lint: ${count} Appendix B strings match @vellum/config; no UI code repeats them`,
  );
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    console.error(`copy-lint: ${error.message}`);
    process.exitCode = 1;
  }
}
