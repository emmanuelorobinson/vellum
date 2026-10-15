import {
  checkViewport,
  DISPLAY_LIST_VERSION,
  type Frame,
  type Renderer,
  type RendererState,
  type Viewport,
} from "@vellum/renderer-api";
export const PACKAGE_NAME = "@vellum/renderer-skia";
/** Thin interface to the emscripten module built from crates/vellum-wasm. No vendor SDK in JS. */
export interface SkiaModule {
  HEAPU8: Uint8Array;
  GL: {
    createContext(
      canvas: HTMLCanvasElement,
      attributes: {
        majorVersion: 2;
        minorVersion: 0;
        alpha: true;
        premultipliedAlpha: true;
        stencil: true;
        antialias: false;
      },
    ): number;
    makeContextCurrent(handle: number): boolean;
    deleteContext(handle: number): void;
  };
  _vellum_display_list_version(): number;
  _vellum_alloc(length: number): number;
  _vellum_free(pointer: number): void;
  _vellum_buffer_len(pointer: number): number;
  _vellum_error(): number;
  _vellum_renderer_init(width: number, height: number): number;
  _vellum_renderer_resize(width: number, height: number): number;
  _vellum_renderer_lost(): void;
  _vellum_renderer_dispose(): void;
  _vellum_resources_clear(): void;
  _vellum_font(low: number, high: number, index: number, buffer: number): number;
  _vellum_image(id: number, buffer: number): number;
  _vellum_scene(buffer: number): number;
  _vellum_render(): number;
  _vellum_zoom(zoom: number): number;
  _vellum_pixels(): number;
}
export type ModuleFactory = (options: {
  canvas: HTMLCanvasElement;
  noInitialRun: boolean;
}) => Promise<SkiaModule>;
export async function createRenderer(
  canvas: HTMLCanvasElement,
  factory: ModuleFactory,
  viewport: Viewport = { width: canvas.width, height: canvas.height, dpr: 1 },
): Promise<Renderer> {
  const size = checkViewport(viewport);
  const module = await factory({ canvas, noInitialRun: true });
  if (module._vellum_display_list_version() !== DISPLAY_LIST_VERSION)
    throw new Error("Renderer display-list version mismatch");
  return new SkiaRenderer(canvas, module, size);
}
class SkiaRenderer implements Renderer {
  state: RendererState = "ready";
  private handle = 0;
  private size: [number, number];
  constructor(
    private canvas: HTMLCanvasElement,
    private module: SkiaModule,
    size: [number, number],
  ) {
    this.size = size;
    this.connect();
    canvas.addEventListener("webglcontextlost", this.lost);
    canvas.addEventListener("webglcontextrestored", this.restored);
  }
  private check(status: number) {
    if (status !== 1) {
      const ptr = this.module._vellum_error();
      throw new Error(new TextDecoder().decode(this.take(ptr)) || "Renderer operation failed");
    }
  }
  private take(ptr: number): Uint8Array {
    if (!ptr) throw new Error("Renderer returned no buffer");
    try {
      return this.module.HEAPU8.slice(ptr, ptr + this.module._vellum_buffer_len(ptr));
    } finally {
      this.module._vellum_free(ptr);
    }
  }
  private upload(bytes: Uint8Array): number {
    if (bytes.byteLength > 64 * 1024 * 1024) throw new Error("Renderer resource too large");
    const ptr = this.module._vellum_alloc(bytes.byteLength);
    if (!ptr) throw new Error("Renderer allocation failed");
    this.module.HEAPU8.set(bytes, ptr); // Re-read HEAPU8: allocation may grow WASM memory.
    return ptr;
  }
  private active() {
    if (this.state !== "ready") throw new Error(`Renderer is ${this.state}`);
    if (!this.module.GL.makeContextCurrent(this.handle))
      throw new Error("Cannot activate WebGL2 context");
  }
  private connect() {
    this.canvas.width = this.size[0];
    this.canvas.height = this.size[1];
    this.handle = this.module.GL.createContext(this.canvas, {
      majorVersion: 2,
      minorVersion: 0,
      alpha: true,
      premultipliedAlpha: true,
      stencil: true,
      antialias: false,
    });
    if (!this.handle || !this.module.GL.makeContextCurrent(this.handle))
      throw new Error("WebGL2 unavailable");
    try {
      this.check(this.module._vellum_renderer_init(...this.size));
    } catch (error) {
      this.module.GL.deleteContext(this.handle);
      throw error;
    }
  }
  resize(viewport: Viewport) {
    const next = checkViewport(viewport);
    this.active();
    if (next[0] === this.size[0] && next[1] === this.size[1]) return;
    this.canvas.width = next[0];
    this.canvas.height = next[1];
    this.check(this.module._vellum_renderer_resize(...next));
    this.size = next;
  }
  draw(frame: Frame) {
    this.active();
    this.check(this.module._vellum_zoom(frame.zoom ?? 1));
    this.module._vellum_resources_clear();
    for (const f of frame.fonts) {
      if (
        f.handle < 0n ||
        f.handle > 0xffffffffffffffffn ||
        !Number.isInteger(f.index) ||
        f.index < 0 ||
        f.index > 0xffffffff
      )
        throw new Error("Invalid font handle/index");
      this.check(
        this.module._vellum_font(
          Number(f.handle & 0xffffffffn),
          Number(f.handle >> 32n),
          f.index,
          this.upload(f.bytes),
        ),
      );
    }
    for (const image of frame.images) {
      const id = this.upload(new TextEncoder().encode(image.id));
      let bytes = 0;
      try {
        bytes = this.upload(image.bytes);
        this.check(this.module._vellum_image(id, bytes));
      } finally {
        this.module._vellum_free(id);
        this.module._vellum_free(bytes);
      }
    }
    const buffer = this.upload(frame.displayList);
    try {
      this.check(this.module._vellum_scene(buffer));
    } finally {
      this.module._vellum_free(buffer);
    }
    this.check(this.module._vellum_render());
  }
  readPixels() {
    this.active();
    return this.take(this.module._vellum_pixels());
  }
  private lost = (event: Event) => {
    event.preventDefault();
    this.module._vellum_renderer_lost();
    this.state = "context-lost";
  };
  private restored = () => {
    if (this.state === "disposed") return;
    this.module.GL.makeContextCurrent(this.handle);
    try {
      this.check(this.module._vellum_renderer_init(...this.size));
      this.state = "ready";
      this.check(this.module._vellum_render());
    } catch {
      this.state = "context-lost";
    }
  };
  dispose() {
    if (this.state === "disposed") return;
    this.canvas.removeEventListener("webglcontextlost", this.lost);
    this.canvas.removeEventListener("webglcontextrestored", this.restored);
    this.module.GL.makeContextCurrent(this.handle);
    this.module._vellum_renderer_dispose();
    this.module.GL.deleteContext(this.handle);
    this.state = "disposed";
  }
}
