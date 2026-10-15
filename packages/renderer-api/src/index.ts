/** The editor consumes the versioned binary display list; no per-command JS calls. */
export const PACKAGE_NAME = "@vellum/renderer-api";
export const DISPLAY_LIST_VERSION = 1;
export interface Viewport {
  width: number;
  height: number;
  dpr: number;
}
export interface FontResource {
  handle: bigint;
  index: number;
  bytes: Uint8Array;
}
export interface ImageResource {
  id: string;
  bytes: Uint8Array;
}
export interface Frame {
  /** Cache hint only. Display coordinates already include zoom and DPR. */
  zoom?: number;
  /** Buffer from vellum-core, already transformed into device coordinates at viewport DPR. */
  displayList: Uint8Array;
  fonts: readonly FontResource[];
  images: readonly ImageResource[];
}
export type RendererState = "ready" | "context-lost" | "disposed";
export interface Renderer {
  readonly state: RendererState;
  resize(viewport: Viewport): void;
  /** Validate/prepare first. A failed frame leaves the previous prepared scene available. */
  draw(frame: Frame): void;
  /** Native and browser CPU paths produce unpremultiplied RGBA, top row first. */
  readPixels(): Uint8Array;
  dispose(): void;
}
export function checkViewport(v: Viewport): [number, number] {
  const w = Math.round(v.width * v.dpr),
    h = Math.round(v.height * v.dpr);
  if (
    ![v.width, v.height, v.dpr].every((n) => Number.isFinite(n) && n > 0) ||
    w < 1 ||
    h < 1 ||
    w > 16384 ||
    h > 16384 ||
    w * h > 64 * 1024 * 1024
  ) {
    throw new Error("Invalid renderer viewport");
  }
  return [w, h];
}
