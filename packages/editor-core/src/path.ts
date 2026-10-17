import type { Point } from "@vellum/types-ts";
import { add, distance, mix, mul, sub } from "./geometry";
import type { Contour, EditablePath, PathAction, PathPoint, PathTarget } from "./tool-types";

export const anchor = (point: Point): PathPoint => ({ point, in: null, out: null, smooth: false });
export const clonePath = (path: EditablePath): EditablePath => ({
  ...(path.viewBox ? { viewBox: { ...path.viewBox } } : {}),
  contours: path.contours.map((c) => ({
    closed: c.closed,
    points: c.points.map((p) => ({
      ...p,
      point: { ...p.point },
      in: p.in && { ...p.in },
      out: p.out && { ...p.out },
    })),
  })),
});
export function segment(path: EditablePath, target: PathTarget): [PathPoint, PathPoint] | null {
  const c = path.contours[target.contour];
  if (!c || target.index < 0 || target.index >= c.points.length - (c.closed ? 0 : 1)) return null;
  const a = c.points[target.index],
    b = c.points[(target.index + 1) % c.points.length];
  return a && b ? [a, b] : null;
}
export function validTarget(path: EditablePath, t: PathTarget): boolean {
  return (
    Number.isInteger(t.contour) &&
    Number.isInteger(t.index) &&
    t.contour >= 0 &&
    t.index >= 0 &&
    (t.type === "segment" ? !!segment(path, t) : !!path.contours[t.contour]?.points[t.index])
  );
}
export function serializePath(path: EditablePath): string {
  const parts: string[] = [];
  const point = (p: Point) => `${p.x} ${p.y}`;
  for (const c of path.contours) {
    const first = c.points[0];
    if (!first) continue;
    parts.push(`M ${point(first.point)}`);
    for (let i = 0; i < c.points.length - (c.closed ? 0 : 1); i++) {
      const a = c.points[i],
        b = c.points[(i + 1) % c.points.length];
      if (!a || !b) continue;
      parts.push(
        a.out || b.in
          ? `C ${point(a.out ?? a.point)} ${point(b.in ?? b.point)} ${point(b.point)}`
          : `L ${point(b.point)}`,
      );
    }
    if (c.closed) parts.push("Z");
  }
  return parts.join(" ");
}
export function allPoints(path: EditablePath): PathTarget[] {
  return path.contours.flatMap((c, contour) =>
    c.points.map((_, index) => ({ type: "point" as const, contour, index })),
  );
}
export function smoothPoint(c: Contour, index: number, smooth: boolean) {
  const p = c.points[index];
  if (!p) return;
  p.smooth = smooth;
  if (!smooth) {
    p.in = null;
    p.out = null;
    return;
  }
  const prev =
      c.points[c.closed ? (index - 1 + c.points.length) % c.points.length : index - 1] ?? p,
    next = c.points[c.closed ? (index + 1) % c.points.length : index + 1] ?? p;
  const direction = sub(next.point, prev.point),
    length = Math.hypot(direction.x, direction.y);
  const unit = length ? mul(direction, 1 / length) : { x: 1, y: 0 };
  p.in = add(p.point, mul(unit, -distance(prev.point, p.point) / 3));
  p.out = add(p.point, mul(unit, distance(next.point, p.point) / 3));
}
/** Splits the cubic with de Casteljau: adding a point does not change its silhouette. */
export function splitSegment(path: EditablePath, target: PathTarget, t: number): boolean {
  const pair = segment(path, target),
    c = path.contours[target.contour];
  if (!c || !pair || t <= 0 || t >= 1 || !Number.isFinite(t)) return false;
  const [a, b] = pair;
  if (!a.out && !b.in) {
    c.points.splice(target.index + 1, 0, anchor(mix(a.point, b.point, t)));
    return true;
  }
  const q0 = mix(a.point, a.out ?? a.point, t),
    q1 = mix(a.out ?? a.point, b.in ?? b.point, t),
    q2 = mix(b.in ?? b.point, b.point, t);
  const r0 = mix(q0, q1, t),
    r1 = mix(q1, q2, t),
    s = mix(r0, r1, t);
  a.out = q0;
  b.in = q2;
  c.points.splice(target.index + 1, 0, {
    point: s,
    in: r0,
    out: r1,
    smooth: true,
  });
  return true;
}
function cutSegments(path: EditablePath, targets: PathTarget[]): boolean {
  if (!targets.length || targets.some((t) => t.type !== "segment" || !segment(path, t)))
    return false;
  const contours: Contour[] = [];
  for (const [contourIndex, c] of path.contours.entries()) {
    const cuts = new Set(targets.filter((t) => t.contour === contourIndex).map((t) => t.index));
    if (!cuts.size) {
      contours.push(c);
      continue;
    }
    const start = c.closed ? (Math.min(...cuts) + 1) % c.points.length : 0;
    let run: PathPoint[] = [];
    for (let i = 0; i < c.points.length; i++) {
      const index = (start + i) % c.points.length,
        point = c.points[index];
      if (point) run.push(point);
      if (cuts.has(index) || i === c.points.length - 1) {
        const first = run[0],
          last = run.at(-1);
        if (first && last) {
          first.in = null;
          last.out = null;
          contours.push({ closed: false, points: run });
        }
        run = [];
      }
    }
  }
  path.contours = contours;
  return true;
}
function reverse(c: Contour) {
  c.points.reverse();
  for (const p of c.points) [p.in, p.out] = [p.out, p.in];
}
function connect(path: EditablePath, a: PathTarget, b: PathTarget): boolean {
  const ca = path.contours[a.contour],
    cb = path.contours[b.contour];
  if (
    !ca ||
    !cb ||
    ca.closed ||
    cb.closed ||
    a.type !== "point" ||
    b.type !== "point" ||
    (a.contour === b.contour && a.index === b.index)
  )
    return false;
  const end = (c: Contour, i: number) => i === 0 || i === c.points.length - 1;
  if (!end(ca, a.index) || !end(cb, b.index)) return false;
  if (ca === cb) {
    ca.closed = true;
    return true;
  }
  if (a.index === 0) reverse(ca);
  if (b.index !== 0) reverse(cb);
  ca.points.push(...cb.points);
  path.contours.splice(b.contour, 1);
  return true;
}
export function pathAction(
  path: EditablePath,
  action: PathAction,
  targets: PathTarget[],
  other?: PathTarget,
  t = 0.5,
): EditablePath | null {
  const first = targets[0];
  if (!first || targets.some((target) => !validTarget(path, target))) return null;
  const result = clonePath(path);
  switch (action) {
    case "connect":
      return connect(result, first, other ?? targets[1] ?? first) ? result : null;
    case "cut":
      return cutSegments(result, targets) ? result : null;
    case "addPoint":
      return splitSegment(result, first, t) ? result : null;
    case "straighten": {
      const pair = segment(result, first);
      if (!pair) return null;
      pair[0].out = null;
      pair[1].in = null;
      pair[0].smooth = false;
      pair[1].smooth = false;
      return result;
    }
    case "toggleSmooth":
      for (const target of targets) {
        const c = result.contours[target.contour],
          p = c?.points[target.index];
        if (!c || !p) return null;
        smoothPoint(c, target.index, !p.smooth);
      }
      return result;
    case "delete": {
      const removed = new Set<PathPoint>();
      for (const target of targets) {
        const point = result.contours[target.contour]?.points[target.index];
        if (!point) continue;
        if (target.type === "point") removed.add(point);
        else if (target.type !== "segment") {
          point[target.type] = null;
          point.smooth = false;
        }
      }
      const segments = targets.filter((t) => t.type === "segment");
      if (segments.length) cutSegments(result, segments);
      for (const c of result.contours) {
        c.points = c.points.filter((p) => !removed.has(p));
        if (c.points.length < 3) c.closed = false;
      }
      result.contours = result.contours.filter((c) => c.points.length > 0);
      return result;
    }
  }
}
export function dragPath(
  path: EditablePath,
  target: PathTarget,
  selected: PathTarget[],
  start: Point,
  current: Point,
  symmetric: boolean,
  broken: boolean,
  fixed: boolean,
): EditablePath {
  const result = clonePath(path),
    c = result.contours[target.contour],
    p = c?.points[target.index];
  if (!c || !p) return result;
  const delta = sub(current, start);
  if (target.type === "segment") {
    const pair = segment(result, target);
    if (!pair) return result;
    const [a, b] = pair,
      t = Math.max(0.01, Math.min(0.99, target.t ?? 0.5));
    // Both controls translate equally: the curve at t follows the pointer exactly.
    const d = mul(delta, 1 / (3 * t * (1 - t)));
    a.out = add(a.out ?? mix(a.point, b.point, 1 / 3), d);
    b.in = add(b.in ?? mix(a.point, b.point, 2 / 3), d);
    if (!fixed) {
      if (a.smooth && a.in) a.in = sub(mul(a.point, 2), a.out);
      if (b.smooth && b.out) b.out = sub(mul(b.point, 2), b.in);
    }
  } else if (target.type === "point" && symmetric) {
    p.out = current;
    p.in = sub(mul(p.point, 2), current);
    p.smooth = true;
  } else if (target.type === "in" || target.type === "out") {
    p[target.type] = current;
    if (p.smooth && !broken) {
      const opposite = target.type === "in" ? "out" : "in",
        old = p[opposite];
      const length = distance(p.point, current),
        oppositeLength = old ? distance(p.point, old) : 0;
      if (length) p[opposite] = add(p.point, mul(sub(p.point, current), oppositeLength / length));
    }
    if (broken) p.smooth = false;
  } else {
    for (const t of selected) {
      if (t.type !== "point") continue;
      const q = result.contours[t.contour]?.points[t.index];
      if (!q) continue;
      q.point = add(q.point, delta);
      if (q.in) q.in = add(q.in, delta);
      if (q.out) q.out = add(q.out, delta);
    }
  }
  return result;
}
