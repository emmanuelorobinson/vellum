// The read-only specification must never be changed by a build, a test or an agent.
// These tests drive scripts/spec-checksums.mjs against throwaway copies of the repo's docs.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const script = join(repo, "scripts", "spec-checksums.mjs");
const run = (root, ...args) =>
  spawnSync("node", [script, ...args, "--root", root], { encoding: "utf8" });

let root;
before(() => {
  root = mkdtempSync(join(tmpdir(), "vellum-checksums-"));
});
after(() => rmSync(root, { recursive: true, force: true }));

const fresh = () => {
  rmSync(root, { recursive: true, force: true });
  mkdirSync(join(root, "docs", "design"), { recursive: true });
  mkdirSync(join(root, "scripts"), { recursive: true });
  cpSync(join(repo, "docs", "product"), join(root, "docs", "product"), { recursive: true });
  cpSync(join(repo, "docs", "design", "DESIGN.md"), join(root, "docs", "design", "DESIGN.md"));
  const update = run(root, "update");
  assert.equal(update.status, 0, update.stderr);
};

test("a pristine copy passes", () => {
  fresh();
  const out = run(root, "check");
  assert.equal(out.status, 0, out.stdout + out.stderr);
});

test("a modified product document fails and names the file", () => {
  fresh();
  writeFileSync(join(root, "docs", "product", "PRODUCT.md"), "tampered\n");
  const out = run(root, "check");
  assert.equal(out.status, 1);
  assert.match(out.stderr, /docs\/product\/PRODUCT\.md/);
  assert.match(out.stderr, /modified/i);
});

test("a deleted DESIGN.md fails", () => {
  fresh();
  rmSync(join(root, "docs", "design", "DESIGN.md"));
  const out = run(root, "check");
  assert.equal(out.status, 1);
  assert.match(out.stderr, /docs\/design\/DESIGN\.md/);
  assert.match(out.stderr, /missing/i);
});

test("a new, unrecorded file under docs/product fails", () => {
  fresh();
  writeFileSync(join(root, "docs", "product", "EXTRA.md"), "new\n");
  const out = run(root, "check");
  assert.equal(out.status, 1);
  assert.match(out.stderr, /docs\/product\/EXTRA\.md/);
});

test("vellum.pen not yet provided is reported as pending, and --strict refuses it", () => {
  fresh();
  const out = run(root, "check");
  assert.equal(out.status, 0);
  assert.match(out.stdout, /pending.*vellum\.pen|vellum\.pen.*pending/is);
  const strict = run(root, "check", "--strict");
  assert.equal(strict.status, 1);
  assert.match(strict.stderr, /vellum\.pen/);
});

test("vellum.pen added without recording it fails; recorded passes; then modified fails", () => {
  fresh();
  const pen = join(root, "docs", "design", "vellum.pen");
  writeFileSync(pen, "pretend design bytes");
  const unrecorded = run(root, "check");
  assert.equal(unrecorded.status, 1);
  assert.match(unrecorded.stderr, /vellum\.pen/);
  assert.match(unrecorded.stderr, /not recorded/i);

  assert.equal(run(root, "update").status, 0);
  assert.equal(run(root, "check", "--strict").status, 0);

  writeFileSync(pen, "pretend design bytes, edited");
  const edited = run(root, "check");
  assert.equal(edited.status, 1);
  assert.match(edited.stderr, /vellum\.pen/);
});

test("the real repository's specification is intact", () => {
  assert.ok(existsSync(join(repo, "scripts", "spec-checksums.json")), "manifest is committed");
  const out = spawnSync("node", [script, "check"], { cwd: repo, encoding: "utf8" });
  assert.equal(out.status, 0, out.stdout + out.stderr);
});
