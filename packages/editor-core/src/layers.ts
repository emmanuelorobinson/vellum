import type { AgentId, DeviceId, Node, NodeId, NodeKind } from "@vellum/types-ts";
import type { Place, ToolOp, ToolTransaction } from "./tool-types";

/** A small agent marker sits beside layers created in the last ten seconds (DESIGN.md C18). */
export const RECENT_AGENT_MS = 10_000;

type Parent = Place["parent"];
export type LayerOrder = "document" | "front-first";
export type DropIndicator = "above" | "below" | "inside";

export interface LayerRow {
  /** Node ids are the addresses outside an instance; descendants of instances are not listed. */
  address: NodeId;
  id: NodeId;
  name: string;
  kind: NodeKind;
  /** A Lucide icon name from DESIGN.md "Icons and motion". */
  icon: string;
  depth: number;
  parent: Parent;
  /** Position among the parent's children in document order (back to front). */
  siblingIndex: number;
  siblings: number;
  visible: boolean;
  /** Visibility is decided by a variable, so the eye cannot write a plain value. */
  visibleBound: boolean;
  hiddenByAncestor: boolean;
  locked: boolean;
  lockedByAncestor: boolean;
  component: boolean;
  instance: boolean;
  container: boolean;
  expandable: boolean;
  expanded: boolean;
  selected: boolean;
  recentAgent: AgentId | null;
}

export interface LayerInput {
  roots: readonly Node[];
  parent: Parent;
  /** Roots of main components, drawn in the Component violet. */
  componentRoots?: ReadonlySet<NodeId>;
  expanded: ReadonlySet<NodeId>;
  selection: readonly string[];
  recent?: Readonly<Record<NodeId, { agent: AgentId; at: number }>>;
  /** Injected clock, in milliseconds. */
  now: number;
  order?: LayerOrder;
}

const CONTAINERS = new Set<NodeKind>(["frame", "group", "boolean"]);
type AnyNode = Node & {
  children?: Node[];
  visible?: unknown;
  locked?: boolean;
  layout?: string;
  slot?: unknown;
};
const childrenOf = (n: Node): readonly Node[] => (n as AnyNode).children ?? [];
const isContainer = (n: Node) => CONTAINERS.has(n.type);

export function layerIcon(n: Node, componentRoot: boolean): string {
  if (componentRoot) return "component";
  const f = n as AnyNode;
  switch (n.type) {
    case "frame":
      if (f.slot) return "square-dashed";
      if (f.layout === "column") return "rows-3";
      if (f.layout === "row") return "columns-3";
      if (f.layout === "grid") return "layout-grid";
      return "frame";
    case "text":
      return "type";
    case "rect":
      return "square";
    case "ellipse":
      return "circle";
    case "polygon":
      return "pentagon";
    case "star":
      return "star";
    case "line":
      return "minus";
    case "vector":
      return "pen-tool";
    case "image":
      return "image";
    case "group":
      return "group";
    case "boolean":
      return "combine";
    case "icon":
      return "shapes";
    case "script":
      return "code";
    case "note":
      return "sticky-note";
    case "context":
      return "scroll-text";
    case "prompt":
      return "message-square-play";
    case "embed":
      return "app-window";
    case "instance":
      return "diamond";
    case "conflict":
      return "triangle-alert";
  }
}

const isBound = (v: unknown) => typeof v === "object" && v !== null && "$var" in v;

export function layerRows(input: LayerInput): LayerRow[] {
  const rows: LayerRow[] = [];
  const selected = new Set(input.selection);
  const front = input.order === "front-first";
  const walk = (
    nodes: readonly Node[],
    parent: Parent,
    depth: number,
    hidden: boolean,
    locked: boolean,
  ) => {
    const indexed = nodes.map((node, siblingIndex) => ({ node, siblingIndex }));
    for (const { node, siblingIndex } of front ? indexed.reverse() : indexed) {
      const n = node as AnyNode;
      const kids = childrenOf(node);
      const expandable = isContainer(node) && kids.length > 0;
      const expanded = expandable && input.expanded.has(node.id);
      const bound = isBound(n.visible);
      const visible = bound ? true : n.visible !== false;
      const recent = input.recent?.[node.id];
      const age = recent ? input.now - recent.at : Number.POSITIVE_INFINITY;
      rows.push({
        address: node.id,
        id: node.id,
        name: n.name,
        kind: node.type,
        icon: layerIcon(node, input.componentRoots?.has(node.id) ?? false),
        depth,
        parent,
        siblingIndex,
        siblings: nodes.length,
        visible,
        visibleBound: bound,
        hiddenByAncestor: hidden,
        locked: n.locked === true,
        lockedByAncestor: locked,
        component: input.componentRoots?.has(node.id) ?? false,
        instance: node.type === "instance",
        container: isContainer(node),
        expandable,
        expanded,
        selected: selected.has(node.id),
        recentAgent: recent && age >= 0 && age < RECENT_AGENT_MS ? recent.agent : null,
      });
      if (expanded)
        walk(
          kids,
          { type: "node", id: node.id },
          depth + 1,
          hidden || !visible,
          locked || n.locked === true,
        );
    }
  };
  walk(input.roots, input.parent, 0, false, false);
  return rows;
}

const key = (p: Parent) => `${p.type}:${p.id}`;

/** Parent and sibling lookups over one snapshot of a page's tree. */
export interface LayerIndex {
  parentOf(id: NodeId): Parent | null;
  childrenOf(parent: Parent): readonly NodeId[];
  node(id: NodeId): Node | undefined;
  /** Position in a depth-first walk, back to front: the document order. */
  order(id: NodeId): number;
  /** True when `id` is `ancestor` or inside it. */
  within(ancestor: NodeId, id: NodeId): boolean;
}

export function createLayerIndex(roots: readonly Node[], parent: Parent): LayerIndex {
  const parents = new Map<NodeId, Parent>();
  const kids = new Map<string, NodeId[]>();
  const nodes = new Map<NodeId, Node>();
  const order = new Map<NodeId, number>();
  kids.set(
    key(parent),
    roots.map((n) => n.id),
  );
  const walk = (list: readonly Node[], p: Parent) => {
    for (const n of list) {
      parents.set(n.id, p);
      nodes.set(n.id, n);
      order.set(n.id, order.size);
      const c = childrenOf(n);
      kids.set(
        key({ type: "node", id: n.id }),
        c.map((x) => x.id),
      );
      walk(c, { type: "node", id: n.id });
    }
  };
  walk(roots, parent);
  return {
    parentOf: (id) => parents.get(id) ?? null,
    childrenOf: (p) => kids.get(key(p)) ?? [],
    node: (id) => nodes.get(id),
    order: (id) => order.get(id) ?? -1,
    within(ancestor, id) {
      for (let at: NodeId | null = id; at !== null; ) {
        if (at === ancestor) return true;
        const p: Parent | null = parents.get(at) ?? null;
        at = p?.type === "node" ? p.id : null;
      }
      return false;
    },
  };
}

/** The ancestors to open, outermost first, so the layer is visible; null when it is not in the tree. */
export function revealPath(roots: readonly Node[], parent: Parent, id: NodeId): NodeId[] | null {
  const index = createLayerIndex(roots, parent);
  if (!index.node(id)) return null;
  const path: NodeId[] = [];
  for (let p = index.parentOf(id); p?.type === "node"; p = index.parentOf(p.id)) path.unshift(p.id);
  return path;
}

const origin = (device: DeviceId): ToolTransaction["origin"] => ({
  actor: { kind: "person", device },
});
const update = (device: DeviceId, id: NodeId, patch: Record<string, unknown>): ToolTransaction => ({
  origin: origin(device),
  ops: [{ type: "update", id, patch }],
});
export function renameTransaction(device: DeviceId, id: NodeId, name: string) {
  const next = name.trim();
  return next ? update(device, id, { name: next }) : null;
}
/** `null` puts a property back to its default, which is how a shown or unlocked layer is written. */
export function setVisibleTransaction(device: DeviceId, id: NodeId, visible: boolean) {
  return update(device, id, { visible: visible ? null : false });
}
export function setLockedTransaction(device: DeviceId, id: NodeId, locked: boolean) {
  return update(device, id, { locked: locked ? true : null });
}

export interface DropTarget {
  indicator: DropIndicator;
  rowIndex: number;
  /** `place.index` counts the parent's current children, before anything is taken out. */
  place: Place;
}

/**
 * Where a drag over `rows[over]` would land. `offset` is the pointer's place on the row, 0 at the top and
 * 1 at the bottom: the outer quarters are above and below, the middle is inside a layer that can hold children.
 */
export function dropTarget(input: {
  rows: readonly LayerRow[];
  index: LayerIndex;
  dragged: readonly NodeId[];
  over: number;
  offset: number;
  order?: LayerOrder;
}): DropTarget | null {
  const { rows, index, dragged, over, offset } = input;
  const row = rows[over];
  if (!row) return null;
  if (dragged.some((d) => index.within(d, row.id))) return null;
  const front = input.order === "front-first";
  const kids = index.childrenOf({ type: "node", id: row.id });
  const inside = (position: number): DropTarget => ({
    indicator: "inside",
    rowIndex: over,
    place: { parent: { type: "node", id: row.id }, index: position },
  });
  const canHold = row.container && !row.instance;
  const zone: DropIndicator = offset < 0.25 ? "above" : offset > 0.75 ? "below" : "inside";
  if (zone === "inside") return canHold ? inside(kids.length) : null;
  // The bottom edge of an open container is the start of what it shows next: its first displayed child.
  if (zone === "below" && row.expanded) return inside(front ? kids.length : 0);
  const before = (zone === "above") !== front;
  return {
    indicator: zone,
    rowIndex: over,
    place: { parent: row.parent, index: row.siblingIndex + (before ? 0 : 1) },
  };
}

/**
 * The ops that put `dragged` together at `to`, in document order. Layers inside other dragged layers go with
 * them. Moving among siblings is `reorder`; to another parent it is `move`. Layers already in place produce no op.
 */
export function planDrop(index: LayerIndex, dragged: readonly NodeId[], to: Place): ToolOp[] {
  const ordered = [...new Set(dragged)]
    .filter((id) => index.node(id))
    .sort((a, b) => index.order(a) - index.order(b));
  const roots = ordered.filter((id) => !ordered.some((o) => o !== id && index.within(o, id)));
  if (roots.length === 0) return [];
  // Never into itself or its own subtree.
  if (
    to.parent.type === "node" &&
    roots.some((id) => index.within(id, (to.parent as { id: NodeId }).id))
  )
    return [];
  const list = [...index.childrenOf(to.parent)];
  let anchor: NodeId | null = null;
  for (let i = Math.min(to.index, list.length) - 1; i >= 0; i--) {
    const id = list[i] as NodeId;
    if (!roots.includes(id)) {
      anchor = id;
      break;
    }
  }
  const ops: ToolOp[] = [];
  let working = list;
  for (const id of roots) {
    const without = working.filter((x) => x !== id);
    const at = anchor === null ? 0 : without.indexOf(anchor) + 1;
    const current = working.indexOf(id);
    const sameParent = current >= 0;
    if (sameParent) {
      if (current !== at) ops.push({ type: "reorder", id, index: at });
    } else {
      ops.push({ type: "move", id, to: { parent: to.parent, index: at } });
    }
    working = [...without.slice(0, at), id, ...without.slice(at)];
    anchor = id;
  }
  return ops;
}
