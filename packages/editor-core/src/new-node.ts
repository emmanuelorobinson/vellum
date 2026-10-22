import type { Node, NodeId } from "@vellum/types-ts";
import type { Box, CreationTool } from "./tool-types";

/** Paints a new layer starts with, and the design panel's defaults for added fills, strokes and shadows.
 * Document data, not chrome colours: a person changes them like any other property. */
export const DOCUMENT_DEFAULTS = Object.freeze({
  fill: "#D9D9D9",
  stroke: "#1F1F1F",
  shadow: "#00000040",
  text: "#000000",
});

const NAMES: Record<CreationTool | "vector", string> = {
  frame: "Frame",
  rect: "Rectangle",
  ellipse: "Ellipse",
  line: "Line",
  polygon: "Polygon",
  star: "Star",
  text: "Text",
  note: "Note",
  vector: "Vector",
};

/** The layer a creation tool draws: absolute at `bounds`, sized as dragged, with the default paints. */
export function createDefaultNode(kind: CreationTool | "vector", id: NodeId, bounds: Box): Node {
  const base = {
    id,
    type: kind,
    name: NAMES[kind],
    position: { type: "absolute", x: bounds.x, y: bounds.y },
    width: bounds.width,
    height: bounds.height,
  };
  const fill = [{ type: "solid", color: DOCUMENT_DEFAULTS.fill }];
  const node: Record<string, unknown> =
    kind === "frame"
      ? { ...base, fills: [{ type: "solid", color: "#FFFFFF" }] }
      : kind === "rect" || kind === "ellipse"
        ? { ...base, fills: fill }
        : kind === "polygon"
          ? { ...base, sides: 3, fills: fill }
          : kind === "star"
            ? { ...base, points: 5, innerRatio: 0.38, fills: fill }
            : kind === "line"
              ? {
                  ...base,
                  height: 0,
                  stroke: {
                    paints: [{ type: "solid", color: DOCUMENT_DEFAULTS.stroke }],
                    width: 1,
                  },
                }
              : kind === "text"
                ? {
                    ...base,
                    width: "hug",
                    height: "hug",
                    content: [{ text: "", font: "Inter", size: 16 }],
                    fills: [{ type: "solid", color: DOCUMENT_DEFAULTS.text }],
                  }
                : kind === "note"
                  ? { ...base, content: "" }
                  : {
                      ...base,
                      path: "",
                      viewBox: {
                        x: 0,
                        y: 0,
                        width: Math.max(bounds.width, 1),
                        height: Math.max(bounds.height, 1),
                      },
                      stroke: {
                        paints: [{ type: "solid", color: DOCUMENT_DEFAULTS.stroke }],
                        width: 1,
                      },
                    };
  return node as unknown as Node;
}
