// TEST DATA: checked ABI fake, used only by contract tests (never an editor fallback).
import type { SkiaModule } from "./index";
export function mockModule() {
  let heap = new Uint8Array(1024 * 1024),
    next = 16,
    width = 0,
    height = 0,
    pixels = new Uint8Array(0);
  const buffers = new Map<number, number>();
  let error = "";
  const alloc = (len: number) => {
    const p = next;
    next += Math.max(len, 1);
    buffers.set(p, len);
    return p;
  };
  const module: SkiaModule = {
    get HEAPU8() {
      return heap;
    },
    GL: { createContext: () => 1, makeContextCurrent: () => true, deleteContext: () => {} },
    _vellum_display_list_version: () => 1,
    _vellum_alloc: alloc,
    _vellum_free: (p) => {
      buffers.delete(p);
    },
    _vellum_buffer_len: (p) => buffers.get(p) ?? 0,
    _vellum_error: () => {
      const bytes = new TextEncoder().encode(error),
        p = alloc(bytes.length);
      heap.set(bytes, p);
      return p;
    },
    _vellum_renderer_init: (w, h) => {
      width = w;
      height = h;
      return 1;
    },
    _vellum_renderer_resize: (w, h) => {
      width = w;
      height = h;
      return 1;
    },
    _vellum_renderer_lost: () => {},
    _vellum_renderer_dispose: () => {
      buffers.clear();
    },
    _vellum_resources_clear: () => {},
    _vellum_font: (_l, _h, _i, p) => {
      buffers.delete(p);
      return 1;
    },
    _vellum_image: (id, p) => {
      buffers.delete(id);
      buffers.delete(p);
      return 1;
    },
    _vellum_shader: (id, p) => {
      buffers.delete(id);
      buffers.delete(p);
      return 1;
    },
    _vellum_shader_errors: () => {
      const bytes = new TextEncoder().encode("[]"),
        p = alloc(bytes.length);
      heap.set(bytes, p);
      return p;
    },
    _vellum_scene: (p) => {
      const len = buffers.get(p);
      buffers.delete(p);
      if (len === 3) {
        error = "Malformed display list";
        return 0;
      }
      return 1;
    },
    _vellum_zoom: () => 1,
    _vellum_render: () => {
      pixels = new Uint8Array(width * height * 4);
      pixels[3] = 255;
      return 1;
    },
    _vellum_pixels: () => {
      const p = alloc(pixels.length);
      heap.set(pixels, p);
      return p;
    },
  };
  return { module, live: () => buffers.size };
}
