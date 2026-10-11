import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(dirname(fileURLToPath(import.meta.url)), "..", "clean.mjs");

test("clean removes build output and nothing that matters", () => {
  const root = mkdtempSync(join(tmpdir(), "vellum-clean-"));
  try {
    const keep = [
      ".design-cache/vellum.readonly.pen",
      "refs/penpot/README.md",
      "node_modules/x/index.js",
      "docs/product/PRODUCT.md",
      "packages/a/src/index.ts",
    ];
    const drop = [
      "target/debug/x",
      "packages/a/dist/index.js",
      "apps/b/.next/cache/x",
      ".turbo/log",
    ];
    for (const f of [...keep, ...drop]) {
      mkdirSync(dirname(join(root, f)), { recursive: true });
      writeFileSync(join(root, f), "x");
    }
    const dry = spawnSync("node", [script, "--dry-run", "--root", root], { encoding: "utf8" });
    assert.equal(dry.status, 0);
    assert.ok(
      drop.every((f) => existsSync(join(root, f))),
      "a dry run must remove nothing",
    );

    const real = spawnSync("node", [script, "--root", root], { encoding: "utf8" });
    assert.equal(real.status, 0, real.stderr);
    for (const f of drop) assert.ok(!existsSync(join(root, f)), `${f} should be gone`);
    for (const f of keep) assert.ok(existsSync(join(root, f)), `${f} must survive a clean`);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
