/** Basic on-screen renderer (S02). Geometry/fonts compile without a GPU; Canvas2D draws the plan. */
import {
  checkViewport,
  type Frame,
  type Renderer,
  type RendererState,
  type Viewport,
} from "@vellum/renderer-api";
import type { RenderPaint } from "@vellum/types-ts";
import { sampleGradient } from "./gradient";
export const PACKAGE_NAME = "@vellum/renderer-canvas2d";
type Matrix = [number, number, number, number, number, number];
type Rect = [number, number, number, number];
export type BasicCommand =
  | { op: "save" | "restore" }
  | { op: "transform"; matrix: Matrix }
  | { op: "clip"; data: string; even_odd: boolean }
  | { op: "layer"; opacity: number; blend: string }
  | { op: "path"; data: string; even_odd: boolean; paint: RenderPaint; bounds: Rect }
  | {
      op: "image";
      pixels: number[];
      size: [number, number];
      clip: Rect;
      transform: Matrix;
      opacity: number;
    };
export interface BasicScene {
  commands: BasicCommand[];
  simplifications: readonly string[];
}
export type Compiler = (frame: Frame) => BasicScene;
export interface BasicRenderer extends Renderer {
  readonly simplifications: readonly string[];
  readonly kind: "basic";
}
type Context = OffscreenCanvasRenderingContext2D;
const identity: Matrix = [1, 0, 0, 1, 0, 0];
const blends: Record<string, GlobalCompositeOperation> = {
  SrcOver: "source-over",
  Darken: "darken",
  Multiply: "multiply",
  ColorBurn: "color-burn",
  Lighten: "lighten",
  Screen: "screen",
  ColorDodge: "color-dodge",
  Plus: "lighter",
  Overlay: "overlay",
  SoftLight: "soft-light",
  HardLight: "hard-light",
  Difference: "difference",
  Exclusion: "exclusion",
  Hue: "hue",
  Saturation: "saturation",
  Color: "color",
  Luminosity: "luminosity",
};
const rgba = (c: readonly number[]) => `rgba(${c[0]},${c[1]},${c[2]},${(c[3] ?? 255) / 255})`;
function context(width: number, height: number): Context {
  const c = new OffscreenCanvas(width, height).getContext("2d");
  if (!c) throw new Error("Canvas2D unavailable");
  return c;
}
function paint(ctx: Context, p: RenderPaint, b: Rect): string | CanvasPattern {
  if ("Solid" in p) return rgba(p.Solid);
  if (!("Gradient" in p)) throw new Error("Unsupported basic paint");
  const m = ctx.getTransform(),
    w = Math.max(1, Math.min(2048, Math.ceil(b[2] * Math.hypot(m.a, m.b)))),
    h = Math.max(1, Math.min(2048, Math.ceil(b[3] * Math.hypot(m.c, m.d))));
  const image = context(w, h),
    pixels = new Uint8ClampedArray(w * h * 4);
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++)
      pixels.set(sampleGradient(p.Gradient, (x + 0.5) / w, (y + 0.5) / h), (y * w + x) * 4);
  image.putImageData(new ImageData(pixels, w, h), 0, 0);
  const pattern = ctx.createPattern(image.canvas, "no-repeat");
  if (!pattern) throw new Error("Cannot create gradient pattern");
  pattern.setTransform(new DOMMatrix([b[2] / w, 0, 0, b[3] / h, b[0], b[1]]));
  return pattern;
}
function render(scene: BasicScene, width: number, height: number): OffscreenCanvas {
  let ctx = context(width, height),
    matrix: Matrix = identity;
  type Clip = { path: Path2D; rule: CanvasFillRule; matrix: Matrix };
  let clips: Clip[] = [];
  const stack: (() => void)[] = [];
  for (const command of scene.commands) {
    switch (command.op) {
      case "save": {
        const c = ctx,
          m = matrix,
          old = clips;
        ctx.save();
        stack.push(() => {
          ctx = c;
          c.restore();
          matrix = m;
          clips = old;
        });
        clips = [...clips];
        break;
      }
      case "restore": {
        const restore = stack.pop();
        if (!restore) throw new Error("Unbalanced basic scene");
        restore();
        break;
      }
      case "transform":
        matrix = command.matrix;
        ctx.setTransform(...matrix);
        break;
      case "clip": {
        const path = new Path2D(command.data),
          rule = command.even_odd ? "evenodd" : "nonzero";
        ctx.clip(path, rule);
        clips = [...clips, { path, rule, matrix }];
        break;
      }
      case "layer": {
        const parent = ctx,
          m = matrix,
          old = clips,
          layer = context(width, height);
        for (const clip of clips) {
          layer.setTransform(...clip.matrix);
          layer.clip(clip.path, clip.rule);
        }
        layer.setTransform(...matrix);
        const blend = blends[command.blend];
        if (!blend) throw new Error("Unsupported basic blend");
        stack.push(() => {
          ctx = parent;
          matrix = m;
          clips = old;
          parent.save();
          parent.setTransform(...identity);
          parent.globalAlpha = command.opacity;
          parent.globalCompositeOperation = blend;
          parent.drawImage(layer.canvas, 0, 0);
          parent.restore();
        });
        ctx = layer;
        break;
      }
      case "path":
        ctx.fillStyle = paint(ctx, command.paint, command.bounds);
        ctx.fill(new Path2D(command.data), command.even_odd ? "evenodd" : "nonzero");
        break;
      case "image": {
        const image = context(...command.size);
        image.putImageData(
          new ImageData(new Uint8ClampedArray(command.pixels), ...command.size),
          0,
          0,
        );
        ctx.save();
        ctx.beginPath();
        ctx.rect(...command.clip);
        ctx.clip();
        ctx.transform(...command.transform);
        ctx.globalAlpha = command.opacity;
        ctx.drawImage(image.canvas, 0, 0);
        ctx.restore();
        break;
      }
    }
  }
  if (stack.length) throw new Error("Unbalanced basic scene");
  return ctx.canvas;
}
/** The compiler is injectable so contracts can use a deterministic reference fake. */
export function createRenderer(
  canvas: HTMLCanvasElement,
  compile: Compiler,
  viewport: Viewport = { width: canvas.width, height: canvas.height, dpr: 1 },
): BasicRenderer {
  const target = canvas.getContext("2d");
  if (!target) throw new Error("Canvas2D unavailable");
  let state: RendererState = "ready",
    size = checkViewport(viewport),
    simplifications: readonly string[] = [];
  canvas.width = size[0];
  canvas.height = size[1];
  const active = () => {
    if (state !== "ready") throw new Error(`Renderer is ${state}`);
  };
  return {
    kind: "basic",
    get state() {
      return state;
    },
    get simplifications() {
      return simplifications;
    },
    resize(v) {
      active();
      const next = checkViewport(v);
      if (next[0] === size[0] && next[1] === size[1]) return;
      size = next;
      canvas.width = size[0];
      canvas.height = size[1];
    },
    draw(frame) {
      active();
      const scene = compile(frame),
        staged = render(scene, ...size);
      target.setTransform(...identity);
      target.clearRect(0, 0, ...size);
      target.drawImage(staged, 0, 0);
      simplifications = scene.simplifications;
    },
    readPixels() {
      active();
      return new Uint8Array(target.getImageData(0, 0, ...size).data);
    },
    dispose() {
      state = "disposed";
      simplifications = [];
    },
  };
}
export { type BasicModule, createCompiler } from "./compiler";
