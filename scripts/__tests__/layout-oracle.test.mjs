// The Chrome layout oracle (guide Prompt 6, D2, Invariant 7): fixtures written as HTML with Vellum's
// sizing semantics, the semantics-to-CSS mapping ADR-002 records, the committed expected boxes, and
// the generator script. Everything here runs without Chrome except the last test, which needs the
// `chrome` lane (VELLUM_LANES=chrome and a Chrome binary, VELLUM_CHROME or the platform default).
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  compareFixtures,
  cssPlan,
  parseFixtureHtml,
  renderCssHtml,
  resolveTree,
  stableJson,
} from "../lib/layout-oracle.mjs";
import { at, bash, discard, scratch } from "./helpers.mjs";

const MARK = "<!-- TEST DATA: layout oracle fixture -->";
const page = (body, title = "A fixture") =>
  `<!doctype html>\n${MARK}\n<title>${title}</title>\n${body}\n`;

/** Parses, resolves and maps `body`; returns the CSS declarations of the node with `id` as an object. */
function cssOf(body, id) {
  const plan = cssPlan(resolveTree(parseFixtureHtml(page(body))));
  const text = plan.get(id);
  assert.ok(text !== undefined, `no CSS for node ${id}`);
  return Object.fromEntries(
    text
      .split(";")
      .map((d) => d.trim())
      .filter(Boolean)
      .map((d) => d.split(":").map((s) => s.trim())),
  );
}

const rejects = (body, pattern) =>
  assert.throws(() => resolveTree(parseFixtureHtml(page(body))), pattern);

// --- parsing ------------------------------------------------------------------------------------

test("parses a fixture into a tree of Vellum props with index-path ids", () => {
  const tree = parseFixtureHtml(
    page(
      `<div data-layout="row" data-gap="10" data-padding="4 8" data-width="hug" data-height="hug">
  <div data-width="40" data-height="30"></div>
  <div data-width="fill" data-width-fallback="20" data-height="30" data-visible="false"></div>
</div>`,
      "Row: two children",
    ),
  );
  assert.equal(tree.title, "Row: two children");
  assert.equal(tree.root.id, "0");
  assert.deepEqual(tree.root.props, {
    gap: 10,
    height: "hug",
    layout: "row",
    padding: [4, 8],
    width: "hug",
  });
  assert.deepEqual(
    tree.root.children.map((c) => c.id),
    ["0.0", "0.1"],
  );
  assert.deepEqual(tree.root.children[1].props, {
    height: 30,
    visible: false,
    width: "fill",
    widthFallback: 20,
  });
});

test("parsing rejects anything that is not pure Vellum semantics", () => {
  const bad = [
    [`<div data-width="1" data-height="1" style="width:2px"></div>`, /attribute "style"/],
    [`<div data-width="1" data-height="1" data-colour="red"></div>`, /attribute "data-colour"/],
    [`<span data-width="1" data-height="1"></span>`, /only <div>/],
    [`<div data-height="1"></div>`, /width/],
    [`<div data-width="1" data-height="1">`, /unclosed/],
    [
      `<div data-width="1" data-height="1"></div><div data-width="1" data-height="1"></div>`,
      /one root/,
    ],
    [`<div data-width="wide" data-height="1"></div>`, /width/],
    [`<div data-width="1" data-height="1" data-padding="1 2 3"></div>`, /padding/],
    [`<div data-width="1" data-height="1" data-justify="around"></div>`, /justify/],
    [`<div data-width="1" data-height="1">text</div>`, /text/],
  ];
  for (const [body, pattern] of bad) {
    assert.throws(() => parseFixtureHtml(page(body)), pattern, body);
  }
  assert.throws(
    () =>
      parseFixtureHtml(
        `<!doctype html>\n<title>t</title>\n<div data-width="1" data-height="1"></div>`,
      ),
    /TEST DATA/,
  );
});

// --- semantics ----------------------------------------------------------------------------------

test("resolving enforces where each sizing and position option can apply", () => {
  rejects(`<div data-width="hug" data-height="10"></div>`, /hug.*no auto-layout.*fallback/);
  rejects(`<div data-width="fill" data-height="10"></div>`, /fill.*fallback/);
  rejects(
    `<div data-layout="grid" data-columns="2" data-justify="space-between" data-width="100" data-height="10"></div>`,
    /space-between.*grid/,
  );
  rejects(`<div data-layout="grid" data-width="100" data-height="10"></div>`, /columns/);
  rejects(
    `<div data-layout="row" data-columns="2" data-width="100" data-height="10"></div>`,
    /columns/,
  );
  rejects(
    `<div data-layout="row" data-width="100" data-height="10"><div data-x="4" data-width="1" data-height="1"></div></div>`,
    /x and y.*absolute/,
  );
  rejects(
    `<div data-width="100" data-height="10"><div data-position="absolute" data-width="1" data-height="1"></div></div>`,
    /absolute.*inside an auto-layout/,
  );
});

test("fill falls back where there is nothing to fill", () => {
  const resolved = resolveTree(
    parseFixtureHtml(
      page(`<div data-layout="row" data-width="hug" data-height="40">
  <div data-width="fill" data-width-fallback="30" data-height="fill"></div>
  <div data-layout="row" data-width="fill" data-height="10"><div data-width="12" data-height="5"></div></div>
  <div data-position="absolute" data-width="fill" data-width-fallback="7" data-height="3"></div>
</div>`),
    ),
  );
  const [a, b, c] = resolved.root.children;
  // Main axis of a hugging row: fill uses its fallback, else hugs.
  assert.deepEqual(a.sizing.width, { kind: "fixed", value: 30 });
  assert.deepEqual(b.sizing.width, { kind: "hug" });
  // Cross axis: fill stretches even when the parent hugs that axis (here the parent is fixed anyway).
  assert.deepEqual(a.sizing.height, { kind: "fill" });
  // Absolute children are outside the layout, so fill falls back.
  assert.deepEqual(c.sizing.width, { kind: "fixed", value: 7 });
  // The root sits on the canvas: fill always falls back there.
  assert.deepEqual(
    resolveTree(
      parseFixtureHtml(
        page(`<div data-width="fill" data-width-fallback="9" data-height="1"></div>`),
      ),
    ).root.sizing.width,
    { kind: "fixed", value: 9 },
  );
});

test("a fill child of a hugging column stretches across it (cross axis)", () => {
  const resolved = resolveTree(
    parseFixtureHtml(
      page(`<div data-layout="column" data-width="hug" data-height="hug">
  <div data-width="fill" data-height="10"></div>
  <div data-width="50" data-height="10"></div>
</div>`),
    ),
  );
  assert.deepEqual(resolved.root.children[0].sizing.width, { kind: "fill" });
});

// --- CSS mapping (ADR-002) ----------------------------------------------------------------------

test("hug maps to fit-content inside a layout and max-content on the canvas", () => {
  const body = `<div data-layout="row" data-width="hug" data-height="hug">
  <div data-layout="column" data-width="hug" data-height="hug"><div data-width="5" data-height="5"></div></div>
</div>`;
  const root = cssOf(body, "0");
  assert.equal(root.width, "max-content");
  assert.equal(root.height, "max-content");
  assert.equal(root.position, "absolute");
  const child = cssOf(body, "0.0");
  assert.equal(child.width, "fit-content");
  assert.equal(child.height, "fit-content");
  assert.equal(child["flex-shrink"], "0", "hug never shrinks below its content");
});

test("fixed sizes never shrink; fill grows from zero on the main axis and stretches on the cross axis", () => {
  const body = `<div data-layout="row" data-width="200" data-height="50">
  <div data-width="40" data-height="30"></div>
  <div data-width="fill" data-height="fill"></div>
</div>`;
  const fixed = cssOf(body, "0.0");
  assert.equal(fixed.width, "40px");
  assert.equal(fixed.height, "30px");
  assert.equal(fixed["flex-shrink"], "0");
  const fill = cssOf(body, "0.1");
  assert.equal(fill["flex-grow"], "1");
  assert.equal(fill["flex-basis"], "0px");
  assert.equal(fill["min-width"], "0px");
  assert.equal(fill["align-self"], "stretch");
  assert.equal(fill.width, undefined);
  assert.equal(fill.height, undefined);

  const column = cssOf(
    `<div data-layout="column" data-width="50" data-height="200"><div data-width="fill" data-height="fill"></div></div>`,
    "0.0",
  );
  assert.equal(column["flex-basis"], "0px");
  assert.equal(column["min-height"], "0px");
  assert.equal(column["align-self"], "stretch");
});

test("container options map to flexbox and grid properties", () => {
  const justify = {
    start: "flex-start",
    center: "center",
    end: "flex-end",
    "space-between": "space-between",
    "space-around": "space-around",
  };
  for (const [v, css] of Object.entries(justify)) {
    const c = cssOf(
      `<div data-layout="row" data-justify="${v}" data-width="9" data-height="9"></div>`,
      "0",
    );
    assert.equal(c["justify-content"], css);
  }
  for (const [v, css] of Object.entries({
    start: "flex-start",
    center: "center",
    end: "flex-end",
  })) {
    const c = cssOf(
      `<div data-layout="column" data-align="${v}" data-width="9" data-height="9"></div>`,
      "0",
    );
    assert.equal(c["align-items"], css);
    assert.equal(c["flex-direction"], "column");
  }
  const row = cssOf(`<div data-layout="row" data-width="9" data-height="9"></div>`, "0");
  assert.equal(row.display, "flex");
  assert.equal(row["flex-wrap"], "nowrap");
  assert.equal(row["align-items"], "flex-start", "Vellum's default is start, not CSS's stretch");
  assert.equal(row["justify-content"], "flex-start");

  const paddings = { 8: "8px", "4 8": "4px 8px", "1 2 3 4": "1px 2px 3px 4px" };
  for (const [v, css] of Object.entries(paddings)) {
    const c = cssOf(
      `<div data-layout="row" data-padding="${v}" data-gap="6" data-width="9" data-height="9"></div>`,
      "0",
    );
    assert.equal(c.padding, css);
    assert.equal(c.gap, "6px");
  }

  const grid = cssOf(
    `<div data-layout="grid" data-columns="3" data-rows="2" data-justify="center" data-align="end" data-width="90" data-height="40"></div>`,
    "0",
  );
  assert.equal(grid.display, "grid");
  assert.equal(grid["grid-template-columns"], "repeat(3, minmax(0px, 1fr))");
  assert.equal(grid["grid-template-rows"], "repeat(2, minmax(0px, 1fr))");
  assert.equal(grid["justify-items"], "center");
  assert.equal(grid["align-items"], "end");
  const autoRows = cssOf(
    `<div data-layout="grid" data-columns="2" data-width="90" data-height="hug"></div>`,
    "0",
  );
  assert.equal(autoRows["grid-template-rows"], undefined);
  assert.equal(autoRows["grid-auto-rows"], "auto");
});

test("grid children, absolute children, hidden and clipped nodes", () => {
  const body = `<div data-layout="grid" data-columns="2" data-width="100" data-height="hug">
  <div data-width="fill" data-height="fill"></div>
  <div data-position="absolute" data-x="5" data-y="6" data-width="hug" data-width-fallback="1" data-height="7"><div data-width="1" data-height="1"></div></div>
  <div data-width="10" data-height="10" data-visible="false"></div>
  <div data-width="10" data-height="10" data-clip="true"></div>
</div>`;
  const fill = cssOf(body, "0.0");
  assert.equal(fill["justify-self"], "stretch");
  assert.equal(fill["align-self"], "stretch");
  assert.equal(fill["min-width"], "0px");
  const abs = cssOf(body, "0.1");
  assert.equal(abs.position, "absolute");
  assert.equal(abs.left, "5px");
  assert.equal(abs.top, "6px");
  assert.equal(abs.width, "1px", "hug on a frame without auto-layout uses its fallback");
  assert.equal(abs.height, "7px");
  assert.equal(cssOf(body, "0.2").display, "none");
  assert.equal(cssOf(body, "0.3").overflow, "hidden");

  const none = cssOf(
    `<div data-width="50" data-height="50"><div data-x="3" data-y="4" data-width="5" data-height="5"></div></div>`,
    "0.0",
  );
  assert.equal(none.position, "absolute");
  assert.equal(none.left, "3px");
  assert.equal(none.top, "4px");
});

test("the rendered CSS page tags every node and carries no Vellum attributes", () => {
  const tree = resolveTree(
    parseFixtureHtml(
      page(
        `<div data-layout="row" data-width="hug" data-height="hug"><div data-width="4" data-height="4"></div></div>`,
      ),
    ),
  );
  const html = renderCssHtml(tree);
  assert.match(html, /data-oracle-id="0"/);
  assert.match(html, /data-oracle-id="0\.0"/);
  assert.doesNotMatch(html, /data-layout|data-width/);
  assert.match(html, /margin: ?0/);
});

// --- comparison and JSON ------------------------------------------------------------------------

test("stableJson sorts keys at every level and ends with a newline", () => {
  assert.equal(
    stableJson({ b: 1, a: { d: [2, { z: 1, y: 2 }], c: null } }),
    `{
  "a": {
    "c": null,
    "d": [
      2,
      {
        "y": 2,
        "z": 1
      }
    ]
  },
  "b": 1
}
`,
  );
});

test("compareFixtures tolerates half a pixel and reports structure and box changes", () => {
  const node = (x, extra = {}) => ({
    box: { height: 10, width: 10, x, y: 0 },
    children: [],
    id: "0",
    props: { height: 10, width: 10 },
    ...extra,
  });
  const fx = (root) => ({ fixture: "f", root, title: "t" });
  assert.deepEqual(compareFixtures(fx(node(0)), fx(node(0.5))), []);
  assert.equal(compareFixtures(fx(node(0)), fx(node(0.6))).length, 1);
  assert.match(compareFixtures(fx(node(0)), fx(node(0, { props: { width: 11 } })))[0], /props/);
  assert.match(compareFixtures(fx(node(0)), fx(node(0, { box: null })))[0], /box/);
});

// --- the committed corpus -----------------------------------------------------------------------

const fixturesDir = at("fixtures", "layout-oracle");
const fixtureNames = () =>
  existsSync(fixturesDir)
    ? readdirSync(fixturesDir)
        .filter((f) => f.endsWith(".html"))
        .map((f) => f.slice(0, -5))
        .sort()
    : [];

test("at least 40 fixtures, all valid, each with expected boxes that match its props", () => {
  const names = fixtureNames();
  assert.ok(names.length >= 40, `only ${names.length} fixtures`);
  const expectedDir = join(fixturesDir, "expected");
  const expectedFiles = readdirSync(expectedDir).filter(
    (f) => f.endsWith(".json") && f !== "manifest.json",
  );
  assert.deepEqual(
    expectedFiles.map((f) => f.slice(0, -5)).sort(),
    names,
    "expected/ matches the fixtures",
  );
  const manifest = JSON.parse(readFileSync(join(expectedDir, "manifest.json"), "utf8"));
  assert.deepEqual(manifest.fixtures, names);
  assert.equal(manifest.tolerancePx, 0.5);
  assert.match(manifest.chrome, /^\d+\.\d+\.\d+\.\d+$/);

  for (const name of names) {
    assert.match(name, /^[a-z0-9]+(_[a-z0-9]+)*$/, name);
    const tree = resolveTree(
      parseFixtureHtml(readFileSync(join(fixturesDir, `${name}.html`), "utf8")),
    );
    const expected = JSON.parse(readFileSync(join(expectedDir, `${name}.json`), "utf8"));
    assert.equal(expected.fixture, name);
    assert.equal(expected.title, tree.title);
    // Props and ids come from the fixture: an edited fixture without regenerated boxes fails here.
    const shape = (n) => ({ id: n.id, props: n.props, children: n.children.map(shape) });
    assert.deepEqual(shape(expected.root), shape(tree.root), `${name}: run pnpm layout-oracle`);
  }
});

test("the corpus exercises every sizing and alignment option", () => {
  const seen = new Set();
  const visit = (n, parent) => {
    const p = n.props;
    seen.add(`layout:${p.layout ?? "none"}`);
    for (const key of ["justify", "align"]) if (p[key]) seen.add(`${key}:${p[key]}`);
    for (const axis of ["width", "height"]) {
      const v = typeof p[axis] === "number" ? "fixed" : p[axis];
      seen.add(`${axis}:${v}`);
      if (p[`${axis}Fallback`] !== undefined) seen.add(`fallback:${v}`);
    }
    if (p.padding) seen.add(`padding:${p.padding.length}`);
    if (p.gap) seen.add("gap");
    if (p.position === "absolute") seen.add(`absolute-in:${parent?.props.layout}`);
    if (p.visible === false) seen.add("hidden");
    if (p.clip) seen.add("clip");
    if (p.rows) seen.add("grid-rows");
    for (const c of n.children) visit(c, n);
  };
  for (const name of fixtureNames()) {
    visit(
      resolveTree(parseFixtureHtml(readFileSync(join(fixturesDir, `${name}.html`), "utf8"))).root,
    );
  }
  const want = [
    "layout:none",
    "layout:row",
    "layout:column",
    "layout:grid",
    "justify:start",
    "justify:center",
    "justify:end",
    "justify:space-between",
    "justify:space-around",
    "align:start",
    "align:center",
    "align:end",
    "width:fixed",
    "width:hug",
    "width:fill",
    "height:fixed",
    "height:hug",
    "height:fill",
    "fallback:hug",
    "fallback:fill",
    "padding:1",
    "padding:2",
    "padding:4",
    "gap",
    "absolute-in:row",
    "absolute-in:column",
    "absolute-in:grid",
    "hidden",
    "clip",
    "grid-rows",
  ];
  assert.deepEqual(
    want.filter((w) => !seen.has(w)),
    [],
  );
});

// --- the generator ------------------------------------------------------------------------------

test("gen-layout-oracle.sh is implemented and documents its options", () => {
  const r = bash("gen-layout-oracle.sh", ["--help"]);
  assert.equal(r.status, 0, r.output);
  assert.match(r.output, /--check/);
  assert.doesNotMatch(r.output, /not implemented/);
});

test("gen-layout-oracle.sh refuses unknown options", () => {
  const r = bash("gen-layout-oracle.sh", ["--bogus"]);
  assert.notEqual(r.status, 0);
  assert.match(r.output, /--bogus/);
});

const lanes = (process.env.VELLUM_LANES ?? "").split(",").map((s) => s.trim());

test("chrome lane: regenerating in headless Chrome reproduces the committed boxes", {
  skip: lanes.includes("chrome") ? false : "chrome lane not enabled (set VELLUM_LANES=chrome)",
}, () => {
  const out = scratch("layout-oracle");
  try {
    const r = bash("gen-layout-oracle.sh", ["--check", "--out", out]);
    assert.equal(r.status, 0, r.output);
    assert.match(r.output, /fixtures match/);
  } finally {
    discard(out);
  }
});
