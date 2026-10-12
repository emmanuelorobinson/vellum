// The spike page shell: one renderer per page load (a canvas holds one WebGL context), pan and zoom,
// an FPS meter, a screenshot button, and `window.__spike` for measure.mjs.
//
// Query: ?renderer=a|b&nodes=1000|5000|10000  or  ?renderer=a|b&fixture=1..20 (256 x 256, no pan/zoom)
//        or ?renderer=a|b&text=1..30 (Prompt 5 text fixtures, 256 x 256, laid out by Parley in WASM)

import { FIXTURE_SIZE, percentile } from "./stats.js";

const params = new URLSearchParams(location.search);
const which = params.get("renderer") === "b" ? "b" : "a";
const textFixture = params.has("text") ? Number(params.get("text")) : null;
const fixture = params.has("fixture")
  ? Number(params.get("fixture"))
  : textFixture !== null
    ? textFixture
    : null;
const fixtureKind = textFixture !== null ? "text" : "fixture";
const nodes = Number(params.get("nodes") ?? 1000);

const canvas = document.getElementById("canvas");
const meter = document.getElementById("meter");
const status = document.getElementById("status");
const rendererSelect = document.getElementById("renderer");
const sceneSelect = document.getElementById("scene");

rendererSelect.value = which;
if (fixture === null) sceneSelect.value = `nodes:${nodes}`;
else {
  const label = fixtureKind === "text" ? "Text fixture" : "Fixture";
  sceneSelect.append(new Option(`${label} ${fixture}`, `${fixtureKind}:${fixture}`));
  sceneSelect.value = `${fixtureKind}:${fixture}`;
}
const reload = () => {
  const [kind, value] = sceneSelect.value.split(":");
  location.search = `?renderer=${rendererSelect.value}&${kind}=${value}`;
};
rendererSelect.onchange = reload;
sceneSelect.onchange = reload;

function sizeCanvas() {
  if (fixture !== null) {
    canvas.classList.add("fixture");
    canvas.width = FIXTURE_SIZE;
    canvas.height = FIXTURE_SIZE;
    return;
  }
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(canvas.clientWidth * dpr);
  canvas.height = Math.round(canvas.clientHeight * dpr);
}

// device px = document px * scale + (tx, ty)
const view = { tx: 0, ty: 0, scale: 1 };
let animate = null; // (t) => view, while benchmarking
let latency = { track: false, pending: null, samples: [] };
let frameTimes = [];
let drawTimes = [];

function attachPanZoom() {
  let drag = null;
  canvas.addEventListener("pointerdown", (e) => {
    canvas.setPointerCapture(e.pointerId);
    drag = { x: e.clientX, y: e.clientY };
  });
  canvas.addEventListener("pointermove", (e) => {
    if (!drag) return;
    const dpr = window.devicePixelRatio || 1;
    view.tx += (e.clientX - drag.x) * dpr;
    view.ty += (e.clientY - drag.y) * dpr;
    drag = { x: e.clientX, y: e.clientY };
    if (latency.track && latency.pending === null) latency.pending = e.timeStamp;
  });
  canvas.addEventListener("pointerup", () => {
    drag = null;
  });
  canvas.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      const dpr = window.devicePixelRatio || 1;
      const factor = Math.exp(-e.deltaY * 0.002);
      const [px, py] = [e.offsetX * dpr, e.offsetY * dpr];
      view.tx = px - (px - view.tx) * factor;
      view.ty = py - (py - view.ty) * factor;
      view.scale *= factor;
    },
    { passive: false },
  );
}

/** Reads the drawing buffer (preserveDrawingBuffer is on) as top-down RGBA8. */
function readCanvas() {
  const gl = canvas.getContext("webgl2");
  const { width, height } = canvas;
  const raw = new Uint8Array(width * height * 4);
  gl.readPixels(0, 0, width, height, gl.RGBA, gl.UNSIGNED_BYTE, raw);
  const rgba = new Uint8ClampedArray(raw.length);
  const row = width * 4;
  for (let y = 0; y < height; y++)
    rgba.set(raw.subarray((height - 1 - y) * row, (height - y) * row), y * row);
  return new ImageData(rgba, width, height);
}

async function capturePng(pixels = readCanvas()) {
  const off = new OffscreenCanvas(pixels.width, pixels.height);
  off.getContext("2d").putImageData(pixels, 0, 0);
  const blob = await off.convertToBlob({ type: "image/png" });
  return blob;
}

function syncGpu() {
  const gl = canvas.getContext("webgl2");
  gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array(4));
}

async function main() {
  sizeCanvas();
  const font = await (await fetch("/font.ttf")).arrayBuffer();
  // The text fixtures' fonts, in FONT_FILES order (fonts.json); only fetched for text fixtures.
  const glyphFonts =
    textFixture === null
      ? []
      : await Promise.all(
          (await (await fetch("/fonts.json")).json()).fonts.map(async (f) => {
            const response = await fetch(`/fonts/${f.file}`);
            if (!response.ok) throw new Error(`${f.file}: run node scripts/fetch-fonts.mjs`);
            return response.arrayBuffer();
          }),
        );
  const module = which === "a" ? await import("./renderer-a.js") : await import("./renderer-b.js");
  const renderer = await module.create(canvas, font, glyphFonts);
  if (textFixture !== null) renderer.loadTextFixture(textFixture);
  else if (fixture !== null) renderer.loadFixture(fixture);
  else {
    renderer.loadDocument(nodes);
    attachPanZoom();
    addEventListener("resize", () => {
      sizeCanvas();
      renderer.resize(canvas.width, canvas.height);
    });
  }
  renderer.render(view);
  syncGpu();
  const firstFrameMs = performance.now();

  const gl = canvas.getContext("webgl2");
  const debug = gl.getExtension("WEBGL_debug_renderer_info");
  const glRenderer = debug
    ? gl.getParameter(debug.UNMASKED_RENDERER_WEBGL)
    : gl.getParameter(gl.RENDERER);
  status.textContent = `${renderer.name} · ${renderer.nodeCount} nodes · first frame ${firstFrameMs.toFixed(0)} ms`;

  let last = performance.now();
  let meterFrames = 0;
  let meterStart = last;
  function frame(now) {
    if (animate) Object.assign(view, animate(now));
    const t0 = performance.now();
    renderer.render(view);
    const t1 = performance.now();
    if (latency.pending !== null) {
      syncGpu();
      latency.samples.push(performance.now() - latency.pending);
      latency.pending = null;
    }
    drawTimes.push(t1 - t0);
    frameTimes.push(now - last);
    last = now;
    meterFrames++;
    if (now - meterStart >= 500) {
      const fps = (meterFrames * 1000) / (now - meterStart);
      meter.textContent = `${fps.toFixed(1)} fps · draw ${percentile(drawTimes.slice(-meterFrames), 0.5).toFixed(2)} ms`;
      meterFrames = 0;
      meterStart = now;
      if (!animate && drawTimes.length > 600) {
        drawTimes = drawTimes.slice(-120);
        frameTimes = frameTimes.slice(-120);
      }
    }
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);

  document.getElementById("screenshot").onclick = async () => {
    const url = URL.createObjectURL(await capturePng());
    const a = Object.assign(document.createElement("a"), {
      href: url,
      download: `spike-${which}-${fixture !== null ? `${fixtureKind}-${fixture}` : `${nodes}`}.png`,
    });
    a.click();
    URL.revokeObjectURL(url);
  };

  return {
    renderer: renderer.name,
    nodeCount: renderer.nodeCount,
    firstFrameMs,
    glRenderer,
    width: canvas.width,
    height: canvas.height,
    /** Pans and zooms continuously over the whole document for `seconds` and reports frame numbers. */
    async bench(seconds, warmup = 1) {
      const fit = Math.min(canvas.width / renderer.docWidth, canvas.height / renderer.docHeight);
      animate = (t) => {
        const k = (Math.sin(t / 900) + 1) / 2; // 0..1
        const scale = fit * (1 + 3 * k);
        return {
          scale,
          tx:
            (canvas.width - renderer.docWidth * scale) / 2 +
            Math.sin(t / 1300) * canvas.width * 0.3 * k,
          ty:
            (canvas.height - renderer.docHeight * scale) / 2 +
            Math.cos(t / 1700) * canvas.height * 0.3 * k,
        };
      };
      await new Promise((ok) => setTimeout(ok, warmup * 1000));
      frameTimes = [];
      drawTimes = [];
      const start = performance.now();
      await new Promise((ok) => setTimeout(ok, seconds * 1000));
      const elapsed = performance.now() - start;
      const frames = frameTimes.slice(1);
      const draws = drawTimes.slice(1);
      animate = null;
      return {
        frames: frames.length,
        fps: (frames.length * 1000) / elapsed,
        frameMsP50: percentile(frames, 0.5),
        frameMsP95: percentile(frames, 0.95),
        drawMsP50: percentile(draws, 0.5),
        drawMsP95: percentile(draws, 0.95),
      };
    },
    trackLatency(on) {
      latency = { track: on, pending: null, samples: [] };
    },
    latency() {
      const s = latency.samples;
      return {
        samples: s.length,
        p50: percentile(s, 0.5),
        p95: percentile(s, 0.95),
        max: Math.max(...s),
      };
    },
    /** PNG of the canvas; with `raster`, Renderer B's CPU raster path inside WASM instead. */
    async captureDataUrl(raster = false) {
      renderer.render(view);
      if (raster && !renderer.rasterPixels) throw new Error(`${renderer.name} has no raster path`);
      const blob = await capturePng(
        raster ? renderer.rasterPixels(canvas.width, canvas.height) : undefined,
      );
      const bytes = new Uint8Array(await blob.arrayBuffer());
      let binary = "";
      for (let i = 0; i < bytes.length; i += 0x8000)
        binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      return `data:image/png;base64,${btoa(binary)}`;
    },
  };
}

window.__spike = main().catch((e) => {
  status.textContent = `Failed: ${e.message}`;
  throw e;
});
