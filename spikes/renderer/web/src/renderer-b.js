// Renderer B: one emscripten module (dist/skia_wasm.{js,wasm}) holds the scene, the display list and
// Skia. JS creates the WebGL2 context, then only passes the view transform once per frame.

export async function create(canvas, fontBytes) {
  const { default: createSkiaSpike } = await import("/dist/skia_wasm.js");
  const M = await createSkiaSpike({ locateFile: (file) => `/dist/${file}` });
  const handle = M.GL.createContext(canvas, {
    majorVersion: 2,
    minorVersion: 0,
    antialias: false,
    preserveDrawingBuffer: true,
    stencil: true,
    depth: false,
    alpha: true,
  });
  if (!handle) throw new Error("could not create a WebGL2 context");
  M.GL.makeContextCurrent(handle);
  const code = M._spike_init(canvas.width, canvas.height);
  if (code !== 0) throw new Error(`spike_init failed (${code})`);
  const ptr = M._spike_alloc(fontBytes.byteLength);
  M.HEAPU8.set(new Uint8Array(fontBytes), ptr);
  M._spike_set_font(ptr, fontBytes.byteLength);

  return {
    name: "B: skia-safe in WASM",
    get nodeCount() {
      return M._spike_node_count();
    },
    get docWidth() {
      return M._spike_doc_width();
    },
    get docHeight() {
      return M._spike_doc_height();
    },
    loadDocument(n) {
      M._spike_load_document(n);
    },
    loadFixture(n) {
      M._spike_load_fixture(n);
    },
    resize(width, height) {
      M._spike_resize(width, height);
    },
    render(view) {
      M._spike_render(view.tx, view.ty, view.scale);
    },
    /** The scene drawn by Skia's CPU rasterizer inside WASM, as top-down RGBA8 (no GPU involved). */
    rasterPixels(width, height) {
      const ptr = M._spike_render_raster(width, height);
      return new ImageData(
        new Uint8ClampedArray(M.HEAPU8.slice(ptr, ptr + width * height * 4)),
        width,
        height,
      );
    },
  };
}
