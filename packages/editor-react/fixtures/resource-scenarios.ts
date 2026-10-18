// TEST DATA: named screens from the read-only design. No sample data in product components.

import type { ResourceHost } from "@vellum/editor-core";
import type { Document, Node } from "@vellum/types-ts";
export function resourceDocument(name: string): Document {
  const text = (id: string, font: string) =>
    ({
      type: "text",
      id,
      name: id,
      width: 180,
      height: 30,
      content: [{ text: "Juniper", font, size: 24 }],
    }) as Node;
  const nodes: Node[] = [
    {
      type: "frame",
      id: "card",
      name: "Step 2",
      width: 208,
      height: 480,
      position: { type: "absolute", x: 560, y: 170 },
      children: [text("heading", "Inter")],
    },
    {
      type: "frame",
      id: "sibling",
      name: "Sibling",
      width: 208,
      height: 480,
      position: { type: "absolute", x: 800, y: 170 },
    },
  ] as Node[];
  if (name === "vector")
    nodes.push({
      type: "vector",
      id: "vector",
      name: "Path",
      width: 360,
      height: 160,
      position: { type: "absolute", x: 520, y: 290 },
      path: "M 0 100 C 80 100 90 0 250 120 C 230 0 250 160 360 60",
      viewBox: { x: 0, y: 0, width: 360, height: 160 },
      metadata: {
        testPathHits: [
          { point: { x: 95, y: 65 }, target: { type: "segment", contour: 0, index: 0, t: 0.5 } },
        ],
        testPath: {
          viewBox: { x: 0, y: 0, width: 360, height: 160 },
          contours: [
            {
              closed: false,
              points: [
                { point: { x: 0, y: 100 }, in: null, out: { x: 80, y: 100 }, smooth: false },
                {
                  point: { x: 250, y: 120 },
                  in: { x: 90, y: 0 },
                  out: { x: 230, y: 0 },
                  smooth: true,
                },
                { point: { x: 360, y: 60 }, in: { x: 250, y: 160 }, out: null, smooth: false },
              ],
            },
          ],
        },
      },
      stroke: { paints: [{ type: "solid", color: "#234D3C" }], width: 3 },
    } as Node);
  const d: Document = {
    meta: { formatVersion: 1, name: "juniper.vellum", createdWith: "test" },
    themes: {
      Mode: ["Light", "Dark"],
      ...(name === "themes" || name === "frame" ? { Device: ["Phone", "Tablet"] } : {}),
    },
    imports: { core: "../libraries/juniper.lib.vellum", mobile: "../libraries/mobile.lib.vellum" },
    fonts: { ["a".repeat(64)]: { family: "Juniper Sans" } },
    variables: {
      surface: {
        type: "color",
        name: "color/surface",
        values: [{ value: "#FFFFFF" }, { value: "#222327", when: { Mode: "Dark" } }],
      },
      primary: {
        type: "color",
        name: "color/primary",
        values: [{ value: "#234D3C" }, { value: "#A5C5A1", when: { Mode: "Dark" } }],
      },
      space: {
        type: "number",
        name: "space/4",
        values: [{ value: 16 }, { value: 24, when: { Device: "Tablet" } }],
      },
      title: {
        type: "number",
        name: "type/title",
        values: [{ value: 24 }, { value: 32, when: { Device: "Tablet" } }],
      },
    },
    pages: [{ id: "p1", name: "Onboarding", children: nodes }],
  };
  if (name === "missingDetails") {
    nodes[0] = {
      type: "instance",
      id: "card",
      name: "Cached instance",
      componentId: "cached",
      library: "core",
      width: 208,
      height: 480,
      position: { type: "absolute", x: 560, y: 170 },
    } as Node;
  }
  if (name === "variables") d.meta.name = "Starter Minimal";
  if (name === "variables")
    d.variables = {
      primary: {
        type: "color",
        name: "color/primary",
        values: [{ value: "#234D3C" }, { value: "#A8C8AD", when: { Mode: "Dark" } }],
      },
      surface: {
        type: "color",
        name: "color/surface",
        values: [{ value: "#FBF9F3" }, { value: "#202923", when: { Mode: "Dark" } }],
      },
      text: {
        type: "color",
        name: "color/text",
        values: [{ value: "#234D3C" }, { value: "#E5EDDE", when: { Mode: "Dark" } }],
      },
      button: { type: "color", name: "color/button", values: [{ $alias: "primary" }] },
      space: { type: "number", name: "space/4", values: [{ value: 16 }] },
      radius: { type: "number", name: "radius/md", values: [{ value: 8 }] },
      body: { type: "string", name: "type/body", values: [{ value: "Inter / 14" }] },
    };
  if (name === "fonts" || name === "manageFonts")
    d.pages[0]?.children?.push(
      ...Array.from({ length: 12 }, (_, i) => text(`custom${i}`, "Juniper Sans")),
      ...Array.from({ length: 6 }, (_, i) => text(`missing${i}`, "Lora")),
    );
  if (name === "frame")
    (nodes[0] as Node & { theme: Record<string, string> }).theme = { Mode: "Dark" };
  return d;
}
export const resourceHost: ResourceHost = {
  libraries: [
    { alias: "core", name: "Juniper Core", state: "ready", updated: "9 Oct 2026" },
    { alias: "mobile", name: "Mobile Patterns", state: "ready", updated: "7 Oct 2026" },
  ],
  bundled: [
    { id: "minimal", name: "Starter Minimal" },
    { id: "editorial", name: "Starter Editorial" },
    { id: "technical", name: "Starter Technical" },
  ],
  recentFonts: ["Inter", "JetBrains Mono"],
  googleFonts: ["IBM Plex Sans Arabic", "Noto Sans"],
  systemFonts: ["Helvetica Neue"],
  availableFonts: ["Inter", "JetBrains Mono", "Helvetica Neue"],
  fallbackFont: "Inter",
};

export const resourcePasteConflicts = [
  { id: "primary", name: "color/primary", existing: "#234D3C", pasted: "#375B4A", sameType: true },
  { id: "space", name: "space/4", existing: "16", pasted: "20", sameType: true },
];
