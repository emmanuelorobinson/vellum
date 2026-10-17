import type { Point } from "@vellum/types-ts";
import type { Box, Viewport } from "./tool-types";
export const add = (a: Point, b: Point): Point => ({ x: a.x + b.x, y: a.y + b.y });
export const sub = (a: Point, b: Point): Point => ({ x: a.x - b.x, y: a.y - b.y });
export const mul = (a: Point, n: number): Point => ({ x: a.x * n, y: a.y * n });
export const mix = (a: Point, b: Point, t: number) => add(mul(a, 1 - t), mul(b, t));
export const distance = (a: Point, b: Point) => Math.hypot(a.x - b.x, a.y - b.y);
export const finitePoint = (p: Point) => Number.isFinite(p.x) && Number.isFinite(p.y);
export const rect = (a: Point, b: Point): Box => ({
  x: Math.min(a.x, b.x),
  y: Math.min(a.y, b.y),
  width: Math.abs(a.x - b.x),
  height: Math.abs(a.y - b.y),
});
export const world = (p: Point, v: Viewport): Point => ({
  x: (p.x - v.x) / v.zoom,
  y: (p.y - v.y) / v.zoom,
});
export function union(boxes: Box[]): Box | null {
  if (!boxes.length) return null;
  const x = Math.min(...boxes.map((b) => b.x)),
    y = Math.min(...boxes.map((b) => b.y));
  return {
    x,
    y,
    width: Math.max(...boxes.map((b) => b.x + b.width)) - x,
    height: Math.max(...boxes.map((b) => b.y + b.height)) - y,
  };
}
export function zoomAt(v: Viewport, screen: Point, zoom: number): Viewport {
  zoom = Math.max(0.01, Math.min(256, zoom));
  const w = world(screen, v);
  return { x: screen.x - w.x * zoom, y: screen.y - w.y * zoom, zoom };
}
