import { DISPLAY_LIST_VERSION, type Frame } from "@vellum/renderer-api";
import type { BasicScene, Compiler } from "./index";
/** Checked geometry-only ABI; no WebGL context, platform fonts, files or network. Use a dedicated module. */
export interface BasicModule {
  HEAPU8: Uint8Array;
  _vellum_display_list_version(): number;
  _vellum_alloc(length: number): number;
  _vellum_free(pointer: number): void;
  _vellum_buffer_len(pointer: number): number;
  _vellum_error(): number;
  _vellum_resources_clear(): void;
  _vellum_font(low: number, high: number, index: number, buffer: number): number;
  _vellum_image(id: number, buffer: number): number;
  _vellum_basic_plan(buffer: number): number;
}
export function createCompiler(module: BasicModule): Compiler {
  if (module._vellum_display_list_version() !== DISPLAY_LIST_VERSION)
    throw new Error("Renderer display-list version mismatch");
  const take = (ptr: number) => {
    if (!ptr) throw new Error("Renderer returned no buffer");
    try {
      return module.HEAPU8.slice(ptr, ptr + module._vellum_buffer_len(ptr));
    } finally {
      module._vellum_free(ptr);
    }
  };
  const error = () =>
    new Error(new TextDecoder().decode(take(module._vellum_error())) || "Basic compilation failed");
  const upload = (bytes: Uint8Array) => {
    if (bytes.length > 64 * 1024 * 1024) throw new Error("Renderer resource too large");
    const p = module._vellum_alloc(bytes.length);
    if (!p) throw new Error("Renderer allocation failed");
    module.HEAPU8.set(bytes, p);
    return p;
  };
  return (frame: Frame) => {
    module._vellum_resources_clear();
    try {
      for (const f of frame.fonts) {
        if (
          f.handle < 0n ||
          f.handle > 0xffffffffffffffffn ||
          !Number.isInteger(f.index) ||
          f.index < 0 ||
          f.index > 0xffffffff
        )
          throw new Error("Invalid font handle/index");
        const p = upload(f.bytes);
        try {
          if (
            module._vellum_font(
              Number(f.handle & 0xffffffffn),
              Number(f.handle >> 32n),
              f.index,
              p,
            ) !== 1
          )
            throw error();
        } finally {
          module._vellum_free(p);
        }
      }
      for (const image of frame.images) {
        const id = upload(new TextEncoder().encode(image.id));
        let p = 0;
        try {
          p = upload(image.bytes);
          if (module._vellum_image(id, p) !== 1) throw error();
        } finally {
          module._vellum_free(id);
          module._vellum_free(p);
        }
      }
      const p = upload(frame.displayList);
      let result: number;
      try {
        result = module._vellum_basic_plan(p);
      } finally {
        module._vellum_free(p);
      }
      if (!result) throw error();
      return JSON.parse(new TextDecoder().decode(take(result))) as BasicScene;
    } finally {
      module._vellum_resources_clear();
    }
  };
}
