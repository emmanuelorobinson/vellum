// The Chrome layout oracle (guide Prompt 6, D2, Invariant 7; ADR-002).
//
// A fixture is a small HTML file in fixtures/layout-oracle/ whose <div>s carry Vellum's layout
// semantics as data-* attributes and nothing else. This module parses a fixture, resolves the
// semantics (where hug and fill fall back), and maps them to CSS. scripts/gen-layout-oracle.mjs
// renders that CSS in headless Chrome and writes the boxes Chrome computes as expected JSON, which
// vellum-testkit's layout_oracle module loads. ADR-002 records the mapping; keep the two in step.

const LAYOUTS = ["none", "row", "column", "grid"];
const JUSTIFY = ["start", "center", "end", "space-between", "space-around"];
const ALIGN = ["start", "center", "end"];
const AXES = ["width", "height"];

// data-* attribute → prop name and parser. Nothing else is allowed on a fixture element.
const ATTRIBUTES = {
  "data-layout": ["layout", oneOf(LAYOUTS)],
  "data-gap": ["gap", number({ min: 0 })],
  "data-padding": ["padding", padding],
  "data-justify": ["justify", oneOf(JUSTIFY)],
  "data-align": ["align", oneOf(ALIGN)],
  "data-columns": ["columns", count],
  "data-rows": ["rows", count],
  "data-width": ["width", size("width")],
  "data-height": ["height", size("height")],
  "data-width-fallback": ["widthFallback", number({ min: 0 })],
  "data-height-fallback": ["heightFallback", number({ min: 0 })],
  "data-position": ["position", oneOf(["auto", "absolute"])],
  "data-x": ["x", number({})],
  "data-y": ["y", number({})],
  "data-visible": ["visible", boolean],
  "data-clip": ["clip", boolean],
};

function oneOf(values) {
  return (v, name) => {
    if (!values.includes(v))
      throw new Error(`${name} must be one of ${values.join(", ")}, not "${v}"`);
    return v;
  };
}

function number({ min }) {
  return (v, name) => {
    const n = Number(v);
    if (v.trim() === "" || !Number.isFinite(n) || (min !== undefined && n < min)) {
      throw new Error(
        `${name} must be a number${min !== undefined ? ` >= ${min}` : ""}, not "${v}"`,
      );
    }
    return n;
  };
}

function count(v, name) {
  const n = Number(v);
  if (!Number.isInteger(n) || n < 1)
    throw new Error(`${name} must be a whole number >= 1, not "${v}"`);
  return n;
}

function size(axis) {
  return (v) => {
    if (v === "hug" || v === "fill") return v;
    const n = Number(v);
    if (v.trim() === "" || !Number.isFinite(n) || n < 0) {
      throw new Error(`${axis} must be a number >= 0, "hug" or "fill", not "${v}"`);
    }
    return n;
  };
}

function padding(v) {
  const parts = v.trim().split(/\s+/).map(Number);
  if (![1, 2, 4].includes(parts.length) || parts.some((n) => !Number.isFinite(n) || n < 0)) {
    throw new Error(`padding takes 1, 2 or 4 numbers >= 0, not "${v}"`);
  }
  return parts;
}

function boolean(v, name) {
  if (v !== "true" && v !== "false")
    throw new Error(`${name} must be "true" or "false", not "${v}"`);
  return v === "true";
}

const sortKeys = (v) =>
  Array.isArray(v)
    ? v.map(sortKeys)
    : v && typeof v === "object"
      ? Object.fromEntries(
          Object.keys(v)
            .sort()
            .map((k) => [k, sortKeys(v[k])]),
        )
      : v;

/** Pretty JSON with keys sorted at every level and a trailing newline: the expected-file format. */
export function stableJson(value) {
  return `${JSON.stringify(sortKeys(value), null, 2)}\n`;
}

// --- parsing ------------------------------------------------------------------------------------

/**
 * Parses a fixture: `{ title, root }`, where each node is `{ id, props, children }` and ids are index
 * paths ("0", "0.1", ...). Fixtures are a strict subset of HTML: a doctype, a "TEST DATA" comment, a
 * <title>, and one root <div> tree. Anything else is an error, so Chrome and this parser cannot
 * disagree about the tree.
 */
export function parseFixtureHtml(html) {
  if (!/<!--[\s\S]*?TEST DATA[\s\S]*?-->/.test(html)) {
    throw new Error('a fixture must carry a "TEST DATA" comment');
  }
  const title = /<title>([^<]*)<\/title>/.exec(html)?.[1]?.trim();
  if (!title) throw new Error("a fixture needs a <title>");
  const body = html
    .replace(/<!doctype html>/i, "")
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/<title>[^<]*<\/title>/, "");

  const roots = [];
  const stack = [];
  const token = /<(\/?)([a-zA-Z][a-zA-Z0-9]*)((?:\s+[^\s=>]+="[^"]*")*)\s*(\/?)>|([^<]+)|(<)/g;
  for (let m = token.exec(body); m; m = token.exec(body)) {
    const [, closing, tag, attrs, , text, stray] = m;
    if (stray) throw new Error(`unexpected "<" at offset ${m.index}`);
    if (text !== undefined) {
      if (text.trim() !== "")
        throw new Error(`fixtures hold no text ("${text.trim().slice(0, 20)}")`);
      continue;
    }
    if (tag.toLowerCase() !== "div")
      throw new Error(`only <div> elements are allowed, not <${tag}>`);
    if (closing) {
      if (!stack.pop()) throw new Error("a </div> without an opening <div>");
      continue;
    }
    const node = { id: "", props: parseProps(attrs), children: [] };
    const parent = stack.at(-1);
    if (parent) parent.children.push(node);
    else roots.push(node);
    stack.push(node);
  }
  if (stack.length) throw new Error("an unclosed <div>");
  if (roots.length !== 1)
    throw new Error(`a fixture has exactly one root <div>, not ${roots.length}`);
  const number = (node, id) => {
    node.id = id;
    node.children.forEach((c, i) => {
      number(c, `${id}.${i}`);
    });
    return node;
  };
  return { title, root: number(roots[0], "0") };
}

function parseProps(attrs) {
  const props = {};
  for (const [, name, value] of attrs.matchAll(/([^\s=]+)="([^"]*)"/g)) {
    const spec = ATTRIBUTES[name];
    if (!spec) throw new Error(`attribute "${name}" is not a Vellum layout property`);
    const [prop, parse] = spec;
    props[prop] = parse(value, prop);
  }
  for (const axis of AXES) {
    if (props[axis] === undefined) throw new Error(`every node needs data-${axis}`);
  }
  return props;
}

// --- semantics ----------------------------------------------------------------------------------

const isFlex = (props) => props.layout === "row" || props.layout === "column";
const layoutOf = (props) => props.layout ?? "none";
const mainAxis = (props) => (props.layout === "row" ? "width" : "height");

/**
 * Resolves each node's sizing per axis to `{ kind: "fixed", value } | { kind: "hug" } | { kind: "fill" }`
 * and checks where options apply. The rules (ADR-002):
 * - hug on a frame without auto-layout uses its fallback (an error without one);
 * - fill with nothing to fill (the root, children of a frame without auto-layout, absolute children)
 *   uses its fallback (an error without one);
 * - fill on the main axis of a row or column that hugs that axis uses its fallback, else hugs;
 * - fill on the cross axis, or in a grid cell, stretches.
 * Returns `{ title, root }` with nodes `{ id, props, children, sizing, positioned, isRoot }`.
 */
export function resolveTree(tree) {
  const visit = (node, parent) => {
    const p = node.props;
    const where = `node ${node.id}`;
    const layout = layoutOf(p);
    if (layout === "grid" && p.columns === undefined)
      throw new Error(`${where}: a grid needs data-columns`);
    if (layout !== "grid" && (p.columns !== undefined || p.rows !== undefined)) {
      throw new Error(`${where}: columns and rows apply only to a grid`);
    }
    if (layout === "grid" && p.justify?.startsWith("space-")) {
      throw new Error(`${where}: justify ${p.justify} does not apply to grid cells`);
    }
    const absolute = p.position === "absolute";
    if (absolute && (!parent || layoutOf(parent.props) === "none")) {
      throw new Error(`${where}: position absolute applies only inside an auto-layout frame`);
    }
    const positioned = absolute || (parent !== null && layoutOf(parent.props) === "none");
    if ((p.x !== undefined || p.y !== undefined) && !positioned) {
      throw new Error(
        `${where}: x and y apply only to absolute children and children of a frame without auto-layout`,
      );
    }

    const hug = (axis) => {
      if (layout !== "none") return { kind: "hug" };
      const fallback = p[`${axis}Fallback`];
      if (fallback === undefined) {
        throw new Error(`${where}: ${axis} hug on a frame with no auto-layout needs a fallback`);
      }
      return { kind: "fixed", value: fallback };
    };
    const sizing = {};
    for (const axis of AXES) {
      const v = p[axis];
      const fallback = p[`${axis}Fallback`];
      if (typeof v === "number") sizing[axis] = { kind: "fixed", value: v };
      else if (v === "hug") sizing[axis] = hug(axis);
      else if (!parent || positioned) {
        if (fallback === undefined) {
          throw new Error(`${where}: ${axis} fill with nothing to fill needs a fallback`);
        }
        sizing[axis] = { kind: "fixed", value: fallback };
      } else if (
        isFlex(parent.props) &&
        mainAxis(parent.props) === axis &&
        parent.sizing[axis].kind === "hug"
      ) {
        sizing[axis] = fallback === undefined ? hug(axis) : { kind: "fixed", value: fallback };
      } else sizing[axis] = { kind: "fill" };
    }
    const resolved = { id: node.id, props: p, children: [], sizing, positioned, isRoot: !parent };
    resolved.children = node.children.map((c) => visit(c, resolved));
    return resolved;
  };
  return { title: tree.title, root: visit(tree.root, null) };
}

// --- CSS mapping --------------------------------------------------------------------------------

const FLEX_JUSTIFY = {
  start: "flex-start",
  center: "center",
  end: "flex-end",
  "space-between": "space-between",
  "space-around": "space-around",
};
const FLEX_ALIGN = { start: "flex-start", center: "center", end: "flex-end" };
const px = (n) => `${n}px`;

function cssFor(node, parent) {
  const p = node.props;
  const d = [];
  if (node.isRoot) d.push(["position", "absolute"], ["left", "0px"], ["top", "0px"]);
  else if (node.positioned)
    d.push(["position", "absolute"], ["left", px(p.x ?? 0)], ["top", px(p.y ?? 0)]);
  else d.push(["position", "relative"]);
  d.push(["box-sizing", "border-box"], ["margin", "0px"]);

  const layout = layoutOf(p);
  if (p.visible === false) d.push(["display", "none"]);
  else d.push(["display", { none: "block", row: "flex", column: "flex", grid: "grid" }[layout]]);
  if (isFlex(p)) {
    d.push(
      ["flex-direction", p.layout],
      ["flex-wrap", "nowrap"],
      ["justify-content", FLEX_JUSTIFY[p.justify ?? "start"]],
      ["align-items", FLEX_ALIGN[p.align ?? "start"]],
    );
  } else if (layout === "grid") {
    d.push(["grid-template-columns", `repeat(${p.columns}, minmax(0px, 1fr))`]);
    if (p.rows !== undefined) d.push(["grid-template-rows", `repeat(${p.rows}, minmax(0px, 1fr))`]);
    else d.push(["grid-auto-rows", "auto"]);
    d.push(["justify-items", p.justify ?? "start"], ["align-items", p.align ?? "start"]);
  }
  if (p.gap !== undefined && layout !== "none") d.push(["gap", px(p.gap)]);
  if (p.padding) d.push(["padding", p.padding.map(px).join(" ")]);
  if (p.clip) d.push(["overflow", "hidden"]);

  const inFlex = parent && !node.positioned && isFlex(parent.props);
  const inGrid = parent && !node.positioned && layoutOf(parent.props) === "grid";
  for (const axis of AXES) {
    const s = node.sizing[axis];
    const min = axis === "width" ? "min-width" : "min-height";
    if (s.kind === "fixed") d.push([axis, px(s.value)]);
    else if (s.kind === "hug")
      d.push([axis, node.isRoot || node.positioned ? "max-content" : "fit-content"]);
    else if (inFlex && mainAxis(parent.props) === axis) {
      d.push(["flex-grow", "1"], ["flex-shrink", "1"], ["flex-basis", "0px"], [min, "0px"]);
    } else if (inFlex) d.push(["align-self", "stretch"]);
    else if (inGrid)
      d.push([axis === "width" ? "justify-self" : "align-self", "stretch"], [min, "0px"]);
    if (inFlex && mainAxis(parent.props) === axis && s.kind !== "fill")
      d.push(["flex-shrink", "0"]);
  }
  return d.map(([k, v]) => `${k}: ${v}`).join("; ");
}

/** Every node's inline CSS, keyed by id, in tree order. */
export function cssPlan(resolved) {
  const plan = new Map();
  const visit = (node, parent) => {
    plan.set(node.id, cssFor(node, parent));
    for (const c of node.children) visit(c, node);
  };
  visit(resolved.root, null);
  return plan;
}

const escapeHtml = (s) =>
  s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

/** The fixture's CSS equivalent: the page Chrome lays out. Elements carry only `data-oracle-id`. */
export function renderCssHtml(resolved) {
  const plan = cssPlan(resolved);
  const render = (node, indent) => {
    const inner = node.children.map((c) => render(c, `${indent}  `)).join("");
    const open = `${indent}<div data-oracle-id="${node.id}" style="${escapeHtml(plan.get(node.id))}">`;
    return inner ? `${open}\n${inner}${indent}</div>\n` : `${open}</div>\n`;
  };
  return `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>${escapeHtml(resolved.title)}</title>
<style>html, body { margin: 0; padding: 0; }</style>
</head>
<body>
${render(resolved.root, "")}</body>
</html>
`;
}

/**
 * Runs inside the page (serialized with Function.prototype.toString, so it must not close over
 * anything): each tagged element's border box relative to its parent's, or null when it is not
 * rendered (display: none, or inside such a subtree).
 */
export function measureInPage() {
  const boxes = {};
  for (const el of document.querySelectorAll("[data-oracle-id]")) {
    if (el.getClientRects().length === 0) {
      boxes[el.dataset.oracleId] = null;
      continue;
    }
    const r = el.getBoundingClientRect();
    const p = el.parentElement.getBoundingClientRect();
    boxes[el.dataset.oracleId] = { x: r.x - p.x, y: r.y - p.y, width: r.width, height: r.height };
  }
  return boxes;
}

/** The expected-file object for a resolved fixture and the boxes Chrome measured. */
export function expectedFixture(name, resolved, boxes) {
  const build = (node) => {
    if (!(node.id in boxes)) throw new Error(`${name}: Chrome did not measure node ${node.id}`);
    return {
      box: boxes[node.id],
      children: node.children.map(build),
      id: node.id,
      props: node.props,
    };
  };
  return { fixture: name, root: build(resolved.root), title: resolved.title };
}

// --- comparison ---------------------------------------------------------------------------------

/** Differences between two expected-file objects; boxes may differ by up to `tolerance` px. */
export function compareFixtures(want, got, tolerance = 0.5) {
  const out = [];
  const where = `${want.fixture}`;
  if (want.fixture !== got.fixture) out.push(`${where}: fixture name ${got.fixture}`);
  if (want.title !== got.title) out.push(`${where}: title "${got.title}" (was "${want.title}")`);
  const visit = (a, b) => {
    const at = `${where} node ${a.id}`;
    if (a.id !== b.id) return out.push(`${at}: id ${b.id}`);
    if (stableJson(a.props) !== stableJson(b.props)) out.push(`${at}: props changed`);
    if ((a.box === null) !== (b.box === null)) {
      out.push(
        `${at}: box ${b.box === null ? "missing" : "present"} (was ${a.box === null ? "none" : "a box"})`,
      );
    } else if (a.box) {
      for (const k of ["x", "y", "width", "height"]) {
        const delta = Math.abs(a.box[k] - b.box[k]);
        if (delta > tolerance)
          out.push(`${at}: ${k} ${b.box[k]} (was ${a.box[k]}, off by ${delta})`);
      }
    }
    if (a.children.length !== b.children.length) {
      return out.push(`${at}: ${b.children.length} children (was ${a.children.length})`);
    }
    a.children.forEach((c, i) => {
      visit(c, b.children[i]);
    });
  };
  visit(want.root, got.root);
  return out;
}
