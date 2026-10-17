// The token usage lint (scripts/lib/token-usage.mjs) and the token-lint entry point (guide Prompt 43).
import assert from "node:assert/strict";
import { test } from "node:test";
import { checkUsage, isLinted } from "../lib/token-usage.mjs";
import { at, run } from "./helpers.mjs";

const HUES = ["teal", "green", "saffron"];
const lint = (path, text) => checkUsage({ files: [{ path, text }], hues: HUES }).errors;
const rules = (path, text) => lint(path, text).map((e) => e.rule);

test("colour literals fail in UI CSS and in UI script strings", () => {
  assert.deepEqual(rules("packages/editor-react/src/panel.css", ".panel { background: #fff; }"), [
    "color-literal",
  ]);
  assert.deepEqual(
    rules("packages/editor-react/src/panel.css", ".panel { color: rgba(0, 0, 0, 0.5); }"),
    ["color-literal"],
  );
  assert.deepEqual(rules("apps/desktop/src/Home.tsx", 'const style = { color: "#17181B" };'), [
    "color-literal",
  ]);
  // biome-ignore lint/suspicious/noTemplateCurlyInString: the string is source code under test
  assert.deepEqual(rules("apps/web/src/a.ts", "const c = `hsl(${h} 50% 50%)`;"), ["color-literal"]);
  const [error] = lint("packages/editor-react/src/x.css", "a {}\n.b { fill: #2F6BFF; }");
  assert.equal(error.line, 2);
});

test("token references, comments, ids and private fields are not colour literals", () => {
  assert.deepEqual(
    rules("packages/editor-react/src/panel.css", ".panel { background: var(--chrome-panel); }"),
    [],
  );
  assert.deepEqual(rules("packages/editor-react/src/panel.css", "/* was #fff */ .panel {}"), []);
  assert.deepEqual(
    rules(
      "packages/editor-react/src/a.ts",
      "// matches #2F6BFF in the design\nclass A { #add = 1; }",
    ),
    [],
  );
  assert.deepEqual(rules("packages/editor-react/src/a.ts", 'document.querySelector("#root");'), []);
});

test("only UI packages are linted, and not their tests or generated files", () => {
  assert.equal(isLinted("packages/editor-react/src/a.tsx"), true);
  assert.equal(isLinted("packages/editor-react/src/a.test.tsx"), false);
  assert.equal(isLinted("packages/ui-tokens/src/generated/tokens.css"), false);
  assert.equal(isLinted("packages/editor-react/node_modules/x/a.css"), false);
  assert.deepEqual(rules("packages/renderer-canvas2d/src/a.ts", 'const c = "#fff";'), []);
});

test("selection blue and component violet never colour a button", () => {
  assert.deepEqual(
    rules(
      "packages/editor-react/src/a.css",
      ".primary-button { background: var(--canvas-selection); }",
    ),
    ["mark-on-button"],
  );
  assert.deepEqual(
    rules(
      "packages/editor-react/src/Toolbar.tsx",
      '<Button style={{ background: cssVar("canvas/component") }}>Ask</Button>',
    ),
    ["mark-on-button"],
  );
  assert.deepEqual(
    rules("packages/editor-react/src/IconButton.tsx", 'const ring = cssVar("canvas/selection");'),
    ["mark-on-button"],
  );
  assert.deepEqual(
    rules(
      "packages/editor-react/src/a.css",
      ".selection-outline { border-color: var(--canvas-selection); }",
    ),
    [],
  );
  assert.deepEqual(
    rules(
      "packages/editor-react/src/Overlay.tsx",
      '<path stroke={cssVar("canvas/selection")} />\n<button>OK</button>',
    ),
    [],
  );
});

test("agent hues appear only on markers, cursors, leases, placeholders and session dots", () => {
  for (const ok of [
    ".agent-marker { background: var(--agent-teal); }",
    ".lease-tab { border-color: var(--agent-green); }",
    ".session-dot[data-hue] { background: var(--agent-saffron); }",
    ".placeholder { outline-color: var(--agent-teal); }",
  ])
    assert.deepEqual(rules("packages/editor-react/src/a.css", ok), [], ok);
  assert.deepEqual(
    // biome-ignore lint/suspicious/noTemplateCurlyInString: the string is source code under test
    rules("packages/editor-react/src/AgentCursor.tsx", "const fill = cssVar(`agent/${hue}`);"),
    [],
  );
  assert.deepEqual(
    rules("packages/editor-react/src/a.css", ".send-button { background: var(--agent-teal); }"),
    ["agent-hue"],
  );
  assert.deepEqual(
    rules(
      "packages/editor-react/src/Composer.tsx",
      '<div style={{ color: cssVar("agent/green") }} />',
    ),
    ["agent-hue"],
  );
  assert.deepEqual(
    // biome-ignore lint/suspicious/noTemplateCurlyInString: the string is source code under test
    rules("packages/editor-react/src/Composer.tsx", "const c = cssVar(`agent/${hue}`);"),
    ["agent-hue"],
  );
});

test("packages/ui-tokens may show every token", () => {
  assert.deepEqual(
    rules(
      "packages/ui-tokens/src/token-page.ts",
      'const a = cssVar("agent/teal"); const b = "<button>" + cssVar("canvas/selection");',
    ),
    [],
  );
});

test("token-lint passes on the repository", () => {
  const result = run("node", [at("scripts/token-lint.mjs")]);
  assert.equal(result.status, 0, result.output);
  assert.match(result.stdout, /contrast checks/);
});

// The FINAL check "Storybook token page vs Tokens snapshots". Needs Chrome and the local snapshot PNG
// (gitignored, made by pnpm design:tokens), so it runs only in the chrome + design lanes.
const lanes = (process.env.VELLUM_LANES ?? "").split(",").map((s) => s.trim());
test("chrome + design lanes: the token page matches the design's Tokens sheet within the decided tolerance", {
  skip:
    lanes.includes("chrome") && lanes.includes("design")
      ? false
      : "chrome and design lanes not enabled (set VELLUM_LANES=chrome,design)",
}, () => {
  const result = run("node", [at("scripts/token-page-compare.mjs")]);
  assert.equal(result.status, 0, result.output);
  assert.match(result.stdout, /token page vs Tokens sheet/);
});
