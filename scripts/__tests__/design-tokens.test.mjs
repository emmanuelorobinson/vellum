// scripts/design-tokens and scripts/lib/design-tokens.mjs (guide Prompt 43), against a fake `pen` CLI in a
// throwaway repository root. The real CLI needs `pen login` or PEN_CLI_KEY; these tests need neither.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  buildTokens,
  checkContrast,
  checkGaps,
  contrast,
  generateCss,
  generateTs,
} from "../lib/design-tokens.mjs";
import { parseTokensRaw, TOKENS_QUERY } from "../lib/pen-queries.mjs";
import { bash, discard, put, scratch } from "./helpers.mjs";

const PEN_BYTES = "FAKE-PEN-BYTES";
const sha = (text) => createHash("sha256").update(text).digest("hex");
const themed = (light, dark, hcLight, hcDark) => ({
  type: "color",
  value: [
    { value: light, theme: { appearance: "light" } },
    { value: dark, theme: { appearance: "dark" } },
    { value: hcLight, theme: { appearance: "hc-light" } },
    { value: hcDark, theme: { appearance: "hc-dark" } },
  ],
});

const VARIABLES = {
  "vellum-ink": { type: "color", value: "#17181B" },
  "chrome/panel": themed("#FFFFFF", "#222327", "#ffffff", "#000000"),
  "chrome/text": themed("#17181B", "#ECEDEF", "#000000", "#FFFFFF"),
  "agent/teal": themed("#0F8F86", "#3CC4B4", "#00675F", "#76E9DE"),
  "radius/xs": { type: "number", value: 4 },
  "motion/panel": { type: "number", value: 120 },
  "font/ui": { type: "string", value: "Inter" },
};
const ORDER = [
  "vellum-ink",
  "chrome/panel",
  "chrome/text",
  "agent/teal",
  "radius/xs",
  "motion/panel",
  "font/ui",
];

function rawOutput(variables = VARIABLES) {
  return [
    "shell chrome that must be ignored",
    "@@vellum-tokens-begin",
    '@T {"appearance":["light","dark","hc-light","hc-dark"]}',
    ...Object.keys(variables)
      .sort()
      .map((k) => `@V ${JSON.stringify([k, variables[k]])}`),
    '@P ["icon","agent/teal","$chrome/panel",3]',
    '@P ["text","chrome/text","$chrome/panel",9]',
    '@P ["text","chrome/text","#FFFFFF",1]',
    "@@vellum-tokens-end",
  ].join("\n");
}

const DESIGN_MD = `# Design

## Visual system

### Color

| Role | Light | Dark | Use |
|---|---|---|---|
| Panel | #FFFFFF | #222327 | Floating panels |
| Primary text / action | #17181B | #ECEDEF | Labels |

**Agent colours.** Each agent gets a hue.

| Hue | Light | Dark |
|---|---|---|
| Teal | #0F8F86 | #3CC4B4 |

### Typography

Use **Inter** for UI.

| Role | Size / line height | Weight | Use |
|---|---:|---:|---|

### Spacing and shape

| Token | Value | Use |
|---|---|---|
| radius-xs | 4px | Value fields |

Panels and popovers fade in over 120ms.

## Next section
`;

const MAP = {
  colors: { Panel: ["chrome/panel"], "Primary text / action": ["chrome/text"] },
  type: {},
  prose: [
    { token: "font/ui", quote: "Use **Inter** for UI", value: "Inter" },
    { token: "motion/panel", quote: "Panels and popovers fade in over 120ms", value: 120 },
  ],
};
const GAPS = "# Design gaps\n\n- 2026-10-11 · legacy · x · check=variable-name:vellum-ink\n";

const tokensFor = (variables = VARIABLES, order = ORDER) =>
  buildTokens({
    raw: parseTokensRaw(rawOutput(variables)),
    order,
    sourceSha256: "abc",
    exporter: "test",
  });

test("the token query is read-only and reads variables and colour pairs", () => {
  for (const write of [
    "Insert(",
    "Update(",
    "Delete(",
    "Replace(",
    "Move(",
    "Copy(",
    "SetVariables(",
    "Generate(",
  ])
    assert.equal(TOKENS_QUERY.includes(write), false, write);
  assert.match(TOKENS_QUERY, /GetVariables\(\)/);
  assert.match(TOKENS_QUERY, /resolveInstances:true/);
});

test("buildTokens keeps the design file's order, every appearance and the used pairs", () => {
  const tokens = tokensFor();
  assert.equal(tokens.axis, "appearance");
  assert.deepEqual(tokens.modes, ["light", "dark", "hc-light", "hc-dark"]);
  assert.deepEqual(
    tokens.tokens.map((t) => t.name),
    ORDER,
  );
  const panel = tokens.tokens.find((t) => t.name === "chrome/panel");
  assert.deepEqual(panel, {
    name: "chrome/panel",
    group: "chrome",
    type: "color",
    values: { light: "#FFFFFF", dark: "#222327", "hc-light": "#FFFFFF", "hc-dark": "#000000" },
  });
  assert.deepEqual(tokens.tokens[0], {
    name: "vellum-ink",
    group: null,
    type: "color",
    value: "#17181B",
  });
  assert.deepEqual(tokens.pairs[0], {
    kind: "icon",
    foreground: "agent/teal",
    background: "chrome/panel",
    count: 3,
  });
  assert.equal(tokens.pairs.find((p) => p.background === "#FFFFFF").count, 1);
});

test("buildTokens refuses variables the inventory does not list, and values missing an appearance", () => {
  assert.throws(() => tokensFor(VARIABLES, ORDER.slice(1)), /inventory/);
  const partial = {
    ...VARIABLES,
    "chrome/panel": {
      type: "color",
      value: themed("#FFFFFF", "#222327", "#FFFFFF", "#000000").value.slice(0, 3),
    },
  };
  assert.throws(() => tokensFor(partial), /hc-dark/);
});

test("generateCss writes scalars once and chrome and canvas colours under their own appearance attribute", () => {
  const css = generateCss(tokensFor());
  assert.match(css, /^\/\* GENERATED by scripts\/design-tokens/);
  assert.match(
    css,
    /:root \{\n {2}--radius-xs: 4px;\n {2}--motion-panel: 120ms;\n {2}--font-ui: "Inter";\n\}/,
  );
  assert.match(css, /:root,\n\[data-appearance="light"\] \{\n {2}--chrome-panel: #FFFFFF;/);
  assert.match(
    css,
    /\[data-appearance="hc-dark"\] \{\n {2}--chrome-panel: #000000;\n {2}--chrome-text: #FFFFFF;\n\}/,
  );
  assert.match(css, /\[data-canvas-appearance="dark"\] \{\n {2}--agent-teal: #3CC4B4;\n\}/);
  assert.equal(css.includes("vellum-ink"), false);
});

test("generated files are deterministic", () => {
  assert.equal(generateCss(tokensFor()), generateCss(tokensFor()));
  const ts = generateTs(tokensFor());
  assert.equal(ts, generateTs(tokensFor()));
  assert.match(ts, /"agent\/teal": \{ group: "canvas", values: \{ "light": "#0F8F86"/);
  assert.match(ts, /"motion\/panel": \{ type: "number", value: 120, unit: "ms" \}/);
});

test("checkGaps passes when DESIGN.md and the design agree and legacy names are recorded", () => {
  const { errors, warnings } = checkGaps({
    tokens: tokensFor(),
    designMd: DESIGN_MD,
    map: MAP,
    gapsText: GAPS,
  });
  assert.deepEqual(errors, []);
  assert.deepEqual(
    warnings.map((w) => w.key),
    ["variable-name:vellum-ink"],
  );
});

test("checkGaps fails on a value DESIGN.md gives differently, until a person records it", () => {
  const changed = {
    ...VARIABLES,
    "agent/teal": themed("#0F8F87", "#3CC4B4", "#00675F", "#76E9DE"),
  };
  const found = checkGaps({
    tokens: tokensFor(changed),
    designMd: DESIGN_MD,
    map: MAP,
    gapsText: GAPS,
  });
  assert.deepEqual(
    found.errors.map((e) => e.key),
    ["token-value:agent/teal"],
  );
  assert.match(found.errors[0].message, /DESIGN.md #0F8F86, vellum.pen #0F8F87/);
  const recorded = `${GAPS}- 2026-10-17 · teal · x · check=token-value:agent/teal\n`;
  assert.deepEqual(
    checkGaps({ tokens: tokensFor(changed), designMd: DESIGN_MD, map: MAP, gapsText: recorded })
      .errors,
    [],
  );
});

test("checkGaps fails on tokens only one side defines", () => {
  const extra = { ...VARIABLES, "chrome/toolbar-height": { type: "number", value: 44 } };
  const found = checkGaps({
    tokens: tokensFor(extra, [...ORDER, "chrome/toolbar-height"]),
    designMd: DESIGN_MD,
    map: MAP,
    gapsText: GAPS,
  });
  assert.deepEqual(
    found.errors.map((e) => e.key),
    ["token-extra:chrome/toolbar-height"],
  );
  const withType = DESIGN_MD.replace(
    "|---|---:|---:|---|\n",
    "|---|---:|---:|---|\n| Window title | 20 / 1.25 | 650 | Desktop home |\n",
  );
  assert.deepEqual(
    checkGaps({ tokens: tokensFor(), designMd: withType, map: MAP, gapsText: GAPS }).errors.map(
      (e) => e.key,
    ),
    ["token-missing:type/window-title"],
  );
  const withRole = DESIGN_MD.replace("| Panel |", "| Ribbon | #EEEEEE | #111111 | x |\n| Panel |");
  assert.deepEqual(
    checkGaps({ tokens: tokensFor(), designMd: withRole, map: MAP, gapsText: GAPS }).errors.map(
      (e) => e.key,
    ),
    ["token-missing:ribbon"],
  );
});

test("checkGaps stops when DESIGN.md no longer says what the map quotes", () => {
  const drifted = DESIGN_MD.replace("fade in over 120ms", "fade in over 140ms");
  assert.throws(
    () => checkGaps({ tokens: tokensFor(), designMd: drifted, map: MAP, gapsText: GAPS }),
    /no longer says/,
  );
});

test("contrast follows WCAG 2.x relative luminance", () => {
  assert.equal(contrast("#FFFFFF", "#000000"), 21);
  assert.equal(contrast("#777777", "#777777"), 1);
  assert.equal(Math.round(contrast("#767676", "#FFFFFF") * 100) / 100, 4.54);
});

test("checkContrast checks used and declared pairs in every appearance and skips unbound backgrounds", () => {
  const tokens = tokensFor();
  const ok = checkContrast({
    tokens,
    declared: { pairs: { ui: [["agent/teal", "chrome/panel"]] } },
  });
  assert.deepEqual(ok.errors, []);
  assert.equal(ok.checked.length, 3 * 4, "two used pairs and one declared, in four appearances");
  assert.equal(ok.skipped.length, 1);
  const low = { ...VARIABLES, "chrome/text": themed("#17181B", "#2A2B2F", "#000000", "#FFFFFF") };
  const found = checkContrast({ tokens: tokensFor(low), declared: { pairs: {} } });
  assert.equal(found.errors.length, 1);
  assert.match(
    found.errors[0].message,
    /text chrome\/text on chrome\/panel \(dark\).*needs 4.5:1 \[vellum.pen\]/,
  );
  assert.throws(
    () =>
      checkContrast({ tokens, declared: { pairs: { text: [["chrome/nope", "chrome/panel"]] } } }),
    /not a token/,
  );
});

// --- The CLI ---

const FAKE_PEN = `#!/usr/bin/env node
const fs = require("node:fs");
const path = require("node:path");
const mode = process.env.FAKE_PEN_MODE || "ok";
const skill = mode === "drift" ? "nothing we know" : "function Get(); function GetVariables(); function Print(); function Export();";
for (const line of fs.readFileSync(0, "utf8").split("\\n")) {
  if (line.startsWith("read_skill(")) console.log(skill);
  const m = line.match(/^execute\\(\\{ input: (".*") \\}\\)$/);
  if (!m) continue;
  const snippet = JSON.parse(m[1]);
  if (snippet.includes("@@vellum-tokens-begin")) { console.log(fs.readFileSync(process.env.FAKE_PEN_QUERY_OUTPUT, "utf8")); continue; }
  const e = snippet.match(/Export\\(\\[(.*?)\\],"png","(.*?)",\\{scale:(\\d)\\}\\)/);
  if (e) {
    fs.mkdirSync(e[2], { recursive: true });
    for (const id of JSON.parse("[" + e[1] + "]")) fs.writeFileSync(path.join(e[2], id + ".png"), "PNG@" + e[3] + "x:" + id);
  }
}
`;

function fixtureRoot() {
  const root = scratch("design-tokens");
  put(root, "docs/design/vellum.pen", PEN_BYTES);
  put(root, "docs/design/DESIGN.md", DESIGN_MD);
  put(root, "docs/design/DESIGN-GAPS.md", GAPS);
  put(root, "packages/ui-tokens/source/design-md.json", JSON.stringify(MAP));
  put(
    root,
    "docs/design/snapshots/inventory.json",
    JSON.stringify({
      version: 1,
      source_sha256: sha(PEN_BYTES),
      frames: [
        {
          frame_id: "TOK",
          name: "Foundations · Tokens · all appearances",
          kind: "foundations",
          title: "Tokens · all appearances",
        },
      ],
      components: [],
      variables: { themes: { appearance: ["light", "dark", "hc-light", "hc-dark"] }, names: ORDER },
    }),
  );
  put(root, "fake-pen.cjs", FAKE_PEN);
  put(root, "query-output.txt", rawOutput());
  assert.equal(bash("design-open", ["--root", root, "--no-open"]).status, 0);
  return root;
}

const tokensCli = (root, extra = [], env = {}) =>
  bash("design-tokens", ["--root", root, ...extra], {
    env: {
      ...process.env,
      VELLUM_PEN_CLI: `node ${join(root, "fake-pen.cjs")}`,
      FAKE_PEN_QUERY_OUTPUT: join(root, "query-output.txt"),
      ...env,
    },
  });
const file = (root, path) => readFileSync(join(root, path), "utf8");

test("design-tokens writes tokens.json with the design's sha256, the generated files and the Tokens sheet", () => {
  const root = fixtureRoot();
  try {
    const result = tokensCli(root);
    assert.equal(result.status, 0, result.output);
    const tokens = JSON.parse(file(root, "packages/ui-tokens/source/tokens.json"));
    assert.equal(tokens.source_sha256, sha(PEN_BYTES));
    assert.equal(tokens.exporter, "pen.dev CLI (pen interactive)");
    assert.equal(file(root, "packages/ui-tokens/src/generated/tokens.css"), generateCss(tokens));
    assert.equal(file(root, "packages/ui-tokens/src/generated/tokens.ts"), generateTs(tokens));
    assert.equal(file(root, "docs/design/snapshots/foundations/Tokens.png"), "PNG@1x:TOK");
    assert.equal(file(root, "docs/design/snapshots/foundations/Tokens@2x.png"), "PNG@2x:TOK");
    const first = file(root, "packages/ui-tokens/source/tokens.json");
    assert.equal(tokensCli(root).status, 0);
    assert.equal(
      file(root, "packages/ui-tokens/source/tokens.json"),
      first,
      "a second run changes nothing",
    );
  } finally {
    discard(root);
  }
});

test("design-tokens --raw builds the same output from a query run in an MCP session", () => {
  const root = fixtureRoot();
  try {
    put(root, "stage/1x/TOK.png", "PNG@1x:TOK");
    put(root, "stage/2x/TOK.png", "PNG@2x:TOK");
    const result = tokensCli(root, [
      "--raw",
      join(root, "query-output.txt"),
      "--staging",
      join(root, "stage"),
    ]);
    assert.equal(result.status, 0, result.output);
    assert.equal(
      JSON.parse(file(root, "packages/ui-tokens/source/tokens.json")).exporter,
      "pen.dev MCP (execute)",
    );
    assert.match(
      tokensCli(root, ["--print-export", "--staging", "/s"]).stdout,
      /Export\(\["TOK"\],"png","\/s\/1x",\{scale:1\}\)/,
    );
  } finally {
    discard(root);
  }
});

test("design-tokens stops when read_skill no longer describes the API it relies on", () => {
  const root = fixtureRoot();
  try {
    const result = tokensCli(root, [], { FAKE_PEN_MODE: "drift" });
    assert.notEqual(result.status, 0);
    assert.match(result.output, /execute/);
    assert.equal(existsSync(join(root, "packages/ui-tokens/source/tokens.json")), false);
  } finally {
    discard(root);
  }
});

test("design-tokens --check fails on a stale tokens.json, a hand-edited generated file and an unrecorded gap", () => {
  const root = fixtureRoot();
  try {
    assert.equal(tokensCli(root).status, 0);
    const passed = tokensCli(root, ["--check"]);
    assert.equal(passed.status, 0, passed.output);

    const css = "packages/ui-tokens/src/generated/tokens.css";
    const original = file(root, css);
    put(root, css, original.replace("#FFFFFF", "#FFFFFE"));
    assert.match(tokensCli(root, ["--check"]).output, /tokens\.css is not generated/);
    put(root, css, original);

    const inventory = JSON.parse(file(root, "docs/design/snapshots/inventory.json"));
    put(
      root,
      "docs/design/snapshots/inventory.json",
      JSON.stringify({ ...inventory, source_sha256: sha("NEWER") }),
    );
    const stale = tokensCli(root, ["--check"]);
    assert.notEqual(stale.status, 0);
    assert.match(stale.output, /stale/);
    put(root, "docs/design/snapshots/inventory.json", JSON.stringify(inventory));

    put(root, "docs/design/DESIGN.md", DESIGN_MD.replace("| Teal | #0F8F86", "| Teal | #0F8F87"));
    const gap = tokensCli(root, ["--check"]);
    assert.notEqual(gap.status, 0);
    assert.match(gap.output, /check=token-value:agent\/teal/);
  } finally {
    discard(root);
  }
});
