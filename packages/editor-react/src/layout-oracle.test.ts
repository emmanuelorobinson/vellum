// TEST DATA: every layout-oracle fixture (fixtures/layout-oracle/expected/*.json, written by Chrome, ADR-002).
// The properties panel must read and write layout in exactly the vocabulary the oracle uses, so that what a person
// sets by hand is what the engine lays out and Chrome agrees with. The boxes themselves are the engine's (the
// vellum-layout-taffy tests); this checks the UI's half: reading each fixture's properties into the panel, and
// writing them back with the panel's own edits, gives the same layout properties.
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  applyPatch,
  type DesignTarget,
  readDesign,
  setAlign,
  setClip,
  setGap,
  setGridCount,
  setJustify,
  setLayout,
  setPaddingAll,
  setPaddingSide,
  setSize,
  setSizeMode,
} from "@vellum/editor-core";
import type { Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";

const dir = fileURLToPath(new URL("../../../fixtures/layout-oracle/expected/", import.meta.url));
type Props = Record<string, unknown>;
/** A hidden node has no box in the oracle. */
type Box = {
  id: string;
  props: Props;
  children: Box[];
  box: { width: number; height: number } | null;
};
const fixtures = readdirSync(dir)
  .filter((f) => f.endsWith(".json") && f !== "manifest.json")
  .sort();

const size = (props: Props, key: "width" | "height") => {
  const fallback = props[`${key}Fallback`];
  return fallback === undefined ? props[key] : { [props[key] as string]: fallback };
};
/** The oracle writes "1, 2 or 4 numbers" as arrays; the file format writes one number as a number. */
const formatPadding = (p: unknown) => (Array.isArray(p) && p.length === 1 ? p[0] : p);
const nodeOf = (b: Box): Node => {
  const p = b.props;
  return {
    type: "frame",
    id: `n${b.id}`,
    name: b.id,
    width: size(p, "width"),
    height: size(p, "height"),
    ...(p.layout !== undefined ? { layout: p.layout } : {}),
    ...(p.gap !== undefined ? { gap: p.gap } : {}),
    ...(p.padding !== undefined ? { padding: formatPadding(p.padding) } : {}),
    ...(p.justify !== undefined ? { justify: p.justify } : {}),
    ...(p.align !== undefined ? { align: p.align } : {}),
    ...(p.columns !== undefined ? { columns: p.columns } : {}),
    ...(p.rows !== undefined ? { rows: p.rows } : {}),
    ...(p.clip !== undefined ? { clip: p.clip } : {}),
    ...(p.position === "absolute"
      ? { position: { type: "absolute", x: p.x ?? 0, y: p.y ?? 0 } }
      : {}),
  } as unknown as Node;
};
const flatten = (b: Box): Box[] => [b, ...b.children.flatMap(flatten)];
const target = (node: Node, box: Box): DesignTarget => ({
  item: {
    node,
    bounds: { x: 0, y: 0, width: box.box?.width ?? 0, height: box.box?.height ?? 0 },
    children: [],
    parent: null,
  },
  inLayout: false,
});
const four = (raw: unknown): number[] => {
  const p = formatPadding(raw);
  if (p === undefined) return [0, 0, 0, 0];
  if (typeof p === "number") return [p, p, p, p];
  const a = p as [number, number, number?, number?];
  return a.length === 2 ? [a[0], a[1], a[0], a[1]] : (a as number[]);
};

/** Reads the node into the panel, then writes what the panel shows back onto an empty frame with the panel's edits. */
function viaPanel(source: Node, box: Box): Record<string, unknown> {
  const m = readDesign({
    targets: [target(source, box)],
    document: null,
    readOnly: false,
    text: { fallback: "Inter" },
  });
  const blank = { type: "frame", id: "x", name: "x", width: 0, height: 0 } as unknown as Node;
  let out = blank;
  const edit = (patch: ReturnType<ReturnType<typeof setGap>> | null) => {
    if (patch) out = applyPatch(out, patch);
  };
  const t = () => target(out, box);
  const layout = m.layout;
  if (layout) {
    if (layout.mode.kind === "value") edit(setLayout(layout.mode.value as "row")(t()));
    if (layout.gap.kind === "value") edit(setGap(layout.gap.value)(t()));
    const pad = layout.padding;
    if (pad.linked && pad.all.kind === "value") edit(setPaddingAll(pad.all.value)(t()));
    else
      pad.sides.forEach((s, i) => {
        if (s.kind === "value") edit(setPaddingSide(i as 0, s.value)(t()));
      });
    if (layout.justify.kind === "value") edit(setJustify(layout.justify.value as "start")(t()));
    if (layout.align.kind === "value") edit(setAlign(layout.align.value as "start")(t()));
    if (layout.grid?.columns.kind === "value" && layout.grid.columns.value !== null)
      edit(setGridCount("columns", layout.grid.columns.value)(t()));
    if (layout.grid?.rows.kind === "value" && layout.grid.rows.value !== null)
      edit(setGridCount("rows", layout.grid.rows.value)(t()));
  }
  if (m.appearance.clip?.kind === "value") edit(setClip(m.appearance.clip.value)(t()));
  for (const axis of ["width", "height"] as const) {
    const a = m.size[axis];
    edit(a.mode === "fixed" ? setSize(axis, a.value)(t()) : setSizeMode(axis, a.mode)(t()));
  }
  return out as unknown as Record<string, unknown>;
}

describe("the properties panel speaks the layout oracle's language", () => {
  it("has the oracle's fixtures", () => {
    expect(fixtures.length).toBeGreaterThan(150);
  });
  for (const file of fixtures) {
    const fixture = JSON.parse(readFileSync(dir + file, "utf8")) as { fixture: string; root: Box };
    it(fixture.fixture, () => {
      for (const box of flatten(fixture.root)) {
        const p = box.props;
        // The panel lays out frames; the oracle's nodes are all boxes.
        const source = nodeOf(box);
        const out = viaPanel(source, box);
        const where = `${fixture.fixture} node ${box.id}`;
        expect(out.layout ?? "none", where).toBe(p.layout ?? "none");
        expect(out.gap ?? 0, where).toBe(p.gap ?? 0);
        expect(four(out.padding), where).toEqual(four(p.padding));
        expect(out.justify ?? "start", where).toBe(p.justify ?? "start");
        expect(out.align ?? "start", where).toBe(p.align ?? "start");
        if (p.layout === "grid") {
          expect(out.columns ?? null, where).toBe(p.columns ?? null);
          expect(out.rows ?? null, where).toBe(p.rows ?? null);
        }
        expect(out.clip ?? false, where).toBe(p.clip ?? false);
        for (const axis of ["width", "height"] as const) {
          // A number stays that number; hug and fill stay keywords. The panel does not write the fallback
          // number the format allows next to them (`widthFallback`): that is set in the file or by an agent.
          expect(out[axis], `${where} ${axis}`).toBe(p[axis]);
        }
      }
    });
  }
});
