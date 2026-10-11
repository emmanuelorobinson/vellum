// Copy lint (guide Appendix B, Prompt 2 skeleton). @vellum/config exports the exact-copy strings of
// Appendix B, nothing else, and UI code never repeats one of them: it imports them instead.
import assert from "node:assert/strict";
import { test } from "node:test";
import { at, discard, put, repo, run, scratch } from "./helpers.mjs";

const GUIDE = [
  "# Appendix B — Exact-Copy Assertions (CI-enforced)",
  "",
  "From DESIGN.md. Copy-lint fails if any string drifts.",
  "",
  "```text",
  "Tagline:                 Design where your code lives.",
  "First open heading:      What are we designing?",
  "Sync scope (A05):        Copies of files you choose to share, never your whole repo",
  "Saved note:              Saved",
  "```",
  "",
  "# Appendix C — Testing Strategy Summary",
  "",
].join("\n");

// The strings Appendix B lists above, keyed the way copy-lint derives them from each label.
const ENTRIES = [
  ["tagline", "Design where your code lives."],
  ["firstOpenHeading", "What are we designing?"],
  ["syncScopeA05", "Copies of files you choose to share, never your whole repo"],
  ["savedNote", "Saved"],
];

const configIndex = (entries = ENTRIES) =>
  [
    'export const PACKAGE_NAME = "@vellum/config";',
    "",
    "export const copy = {",
    ...entries.map(([key, value]) => `  ${key}: "${value}",`),
    "} as const;",
    "",
    'export const brand = { name: "Vellum", tagline: copy.tagline } as const;',
    "",
  ].join("\n");

function project({ guide = GUIDE, index = configIndex(), files = {} } = {}) {
  const root = scratch("copy-lint");
  put(root, "docs/implementation/vellum-implementation-guide.md", guide);
  put(root, "packages/config/src/index.ts", index);
  for (const [file, text] of Object.entries(files)) put(root, file, text);
  return root;
}

const lint = (root) => run("node", [at("scripts", "copy-lint.mjs"), "--root", root]);

test("the real repository's @vellum/config matches Appendix B and no UI code repeats it", () => {
  const result = run("node", [at("scripts", "copy-lint.mjs")], { cwd: repo });
  assert.equal(result.status, 0, result.output);
});

test("a fixture that matches Appendix B passes", () => {
  const root = project();
  try {
    const result = lint(root);
    assert.equal(result.status, 0, result.output);
  } finally {
    discard(root);
  }
});

test("a drifted string fails and names its Appendix B label", () => {
  const root = project({
    index: configIndex(ENTRIES.map(([k, v]) => [k, k === "firstOpenHeading" ? "What now?" : v])),
  });
  try {
    const result = lint(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /First open heading/);
  } finally {
    discard(root);
  }
});

test("a missing Appendix B string fails", () => {
  const root = project({ index: configIndex(ENTRIES.filter(([k]) => k !== "savedNote")) });
  try {
    const result = lint(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /Saved note/);
  } finally {
    discard(root);
  }
});

test("a string that is not in Appendix B fails", () => {
  const root = project({ index: configIndex([...ENTRIES, ["extraCopy", "Something else"]]) });
  try {
    const result = lint(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /extraCopy/);
  } finally {
    discard(root);
  }
});

test("UI code that repeats an Appendix B string fails and names the file", () => {
  const root = project({
    files: {
      "packages/editor-react/src/home.tsx": 'export const h = "What are we designing?";\n',
    },
  });
  try {
    const result = lint(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /home\.tsx/);
  } finally {
    discard(root);
  }
});

test("an app test that repeats the tagline fails; importing it from config passes", () => {
  const repeated = project({
    files: {
      "apps/web/tests/page.test.tsx": 'expect(html).toContain("Design where your code lives.");\n',
    },
  });
  try {
    assert.notEqual(lint(repeated).status, 0);
  } finally {
    discard(repeated);
  }

  const imported = project({
    files: { "apps/web/tests/page.test.tsx": "expect(html).toContain(brand.tagline);\n" },
  });
  try {
    assert.equal(lint(imported).status, 0);
  } finally {
    discard(imported);
  }
});

test("exclamation marks and emoji are refused in copy", () => {
  const root = project({
    index: configIndex(ENTRIES.map(([k, v]) => [k, k === "savedNote" ? "Saved!" : v])),
  });
  try {
    const result = lint(root);
    assert.notEqual(result.status, 0);
    assert.match(result.output, /exclamation/);
  } finally {
    discard(root);
  }
});
