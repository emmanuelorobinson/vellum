import type { Point } from "@vellum/types-ts";
/** Invert the Scene's affine world→viewBox mapping. The Scene owns every ancestor transform. */
export function invertPathMapping(
  toLocal: (point: Point) => Point,
): ((point: Point) => Point) | null {
  const o = toLocal({ x: 0, y: 0 }),
    x = toLocal({ x: 1, y: 0 }),
    y = toLocal({ x: 0, y: 1 });
  const a = x.x - o.x,
    b = y.x - o.x,
    c = x.y - o.y,
    d = y.y - o.y,
    det = a * d - b * c;
  if (!Number.isFinite(det) || Math.abs(det) < 1e-14) return null;
  return (p) => ({
    x: (d * (p.x - o.x) - b * (p.y - o.y)) / det,
    y: (a * (p.y - o.y) - c * (p.x - o.x)) / det,
  });
}
