import type { Point } from "@vellum/types-ts";
import { world } from "./geometry";
import type { Box, Viewport } from "./tool-types";

export const MIN_ZOOM = 0.01;
export const MAX_ZOOM = 256;
/** The stops ⌘+ and ⌘− walk through, as ratios. */
const STOPS = [0.01, 0.02, 0.05, 0.1, 0.125, 0.25, 0.5, 1, 2, 4, 8, 16, 32, 64, 128, 256];

const clampZoom = (z: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));

/** Zoom to `zoom` keeping the world point under `screen` where it is. */
export function zoomAbout(v: Viewport, screen: Point, zoom: number): Viewport {
  const next = clampZoom(zoom);
  const w = world(screen, v);
  return { x: screen.x - w.x * next, y: screen.y - w.y * next, zoom: next };
}

export function nextZoomStep(zoom: number, direction: "in" | "out"): number {
  const eps = 1e-9;
  if (direction === "in") return STOPS.find((s) => s > zoom + eps) ?? MAX_ZOOM;
  return [...STOPS].reverse().find((s) => s < zoom - eps) ?? MIN_ZOOM;
}

/**
 * The viewport that shows `box` centred in the part of the window the docked chrome leaves free.
 * `insets` is what docked panels cover (WorkspaceLayout.canvasInsets).
 */
export function fitViewport(
  box: Box,
  size: { width: number; height: number },
  insets: { left: number; right: number },
  options: { margin?: number; maxZoom?: number } = {},
): Viewport {
  const margin = options.margin ?? 48;
  const freeX = insets.left;
  const freeW = Math.max(1, size.width - insets.left - insets.right);
  const cx = freeX + freeW / 2;
  const cy = size.height / 2;
  if (box.width <= 0 || box.height <= 0)
    return { x: cx - box.x - box.width / 2, y: cy - box.y - box.height / 2, zoom: 1 };
  const zoom = clampZoom(
    Math.min(
      options.maxZoom ?? MAX_ZOOM,
      Math.max(1, freeW - margin * 2) / box.width,
      Math.max(1, size.height - margin * 2) / box.height,
    ),
  );
  return {
    x: cx - (box.x + box.width / 2) * zoom,
    y: cy - (box.y + box.height / 2) * zoom,
    zoom,
  };
}
