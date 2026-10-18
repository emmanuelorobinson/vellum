// TEST DATA helper: a read-only Scene over a document snapshot for the browser tests. It answers geometry
// questions about literal positions and sizes only; it is not a layout engine and never changes a document.
import type { SceneItem, ToolScene } from "@vellum/editor-core";
import type { Document, Node, Point } from "@vellum/types-ts";

type Positioned = Node & {
  position?: { type: string; x: number; y: number };
  width: number;
  height: number;
  locked?: boolean;
  visible?: unknown;
  children?: Node[];
};

export function sceneOf(read: () => Document | null): ToolScene {
  const items = () => {
    const out = new Map<string, SceneItem & { order: number }>();
    const doc = read();
    const page = doc?.pages[0];
    let order = 0;
    const walk = (nodes: readonly Node[], parent: string | null, ox: number, oy: number) => {
      for (const node of nodes) {
        const n = node as Positioned;
        const x = ox + (n.position?.type === "absolute" ? n.position.x : 0);
        const y = oy + (n.position?.type === "absolute" ? n.position.y : 0);
        const kids = n.children ?? [];
        out.set(node.id, {
          node,
          bounds: { x, y, width: Number(n.width), height: Number(n.height) },
          children: kids.map((k) => k.id),
          parent,
          selectable: n.locked !== true && n.visible !== false,
          order: order++,
        });
        walk(kids, node.id, x, y);
      }
    };
    walk(page?.children ?? [], null, 0, 0);
    return out;
  };
  const inside = (b: SceneItem["bounds"], p: Point) =>
    p.x >= b.x && p.x <= b.x + b.width && p.y >= b.y && p.y <= b.y + b.height;
  const selectable = (i: SceneItem) => i.selectable !== false;
  return {
    get: (id) => items().get(id),
    roots: () => [...items().values()].filter((i) => i.parent === null).map((i) => i.node.id),
    hitPoint(point, depth) {
      const hits = [...items().values()]
        .filter((i) => selectable(i) && inside(i.bounds, point))
        .sort((a, b) => b.order - a.order);
      if (depth === "deepest") return hits[0]?.node.id ?? null;
      return hits.find((i) => i.parent === null)?.node.id ?? null;
    },
    hitRect(box, depth, mode) {
      return [...items().values()]
        .filter((i) => selectable(i) && (depth === "deepest" || i.parent === null))
        .filter((i) => {
          const b = i.bounds;
          return mode === "contained"
            ? b.x >= box.x &&
                b.y >= box.y &&
                b.x + b.width <= box.x + box.width &&
                b.y + b.height <= box.y + box.height
            : b.x < box.x + box.width &&
                b.x + b.width > box.x &&
                b.y < box.y + box.height &&
                b.y + b.height > box.y;
        })
        .map((i) => i.node.id);
    },
    snap: () => ({ offset: { x: 0, y: 0 }, guides: [] }),
    toParent: (_a, p) => p,
    toLocal: (_a, p) => p,
    path: () => null,
  };
}
