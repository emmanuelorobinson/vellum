// packages/types-ts is generated from vellum-types by ts-rs and must never drift or be hand edited.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  appendFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const script = join(repo, "scripts", "types-gen.mjs");
const committed = join(repo, "packages", "types-ts", "src");

const check = (target) =>
  spawnSync("node", [script, "--check", "--target", target], { cwd: repo, encoding: "utf8" });

test("committed bindings match what vellum-types generates", () => {
  const out = check(committed);
  assert.equal(out.status, 0, out.stdout + out.stderr);
});

test("a hand-edited generated file is detected", () => {
  const tmp = mkdtempSync(join(tmpdir(), "vellum-types-"));
  try {
    cpSync(committed, tmp, { recursive: true });
    appendFileSync(join(tmp, "generated", "Document.ts"), "\n// hand edit\n");
    const out = check(tmp);
    assert.equal(out.status, 1);
    assert.match(out.stderr, /Document\.ts/);
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
});

// ts-rs writes the types it borrows from other crates into subdirectories (serde_json/JsonValue.ts for
// a node's free-form metadata). The generated files import them, so they must be generated too.
test("types borrowed from other crates are generated, not just the crate's own", () => {
  assert.ok(
    existsSync(join(committed, "generated", "serde_json", "JsonValue.ts")),
    "generated/serde_json/JsonValue.ts is missing",
  );
  const index = readFileSync(join(committed, "index.ts"), "utf8");
  assert.match(index, /serde_json\/JsonValue/);
});

test("a missing nested generated file is detected", () => {
  const tmp = mkdtempSync(join(tmpdir(), "vellum-types-"));
  try {
    cpSync(committed, tmp, { recursive: true });
    rmSync(join(tmp, "generated", "serde_json"), { recursive: true, force: true });
    const out = check(tmp);
    assert.equal(out.status, 1);
    assert.match(out.stderr, /JsonValue\.ts/);
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
});

test("a stale nested generated file is detected", () => {
  const tmp = mkdtempSync(join(tmpdir(), "vellum-types-"));
  try {
    cpSync(committed, tmp, { recursive: true });
    mkdirSync(join(tmp, "generated", "old_crate"), { recursive: true });
    writeFileSync(join(tmp, "generated", "old_crate", "Gone.ts"), "export type Gone = string;\n");
    const out = check(tmp);
    assert.equal(out.status, 1);
    assert.match(out.stderr, /stale: generated\/old_crate\/Gone\.ts/);
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
});
