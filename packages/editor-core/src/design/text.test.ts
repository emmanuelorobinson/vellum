// TEST DATA: the Juniper heading of design screen C14.
import type { Document, Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import type { SceneItem } from "../tool-types";
import {
  alignPatch,
  contentPatch,
  fontPatch,
  fontsInUse,
  letterSpacingPatch,
  lineHeightPatch,
  plainText,
  readText,
  resizePatch,
  sizePatch,
  stylePatch,
  weightPatch,
} from "./text";

const run = (extra: Record<string, unknown> = {}) => ({
  text: "What would you like to grow?",
  font: "Inter",
  weight: 500,
  size: 24,
  lineHeight: 28.8,
  letterSpacing: -0.5,
  ...extra,
});
const text = (runs: unknown[], extra: Record<string, unknown> = {}, id = "t1"): SceneItem => ({
  node: {
    type: "text",
    id,
    name: id,
    width: 160,
    height: 40,
    content: runs,
    ...extra,
  } as unknown as Node,
  bounds: { x: 0, y: 0, width: 160, height: 40 },
  children: [],
  parent: null,
});
const doc = {
  meta: { formatVersion: 1, name: "t", createdWith: "t" },
  styles: {
    s1: {
      type: "text",
      name: "Heading / Medium",
      font: "Inter",
      weight: 500,
      size: 24,
      lineHeight: 28.8,
      letterSpacing: -0.5,
    },
    s2: { type: "effect", name: "Soft", effects: [] },
  },
  variables: { v1: { type: "number", name: "type/size", values: [{ value: 24 }] } },
  pages: [],
} as unknown as Document;
const ctx = { available: (f: string) => f === "Inter", fallback: "Inter" };
const read = (items: SceneItem[]) => readText(items, doc, {}, ctx);

describe("readText", () => {
  it("reads one run the way the Text section shows it", () => {
    const t = read([text([run()])]);
    expect(t.font).toEqual({ kind: "value", value: "Inter" });
    expect(t.weight).toEqual({ kind: "value", value: 500 });
    expect(t.size).toEqual({ kind: "value", value: 24 });
    expect(t.lineHeight).toEqual({ kind: "value", value: 120 });
    expect(t.letterSpacing).toEqual({ kind: "value", value: -0.5 });
    expect(t.align).toEqual({ kind: "value", value: "left", isDefault: true });
    expect(t.resize).toEqual({ kind: "value", value: "fixed", isDefault: true });
    expect(t.content).toBe("What would you like to grow?");
  });
  it("shows the defaults for what the file leaves out", () => {
    const t = read([text([{ text: "x", font: "Inter", size: 12 }])]);
    expect(t.weight).toEqual({ kind: "value", value: 400, isDefault: true });
    expect(t.lineHeight).toEqual({ kind: "value", value: "auto", isDefault: true });
    expect(t.letterSpacing).toEqual({ kind: "value", value: 0, isDefault: true });
  });
  it("is mixed across runs and across layers", () => {
    const t = read([text([run(), run({ weight: 700 })]), text([run({ size: 12 })], {}, "t2")]);
    expect(t.weight).toEqual({ kind: "mixed" });
    expect(t.size).toEqual({ kind: "mixed" });
    expect(t.font).toEqual({ kind: "value", value: "Inter" });
    expect(t.content).toBeNull();
  });
  it("shows a bound size as bound", () => {
    const t = read([text([run({ size: { $var: "v1" } })])]);
    expect(t.size).toMatchObject({ kind: "bound", variable: { name: "type/size", value: 24 } });
  });
  it("names the text style, and lists the text styles of the document, not the effect styles", () => {
    expect(read([text([run()], { textStyle: "s1" })]).style).toEqual({
      kind: "value",
      value: "s1",
    });
    expect(read([text([run()])]).style).toEqual({ kind: "value", value: null });
    expect(read([text([run()])]).styles).toEqual([{ id: "s1", name: "Heading / Medium" }]);
  });
  it("says which fonts are missing, and shows the one in use instead", () => {
    const missing = read([text([run({ font: "Sprig Sans" })])]);
    expect(missing.missing).toEqual(["Sprig Sans"]);
    expect(missing.font).toEqual({ kind: "value", value: "Inter" });
    expect(read([text([run()])]).missing).toEqual([]);
  });
  it("trusts every font when the host cannot say which exist", () => {
    const t = readText([text([run({ font: "Sprig Sans" })])], doc, {}, { fallback: "Inter" });
    expect(t.missing).toEqual([]);
  });
});

describe("fontsInUse", () => {
  it("lists each family once", () => {
    expect(fontsInUse([text([run(), run({ font: "Fraunces" })]), text([run()], {}, "t2")])).toEqual(
      ["Inter", "Fraunces"],
    );
  });
});

describe("plainText", () => {
  it("joins the runs", () => {
    expect(plainText(text([run({ text: "Hello " }), run({ text: "there" })]).node)).toBe(
      "Hello there",
    );
  });
});

describe("text patches", () => {
  const item = text([run(), run({ text: "!", weight: 700 })]);
  const runsOf = (patch: Record<string, unknown> | null) =>
    (patch as { content: Record<string, unknown>[] }).content;
  it("sets the font on every run", () => {
    expect(runsOf(fontPatch(item, "Fraunces")).map((r) => r.font)).toEqual([
      "Fraunces",
      "Fraunces",
    ]);
  });
  it("sets the weight, and leaves the key out for the regular weight", () => {
    expect(runsOf(weightPatch(item, 600)).map((r) => r.weight)).toEqual([600, 600]);
    expect(runsOf(weightPatch(item, 400)).every((r) => !("weight" in r))).toBe(true);
  });
  it("keeps the line height in proportion when the size changes", () => {
    const out = runsOf(sizePatch(item, 48));
    expect(out[0]).toMatchObject({ size: 48, lineHeight: 57.6 });
  });
  it("leaves an automatic line height automatic", () => {
    const auto = text([run({ lineHeight: undefined })]);
    expect("lineHeight" in (runsOf(sizePatch(auto, 48))[0] as object)).toBe(false);
  });
  it("sets the line height as a percentage of each run's size, or back to automatic", () => {
    expect(runsOf(lineHeightPatch(item, 150))[0]).toMatchObject({ lineHeight: 36 });
    expect("lineHeight" in (runsOf(lineHeightPatch(item, null))[0] as object)).toBe(false);
  });
  it("sets the letter spacing, and leaves the key out for 0", () => {
    expect(runsOf(letterSpacingPatch(item, 1))[0]).toMatchObject({ letterSpacing: 1 });
    expect("letterSpacing" in (runsOf(letterSpacingPatch(item, 0))[0] as object)).toBe(false);
  });
  it("sets alignment and resizing on the layer, and nothing for a layer that is not text", () => {
    expect(alignPatch(item, "center")).toEqual({ align: "center" });
    expect(alignPatch(item, "left")).toEqual({ align: null });
    expect(resizePatch(item, "auto-height")).toEqual({ resize: "auto-height" });
    expect(resizePatch(item, "fixed")).toEqual({ resize: null });
    const frame = { ...item, node: { ...item.node, type: "frame" } } as unknown as SceneItem;
    expect(alignPatch(frame, "center")).toBeNull();
    expect(fontPatch(frame, "x")).toBeNull();
  });
  it("applies a text style: its settings go into the runs and the layer remembers the style", () => {
    const patch = stylePatch(item, "s1", doc) as {
      textStyle: string;
      content: Record<string, unknown>[];
    };
    expect(patch.textStyle).toBe("s1");
    expect(patch.content.map((r) => r.text)).toEqual(["What would you like to grow?", "!"]);
    expect(patch.content[1]).toMatchObject({
      font: "Inter",
      weight: 500,
      size: 24,
      lineHeight: 28.8,
      letterSpacing: -0.5,
    });
  });
  it("removes a text style without touching the runs", () => {
    expect(stylePatch(item, null, doc)).toEqual({ textStyle: null });
  });
  it("ignores a style that is not there", () => {
    expect(stylePatch(item, "nope", doc)).toBeNull();
  });
});

describe("contentPatch", () => {
  it("replaces the characters and keeps the first run's settings", () => {
    const item = text([run({ text: "a" }), run({ text: "b", weight: 700 })]);
    expect(contentPatch(item, "Hello")).toEqual({ content: [run({ text: "Hello" })] });
  });
  it("keeps one empty run for empty text, so the layer still has a font", () => {
    expect(contentPatch(text([run()]), "")).toEqual({ content: [run({ text: "" })] });
  });
  it("is nothing when the text is the same", () => {
    expect(contentPatch(text([run()]), "What would you like to grow?")).toBeNull();
  });
});
