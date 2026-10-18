// TEST DATA: the Juniper documents behind design screens C13, C14, C16, C33 and C33-effects, for the browser tests.
// The engine is scripted, so these are only what the properties panel reads; nothing here applies an operation.
import type { Document, Node } from "@vellum/types-ts";

/** Colours the browser tests type and expect (the lint keeps literals out of test code outside fixtures). */
export const COLOURS = {
  typed: "#112233",
  white: "#FFFFFF",
  surface: "#FBF9F3",
  primary: "#234D3C",
  bottomRight: "#81976E",
};

/** The colours new paints start with in the test app. */
export const DESIGN_DEFAULTS = { fill: "#D9D9D9", stroke: "#1F1F1F", shadow: "#00000040" };

export type DesignScenario = "c13" | "c14" | "c16" | "c16i" | "c33" | "c33e" | "c33i";

const bounds = (width: number, height: number) => ({ metadata: { testBounds: { width, height } } });
const step2 = (extra: Record<string, unknown>) =>
  ({
    type: "frame",
    id: "step2",
    name: "Step 2",
    width: "hug",
    height: "hug",
    position: { type: "absolute", x: 320, y: 120 },
    layout: "column",
    gap: { $var: "v1" },
    padding: 24,
    cornerRadius: 8,
    fills: [{ type: "solid", color: { $var: "v2" } }],
    ...bounds(240, 440),
    ...extra,
  }) as unknown as Node;

const mesh = {
  type: "mesh",
  columns: 2,
  rows: 2,
  points: [
    { x: 0, y: 0, color: "#234D3C" },
    { x: 1, y: 0, color: "#A5C5A1" },
    { x: 0, y: 1, color: "#E5EDDE" },
    { x: 1, y: 1, color: "#81976E" },
  ],
};

const nodes = (name: DesignScenario): Node[] => {
  switch (name) {
    case "c13":
      return [
        step2({}),
        {
          type: "rect",
          id: "card",
          name: "Habit card",
          width: 100,
          height: 40,
          position: { type: "absolute", x: 600, y: 200 },
          opacity: 0.5,
          cornerRadius: 12,
        } as unknown as Node,
      ];
    case "c14":
      return [
        {
          type: "text",
          id: "title",
          name: "Title",
          width: 160,
          height: 56,
          position: { type: "absolute", x: 320, y: 200 },
          textStyle: "s1",
          fills: [{ type: "solid", color: "#234D3C" }],
          content: [
            {
              text: "What would you like to grow?",
              font: "Fraunces",
              weight: 500,
              size: 24,
              lineHeight: 28.8,
              letterSpacing: -0.5,
            },
          ],
        } as unknown as Node,
      ];
    case "c16":
      return [
        {
          type: "frame",
          id: "c1root",
          name: "Button / Juniper",
          width: 120,
          height: 40,
          position: { type: "absolute", x: 320, y: 200 },
        } as unknown as Node,
      ];
    case "c16i":
      return [
        {
          type: "instance",
          id: "cont",
          name: "Continue",
          componentId: "c1",
          width: 120,
          height: 40,
          position: { type: "absolute", x: 320, y: 200 },
          variant: { Variant: "Secondary", Size: "Medium" },
          props: { Label: "Next step", "Show icon": false },
        } as unknown as Node,
      ];
    case "c33":
      return [step2({ fills: [mesh], gap: undefined, padding: undefined, layout: undefined })];
    case "c33i":
      return [
        step2({
          fills: [{ type: "image", asset: "a1" }],
          gap: undefined,
          padding: undefined,
          layout: undefined,
        }),
      ];
    case "c33e":
      return [
        step2({
          fills: undefined,
          gap: undefined,
          padding: undefined,
          layout: undefined,
          stroke: {
            paints: [{ type: "solid", color: "#234D3C" }],
            width: [1, 1, 2, 1],
            align: "inside",
            cap: "round",
            join: "round",
            dash: [4, 4],
          },
          effects: [
            { type: "drop-shadow", x: 0, y: 4, blur: 16, color: "#00000040" },
            { type: "inner-shadow", x: 0, y: 1, blur: 4, color: "#00000040" },
            { type: "layer-blur", radius: 8, visible: false },
            { type: "background-blur", radius: 12, visible: false },
            { type: "shader", src: "grain.glsl" },
          ],
        }),
      ];
  }
};

export const designSelection = (name: DesignScenario): string => {
  switch (name) {
    case "c14":
      return "title";
    case "c16":
      return "c1root";
    case "c16i":
      return "cont";
    default:
      return "step2";
  }
};

export function designDocument(name: DesignScenario): Document {
  return {
    meta: { formatVersion: 1, name: "juniper.vellum", createdWith: "test" },
    themes: { Mode: ["Light", "Dark"] },
    variables: {
      v1: { type: "number", name: "space/4", values: [{ value: 16 }] },
      v2: {
        type: "color",
        name: "color/surface",
        values: [{ value: "#FBF9F3" }, { value: "#17181B", when: { Mode: "Dark" } }],
      },
      v3: { type: "color", name: "color/primary", values: [{ value: "#234D3C" }] },
      v4: { type: "number", name: "space/8", values: [{ value: 32 }] },
    },
    styles: {
      s1: {
        type: "text",
        name: "Heading / Medium",
        font: "Fraunces",
        weight: 500,
        size: 24,
        lineHeight: 28.8,
        letterSpacing: -0.5,
      },
    },
    components: {
      c1: {
        name: "Button / Juniper",
        props: {
          "Show icon": { type: "boolean", default: true },
          Label: { type: "text", default: "Continue" },
        },
        root: { type: "frame", id: "c1root", name: "Button / Juniper", width: 120, height: 40 },
        variants: ["Primary", "Secondary", "Disabled"].flatMap((v) =>
          ["Small", "Medium", "Large"].map((size) => ({ properties: { Variant: v, Size: size } })),
        ),
      },
      c2: {
        name: "Card",
        root: { type: "frame", id: "c2root", name: "Card", width: 100, height: 100 },
      },
    },
    pages: [{ id: "p1", name: "Onboarding", children: nodes(name) }],
  } as unknown as Document;
}
