import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { at } from "./helpers.mjs";

const run = (...args) =>
  spawnSync(process.execPath, [at("scripts/doc-api-gen.mjs"), ...args], {
    cwd: at("."),
    encoding: "utf8",
  });
test("doc API declarations match the runtime registry", () => {
  const r = run("--check");
  assert.equal(r.status, 0, r.stdout + r.stderr);
});
test("doc API drift check rejects edited and missing declarations", () => {
  const dir = mkdtempSync(join(tmpdir(), "vellum-doc-api-"));
  const path = join(dir, "vellum-doc.d.ts");
  try {
    assert.equal(run("--check", "--target", path).status, 1);
    writeFileSync(
      path,
      `${readFileSync(at("packages/skills/src/generated/vellum-doc.d.ts"), "utf8")}\n// changed\n`,
    );
    assert.equal(run("--check", "--target", path).status, 1);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
