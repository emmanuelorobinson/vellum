// TEST DATA: strokes and effects as DESIGN.md C33 shows them.
import type { Effect, Stroke } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import {
  addEffect,
  EFFECT_KINDS,
  effectLabel,
  effectSummary,
  newEffect,
  removeEffect,
  replaceEffect,
  setBlurRadius,
  setEffectVisible,
  setShadowField,
} from "./effects";
import {
  formatDashes,
  newStroke,
  parseDashes,
  setStrokeAlign,
  setStrokeCap,
  setStrokeDash,
  setStrokeJoin,
  setStrokeSide,
  setStrokeWidth,
  strokeSides,
  strokeSummary,
} from "./stroke";

const stroke = (width: unknown, extra: Record<string, unknown> = {}) =>
  ({ paints: [{ type: "solid", color: "#234D3C" }], width, ...extra }) as unknown as Stroke;
const shadow = (extra: Record<string, unknown> = {}) =>
  ({
    type: "drop-shadow",
    x: 0,
    y: 4,
    blur: 16,
    color: "#00000040",
    ...extra,
  }) as unknown as Effect;

describe("newStroke", () => {
  it("is one visible paint of the colour given, 1 px wide, with every other setting at its default", () => {
    expect(newStroke("#234D3C")).toEqual({
      paints: [{ type: "solid", color: "#234D3C" }],
      width: 1,
    });
  });
});

describe("strokeSides and strokeSummary", () => {
  it("reads one width as four equal sides", () => {
    expect(strokeSides(stroke(2))).toEqual([2, 2, 2, 2]);
    expect(strokeSides(stroke([1, 1, 2, 1]))).toEqual([1, 1, 2, 1]);
  });
  it("says one width in px and four widths side by side, as the design does", () => {
    const resolve = (v: unknown) => (typeof v === "number" ? v : 0);
    expect(strokeSummary(stroke(1), resolve)).toBe("1 px");
    expect(strokeSummary(stroke([1, 1, 2, 1]), resolve)).toBe(
      "Top 1 · Right 1 · Bottom 2 · Left 1",
    );
    expect(strokeSummary(stroke([2, 2, 2, 2]), resolve)).toBe("2 px");
  });
});

describe("stroke widths", () => {
  it("sets all sides at once", () => {
    expect(setStrokeWidth(stroke([1, 2, 3, 4]), 3).width).toBe(3);
  });
  it("sets one side: the width becomes one per side", () => {
    expect(setStrokeSide(stroke(1), 2, 5).width).toEqual([1, 1, 5, 1]);
    expect(setStrokeSide(stroke([1, 1, 2, 1]), 0, 2).width).toEqual([2, 1, 2, 1]);
  });
  it("goes back to one width when the sides agree again", () => {
    expect(setStrokeSide(stroke([1, 1, 2, 1]), 2, 1).width).toBe(1);
  });
  it("never goes below zero", () => {
    expect(setStrokeWidth(stroke(1), -4).width).toBe(0);
    expect(setStrokeSide(stroke(1), 0, -4).width).toEqual([0, 1, 1, 1]);
  });
});

describe("align, caps and joins", () => {
  it("writes the setting, and leaves the key out when it is the default", () => {
    expect(setStrokeAlign(stroke(1), "inside")).toMatchObject({ align: "inside" });
    expect("align" in setStrokeAlign(stroke(1, { align: "inside" }), "center")).toBe(false);
    expect(setStrokeCap(stroke(1), "round")).toMatchObject({ cap: "round" });
    expect("cap" in setStrokeCap(stroke(1, { cap: "round" }), "butt")).toBe(false);
    expect(setStrokeJoin(stroke(1), "round")).toMatchObject({ join: "round" });
    expect("join" in setStrokeJoin(stroke(1, { join: "round" }), "miter")).toBe(false);
  });
});

describe("dashes", () => {
  it("reads lengths separated by commas or spaces", () => {
    expect(parseDashes("4, 4")).toEqual({ ok: true, dash: [4, 4] });
    expect(parseDashes("4 2 1")).toEqual({ ok: true, dash: [4, 2, 1] });
    expect(parseDashes("2.5,1.5")).toEqual({ ok: true, dash: [2.5, 1.5] });
  });
  it("reads nothing as a solid line", () => {
    expect(parseDashes("")).toEqual({ ok: true, dash: [] });
    expect(parseDashes("  ")).toEqual({ ok: true, dash: [] });
  });
  it("refuses negative or non-numeric lengths and an all-zero pattern", () => {
    for (const bad of ["a, b", "4, -1", "0, 0", "4;4"])
      expect(parseDashes(bad), bad).toEqual({
        ok: false,
        message: "Enter lengths such as 4, 4",
      });
  });
  it("prints a pattern the way it was typed, and a solid line as None", () => {
    expect(formatDashes([4, 4])).toBe("4, 4");
    expect(formatDashes([])).toBe("None");
    expect(formatDashes(undefined)).toBe("None");
  });
  it("stores the pattern, and no key for a solid line", () => {
    expect(setStrokeDash(stroke(1), [4, 4]).dash).toEqual([4, 4]);
    expect("dash" in setStrokeDash(stroke(1, { dash: [4, 4] }), [])).toBe(false);
  });
});

describe("effects", () => {
  it("lists the kinds of the design, in its order", () => {
    expect(EFFECT_KINDS).toEqual([
      "drop-shadow",
      "inner-shadow",
      "layer-blur",
      "background-blur",
      "shader",
    ]);
  });
  it("labels them as the design does", () => {
    expect(effectLabel(shadow())).toBe("Drop shadow");
    expect(effectLabel(shadow({ type: "inner-shadow" }))).toBe("Inner shadow");
    expect(effectLabel({ type: "layer-blur", radius: 8 } as unknown as Effect)).toBe("Layer blur");
    expect(effectLabel({ type: "background-blur", radius: 12 } as unknown as Effect)).toBe(
      "Backdrop blur",
    );
    expect(effectLabel({ type: "shader", src: "grain.glsl" } as unknown as Effect)).toBe("Shader");
  });
  it("summarises a row the way the design does, and says when it is hidden", () => {
    const num = (v: unknown) => Number(v);
    expect(effectSummary(shadow(), num)).toBe("0, 4 · blur 16");
    expect(effectSummary(shadow({ type: "inner-shadow", y: 1, blur: 4 }), num)).toBe(
      "0, 1 · blur 4",
    );
    expect(effectSummary(shadow({ spread: 2 }), num)).toBe("0, 4 · blur 16 · spread 2");
    expect(
      effectSummary({ type: "layer-blur", radius: 8, visible: false } as unknown as Effect, num),
    ).toBe("8 px · hidden");
    expect(effectSummary({ type: "background-blur", radius: 12 } as unknown as Effect, num)).toBe(
      "12 px",
    );
    expect(effectSummary(shadow({ visible: false }), num)).toBe("0, 4 · blur 16 · hidden");
    expect(effectSummary({ type: "shader", src: "grain.glsl" } as unknown as Effect, num)).toBe(
      "grain.glsl · v1.1",
    );
  });
  it("makes a new effect of each kind from the defaults it is given", () => {
    const defaults = { shadow: "#00000040" };
    expect(newEffect("drop-shadow", defaults)).toEqual({
      type: "drop-shadow",
      x: 0,
      y: 4,
      blur: 16,
      color: "#00000040",
    });
    expect(newEffect("inner-shadow", defaults)).toEqual({
      type: "inner-shadow",
      x: 0,
      y: 1,
      blur: 4,
      color: "#00000040",
    });
    expect(newEffect("layer-blur", defaults)).toEqual({ type: "layer-blur", radius: 8 });
    expect(newEffect("background-blur", defaults)).toEqual({ type: "background-blur", radius: 12 });
    expect(newEffect("shader", defaults)).toBeNull();
  });
  it("adds at the end, removes, replaces and hides without losing the settings", () => {
    const a = shadow();
    const b = { type: "layer-blur", radius: 8 } as unknown as Effect;
    expect(addEffect([a], b)).toEqual([a, b]);
    expect(removeEffect([a, b], 0)).toEqual([b]);
    expect(
      replaceEffect([a, b], 1, { type: "layer-blur", radius: 3 } as unknown as Effect)[1],
    ).toMatchObject({ radius: 3 });
    expect(setEffectVisible(a, false)).toMatchObject({ visible: false, blur: 16 });
    expect("visible" in setEffectVisible(setEffectVisible(a, false), true)).toBe(false);
  });
  it("sets a shadow's numbers and a blur's radius, never below zero for a blur", () => {
    expect(setShadowField(shadow(), "x", -3)).toMatchObject({ x: -3 });
    expect(setShadowField(shadow(), "blur", -3)).toMatchObject({ blur: 0 });
    expect(setShadowField(shadow(), "spread", 4)).toMatchObject({ spread: 4 });
    expect("spread" in setShadowField(shadow({ spread: 4 }), "spread", 0)).toBe(false);
    expect(setBlurRadius({ type: "layer-blur", radius: 8 } as unknown as Effect, -1)).toMatchObject(
      { radius: 0 },
    );
  });
});
