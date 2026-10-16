import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { checkCoverage, parityRows } from "../lib/parity.mjs";
import { at } from "./helpers.mjs";

const architecture = `### 7.4 pen.dev parity check
| Source | Vellum | Release |
|---|---|---|
| One | first | v1 |
| Two | second | v1 / v1.1 |
| Three | third | v1.1 |
| Four | fourth | v1 (extra v1.1) |
| Five | fifth | later |
### 7.5 Next
| Elsewhere | unrelated | v1 |
`;
const source =
  '#[test]\nfn first_test() {}\n#[test]\n#[ignore = "pending: needs Prompt 39"]\nfn pending_test() { panic!("not implemented yet"); }';
const target = (name) => ({ file: "tests/parity.rs", name });
const manifest = [
  { capability: "One", tests: [target("first_test")] },
  {
    capability: "Two",
    tests: [target("pending_test")],
    pending: { prompt: 39, reason: "Needs orchestration" },
  },
  { capability: "Four", tests: [target("first_test")] },
];
const check = (entries = manifest, read = () => source, doc = architecture) =>
  checkCoverage(doc, entries, read);

test("parity includes mixed v1 rows and excludes v1.1-only rows and other sections", () => {
  assert.deepEqual(
    parityRows(architecture).map((r) => r.capability),
    ["One", "Two", "Four"],
  );
  const result = check();
  assert.equal(result.total, 3);
  assert.equal(result.covered, 2);
  assert.equal(result.pending.length, 1);
});

test("parity fails for a new unmapped row, empty mapping, stale mapping or duplicate", () => {
  assert.throws(() => check(manifest.slice(1)), /One.*no named test/);
  assert.throws(
    () => check([{ capability: "One", tests: [] }, ...manifest.slice(1)]),
    /no named test/,
  );
  assert.throws(
    () => check([...manifest, { capability: "Gone", tests: [target("first_test")] }]),
    /Gone.*not.*v1 row/,
  );
  assert.throws(() => check([...manifest, manifest[0]]), /duplicate/i);
  assert.throws(() => check([], () => source, "no parity table"), /7\.4/);
});

test("parity requires real named tests and distinguishes pending from coverage", () => {
  assert.throws(() => check(undefined, () => "fn first_test() {}"), /not a.*test/);
  assert.throws(
    () =>
      check(undefined, () => {
        throw new Error("missing file");
      }),
    /missing file/,
  );
  const unlabelled = manifest.map(({ pending: _pending, ...entry }) => entry);
  assert.throws(() => check(unlabelled), /ignored.*pending/);
  assert.throws(
    () => check(manifest, () => source.replace('#[ignore = "pending: needs Prompt 39"]', "")),
    /pending.*ignored/,
  );
  assert.throws(
    () =>
      check([
        { ...manifest[0], tests: [{ file: "../escape.rs", name: "first_test" }] },
        ...manifest.slice(1),
      ]),
    /path/,
  );
});

test("every real architecture v1 row maps to a named present or explicit pending test", () => {
  const result = checkCoverage(
    readFileSync(at("docs/architecture/ARCHITECTURE.md"), "utf8"),
    JSON.parse(readFileSync(at("fixtures/parity/coverage.json"), "utf8")),
    (file) => readFileSync(at(file), "utf8"),
  );
  assert.ok(result.total >= 40);
  assert.ok(result.covered > 0);
  assert.ok(result.pending.length > 0, "later prompts stay visible");
});
