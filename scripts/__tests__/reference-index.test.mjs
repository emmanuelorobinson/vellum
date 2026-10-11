// The reference index (guide Prompt 2): docs/reference/REFERENCE-INDEX.md is generated from guide §3.2
// and §3.3, and every path it lists must exist in refs/.
import assert from "node:assert/strict";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { parseGuide } from "../reference-index.mjs";
import { at, discard, guideText, put, repo, run, scratch } from "./helpers.mjs";

// A guide with the grammar the real §3.2 and §3.3 use: brace lists, paths relative to the previous
// path, ';' between repositories, a segment that inherits its repository, and a parenthesised list.
const GUIDE = [
  "# 3. Reference Repositories",
  "",
  "## 3.2 Hard problem → where it is already solved",
  "",
  "| # | Vellum hard problem | Study | Used in |",
  "|---|---|---|---|",
  "| R1 | A renderer | penpot `render-wasm/docs/rendering_architecture.md`, `tile_rendering.md`, `src/render/{surfaces,fills}.rs`; graphite `frontend/wrapper/src/{editor_wrapper,wasm_value}.rs` | P4 |",
  "| R2 | Text | parley `parley/src/editing/{editor,cursor}.rs`, `fontique/src/fallback.rs`; penpot `docs/text_editor.md` | P5 |",
  "| R3 | Layout | taffy `src/compute/grid/`; **Chrome-generated fixtures**: `scripts/gentest/` (`main.rs`, `test_helper.js`), `test_fixtures/` | P6 |",
  "| R4 | Code mode | codex `codex-rs/code-mode/src/lib.rs`, `codex-rs/code-mode-runtime/src/cell_actor/` | P25 |",
  "| R5 | Notes | tldraw (docs only) editor managers concept | P45 |",
  "",
  "## 3.3 Other repositories and docs worth reading",
  "",
  "| Source | Read for |",
  "|---|---|",
  "| `rust-skia/rust-skia` (skia-safe) | Building with the `textlayout` feature |",
  "| Codex docs `docs/exec.md`, `docs/config.md` | `codex` engine fallback |",
  "",
  "# 4. Product Invariants",
  "",
].join("\n");

const MANIFEST = [
  "name\turl\tsha\tlicence\tuse",
  `penpot\thttps://github.com/penpot/penpot\t${"a".repeat(40)}\tMPL-2.0\tREAD ONLY, never port`,
  `graphite\thttps://github.com/GraphiteEditor/Graphite\t${"b".repeat(40)}\tMIT\t-`,
  `parley\thttps://github.com/linebender/parley\t${"c".repeat(40)}\tMIT\t-`,
  `taffy\thttps://github.com/DioxusLabs/taffy\t${"d".repeat(40)}\tMIT\t-`,
  `codex\thttps://github.com/openai/codex\t${"e".repeat(40)}\tApache-2.0\t-`,
  `tldraw\thttps://github.com/tldraw/tldraw\t${"f".repeat(40)}\ttldraw license\tDOCS ONLY`,
].join("\n");

// The files the fixture guide points at, one per resolved path (directories are implied by them).
const REFS = [
  "penpot/render-wasm/docs/rendering_architecture.md",
  "penpot/render-wasm/docs/tile_rendering.md",
  "penpot/render-wasm/src/render/surfaces.rs",
  "penpot/render-wasm/src/render/fills.rs",
  "penpot/docs/text_editor.md",
  "graphite/frontend/wrapper/src/editor_wrapper.rs",
  "graphite/frontend/wrapper/src/wasm_value.rs",
  "parley/parley/src/editing/editor.rs",
  "parley/parley/src/editing/cursor.rs",
  "parley/fontique/src/fallback.rs",
  "taffy/src/compute/grid/mod.rs",
  "taffy/scripts/gentest/main.rs",
  "taffy/scripts/gentest/test_helper.js",
  "taffy/test_fixtures/case.json",
  "codex/codex-rs/code-mode/src/lib.rs",
  "codex/codex-rs/code-mode-runtime/src/cell_actor/mod.rs",
  "codex/docs/exec.md",
  "codex/docs/config.md",
  "tldraw/README.md",
];

function project(guide = GUIDE) {
  const root = scratch("reference-index");
  put(root, "docs/implementation/vellum-implementation-guide.md", guide);
  put(root, "scripts/refs.tsv", `${MANIFEST}\n`);
  for (const file of REFS) put(root, join("refs", file), "fixture\n");
  return root;
}

const index = (root) => join(root, "docs", "reference", "REFERENCE-INDEX.md");
const generate = (root) => run("node", [at("scripts", "reference-index.mjs"), "--root", root]);
const check = (root) =>
  run("node", [at("scripts", "reference-index.mjs"), "--check", "--root", root]);

test("the parser reads every §3.2 and §3.3 row of the real guide", () => {
  const parsed = parseGuide(guideText());
  assert.deepEqual(
    parsed.problems.map((p) => p.id),
    Array.from({ length: 22 }, (_, i) => `R${i + 1}`),
  );
  assert.equal(parsed.sources.length, 11);
});

test("the index lists each resolved path, including paths relative to the previous one", () => {
  const root = project();
  try {
    const result = generate(root);
    assert.equal(result.status, 0, result.output);
    const text = readFileSync(index(root), "utf8");
    // Relative to the previous path's directory (tile_rendering.md, the second brace item).
    assert.match(text, /refs\/penpot\/render-wasm\/docs\/tile_rendering\.md/);
    assert.match(text, /refs\/penpot\/render-wasm\/src\/render\/fills\.rs/);
    // fontique sits at the parley repository root: found by walking up from parley/src/editing/.
    assert.match(text, /refs\/parley\/fontique\/src\/fallback\.rs/);
    // Directories end with a slash, and a segment with no leading name inherits its repository.
    assert.match(text, /refs\/taffy\/test_fixtures\//);
    assert.match(text, /refs\/taffy\/scripts\/gentest\/test_helper\.js/);
    assert.match(text, /refs\/codex\/docs\/config\.md/);
    // A row that names a repository with no file path says so.
    assert.match(text, /tldraw[^\n]*concept only/);
  } finally {
    discard(root);
  }
});

test("--check passes straight after generation", () => {
  const root = project();
  try {
    assert.equal(generate(root).status, 0);
    const result = check(root);
    assert.equal(result.status, 0, result.output);
  } finally {
    discard(root);
  }
});

test("--check fails and names the path when a listed file is missing", () => {
  const root = project();
  try {
    assert.equal(generate(root).status, 0);
    rmSync(join(root, "refs", "codex", "docs", "config.md"));
    const result = check(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /docs\/config\.md/);
  } finally {
    discard(root);
  }
});

test("--check fails when the committed index is out of date", () => {
  const root = project();
  try {
    assert.equal(generate(root).status, 0);
    writeFileSync(index(root), `${readFileSync(index(root), "utf8")}\nhand edit\n`);
    const result = check(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /out of date/);
  } finally {
    discard(root);
  }
});

test("a §3.2 row naming a repository that is not pinned fails and says which one", () => {
  const root = project(GUIDE.replace("tldraw (docs only)", "nosuchrepo `src/x.rs`"));
  try {
    const result = generate(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /nosuchrepo/);
    assert.equal(existsSync(index(root)), false, "a failed generation writes nothing");
  } finally {
    discard(root);
  }
});

test("docs/reference/REFERENCE-INDEX.md is current and every path exists in refs/", {
  skip: existsSync(at("refs")) ? false : "needs refs/ (run pnpm refs first)",
}, () => {
  const result = run("node", [at("scripts", "reference-index.mjs"), "--check"], { cwd: repo });
  assert.equal(result.status, 0, result.output);
});
