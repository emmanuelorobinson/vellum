import type {
  Component,
  ComponentId,
  DeviceId,
  Document,
  Node,
  NodeId,
  NodeIdPath,
  NodePatch,
  Origin,
  PageId,
  Point,
  PropDef,
  Variable,
  VariableId,
  Variant,
} from "@vellum/types-ts";

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
export type DocumentShell = Required<
  Pick<Document, "meta" | "imports" | "themes" | "styles" | "fonts">
>;
export type ToolOp =
  | { type: "convertScript"; id: NodeId }
  | { type: "makeSlot"; id: NodeId; suggested: ComponentId[] }
  | { type: "setComponent"; id: ComponentId; component: Component | null }
  | { type: "setVariables"; set: Record<VariableId, Variable>; remove: VariableId[] }
  | { type: "setShell"; shell: DocumentShell }
  | { type: "insert"; to: Place; node: Node }
  | { type: "update"; id: NodeId; patch: NodePatch }
  | { type: "replace"; id: NodeId; node: Node }
  | { type: "replaceInInstance"; id: NodeId; path: NodeIdPath; node: Node }
  | { type: "scale"; id: NodeId; factor: number; anchor: Point }
  /** To another parent or page; `to.index` is the final index among the new siblings. */
  | { type: "move"; id: NodeId; to: Place }
  /** Among the node's siblings; `index` is the final index, after the node is taken out. */
  | { type: "reorder"; id: NodeId; index: number }
  /** Expands one instance into its tree; nested instances and variable links stay linked (Op::Detach). */
  | { type: "detach"; id: NodeId }
  /** A component's name, props and variants, as they should read (Op::UpdateComponent); its tree stays. */
  | {
      type: "updateComponent";
      id: ComponentId;
      name: string;
      props: Record<string, PropDef>;
      variants: Variant[];
    };
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
  /** Engine-owned nearest path target, including segments and their parameter. World coordinates/tolerance.
   * An older binding may omit it; point handles still work, segment gestures are then unavailable.
   */
  hitPath?(address: NodeIdPath, point: Point, tolerance: number): PathTarget | null;
}
