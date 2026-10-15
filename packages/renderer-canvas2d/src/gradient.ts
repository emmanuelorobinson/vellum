import type { RenderPaint } from "@vellum/types-ts";

type Gradient = Extract<RenderPaint, { Gradient: unknown }>["Gradient"];
/** Analytic unit-gradient sampling: inverse affine map, sRGB unpremultiplied stop interpolation. */
export function sampleGradient(
  g: Gradient,
  u: number,
  v: number,
): [number, number, number, number] {
  const [a, b, c, d, e, f] = g.transform,
    det = a * d - b * c;
  if (!Number.isFinite(det) || det === 0) throw new Error("Invalid gradient transform");
  const x = (d * (u - e) - c * (v - f)) / det,
    y = (-b * (u - e) + a * (v - f)) / det;
  let t =
    g.kind === "linear"
      ? x
      : g.kind === "radial"
        ? Math.hypot(x - 0.5, y - 0.5) * 2
        : (((Math.atan2(y - 0.5, x - 0.5) / Math.PI / 2) % 1) + 1) % 1;
  t = Math.max(0, Math.min(1, t));
  const first = g.stops[0],
    last = g.stops.at(-1);
  if (!first || !last) throw new Error("Empty gradient");
  if (t < first.position) return first.color;
  if (t >= last.position) return last.color;
  let left = first,
    right = last;
  for (let i = 1; i < g.stops.length; i++) {
    const next = g.stops[i];
    if (next && next.position > t) {
      right = next;
      break;
    }
    if (next) left = next;
  }
  const fraction =
    right.position === left.position ? 1 : (t - left.position) / (right.position - left.position);
  return left.color.map((value, i) =>
    Math.round(value + ((right.color[i] ?? value) - value) * fraction),
  ) as [number, number, number, number];
}
