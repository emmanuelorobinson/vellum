import type { DeviceId, Node, NodeId, NodeIdPath, Point, TextRun } from "@vellum/types-ts";

export const TOOLS = [
  "move",
  "scale",
  "frame",
  "rect",
  "ellipse",
  "line",
  "polygon",
  "star",
  "pen",
  "path",
  "text",
  "hand",
  "zoom",
  "note",
] as const;
export type ToolId = (typeof TOOLS)[number];
export type CreationTool = Exclude<ToolId, "move" | "scale" | "path" | "hand" | "zoom" | "pen">;

import type {
  Box,
  EditablePath,
  PathAction,
  PathTarget,
  Place,
  SnapGuide,
  SnapOptions,
  ToolOp,
  ToolScene,
  ToolTransaction,
  Viewport,
} from "@vellum/engine-client";

export type {
  Box,
  Contour,
  EditablePath,
  PathAction,
  PathPoint,
  PathTarget,
  Place,
  SceneItem,
  SnapGuide,
  SnapOptions,
  ToolOp,
  ToolScene,
  ToolTransaction,
  Viewport,
} from "@vellum/engine-client";
export interface ToolContext {
  device: DeviceId;
  readOnly?: boolean;
  nextNodeId(): NodeId;
  scene: ToolScene;
  placement: Place;
  /** User/tool defaults (paint, font, note size); the controller invents no design tokens. */
  createNode(kind: CreationTool | "vector", id: NodeId, bounds: Box): Node;
  viewportSize: { width: number; height: number };
  dragThreshold: number;
  snap: Omit<SnapOptions, "zoom">;
}
export interface Modifiers {
  command?: boolean;
  shift?: boolean;
  alt?: boolean;
  canvasFocus?: boolean;
}
export type PointerEvent = Modifiers & {
  type: "pointerDown" | "pointerMove" | "pointerUp" | "doubleClick";
  point: Point;
  target?: PathTarget;
  scaleHandle?: Point;
};
export type ToolEvent =
  | PointerEvent
  | (Modifiers & { type: "keyDown" | "keyUp"; key: string; repeat?: boolean })
  | { type: "tool"; tool: ToolId }
  | { type: "cancel" | "finish" }
  | { type: "scaleFactor"; factor: string | number }
  | { type: "scaleAnchor"; anchor: Point }
  | { type: "content"; content: string | TextRun[] }
  | { type: "pathAction"; action: PathAction; target?: PathTarget; other?: PathTarget; t?: number };
export interface PathSession {
  address: NodeIdPath;
  node: Node;
  path: EditablePath;
  selected: PathTarget[];
  pen: boolean;
}
export type ToolMode =
  | { type: "idle" }
  | {
      type: "pressed";
      start: Point;
      hit: NodeIdPath | null;
      before: NodeIdPath[];
      command: boolean;
      shift: boolean;
    }
  | {
      type: "dragging";
      start: Point;
      ids: NodeIdPath[];
      before: NodeIdPath[];
      marquee: boolean;
      deep: boolean;
      additive: boolean;
    }
  | { type: "creating"; start: Point; node: Node; to: Place; kind: CreationTool }
  | {
      type: "editing";
      node: Node;
      address: NodeIdPath | null;
      to: Place;
      original: string | TextRun[];
    }
  | { type: "pen"; node: Node; path: EditablePath; to: Place; dragging: boolean }
  | ({
      type: "path";
      box?: { start: Point; before: PathTarget[]; additive: boolean };
      drag?: {
        start: Point;
        target: PathTarget;
        original: EditablePath;
        symmetric: boolean;
        broken: boolean;
        fixed: boolean;
      };
    } & PathSession)
  | {
      type: "scaling";
      factor: number;
      anchor: Point;
      ids: NodeIdPath[];
      drag?: { start: Point; bounds: Box; handle: Point };
    }
  | { type: "panning" | "zooming"; start: Point; viewport: Viewport; alt: boolean };
export interface ToolState {
  tool: ToolId;
  mode: ToolMode;
  selection: NodeIdPath[];
  viewport: Viewport;
  preview: ToolOp[];
  guides: SnapGuide[];
  previewBounds: Box[];
  marquee: Box | null;
  temporary: {
    held: (" " | "z")[];
    base: ToolId;
    suspended: Pick<ToolState, "mode" | "preview" | "guides" | "previewBounds" | "marquee">;
  } | null;
}
export interface Overlay {
  handles: { point: Point; kind: "corner" | "point" | "in" | "out"; target?: PathTarget }[];
  guides: SnapGuide[];
  measurements: Box[];
  marquee: Box | null;
  /** Path handles are in viewBox space; the view applies this occurrence's transform. */
  pathAddress: NodeIdPath | null;
}
export interface ToolResult {
  state: ToolState;
  preview: ToolOp[];
  transactions: ToolTransaction[];
  overlay: Overlay;
  error?: "invalid-input" | "missing-data" | "read-only";
}
