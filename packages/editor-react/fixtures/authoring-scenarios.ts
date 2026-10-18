// TEST DATA: C12/C28/C29/C30 fixture data, scripted port results and native output diagnostics.
import type { AuthoringHost, ScriptView } from "@vellum/editor-core";
import type { Document, Node } from "@vellum/types-ts";
export const AUTHORING_GUIDELINES =
  "# Starter Minimal\n\n## Layout\nUse a 4-point spacing grid. Keep one primary action per screen.\n\n## Tokens\nBind fills to color/primary and color/surface.\nUse space/4 for gaps and radius/md for cards.\n\n## Content\nName people Sam. Use Juniper for the example project.\nKeep copy under 10 words per line.\n\n## Assets\nUse the illustration slot; never draw a mascot.";
export function authoringDocument(name: string): Document {
  const slot: Node = {
    type: "frame",
    id: "slot",
    name: "Content",
    width: 312,
    height: 168,
    position: { type: "absolute", x: 24, y: 65 },
    slot: { suggested: ["habit", "stat"] },
  };
  const card: Node = {
    type: "frame",
    id: "cardroot",
    name: "Card",
    width: 360,
    height: 257,
    position: { type: "absolute", x: 524, y: 204 },
    children: [slot],
  };
  const habit: Node = { type: "frame", id: "habitroot", name: "HabitRow", width: 280, height: 36 };
  const stat: Node = { type: "frame", id: "statroot", name: "StatTile", width: 280, height: 36 };
  const script: Node = {
    type: "script",
    id: "script",
    name: "Weekly stats",
    width: 480,
    height: 384,
    position: { type: "absolute", x: 468, y: 168 },
    src: name === "c28-empty" ? "" : "design/charts/weekly.js",
    inputs: { values: "3, 5, 4, 7, 6, 8, 5", bar: { $var: "primary" }, gap: 16 },
  };
  const nodes: Node[] = name.startsWith("c28")
    ? [script]
    : name === "c29"
      ? [card]
      : name === "c29-filled"
        ? [
            {
              type: "instance",
              id: "instance",
              name: "Card",
              componentId: "card",
              width: 360,
              height: 257,
              position: { type: "absolute", x: 524, y: 204 },
              slotContent: {
                slot: [
                  {
                    type: "instance",
                    id: "habit1",
                    componentId: "habit",
                    name: "HabitRow",
                    width: 280,
                    height: 36,
                  },
                  {
                    type: "instance",
                    id: "habit2",
                    componentId: "habit",
                    name: "HabitRow",
                    width: 280,
                    height: 36,
                  },
                ],
              },
            },
          ]
        : name.startsWith("c30")
          ? [
              {
                type: "frame",
                id: "photo",
                name: "Photo card",
                width: 440,
                height: 400,
                position: { type: "absolute", x: 464, y: 160 },
                fills: [{ type: "image", asset: "a".repeat(64), fit: "fill" }],
              },
            ]
          : [
              {
                type: "frame",
                id: "frame",
                name: "Step 2",
                width: 240,
                height: 440,
                position: { type: "absolute", x: 620, y: 120 },
                agentNotes:
                  "Copy under 10 words per line. Use the illustration slot, don't draw one.",
              },
            ];
  return {
    meta: { formatVersion: 1, name: "Juniper", createdWith: "test" },
    pages: [{ id: "p1", name: "Onboarding", children: nodes }],
    variables: {
      primary: { type: "color", name: "color/primary", values: [{ value: "#234D3C" }] },
      spacing: { type: "number", name: "space/4", values: [{ value: 16 }] },
    },
    components: {
      card: { name: "Card", root: card as never },
      habit: { name: "HabitRow", root: habit as never },
      stat: { name: "StatTile", root: stat as never },
    },
  };
}
export function authoringHost(
  name: string,
  revision: () => number,
  requests: unknown[],
): AuthoringHost & { emitScript(view: ScriptView): void } {
  const inputs: import("@vellum/editor-core").ScriptInput[] = [
    { name: "values", label: "Values", kind: "string" as const },
    { name: "bar", label: "Bar color", kind: "color" as const },
    { name: "gap", label: "Gap", kind: "number" as const, min: 0, max: 100 },
  ];
  if (name === "c28-inputs")
    inputs.push(
      { name: "label", kind: "string", multiline: true, default: "First line" },
      { name: "mode", kind: "enum", options: ["bars", "lines"], default: "bars" },
      { name: "showLegend", label: "Show legend", kind: "boolean", default: false },
      { name: "source", kind: "ref", refKind: "node", default: "script" },
    );
  let view: ScriptView =
    name === "c28-error"
      ? {
          status: "error",
          inputs: [],
          message: "values.map is not a function",
          line: 18,
          excerpt: "18  values.map(drawBar)\n    ^ expected an array",
        }
      : name === "c28-empty"
        ? { status: "empty", inputs: [] }
        : { status: "ready", inputs };
  const attr = {
    provider: "Pixabay",
    author: "Example photographer",
    authorUrl: "https://pixabay.com/users/example/",
    sourceUrl: "https://pixabay.com/photos/example/",
    licence: "Content License",
    licenceUrl: "https://pixabay.com/service/license-summary/",
  };
  const preview =
    "data:image/svg+xml," +
    encodeURIComponent(
      '<svg xmlns="http://www.w3.org/2000/svg" width="296" height="108"><rect width="296" height="108" fill="#234D3C"/><circle cx="148" cy="54" r="32" fill="#A5C5A1"/></svg>',
    );
  const listeners = new Set<() => void>();
  return {
    emitScript(next) {
      view = next;
      for (const l of listeners) l();
    },
    subscribeScripts(listener) {
      listeners.add(listener);
      return { dispose: () => void listeners.delete(listener) };
    },
    scriptSnapshot: () => ({ revision: revision(), entries: { script: view } }),
    loadGuidelines: async () => ({
      name: "Starter Minimal",
      path: "design-systems/starter/guidelines.md",
      text: AUTHORING_GUIDELINES,
      version: "1",
      writable: true,
    }),
    saveGuidelines: async (r) => {
      requests.push({ type: "saveGuidelines", ...r });
      return { version: "2" };
    },
    openScript: (src) => requests.push({ type: "openScript", src }),
    chooseScript: async () => "design/charts/weekly.js",
    askScript: (address, intent) => requests.push({ type: "askScript", address, intent }),
    chooseImage: async () => ({ asset: "b".repeat(64) }),
    uploadImage: async () => ({ asset: "c".repeat(64) }),
    searchStock: async (query, page) => {
      requests.push({ type: "searchStock", query, page });
      return {
        photos: [
          { id: "plant1", description: "foliage", previewUrl: preview, attribution: attr },
          { id: "plant2", description: "succulent", previewUrl: preview, attribution: attr },
        ],
        total: 2,
      };
    },
    importStock: async (photo) => {
      requests.push({ type: "importStock", id: photo.id });
      return { asset: "d".repeat(64), attribution: photo.attribution };
    },
  };
}
