import type {
  DeviceId,
  Node,
  NodeId,
  NodeIdPath,
  NodePatch,
  Origin,
  PageId,
  Point,
  TextRun,
} from "@vellum/types-ts";

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
export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}
export interface Viewport {
  x: number;
  y: number;
  zoom: number;
}
export interface Place {
  parent: { type: "page"; id: PageId } | { type: "node"; id: NodeId };
  index: number;
}
/** In-process requests corresponding to vellum_core::ops::Op, NOT a wire format or a DocStore. */
export type ToolOp =
  | { type: "insert"; to: Place; node: Node }
  | { type: "update"; id: NodeId; patch: NodePatch }
  | { type: "replace"; id: NodeId; node: Node }
  | { type: "replaceInInstance"; id: NodeId; path: NodeIdPath; node: Node }
  | { type: "scale"; id: NodeId; factor: number; anchor: Point };
export interface ToolTransaction {
  origin: Origin & { actor: { kind: "person"; device: DeviceId } };
  ops: ToolOp[];
}
export interface PathPoint {
  point: Point;
  in: Point | null;
  out: Point | null;
  smooth: boolean;
}
export interface Contour {
  points: PathPoint[];
  closed: boolean;
}
/** Cubic paths in the node's viewBox coordinates, prepared by the engine's PathOps adapter. */
export interface EditablePath {
  /** Resolved primitive geometry; required for non-literal sizes on shape-to-vector conversion. */
  viewBox?: Box;
  contours: Contour[];
}
export interface PathTarget {
  type: "point" | "segment" | "in" | "out";
  contour: number;
  index: number;
  t?: number;
}
export type PathAction = "toggleSmooth" | "cut" | "connect" | "straighten" | "addPoint" | "delete";
export interface SceneItem {
  node: Node;
  bounds: Box;
  children: NodeIdPath[];
  parent: NodeIdPath | null;
  /** A descendant of an instance must become an override, never a component-definition write. */
  instance?: { id: NodeId; path: NodeIdPath };
  /** Inherited from the resolved Scene, not only the node's own flags. */
  selectable?: boolean;
}
export interface SnapGuide {
  axis: "x" | "y";
  kind: "edge" | "centre" | "equalSpacing" | "grid";
  coordinate: number;
  targets: NodeIdPath[];
  gap: number | null;
}
export interface SnapOptions {
  search: Box;
  tolerance: number;
  edges: boolean;
  centres: boolean;
  equalSpacing: boolean;
  grid: number | null;
  zoom: number;
}
/** Read-only facade over vellum-core Scene; no second spatial index or snapping implementation. */
export interface ToolScene {
  get(address: NodeIdPath): SceneItem | undefined;
  roots(): NodeIdPath[];
  hitPoint(point: Point, depth: "topLevel" | "deepest"): NodeIdPath | null;
  hitRect(box: Box, depth: "topLevel" | "deepest", mode: "intersects" | "contained"): NodeIdPath[];
  snap(
    box: Box,
    exclude: NodeIdPath[],
    options: SnapOptions,
  ): { offset: Point; guides: SnapGuide[] };
  /** World point to parent coordinates, including inverse ancestor rotation/flip. */
  toParent(address: NodeIdPath | Place, point: Point): Point;
  /** World point to path viewBox coordinates. */
  toLocal(address: NodeIdPath, point: Point): Point;
  path(address: NodeIdPath): EditablePath | null;
}
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
