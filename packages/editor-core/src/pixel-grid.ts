import type { Viewport } from "./tool-types";

/** The pixel grid shows from 800% zoom, where one model pixel is eight screen pixels wide. */
export const PIXEL_GRID_ZOOM = 8;
const MAX_LINES = 4096;

/** Screen positions of the lines between model pixels across a canvas of `size` CSS pixels; `null` below
 * {@link PIXEL_GRID_ZOOM}. `world = (screen - {x, y}) / zoom`, so model column k is at `x + k * zoom`. */
export function pixelGridLines(
  viewport: Viewport,
  size: { width: number; height: number },
): { x: number[]; y: number[] } | null {
  if (!(viewport.zoom >= PIXEL_GRID_ZOOM)) return null;
  const axis = (offset: number, extent: number) => {
    const out: number[] = [];
    for (let k = Math.ceil(-offset / viewport.zoom); out.length < MAX_LINES; k++) {
      const at = offset + k * viewport.zoom;
      if (at > extent) break;
      out.push(at);
    }
    return out;
  };
  return { x: axis(viewport.x, size.width), y: axis(viewport.y, size.height) };
}
