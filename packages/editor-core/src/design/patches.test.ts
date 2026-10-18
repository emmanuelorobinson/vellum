// TEST DATA: patches for the Step 2 frame of design screen C13.
import type { Effect, Node, Paint, Stroke } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import type { SceneItem } from "../tool-types";
import type { DesignTarget } from "./model";
import * as p from "./patches";

const target = (
  extra: Record<string, unknown> = {},
  over: Partial<DesignTarget> = {},
): DesignTarget => ({
  item: {
    node: {
      type: "frame",
      id: "step2",
      name: "Step 2",
      width: "hug",
      height: 440,
      position: { type: "absolute", x: 320, y: 120 },
      layout: "column",
      padding: 24,
      ...extra,
    } as unknown as Node,
    bounds: { x: 320, y: 120, width: 240.5, height: 440 },
    children: [],
    parent: null,
  } as SceneItem,
  inLayout: false,
  ...over,
});
const rect = (extra: Record<string, unknown> = {}) =>
  target({ type: "rect", layout: undefined, ...extra });
const solid = (color: string) => ({ type: "solid", color }) as unknown as Paint;

describe("position and rotation", () => {
  it("moves one axis and keeps the other", () => {
    expect(p.setPosition("x", 100)(target())).toEqual({
      position: { type: "absolute", x: 100, y: 120 },
    });
    expect(p.setPosition("y", 50)(target())).toEqual({
      position: { type: "absolute", x: 320, y: 50 },
    });
  });
  it("starts from the origin for a layer with no position", () => {
    expect(p.setPosition("x", 5)(target({ position: undefined }))).toEqual({
      position: { type: "absolute", x: 5, y: 0 },
    });
  });
  it("leaves a layer that its parent's layout places alone", () => {
    expect(
      p.setPosition("x", 5)(target({ position: { type: "auto" } }, { inLayout: true })),
    ).toBeNull();
  });
  it("sets rotation, leaving the key out for 0 and keeping a variable", () => {
    expect(p.setRotation(15)(target())).toEqual({ rotation: 15 });
    expect(p.setRotation(0)(target())).toEqual({ rotation: null });
    expect(p.setRotation({ $var: "v" } as never)(target())).toEqual({ rotation: { $var: "v" } });
  });
});

describe("size", () => {
  it("hug and fill are the keywords", () => {
    expect(p.setSizeMode("height", "hug")(target())).toEqual({ height: "hug" });
    expect(p.setSizeMode("width", "fill")(target())).toEqual({ width: "fill" });
  });
  it("fixed takes the size the layer has now", () => {
    expect(p.setSizeMode("width", "fixed")(target())).toEqual({ width: 240.5 });
  });
  it("a typed size is fixed, and never negative", () => {
    expect(p.setSize("width", 120)(target())).toEqual({ width: 120 });
    expect(p.setSize("width", -4)(target())).toEqual({ width: 0 });
    expect(p.setSize("width", { $var: "v" } as never)(target())).toEqual({ width: { $var: "v" } });
  });
});

describe("layout", () => {
  it("sets the layout, with Free as no layout", () => {
    expect(p.setLayout("row")(target())).toEqual({ layout: "row" });
    expect(p.setLayout("none")(target())).toEqual({ layout: null });
    expect(p.setLayout("row")(rect())).toBeNull();
  });
  it("sets the gap, which may be a variable", () => {
    expect(p.setGap(16)(target())).toEqual({ gap: 16 });
    expect(p.setGap(0)(target())).toEqual({ gap: null });
    expect(p.setGap({ $var: "v1" } as never)(target())).toEqual({ gap: { $var: "v1" } });
    expect(p.setGap(4)(rect())).toBeNull();
  });
  it("sets all four paddings at once", () => {
    expect(p.setPaddingAll(8)(target())).toEqual({ padding: 8 });
    expect(p.setPaddingAll(0)(target())).toEqual({ padding: null });
  });
  it("sets one side: the padding becomes four values", () => {
    expect(p.setPaddingSide(0, 8)(target())).toEqual({ padding: [8, 24, 24, 24] });
    expect(p.setPaddingSide(3, 1)(target({ padding: [1, 2, 3, 4] }))).toEqual({
      padding: [1, 2, 3, 1],
    });
    expect(p.setPaddingSide(1, 5)(target({ padding: [8, 16] }))).toEqual({
      padding: [8, 5, 8, 16],
    });
  });
  it("goes back to one value when the sides agree again", () => {
    expect(p.setPaddingSide(0, 24)(target({ padding: [8, 24, 24, 24] }))).toEqual({ padding: 24 });
    expect(p.setPaddingSide(0, 0)(target({ padding: [8, 0, 0, 0] }))).toEqual({ padding: null });
  });
  it("links and unlinks: unlinking keeps the value on every side, linking takes the top", () => {
    expect(p.setPaddingLinked(false)(target())).toEqual({ padding: [24, 24, 24, 24] });
    expect(p.setPaddingLinked(true)(target({ padding: [8, 16, 8, 16] }))).toEqual({ padding: 8 });
    expect(p.setPaddingLinked(true)(target())).toBeNull();
  });
  it("sets alignment and distribution, leaving out the default", () => {
    expect(p.setAlign("center")(target())).toEqual({ align: "center" });
    expect(p.setAlign("start")(target())).toEqual({ align: null });
    expect(p.setJustify("space-between")(target())).toEqual({ justify: "space-between" });
  });
  it("sets grid columns and rows", () => {
    expect(p.setGridCount("columns", 3)(target({ layout: "grid" }))).toEqual({ columns: 3 });
    expect(p.setGridCount("rows", null)(target({ layout: "grid" }))).toEqual({ rows: null });
    expect(p.setGridCount("columns", 0)(target({ layout: "grid" }))).toEqual({ columns: 1 });
    expect(p.setGridCount("columns", 2.7)(target({ layout: "grid" }))).toEqual({ columns: 3 });
    expect(p.setGridCount("columns", 3)(target())).toBeNull();
  });
});

describe("appearance", () => {
  it("sets opacity, leaving the key out for 1", () => {
    expect(p.setOpacity(0.5)(target())).toEqual({ opacity: 0.5 });
    expect(p.setOpacity(1)(target())).toEqual({ opacity: null });
  });
  it("sets the corner radius on the layers that have one", () => {
    expect(p.setRadius(8)(target())).toEqual({ cornerRadius: 8 });
    expect(p.setRadius(0)(target())).toEqual({ cornerRadius: null });
    expect(p.setRadius(8)(target({ type: "ellipse" }))).toBeNull();
  });
  it("clips frames", () => {
    expect(p.setClip(true)(target())).toEqual({ clip: true });
    expect(p.setClip(false)(target())).toEqual({ clip: null });
    expect(p.setClip(true)(rect())).toBeNull();
  });
  it("sets the blend mode, leaving out normal", () => {
    expect(p.setBlend("multiply")(target())).toEqual({ blendMode: "multiply" });
    expect(p.setBlend("normal")(target())).toEqual({ blendMode: null });
  });
  it("writes notes for agents, and clears them when emptied", () => {
    expect(p.setNotes("Copy under 10 words.")(target())).toEqual({
      agentNotes: "Copy under 10 words.",
    });
    expect(p.setNotes("   ")(target())).toEqual({ agentNotes: null });
  });
});

describe("fills", () => {
  it("adds a paint at the top of the list", () => {
    expect(p.addFill(solid("#111111"))(target({ fills: [solid("#222222")] }))).toEqual({
      fills: [solid("#222222"), solid("#111111")],
    });
  });
  it("removes a paint, and the key when none is left", () => {
    expect(p.removeFill(0)(target({ fills: [solid("#222222"), solid("#111111")] }))).toEqual({
      fills: [solid("#111111")],
    });
    expect(p.removeFill(0)(target({ fills: [solid("#222222")] }))).toEqual({ fills: null });
  });
  it("changes one paint", () => {
    expect(
      p.updateFill(
        0,
        (x) => ({ ...x, color: "#333333" }) as unknown as Paint,
      )(target({ fills: [solid("#222222")] })),
    ).toEqual({ fills: [solid("#333333")] });
  });
  it("is nothing for a paint that is not there, or when nothing changed", () => {
    expect(p.updateFill(3, (x) => x)(target({ fills: [solid("#222222")] }))).toBeNull();
    expect(p.updateFill(0, (x) => x)(target({ fills: [solid("#222222")] }))).toBeNull();
  });
  it("moves a paint", () => {
    expect(p.moveFill(0, 1)(target({ fills: [solid("#1"), solid("#2")] }))).toEqual({
      fills: [solid("#2"), solid("#1")],
    });
  });
  it("sets the colour of a solid paint, or binds it", () => {
    expect(p.setFillColour(0, "#444444")(target({ fills: [solid("#222222")] }))).toEqual({
      fills: [solid("#444444")],
    });
    expect(
      p.setFillColour(0, { $var: "c1" } as never)(target({ fills: [solid("#222222")] })),
    ).toEqual({
      fills: [{ type: "solid", color: { $var: "c1" } }],
    });
    expect(
      p.setFillColour(0, "#444444")(target({ fills: [{ type: "image", asset: "a" }] })),
    ).toBeNull();
  });
});

describe("stroke", () => {
  const stroke = (extra: Record<string, unknown> = {}) =>
    ({ paints: [solid("#234D3C")], width: 1, ...extra }) as unknown as Stroke;
  it("adds a stroke once", () => {
    expect(p.addStroke("#234D3C")(target())).toEqual({ stroke: stroke() });
    expect(p.addStroke("#234D3C")(target({ stroke: stroke() }))).toBeNull();
  });
  it("removes it", () => {
    expect(p.removeStroke(target({ stroke: stroke() }))).toEqual({ stroke: null });
    expect(p.removeStroke(target())).toBeNull();
  });
  it("changes its settings", () => {
    expect(p.updateStroke((s) => ({ ...s, width: 2 }))(target({ stroke: stroke() }))).toEqual({
      stroke: stroke({ width: 2 }),
    });
    expect(p.updateStroke((s) => ({ ...s, width: 2 }))(target())).toBeNull();
  });
  it("changes one of its paints", () => {
    expect(p.updateStrokePaint(0, () => solid("#000000"))(target({ stroke: stroke() }))).toEqual({
      stroke: stroke({ paints: [solid("#000000")] }),
    });
  });
});

describe("effects", () => {
  const drop = {
    type: "drop-shadow",
    x: 0,
    y: 4,
    blur: 16,
    color: "#00000040",
  } as unknown as Effect;
  const blur = { type: "layer-blur", radius: 8 } as unknown as Effect;
  it("adds at the end", () => {
    expect(p.addEffectTo(blur)(target({ effects: [drop] }))).toEqual({ effects: [drop, blur] });
  });
  it("removes, and clears the key when none is left", () => {
    expect(p.removeEffectAt(0)(target({ effects: [drop, blur] }))).toEqual({ effects: [blur] });
    expect(p.removeEffectAt(0)(target({ effects: [drop] }))).toEqual({ effects: null });
  });
  it("changes one", () => {
    expect(
      p.updateEffectAt(
        1,
        (e) => ({ ...e, radius: 3 }) as unknown as Effect,
      )(target({ effects: [drop, blur] })),
    ).toEqual({ effects: [drop, { type: "layer-blur", radius: 3 }] });
  });
});

describe("combine", () => {
  it("is one patch per layer from several edits, the later one winning where they overlap", () => {
    const both = p.combine(
      p.setSizeMode("width", "hug"),
      p.setSizeMode("height", "fill"),
      p.setSize("width", 9),
    );
    expect(both(target())).toEqual({ width: 9, height: "fill" });
  });
  it("skips an edit that does not apply and is nothing when none do", () => {
    expect(p.combine(p.setClip(true), p.setGap(8))(rect())).toBeNull();
    expect(p.combine(p.setClip(true), p.setOpacity(0.5))(rect())).toEqual({ opacity: 0.5 });
  });
});
