// Guide Prompt 3: "a checksum test proves no test or script modifies docs/design/vellum.pen".
// Runs the design scripts that touch the design against the real repository, then compares the file
// with the recorded checksum and with its own hash from before.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { at, bash, run } from "./helpers.mjs";

const penPath = at("docs", "design", "vellum.pen");
const sha = () => createHash("sha256").update(readFileSync(penPath)).digest("hex");

test("design-open, design-check and the checksum script leave vellum.pen as recorded", () => {
  const recorded = JSON.parse(readFileSync(at("scripts", "spec-checksums.json"), "utf8")).files[
    "docs/design/vellum.pen"
  ];
  assert.ok(recorded, "docs/design/vellum.pen has no recorded checksum");
  const before = sha();
  assert.equal(before, recorded, "vellum.pen does not match scripts/spec-checksums.json");

  assert.equal(bash("design-open", ["--no-open"]).status, 0);
  bash("design-check"); // may fail while the inventory is stale; it must still never write
  const sums = run("node", [at("scripts", "spec-checksums.mjs"), "check", "--strict"]);
  assert.equal(sums.status, 0, sums.output);

  assert.equal(sha(), before);
  assert.equal(sha(), recorded);
});

test("the read-only copy is byte-identical to the original and has no write bit", () => {
  assert.equal(bash("design-open", ["--no-open"]).status, 0);
  const copy = at(".design-cache", "vellum.readonly.pen");
  assert.ok(existsSync(copy));
  assert.equal(createHash("sha256").update(readFileSync(copy)).digest("hex"), sha());
});
