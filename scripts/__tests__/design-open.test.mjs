// scripts/design-open copies the read-only specification to .design-cache/ and opens that copy.
// The tests use a throwaway repository root and a fake opener, so no pen.dev app is needed.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { chmodSync, existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { bash, discard, put, scratch } from "./helpers.mjs";

const sha = (file) => createHash("sha256").update(readFileSync(file)).digest("hex");

function fixtureRoot() {
  const root = scratch("design-open");
  put(root, "docs/design/vellum.pen", "FAKE-PEN-BYTES");
  put(root, "opener.sh", `#!/usr/bin/env bash\necho "$@" > "${root}/opened.txt"\n`);
  return root;
}

const openWith = (root, extra = []) =>
  bash("design-open", ["--root", root, ...extra], {
    env: { ...process.env, VELLUM_DESIGN_OPENER: `bash ${join(root, "opener.sh")}` },
  });

test("design-open copies the design to .design-cache and opens the copy", () => {
  const root = fixtureRoot();
  try {
    const result = openWith(root);
    assert.equal(result.status, 0, result.output);
    const copy = join(root, ".design-cache", "vellum.readonly.pen");
    assert.equal(readFileSync(copy, "utf8"), "FAKE-PEN-BYTES");
    assert.ok(result.stdout.includes(copy), `stdout must print the copy path: ${result.stdout}`);
    assert.equal(readFileSync(join(root, "opened.txt"), "utf8").trim(), copy);
  } finally {
    discard(root);
  }
});

test("design-open leaves the original untouched and makes the copy read-only", () => {
  const root = fixtureRoot();
  try {
    const original = join(root, "docs", "design", "vellum.pen");
    const before = sha(original);
    const result = openWith(root);
    assert.equal(result.status, 0, result.output);
    assert.equal(sha(original), before);
    const mode = statSync(join(root, ".design-cache", "vellum.readonly.pen")).mode & 0o222;
    assert.equal(mode, 0, "the copy must have no write bit");
  } finally {
    discard(root);
  }
});

test("design-open replaces a stale or edited copy on every run", () => {
  const root = fixtureRoot();
  try {
    assert.equal(openWith(root).status, 0);
    const copy = join(root, ".design-cache", "vellum.readonly.pen");
    // A session that edited the copy (or an old copy of an older design) must not survive.
    chmodSync(copy, 0o644);
    writeFileSync(copy, "EDITED");
    put(root, "docs/design/vellum.pen", "NEWER-PEN-BYTES");
    const result = openWith(root);
    assert.equal(result.status, 0, result.output);
    assert.equal(readFileSync(copy, "utf8"), "NEWER-PEN-BYTES");
  } finally {
    discard(root);
  }
});

test("design-open copies docs/design/assets next to the copy when it exists", () => {
  const root = fixtureRoot();
  try {
    put(root, "docs/design/assets/logo.svg", "<svg/>");
    const result = openWith(root);
    assert.equal(result.status, 0, result.output);
    assert.equal(readFileSync(join(root, ".design-cache", "assets", "logo.svg"), "utf8"), "<svg/>");
  } finally {
    discard(root);
  }
});

test("design-open --no-open copies and prints the path without launching anything", () => {
  const root = fixtureRoot();
  try {
    const result = openWith(root, ["--no-open"]);
    assert.equal(result.status, 0, result.output);
    assert.ok(existsSync(join(root, ".design-cache", "vellum.readonly.pen")));
    assert.equal(existsSync(join(root, "opened.txt")), false);
  } finally {
    discard(root);
  }
});

test("design-open fails with a clear message when the design file is missing", () => {
  const root = scratch("design-open-missing");
  try {
    const result = openWith(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /docs\/design\/vellum\.pen/);
    assert.equal(existsSync(join(root, ".design-cache", "vellum.readonly.pen")), false);
  } finally {
    discard(root);
  }
});

test("design-open tells the person to confirm the document with get_app_state", () => {
  const root = fixtureRoot();
  try {
    const result = openWith(root);
    assert.match(result.stdout, /get_app_state/);
  } finally {
    discard(root);
  }
});
