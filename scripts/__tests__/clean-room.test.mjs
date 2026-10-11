// Clean-room checks (guide Prompt 2, Invariant 14). No file may contain a string from the human-maintained
// denylist, and a file that declares `ported-from:` needs a THIRD_PARTY_NOTICES entry for an MIT or
// Apache-2.0 source. Penpot (MPL-2.0) and tldraw (commercial) are never accepted as sources.
import assert from "node:assert/strict";
import { test } from "node:test";
import { bash, discard, put, scratch } from "./helpers.mjs";

const NOTICES_HEADER = [
  "| Project | Source repository | Path | Commit | Licence | Vellum file |",
  "|---|---|---|---|---|---|",
  "",
].join("\n");
const GOOSE = "3bd852002903e016ff30947e973f76e2fcfcf90f";
const goose = (licence) =>
  `| goose | block/goose | crates/goose/src/providers/claude_code.rs | ${GOOSE} | ${licence} | packages/editor-core/src/port.ts |\n`;
const header = (source, commit = GOOSE) =>
  `// ported-from: ${source} crates/goose/src/providers/claude_code.rs ${commit}\nexport const port = 1;\n`;

function project({ denylist = "", notices = NOTICES_HEADER, files = {} } = {}) {
  const root = scratch("clean-room");
  put(root, "scripts/clean-room-denylist.txt", denylist);
  put(root, "THIRD_PARTY_NOTICES.md", notices);
  for (const [file, text] of Object.entries(files)) put(root, file, text);
  return root;
}

function check(root) {
  return bash("check-clean-room.sh", ["--root", root]);
}

test("the real repository passes the clean-room checks", () => {
  const result = bash("check-clean-room.sh");
  assert.equal(result.status, 0, result.output);
});

test("a file containing a denylisted string fails and names the file", () => {
  const root = project({
    denylist: "# comments are ignored\nDistinctive phrase 17\n",
    files: { "packages/editor-core/src/copied.ts": "// Distinctive phrase 17 appears here\n" },
  });
  try {
    const result = check(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /copied\.ts/);
  } finally {
    discard(root);
  }
});

test("a ported file with no THIRD_PARTY_NOTICES entry fails and names the source", () => {
  const root = project({ files: { "packages/editor-core/src/port.ts": header("block/goose") } });
  try {
    const result = check(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /block\/goose/);
  } finally {
    discard(root);
  }
});

test("a ported file with an MIT or Apache-2.0 notice passes", () => {
  const root = project({
    notices: NOTICES_HEADER + goose("Apache-2.0"),
    files: { "packages/editor-core/src/port.ts": header("block/goose") },
  });
  try {
    const result = check(root);
    assert.equal(result.status, 0, result.output);
  } finally {
    discard(root);
  }
});

test("a notice whose licence is not MIT or Apache-2.0 does not clear a port", () => {
  const root = project({
    notices: NOTICES_HEADER + goose("MPL-2.0"),
    files: { "packages/editor-core/src/port.ts": header("block/goose") },
  });
  try {
    const result = check(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /block\/goose/);
  } finally {
    discard(root);
  }
});

test("porting from penpot is refused even with a notice", () => {
  const penpot = "a".repeat(40);
  const root = project({
    notices:
      NOTICES_HEADER +
      `| penpot | penpot/penpot | render-wasm/src/render.rs | ${penpot} | MIT | packages/editor-core/src/port.ts |\n`,
    files: {
      "packages/editor-core/src/port.ts": `// ported-from: penpot/penpot render-wasm/src/render.rs ${penpot}\n`,
    },
  });
  try {
    const result = check(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /penpot/);
  } finally {
    discard(root);
  }
});

test("a ported-from header without a full commit SHA is rejected", () => {
  const root = project({
    notices: NOTICES_HEADER + goose("Apache-2.0"),
    files: { "packages/editor-core/src/port.ts": header("block/goose", "main") },
  });
  try {
    const result = check(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /ported-from/);
  } finally {
    discard(root);
  }
});

test("prose that mentions the marker is not a header", () => {
  const root = project({
    files: { "docs/notes.md": "Ported files start with a `ported-from:` header line.\n" },
  });
  try {
    const result = check(root);
    assert.equal(result.status, 0, result.output);
  } finally {
    discard(root);
  }
});
