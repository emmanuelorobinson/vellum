// Renderer A: Rust (wasm32-unknown-unknown, dist/spike_scene_wasm.wasm) builds the display list;
// this file walks it every frame and draws with CanvasKit. The opcodes must match
// crates/scene/src/display_list.rs.

const OP = {
  SAVE: 0,
  RESTORE: 1,
  TRANSLATE: 2,
  ROTATE: 3,
  CLIP_RRECT: 4,
  SAVE_LAYER_ALPHA: 5,
  FILL_RRECT: 10,
  STROKE_RRECT: 11,
  FILL_OVAL: 12,
  STROKE_OVAL: 13,
  FILL_POLY: 14,
  IMAGE: 15,
  TEXT: 16,
};
const PAINT = { SOLID: 0, LINEAR: 1, RADIAL: 2 };
const BACKGROUND = [232 / 255, 232 / 255, 236 / 255, 1]; // spike_scene::CANVAS_BACKGROUND

function loadScript(src) {
  return new Promise((ok, fail) => {
    const script = Object.assign(document.createElement("script"), {
      src,
      onload: ok,
      onerror: fail,
    });
    document.head.append(script);
  });
}

export async function create(canvas, fontBytes) {
  const [, scene] = await Promise.all([
    loadScript("/canvaskit/canvaskit.js"),
    WebAssembly.instantiateStreaming(fetch("/dist/spike_scene_wasm.wasm"), {}),
  ]);
  const CK = await window.CanvasKitInit({ locateFile: (file) => `/canvaskit/${file}` });
  const wasm = scene.instance.exports;
  const webgl = { antialias: 0, preserveDrawingBuffer: 1, majorVersion: 2, stencil: 1, depth: 0 };
  let surface = CK.MakeWebGLCanvasSurface(canvas, CK.ColorSpace.SRGB, webgl);
  if (!surface) throw new Error("CanvasKit could not create a WebGL2 surface");
  const typeface = CK.Typeface.MakeTypefaceFromData(fontBytes);

  const fill = new CK.Paint();
  fill.setAntiAlias(true);
  const stroke = new CK.Paint();
  stroke.setAntiAlias(true);
  stroke.setStyle(CK.PaintStyle.Stroke);
  const imagePaint = new CK.Paint();
  const layer = new CK.Paint();
  const textPaint = new CK.Paint();
  textPaint.setAntiAlias(true);
  const blendModes = [CK.BlendMode.SrcOver, CK.BlendMode.Multiply, CK.BlendMode.Screen];
  // Scratch geometry, reused so the per-frame cost is CanvasKit calls rather than JS allocation.
  const rect = new Float32Array(4);
  const rrect = new Float32Array(12);
  const fonts = new Map();

  let ops = new Float32Array(0);
  let strings = [];
  let images = [];
  let built = { nodeCount: 0, width: 0, height: 0 };

  function adopt(nodeCount) {
    const memory = wasm.memory.buffer;
    ops = new Float32Array(memory, wasm.scene_ops_ptr(), wasm.scene_ops_len());
    const bytes = new Uint8Array(memory, wasm.scene_strings_ptr(), wasm.scene_strings_len());
    const index = new Uint32Array(
      memory,
      wasm.scene_string_index_ptr(),
      wasm.scene_string_count() * 2,
    );
    const decoder = new TextDecoder();
    strings = [];
    for (let i = 0; i < index.length; i += 2)
      strings.push(decoder.decode(bytes.subarray(index[i], index[i] + index[i + 1])));
    for (const image of images) image.delete();
    images = [];
    for (let i = 0; i < wasm.scene_image_count(); i++) {
      const width = wasm.scene_image_width(i);
      const height = wasm.scene_image_height(i);
      const pixels = new Uint8Array(memory, wasm.scene_image_rgba_ptr(i), width * height * 4);
      const info = {
        width,
        height,
        alphaType: CK.AlphaType.Unpremul,
        colorType: CK.ColorType.RGBA_8888,
        colorSpace: CK.ColorSpace.SRGB,
      };
      images.push(CK.MakeImage(info, pixels.slice(), width * 4));
    }
    built = { nodeCount, width: wasm.scene_doc_width(), height: wasm.scene_doc_height() };
  }

  function setRect(i) {
    rect[0] = ops[i];
    rect[1] = ops[i + 1];
    rect[2] = ops[i] + ops[i + 2];
    rect[3] = ops[i + 1] + ops[i + 3];
    return rect;
  }

  function setRRect(i) {
    rrect[0] = ops[i];
    rrect[1] = ops[i + 1];
    rrect[2] = ops[i] + ops[i + 2];
    rrect[3] = ops[i + 1] + ops[i + 3];
    rrect.fill(ops[i + 4], 4);
    return rrect;
  }

  function readStops(i) {
    const n = ops[i];
    const colors = new Float32Array(n * 4);
    const positions = new Array(n);
    for (let k = 0; k < n; k++) {
      const s = i + 1 + k * 5;
      positions[k] = ops[s];
      colors.set(ops.subarray(s + 1, s + 5), k * 4);
    }
    return [colors, positions, i + 1 + n * 5];
  }

  /** Reads `[blend, kind, args...]` into `fill`; returns [next index, shader to delete or null]. */
  function readPaint(i) {
    fill.setBlendMode(blendModes[ops[i]] ?? CK.BlendMode.SrcOver);
    const kind = ops[i + 1];
    i += 2;
    if (kind === PAINT.SOLID) {
      fill.setShader(null);
      fill.setColorComponents(ops[i], ops[i + 1], ops[i + 2], ops[i + 3]);
      return [i + 4, null];
    }
    let shader;
    let next;
    if (kind === PAINT.LINEAR) {
      const [colors, positions, after] = readStops(i + 4);
      shader = CK.Shader.MakeLinearGradient(
        [ops[i], ops[i + 1]],
        [ops[i + 2], ops[i + 3]],
        colors,
        positions,
        CK.TileMode.Clamp,
      );
      next = after;
    } else {
      const [colors, positions, after] = readStops(i + 3);
      shader = CK.Shader.MakeRadialGradient(
        [ops[i], ops[i + 1]],
        ops[i + 2],
        colors,
        positions,
        CK.TileMode.Clamp,
      );
      next = after;
    }
    fill.setColorComponents(0, 0, 0, 1);
    fill.setShader(shader);
    return [next, shader];
  }

  function readStroke(i) {
    stroke.setStrokeWidth(ops[i]);
    stroke.setColorComponents(ops[i + 1], ops[i + 2], ops[i + 3], ops[i + 4]);
    return i + 5;
  }

  function fontFor(size) {
    let font = fonts.get(size);
    if (!font) {
      font = new CK.Font(typeface, size);
      fonts.set(size, font);
    }
    return font;
  }

  function draw(c) {
    let i = 0;
    let shader = null;
    while (i < ops.length) {
      const code = ops[i++];
      switch (code) {
        case OP.SAVE:
          c.save();
          break;
        case OP.RESTORE:
          c.restore();
          break;
        case OP.TRANSLATE:
          c.translate(ops[i], ops[i + 1]);
          i += 2;
          break;
        case OP.ROTATE:
          c.rotate(ops[i], ops[i + 1], ops[i + 2]);
          i += 3;
          break;
        case OP.CLIP_RRECT:
          c.clipRRect(setRRect(i), CK.ClipOp.Intersect, true);
          i += 5;
          break;
        case OP.SAVE_LAYER_ALPHA:
          layer.setAlphaf(ops[i]);
          c.saveLayer(layer);
          i += 1;
          break;
        case OP.FILL_RRECT: {
          const shape = setRRect(i);
          [i, shader] = readPaint(i + 5);
          c.drawRRect(shape, fill);
          break;
        }
        case OP.STROKE_RRECT: {
          const shape = setRRect(i);
          i = readStroke(i + 5);
          c.drawRRect(shape, stroke);
          break;
        }
        case OP.FILL_OVAL: {
          const shape = setRect(i);
          [i, shader] = readPaint(i + 4);
          c.drawOval(shape, fill);
          break;
        }
        case OP.STROKE_OVAL: {
          const shape = setRect(i);
          i = readStroke(i + 4);
          c.drawOval(shape, stroke);
          break;
        }
        case OP.FILL_POLY: {
          const n = ops[i];
          const points = ops.slice(i + 1, i + 1 + n * 2);
          [i, shader] = readPaint(i + 1 + n * 2);
          const path = new CK.PathBuilder().addPolygon(points, true).detach();
          c.drawPath(path, fill);
          path.delete();
          break;
        }
        case OP.IMAGE: {
          const image = images[ops[i]];
          const filter = ops[i + 5] > 0.5 ? CK.FilterMode.Linear : CK.FilterMode.Nearest;
          const src = [0, 0, image.width(), image.height()];
          c.drawImageRectOptions(
            image,
            src,
            setRect(i + 1),
            filter,
            CK.MipmapMode.None,
            imagePaint,
          );
          i += 6;
          break;
        }
        case OP.TEXT: {
          const text = strings[ops[i]];
          textPaint.setColorComponents(ops[i + 4], ops[i + 5], ops[i + 6], ops[i + 7]);
          c.drawText(text, ops[i + 1], ops[i + 2], textPaint, fontFor(ops[i + 3]));
          i += 8;
          break;
        }
        default:
          throw new Error(`unknown display-list opcode ${code} at ${i - 1}`);
      }
      if (shader) {
        shader.delete();
        shader = null;
      }
    }
  }

  return {
    name: "A: CanvasKit in TS",
    get nodeCount() {
      return built.nodeCount;
    },
    get docWidth() {
      return built.width;
    },
    get docHeight() {
      return built.height;
    },
    loadDocument(n) {
      adopt(wasm.scene_build_document(n));
    },
    loadFixture(n) {
      adopt(wasm.scene_build_fixture(n));
    },
    resize() {
      surface.delete();
      surface = CK.MakeWebGLCanvasSurface(canvas, CK.ColorSpace.SRGB, webgl);
    },
    render(view) {
      const c = surface.getCanvas();
      c.clear(BACKGROUND);
      c.save();
      c.translate(view.tx, view.ty);
      c.scale(view.scale, view.scale);
      draw(c);
      c.restore();
      surface.flush();
    },
  };
}
