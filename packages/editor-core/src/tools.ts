import type { Node, NodeIdPath, Point } from "@vellum/types-ts";
import { add, distance, finitePoint, mix, mul, rect, sub, union, world, zoomAt } from "./geometry";
import {
  allPoints,
  anchor,
  clonePath,
  dragPath,
  pathAction,
  serializePath,
  validTarget,
} from "./path";
import {
  type Box,
  type CreationTool,
  type EditablePath,
  type Overlay,
  type PathSession,
  type PathTarget,
  type PointerEvent,
  type SceneItem,
  TOOLS,
  type ToolContext,
  type ToolEvent,
  type ToolId,
  type ToolMode,
  type ToolOp,
  type ToolResult,
  type ToolState,
} from "./tool-types";

export function initialToolState(): ToolState {
  return {
    tool: "move",
    mode: { type: "idle" },
    selection: [],
    viewport: { x: 0, y: 0, zoom: 1 },
    preview: [],
    guides: [],
    previewBounds: [],
    marquee: null,
    temporary: null,
  };
}
const shortcuts: Record<string, ToolId> = {
  v: "move",
  k: "scale",
  f: "frame",
  r: "rect",
  o: "ellipse",
  l: "line",
  p: "pen",
  t: "text",
  n: "note",
  h: "hand",
};
const sameTarget = (a: PathTarget, b: PathTarget) =>
  a.type === b.type && a.contour === b.contour && a.index === b.index;
function selectable(item: SceneItem | undefined): item is SceneItem {
  return (
    !!item && item.selectable !== false && item.node.locked !== true && item.node.visible !== false
  );
}
function roots(ids: NodeIdPath[], ctx: ToolContext): NodeIdPath[] {
  return [...new Set(ids)].filter((id) => {
    let parent = ctx.scene.get(id)?.parent;
    const seen = new Set([id]);
    while (parent) {
      if (ids.includes(parent)) return false;
      if (seen.has(parent)) return false;
      seen.add(parent);
      parent = ctx.scene.get(parent)?.parent;
    }
    return true;
  });
}
function boxes(ids: NodeIdPath[], ctx: ToolContext): Box[] {
  return ids.flatMap((id) => {
    const item = ctx.scene.get(id);
    return item ? [item.bounds] : [];
  });
}
function reset(s: ToolState, mode: ToolMode = { type: "idle" }): ToolState {
  return { ...s, mode, preview: [], guides: [], previewBounds: [], marquee: null };
}
function overlay(s: ToolState, ctx: ToolContext): Overlay {
  const result: Overlay = {
    handles: [],
    guides: s.guides,
    measurements: s.previewBounds.length ? s.previewBounds : boxes(s.selection, ctx),
    marquee: s.marquee,
    pathAddress: null,
  };
  if (s.mode.type === "path" || s.mode.type === "pen") {
    result.pathAddress = s.mode.type === "path" ? s.mode.address : null;
    for (const [contour, c] of s.mode.path.contours.entries())
      for (const [index, p] of c.points.entries()) {
        for (const kind of ["point", "in", "out"] as const) {
          const point = kind === "point" ? p.point : p[kind];
          if (point) result.handles.push({ point, kind, target: { type: kind, contour, index } });
        }
      }
  } else {
    const b = union(result.measurements);
    if (b)
      result.handles = [
        { x: b.x, y: b.y },
        { x: b.x + b.width, y: b.y },
        { x: b.x + b.width, y: b.y + b.height },
        { x: b.x, y: b.y + b.height },
      ].map((point) => ({ point, kind: "corner" }));
  }
  return result;
}
function result(
  s: ToolState,
  ctx: ToolContext,
  ops: ToolOp[] = [],
  error?: ToolResult["error"],
): ToolResult {
  return {
    state: s,
    preview: s.preview,
    transactions:
      ops.length && !error
        ? [{ origin: { actor: { kind: "person", device: ctx.device } }, ops }]
        : [],
    overlay: overlay(s, ctx),
    ...(error ? { error } : {}),
  };
}
function write(s: ToolState, ctx: ToolContext, ops: ToolOp[], next = reset(s)): ToolResult {
  if (ctx.readOnly && ops.length) return result(reset(s), ctx, [], "read-only");
  return result(next, ctx, ops);
}
function switchTool(s: ToolState, tool: ToolId, ctx: ToolContext): ToolState {
  s = reset({ ...s, tool });
  if (tool === "scale")
    s.mode = {
      type: "scaling",
      ids: roots(s.selection, ctx),
      anchor: { x: 0.5, y: 0.5 },
      factor: 1,
    };
  return s;
}
function replacement(item: SceneItem, node: Node): ToolOp {
  return item.instance
    ? { type: "replaceInInstance", ...item.instance, node }
    : { type: "replace", id: item.node.id, node };
}
function update(item: SceneItem, patch: Record<string, unknown>): ToolOp {
  return item.instance
    ? replacement(item, { ...item.node, ...patch } as Node)
    : { type: "update", id: item.node.id, patch };
}
function scales(s: ToolState, ctx: ToolContext): ToolOp[] {
  if (s.mode.type !== "scaling" || s.mode.factor === 1) return [];
  // Scale accepts a subtree root; an occurrence descendant needs the engine's instance-aware scale
  // mapping, which is not in the core Op. Refuse rather than mutate the component definition.
  const { ids, factor, anchor } = s.mode;
  return ids.flatMap((address) => {
    const item = ctx.scene.get(address);
    return item ? [{ type: "scale" as const, id: item.node.id, factor, anchor }] : [];
  });
}
function scalePreview(s: ToolState, ctx: ToolContext): ToolResult {
  if (s.mode.type !== "scaling") return result(s, ctx);
  if (s.mode.ids.some((id) => !ctx.scene.get(id))) return result(s, ctx, [], "missing-data");
  if (s.mode.ids.some((id) => ctx.scene.get(id)?.instance)) return result(s, ctx, [], "read-only");
  const m = s.mode;
  return result(
    {
      ...s,
      preview: scales(s, ctx),
      previewBounds: boxes(m.ids, ctx).map((b) => ({
        x: b.x - b.width * (m.factor - 1) * m.anchor.x,
        y: b.y - b.height * (m.factor - 1) * m.anchor.y,
        width: b.width * m.factor,
        height: b.height * m.factor,
      })),
    },
    ctx,
  );
}
function pathNode(session: Pick<PathSession, "node" | "path">): Node {
  if (session.node.type === "vector") return { ...session.node, path: serializePath(session.path) };
  // Keep common appearance/transform/bindings, drop only primitive-specific geometry fields.
  const { type: _type, ...common } = session.node;
  const values = { ...common } as Record<string, unknown>;
  for (const key of [
    "cornerRadius",
    "sides",
    "points",
    "innerRatio",
    "innerRadius",
    "startAngle",
    "sweepAngle",
    "children",
    "op",
  ])
    delete values[key];
  const width = typeof common.width === "number" ? common.width : 1,
    height = typeof common.height === "number" ? common.height : 1;
  return {
    ...values,
    type: "vector",
    id: common.id,
    name: common.name,
    width: common.width,
    height: common.height,
    path: serializePath(session.path),
    viewBox: session.path.viewBox ?? {
      x: 0,
      y: 0,
      width: Math.max(width, 1),
      height: Math.max(height, 1),
    },
  } as Node;
}
function penNode(node: Node, path: EditablePath): Node {
  const points = path.contours.flatMap((c) =>
    c.points.flatMap((p) => [p.point, ...(p.in ? [p.in] : []), ...(p.out ? [p.out] : [])]),
  );
  const bounds = union(points.map((p) => ({ ...p, width: 0, height: 0 }))) ?? {
    x: 0,
    y: 0,
    width: 1,
    height: 1,
  };
  const viewBox = {
    ...bounds,
    width: Math.max(bounds.width, 1),
    height: Math.max(bounds.height, 1),
  };
  return {
    ...node,
    type: "vector",
    path: serializePath(path),
    position: { type: "absolute", x: viewBox.x, y: viewBox.y },
    width: viewBox.width,
    height: viewBox.height,
    viewBox,
  } as Node;
}
function pathOp(session: PathSession, ctx: ToolContext): ToolOp | null {
  const item = ctx.scene.get(session.address);
  if (!item) return null;
  const node = pathNode({ ...session, node: item.node });
  return item.instance
    ? replacement(item, node)
    : session.node.type === "vector"
      ? {
          type: "update",
          id: node.id,
          patch: { path: (node as Extract<Node, { type: "vector" }>).path },
        }
      : replacement(item, node);
}
function enterPath(s: ToolState, address: NodeIdPath, ctx: ToolContext): ToolResult {
  const item = ctx.scene.get(address),
    path = ctx.scene.path(address);
  if (!selectable(item) || !path) return result(s, ctx, [], "missing-data");
  const node = item.node;
  // viewBox must be supplied by normalized geometry for bound/hug/fill shapes.
  if (
    node.type !== "vector" &&
    !path.viewBox &&
    (typeof node.width !== "number" || typeof node.height !== "number")
  )
    return result(s, ctx, [], "missing-data");
  return result(
    reset(
      { ...s, tool: "path" },
      { type: "path", address, node, path: clonePath(path), selected: [], pen: false },
    ),
    ctx,
  );
}
function snapPoint(
  p: Point,
  s: ToolState,
  ctx: ToolContext,
): { point: Point; guides: ToolState["guides"] } {
  const snap = ctx.scene.snap({ ...p, width: 0, height: 0 }, s.selection, {
    ...ctx.snap,
    zoom: s.viewport.zoom,
  });
  return { point: add(p, snap.offset), guides: snap.guides };
}
function creation(s: ToolState, e: PointerEvent, ctx: ToolContext): ToolState {
  if (s.mode.type !== "creating") return s;
  const m = s.mode,
    snapped = snapPoint(world(e.point, s.viewport), s, ctx);
  const a = ctx.scene.toParent(m.to, m.start);
  let b = ctx.scene.toParent(m.to, snapped.point);
  if (e.shift && m.kind !== "line") {
    const delta = sub(b, a),
      size = Math.max(Math.abs(delta.x), Math.abs(delta.y));
    b = add(a, { x: Math.sign(delta.x || 1) * size, y: Math.sign(delta.y || 1) * size });
  }
  const box = rect(a, b);
  let node = {
    ...m.node,
    position: { type: "absolute", x: box.x, y: box.y },
    width: box.width,
    height: box.height,
  } as Node;
  if (m.kind === "line") {
    let delta = sub(b, a);
    if (e.shift) {
      const angle = (Math.round(Math.atan2(delta.y, delta.x) / (Math.PI / 4)) * Math.PI) / 4;
      delta = {
        x: Math.cos(angle) * Math.hypot(delta.x, delta.y),
        y: Math.sin(angle) * Math.hypot(delta.x, delta.y),
      };
    }
    const length = Math.hypot(delta.x, delta.y);
    // Rotation in the model is about the box centre; place that centre at the segment midpoint.
    const centre = add(a, mul(delta, 0.5));
    node = {
      ...node,
      position: { type: "absolute", x: centre.x - length / 2, y: centre.y },
      width: length,
      height: 0,
      rotation: (Math.atan2(delta.y, delta.x) * 180) / Math.PI,
    };
  }
  if (e.alt && m.kind !== "line")
    node = {
      ...node,
      position: { type: "absolute", x: a.x - box.width, y: a.y - box.height },
      width: box.width * 2,
      height: box.height * 2,
    };
  return {
    ...s,
    mode: { ...m, node },
    preview: [{ type: "insert", to: m.to, node }],
    guides: snapped.guides,
    previewBounds: [rect(m.start, snapped.point)],
  };
}
function moves(s: ToolState, e: PointerEvent, ctx: ToolContext): ToolResult {
  const m = s.mode;
  if (m.type !== "dragging") return result(s, ctx);
  const current = world(e.point, s.viewport),
    start = world(m.start, s.viewport);
  if (m.marquee) {
    const box = rect(start, current),
      ids = ctx.scene
        .hitRect(box, m.deep ? "deepest" : "topLevel", "intersects")
        .filter((id) => selectable(ctx.scene.get(id)));
    return result(
      { ...s, selection: [...new Set([...(m.additive ? m.before : []), ...ids])], marquee: box },
      ctx,
    );
  }
  const bounds = union(boxes(m.ids, ctx));
  if (!bounds || m.ids.some((id) => !ctx.scene.get(id)))
    return result(reset(s), ctx, [], "missing-data");
  let delta = sub(current, start);
  if (e.shift)
    delta = Math.abs(delta.x) >= Math.abs(delta.y) ? { x: delta.x, y: 0 } : { x: 0, y: delta.y };
  const moving = { ...bounds, x: bounds.x + delta.x, y: bounds.y + delta.y };
  const snap = ctx.scene.snap(moving, m.ids, { ...ctx.snap, zoom: s.viewport.zoom });
  delta = add(delta, snap.offset);
  if (e.shift)
    delta =
      Math.abs(current.x - start.x) >= Math.abs(current.y - start.y)
        ? { x: delta.x, y: 0 }
        : { x: 0, y: delta.y };
  const preview: ToolOp[] = [];
  for (const id of m.ids) {
    const item = ctx.scene.get(id);
    if (!item) return result(reset(s), ctx, [], "missing-data");
    const localDelta = sub(
      ctx.scene.toParent(id, add(start, delta)),
      ctx.scene.toParent(id, start),
    );
    // Auto-layout positions are resolved from Scene rather than guessed from raw node fields.
    const base =
      item.node.position?.type === "absolute"
        ? item.node.position
        : ctx.scene.toParent(id, { x: item.bounds.x, y: item.bounds.y });
    const position = add(base, localDelta);
    if (delta.x || delta.y)
      preview.push(update(item, { position: { type: "absolute", ...position } }));
  }
  return result(
    {
      ...s,
      preview,
      guides: snap.guides,
      previewBounds: boxes(m.ids, ctx).map((b) => ({ ...b, x: b.x + delta.x, y: b.y + delta.y })),
    },
    ctx,
  );
}
function restoreTemporary(s: ToolState): ToolState {
  if (!s.temporary) return s;
  if (s.temporary.held.length)
    return { ...s, tool: s.temporary.held.at(-1) === " " ? "hand" : "zoom" };
  return { ...s, ...s.temporary.suspended, tool: s.temporary.base, temporary: null };
}
function finish(s: ToolState, ctx: ToolContext): ToolResult {
  const m = s.mode;
  if (m.type === "scaling") {
    if (m.ids.some((id) => !ctx.scene.get(id))) return result(reset(s), ctx, [], "missing-data");
    if (m.ids.some((id) => ctx.scene.get(id)?.instance))
      return result(reset({ ...s, tool: "move" }), ctx, [], "read-only");
    return write(s, ctx, scales(s, ctx), reset({ ...s, tool: "move" }));
  }
  if (m.type === "pen") {
    if ((m.path.contours[0]?.points.length ?? 0) < 2)
      return result(reset({ ...s, tool: "move" }), ctx);
    const node = penNode(m.node, m.path);
    return write(
      s,
      ctx,
      [{ type: "insert", node, to: m.to }],
      reset({ ...s, tool: "move", selection: [node.id] }),
    );
  }
  if (m.type === "path") return result(reset({ ...s, tool: "move" }), ctx);
  if (m.type === "editing") {
    const content = m.node.type === "text" || m.node.type === "note" ? m.node.content : null;
    if (JSON.stringify(content) === JSON.stringify(m.original))
      return result(reset({ ...s, tool: "move" }), ctx);
    const item = m.address ? ctx.scene.get(m.address) : undefined;
    if (m.address && !item) return result(reset(s), ctx, [], "missing-data");
    const op: ToolOp = item
      ? update(item, { content })
      : { type: "insert", to: m.to, node: m.node };
    return write(s, ctx, [op], reset({ ...s, tool: "move", selection: [m.address ?? m.node.id] }));
  }
  return result(s, ctx);
}
function cancel(s: ToolState): ToolState {
  const m = s.mode;
  if (m.type === "path" && m.drag)
    return reset(s, { ...m, path: m.drag.original, drag: undefined });
  if (m.type === "dragging" && m.marquee) return reset({ ...s, selection: m.before });
  if (m.type === "panning" || m.type === "zooming")
    return restoreTemporary(reset({ ...s, viewport: m.viewport }));
  return restoreTemporary(reset(s));
}

/** All events are normalized, screen coordinates in, world/viewBox geometry out. No DOM or effects. */
export function transitionTool(state: ToolState, event: ToolEvent, ctx: ToolContext): ToolResult {
  if (
    ("point" in event && !finitePoint(event.point)) ||
    !Number.isFinite(state.viewport.zoom) ||
    state.viewport.zoom <= 0
  )
    return result(state, ctx, [], "invalid-input");
  const r = step(state, event, ctx);
  // Arithmetic overflow must never produce corrupt geometry, including in local previews.
  const finite = (v: unknown): boolean =>
    typeof v === "number"
      ? Number.isFinite(v)
      : Array.isArray(v)
        ? v.every(finite)
        : v !== null && typeof v === "object"
          ? Object.values(v).every(finite)
          : true;
  return finite([r.state, r.transactions]) ? r : result(state, ctx, [], "invalid-input");
}
function step(s: ToolState, e: ToolEvent, ctx: ToolContext): ToolResult {
  if ("canvasFocus" in e && e.canvasFocus === false && e.type !== "keyUp") return result(s, ctx);
  if (e.type === "cancel") return result(cancel(s), ctx);
  if (e.type === "finish") return finish(s, ctx);
  if (e.type === "tool") {
    if (!TOOLS.includes(e.tool)) return result(s, ctx, [], "invalid-input");
    if (s.mode.type === "path" && (e.tool === "pen" || e.tool === "move" || e.tool === "path"))
      return result(
        {
          ...s,
          tool: e.tool === "pen" ? "pen" : "path",
          mode: { ...s.mode, pen: e.tool === "pen" },
        },
        ctx,
      );
    if (e.tool === "path")
      return s.selection.length === 1 && s.selection[0]
        ? enterPath(s, s.selection[0], ctx)
        : result(s, ctx, [], "missing-data");
    return result(switchTool({ ...s, temporary: null }, e.tool, ctx), ctx);
  }
  if (e.type === "keyUp") {
    const key = e.key.toLowerCase();
    if ((key === " " || key === "z") && s.temporary) {
      const temporary = { ...s.temporary, held: s.temporary.held.filter((k) => k !== key) };
      s = { ...s, temporary };
      if (s.mode.type === "idle")
        s = temporary.held.length
          ? { ...s, tool: temporary.held.at(-1) === " " ? "hand" : "zoom" }
          : restoreTemporary(s);
    }
    return result(s, ctx);
  }
  if (e.type === "keyDown") {
    const key = e.key.toLowerCase();
    if (e.repeat) return result(s, ctx);
    if (key === "escape") {
      if (s.mode.type === "idle") return result({ ...s, selection: [] }, ctx);
      if (s.mode.type === "path" && !s.mode.drag) return finish(s, ctx);
      return result(cancel(s), ctx);
    }
    if (s.mode.type === "editing") return result(s, ctx); // Text input/IME belongs to the text editor.
    if (key === "enter") {
      if (s.mode.type !== "idle") return finish(s, ctx);
      if (e.shift) {
        const parents = s.selection.flatMap((id) => {
          const p = ctx.scene.get(id)?.parent;
          return p ? [p] : [];
        });
        return result({ ...s, selection: [...new Set(parents)] }, ctx);
      }
      const children = s.selection
        .flatMap((id) => ctx.scene.get(id)?.children ?? [])
        .filter((id) => selectable(ctx.scene.get(id)));
      if (children.length) return result({ ...s, selection: children }, ctx);
      return s.selection.length === 1 && s.selection[0]
        ? enterPath(s, s.selection[0], ctx)
        : result(s, ctx);
    }
    if (s.mode.type === "path") {
      if (key === "v" && !e.command)
        return result({ ...s, tool: "path", mode: { ...s.mode, pen: false } }, ctx);
      if (key === "p" && !e.command)
        return result({ ...s, tool: "pen", mode: { ...s.mode, pen: true } }, ctx);
      if (key === "a" && e.command)
        return result({ ...s, mode: { ...s.mode, selected: allPoints(s.mode.path) } }, ctx);
      if (key === "x" || key === "delete" || key === "backspace" || (key === "j" && e.command))
        return step(
          s,
          { type: "pathAction", action: key === "x" ? "cut" : key === "j" ? "connect" : "delete" },
          ctx,
        );
    }
    if (key === "a" && e.command)
      return result(
        { ...s, selection: ctx.scene.roots().filter((id) => selectable(ctx.scene.get(id))) },
        ctx,
      );
    if (
      !e.command &&
      !e.alt &&
      !e.shift &&
      (key === " " || key === "z") &&
      (s.mode.type === "idle" ||
        (s.mode.type === "path" && !s.mode.drag) ||
        (s.mode.type === "pen" && !s.mode.dragging) ||
        (s.mode.type === "scaling" && !s.mode.drag) ||
        s.temporary)
    ) {
      const temporary = s.temporary ?? {
        held: [],
        base: s.tool,
        suspended: {
          mode: s.mode,
          preview: s.preview,
          guides: s.guides,
          previewBounds: s.previewBounds,
          marquee: s.marquee,
        },
      };
      return result(
        {
          ...reset(s),
          tool: key === " " ? "hand" : "zoom",
          temporary: { ...temporary, held: [...new Set<" " | "z">([...temporary.held, key])] },
        },
        ctx,
      );
    }
    const tool = !e.command && !e.alt && !e.shift ? shortcuts[key] : undefined;
    return tool ? step(s, { type: "tool", tool }, ctx) : result(s, ctx);
  }
  if (e.type === "scaleFactor" || e.type === "scaleAnchor") {
    if (s.mode.type !== "scaling") return result(s, ctx);
    if (e.type === "scaleAnchor") {
      if (
        !finitePoint(e.anchor) ||
        e.anchor.x < 0 ||
        e.anchor.x > 1 ||
        e.anchor.y < 0 ||
        e.anchor.y > 1
      )
        return result(s, ctx, [], "invalid-input");
      s = { ...s, mode: { ...s.mode, anchor: e.anchor } };
    } else {
      const text = String(e.factor).trim(),
        percent = text.endsWith("%"),
        number = percent ? text.slice(0, -1).trim() : text;
      const factor = Number(number) / (percent ? 100 : 1);
      if (!number || !Number.isFinite(factor) || factor <= 0)
        return result(s, ctx, [], "invalid-input");
      s = { ...s, mode: { ...s.mode, factor } };
    }
    return scalePreview(s, ctx);
  }
  if (e.type === "content") {
    if (s.mode.type !== "editing") return result(s, ctx);
    const m = s.mode;
    if (
      (m.node.type === "note" && typeof e.content !== "string") ||
      (m.node.type === "text" && !Array.isArray(e.content))
    )
      return result(s, ctx, [], "invalid-input");
    const node = { ...m.node, content: e.content } as Node,
      item = m.address ? ctx.scene.get(m.address) : undefined;
    if (m.address && !item) return result(s, ctx, [], "missing-data");
    return result(
      {
        ...s,
        mode: { ...m, node },
        preview: [item ? update(item, { content: e.content }) : { type: "insert", to: m.to, node }],
      },
      ctx,
    );
  }
  if (e.type === "pathPoint") {
    const m = s.mode;
    if (m.type !== "path" || m.drag) return result(s, ctx);
    if (e.target.type !== "point" || !validTarget(m.path, e.target) || !finitePoint(e.point))
      return result(s, ctx, [], "invalid-input");
    const from = m.path.contours[e.target.contour]?.points[e.target.index]?.point;
    if (!from) return result(s, ctx, [], "missing-data");
    const path = dragPath(m.path, e.target, [e.target], from, e.point, false, false, false);
    const next = { ...m, path, selected: [e.target] };
    const op = pathOp(next, ctx);
    if (!op) return result(s, ctx, [], "missing-data");
    return write(s, ctx, [op], reset(s, { ...next, node: pathNode(next) }));
  }
  if (e.type === "pathAction") {
    const m = s.mode;
    if (m.type !== "path" || m.drag) return result(s, ctx);
    const selected = e.target ? [e.target] : m.selected;
    const path = pathAction(m.path, e.action, selected, e.other, e.t ?? e.target?.t ?? 0.5);
    if (!path) return result(s, ctx, [], "invalid-input");
    const session = { ...m, path, selected: [] },
      op = pathOp(session, ctx);
    if (!op) return result(s, ctx, [], "missing-data");
    return write(s, ctx, [op], reset(s, { ...session, node: pathNode(session) }));
  }
  return "point" in e ? pointer(s, e, ctx) : result(s, ctx);
}
function pointer(s: ToolState, e: PointerEvent, ctx: ToolContext): ToolResult {
  const m = s.mode,
    point = world(e.point, s.viewport);
  if (e.type === "doubleClick") {
    // The move tool opens text for typing (DESIGN.md C14 route: double-click text).
    if (m.type === "idle" && s.tool === "move" && !ctx.readOnly) {
      const hit = ctx.scene.hitPoint(point, "deepest"),
        item = hit ? ctx.scene.get(hit) : undefined;
      if (hit && item?.node.type === "text" && selectable(item))
        return result(
          {
            ...s,
            mode: {
              type: "editing",
              node: item.node,
              address: hit,
              to: ctx.placement,
              original: item.node.content,
            },
            selection: [hit],
          },
          ctx,
        );
    }
    if (m.type !== "path" || !e.target) return result(s, ctx);
    const target = e.target;
    if (target.type === "point")
      return step(
        reset(s, { ...m, drag: undefined }),
        { type: "pathAction", action: "toggleSmooth", target },
        ctx,
      );
    if (target.type === "segment" && e.command)
      return step(
        reset(s, { ...m, drag: undefined }),
        { type: "pathAction", action: "straighten", target },
        ctx,
      );
    return result(s, ctx);
  }
  if (e.type === "pointerDown") {
    if (m.type === "path") {
      if (m.drag) return result(s, ctx);
      const target = e.target;
      if (!target && m.pen) {
        const path = clonePath(m.path),
          c = path.contours.at(-1);
        if (!c || c.closed)
          path.contours.push({
            closed: false,
            points: [anchor(ctx.scene.toLocal(m.address, point))],
          });
        else c.points.push(anchor(ctx.scene.toLocal(m.address, point)));
        const session = { ...m, path },
          op = pathOp(session, ctx);
        return op
          ? write(s, ctx, [op], reset(s, { ...session, node: pathNode(session) }))
          : result(s, ctx, [], "missing-data");
      }
      if (!target)
        return result(
          {
            ...s,
            mode: {
              ...m,
              box: {
                start: ctx.scene.toLocal(m.address, point),
                before: m.selected,
                additive: !!e.shift,
              },
              selected: e.shift ? m.selected : [],
            },
          },
          ctx,
        );
      if (!validTarget(m.path, target)) return result(s, ctx, [], "invalid-input");
      if (target.type === "segment" && e.alt && !e.command)
        return step(s, { type: "pathAction", action: "addPoint", target }, ctx);
      const selected = e.shift
        ? m.selected.some((t) => sameTarget(t, target))
          ? m.selected.filter((t) => !sameTarget(t, target))
          : [...m.selected, target]
        : m.selected.some((t) => sameTarget(t, target))
          ? m.selected
          : [target];
      if (target.type === "segment" && !e.command)
        return result({ ...s, mode: { ...m, selected } }, ctx);
      return result(
        {
          ...s,
          mode: {
            ...m,
            selected,
            drag: {
              start: ctx.scene.toLocal(m.address, point),
              target,
              original: clonePath(m.path),
              symmetric: !!e.command && target.type === "point",
              broken: !!e.alt,
              fixed: !!e.alt && !!e.command,
            },
          },
        },
        ctx,
      );
    }
    if (m.type === "pen") {
      if (m.dragging) return result(s, ctx);
      const snapped = snapPoint(point, s, ctx),
        local = ctx.scene.toParent(m.to, snapped.point),
        path = clonePath(m.path),
        c = path.contours[0];
      if (!c) return result(s, ctx, [], "missing-data");
      const first = c.points[0];
      if (
        c.points.length >= 3 &&
        first &&
        distance(first.point, local) * s.viewport.zoom <= ctx.dragThreshold
      ) {
        c.closed = true;
        return finish({ ...s, mode: { ...m, path } }, ctx);
      }
      c.points.push(anchor(local));
      return result({ ...s, mode: { ...m, path, dragging: true }, guides: snapped.guides }, ctx);
    }
    if (m.type === "scaling") {
      if (
        !e.scaleHandle ||
        !finitePoint(e.scaleHandle) ||
        ![0, 1].includes(e.scaleHandle.x) ||
        ![0, 1].includes(e.scaleHandle.y)
      )
        return result(s, ctx);
      const bounds = union(boxes(m.ids, ctx));
      if (!bounds) return result(s, ctx, [], "missing-data");
      return result(
        {
          ...s,
          mode: {
            ...m,
            factor: 1,
            anchor: { x: 1 - e.scaleHandle.x, y: 1 - e.scaleHandle.y },
            drag: { start: point, bounds, handle: e.scaleHandle },
          },
        },
        ctx,
      );
    }
    if (m.type !== "idle") return result(s, ctx);
    if (s.tool === "hand" || s.tool === "zoom")
      return result(
        {
          ...s,
          mode: {
            type: s.tool === "hand" ? "panning" : "zooming",
            start: e.point,
            viewport: s.viewport,
            alt: !!e.alt,
          },
        },
        ctx,
      );
    if (s.tool === "move") {
      const hit = ctx.scene.hitPoint(point, e.command ? "deepest" : "topLevel");
      const id = hit && selectable(ctx.scene.get(hit)) ? hit : null;
      return result(
        {
          ...s,
          mode: {
            type: "pressed",
            start: e.point,
            hit: id,
            before: s.selection,
            command: !!e.command,
            shift: !!e.shift,
          },
        },
        ctx,
      );
    }
    if (s.tool === "path") return result(s, ctx, [], "missing-data");
    if (ctx.readOnly) return result(s, ctx, [], "read-only");
    if (s.tool === "text" || s.tool === "note") {
      const hit = ctx.scene.hitPoint(point, "deepest"),
        item = hit ? ctx.scene.get(hit) : undefined;
      if (hit && item?.node.type === s.tool && selectable(item))
        return result(
          {
            ...s,
            mode: {
              type: "editing",
              node: item.node,
              address: hit,
              to: ctx.placement,
              original: item.node.content,
            },
            selection: [hit],
          },
          ctx,
        );
    }
    const to = ctx.placement,
      snapped = snapPoint(point, s, ctx),
      local = ctx.scene.toParent(to, snapped.point);
    const kind = s.tool === "pen" ? "vector" : (s.tool as CreationTool);
    const node = ctx.createNode(kind, ctx.nextNodeId(), { ...local, width: 0, height: 0 });
    if (node.type !== kind) return result(s, ctx, [], "invalid-input");
    if (s.tool === "pen")
      return result(
        {
          ...s,
          mode: {
            type: "pen",
            node,
            to,
            path: { contours: [{ closed: false, points: [anchor(local)] }] },
            dragging: true,
          },
          guides: snapped.guides,
        },
        ctx,
      );
    return result(
      {
        ...s,
        mode: { type: "creating", node, kind: s.tool as CreationTool, start: snapped.point, to },
        guides: snapped.guides,
      },
      ctx,
    );
  }
  if (m.type === "pressed") {
    if (distance(e.point, m.start) < ctx.dragThreshold) {
      if (e.type !== "pointerUp") return result(s, ctx);
      const selection = m.hit
        ? m.shift
          ? m.before.includes(m.hit)
            ? m.before.filter((id) => id !== m.hit)
            : [...m.before, m.hit]
          : [m.hit]
        : m.shift
          ? m.before
          : [];
      return result(reset({ ...s, selection }), ctx);
    }
    const marquee = m.command || !m.hit;
    const selection =
      m.hit && !marquee
        ? m.before.includes(m.hit)
          ? m.before
          : m.shift
            ? [...m.before, m.hit]
            : [m.hit]
        : m.before;
    s = {
      ...s,
      selection,
      mode: {
        type: "dragging",
        start: m.start,
        before: m.before,
        ids: roots(selection, ctx),
        marquee,
        deep: m.command,
        additive: m.shift,
      },
    };
    const r = moves(s, e, ctx);
    return e.type === "pointerUp" ? write(r.state, ctx, r.preview, reset(r.state)) : r;
  }
  if (m.type === "dragging") {
    const r = moves(s, e, ctx);
    return e.type === "pointerUp" ? write(r.state, ctx, r.preview, reset(r.state)) : r;
  }
  if (m.type === "creating") {
    const next = creation(s, e, ctx),
      mode = next.mode;
    if (mode.type !== "creating" || e.type !== "pointerUp") return result(next, ctx);
    if (m.kind === "text" || m.kind === "note") {
      const node =
        distance(point, m.start) * s.viewport.zoom < ctx.dragThreshold ? m.node : mode.node;
      const content = node.type === "text" || node.type === "note" ? node.content : "";
      return result(
        {
          ...next,
          mode: { type: "editing", node, address: null, to: m.to, original: content },
        },
        ctx,
      );
    }
    const moved = distance(point, m.start) * s.viewport.zoom >= ctx.dragThreshold;
    return moved
      ? write(next, ctx, next.preview, reset({ ...next, tool: "move", selection: [mode.node.id] }))
      : result(reset(s), ctx);
  }
  if (m.type === "scaling" && m.drag) {
    const a = {
      x: m.drag.bounds.x + m.drag.bounds.width * m.anchor.x,
      y: m.drag.bounds.y + m.drag.bounds.height * m.anchor.y,
    };
    const start = sub(m.drag.start, a),
      current = sub(point, a),
      length = start.x * start.x + start.y * start.y;
    const factor = length ? (current.x * start.x + current.y * start.y) / length : 1;
    if (!Number.isFinite(factor) || factor <= 0) return result(s, ctx, [], "invalid-input");
    const next = scalePreview({ ...s, mode: { ...m, factor } }, ctx);
    return e.type === "pointerUp" ? finish(next.state, ctx) : next;
  }
  if (m.type === "pen") {
    if (!m.dragging) return result(s, ctx);
    const path = clonePath(m.path),
      p = path.contours[0]?.points.at(-1);
    if (!p) return result(s, ctx);
    const local = ctx.scene.toParent(m.to, point);
    if (distance(p.point, local) * s.viewport.zoom >= ctx.dragThreshold) {
      p.out = local;
      p.in = sub(mul(p.point, 2), local);
      p.smooth = true;
    }
    const node = penNode(m.node, path);
    return result(
      {
        ...s,
        mode: { ...m, path, dragging: e.type !== "pointerUp" },
        preview: [{ type: "insert", node, to: m.to }],
      },
      ctx,
    );
  }
  if (m.type === "path" && m.box) {
    const box = rect(m.box.start, ctx.scene.toLocal(m.address, point));
    const points = allPoints(m.path).filter((t) => {
      const p = m.path.contours[t.contour]?.points[t.index]?.point;
      return (
        p && p.x >= box.x && p.x <= box.x + box.width && p.y >= box.y && p.y <= box.y + box.height
      );
    });
    const selected = [...(m.box.additive ? m.box.before : []), ...points].filter(
      (t, i, a) => a.findIndex((other) => sameTarget(t, other)) === i,
    );
    const next = {
      ...s,
      mode: { ...m, selected, box: e.type === "pointerUp" ? undefined : m.box },
      marquee: e.type === "pointerUp" ? null : box,
    };
    return result(next, ctx);
  }
  if (m.type === "path" && m.drag) {
    const d = m.drag,
      path = dragPath(
        d.original,
        d.target,
        m.selected,
        d.start,
        ctx.scene.toLocal(m.address, point),
        d.symmetric || !!e.command,
        d.broken || !!e.alt,
        d.fixed,
      );
    const session = { ...m, path },
      op = pathOp(session, ctx);
    if (!op) return result(reset(s), ctx, [], "missing-data");
    if (e.type === "pointerUp") {
      const changed = serializePath(path) !== serializePath(d.original);
      return write(
        s,
        ctx,
        changed ? [op] : [],
        reset(s, { ...session, node: changed ? pathNode(session) : session.node, drag: undefined }),
      );
    }
    return result({ ...s, mode: session, preview: [op] }, ctx);
  }
  if (m.type === "panning") {
    const delta = sub(e.point, m.start),
      viewport = { ...m.viewport, x: m.viewport.x + delta.x, y: m.viewport.y + delta.y };
    const next = { ...s, viewport };
    return result(e.type === "pointerUp" ? restoreTemporary(reset(next)) : next, ctx);
  }
  if (m.type === "zooming") {
    const area = rect(m.start, e.point);
    if (e.type !== "pointerUp")
      return result(
        { ...s, marquee: rect(world(m.start, m.viewport), world(e.point, m.viewport)) },
        ctx,
      );
    const zoom =
      distance(e.point, m.start) < ctx.dragThreshold
        ? zoomAt(m.viewport, m.start, m.viewport.zoom * (m.alt ? 0.5 : 2))
        : (() => {
            if (!area.width || !area.height) return m.viewport;
            const centre = world(mix(m.start, e.point, 0.5), m.viewport),
              factor = Math.max(
                0.01,
                Math.min(
                  256,
                  m.viewport.zoom *
                    Math.min(
                      ctx.viewportSize.width / area.width,
                      ctx.viewportSize.height / area.height,
                    ),
                ),
              );
            return {
              x: ctx.viewportSize.width / 2 - centre.x * factor,
              y: ctx.viewportSize.height / 2 - centre.y * factor,
              zoom: factor,
            };
          })();
    return result(restoreTemporary(reset({ ...s, viewport: zoom })), ctx);
  }
  return result(s, ctx);
}
