#!/usr/bin/env node
// Compares the Storybook token page (packages/ui-tokens/src/token-page.ts) with the design's Tokens sheet
// (docs/design/snapshots/foundations/Tokens.png, made by scripts/design-tokens) in headless Chrome
// (guide Prompt 43 FINAL). Tagged chrome + design: it needs Chrome (VELLUM_CHROME or the usual install)
// and the local snapshot PNG, which is gitignored like every snapshot.
//
//   node scripts/token-page-compare.mjs [--root <dir>]   writes target/token-page/{actual,diff}.png
//
// Tolerance: DESIGN-DECISIONS.md "UI golden tolerance vs design snapshots" = 1 % of pixels may differ. A
// pixel differs when any premultiplied channel is more than CHANNEL apart, so anti-aliasing that both
// renderers place on the same edge does not count.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { dirname, extname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { launchChrome } from "./lib/chrome.mjs";

export const TOLERANCE = { fraction: 0.01, channel: 48, p95: 255 };
const WIDTH = 1600;

// Inter is the repo's bundled face (Google Fonts build, the one pen.dev renders with). JetBrains Mono is
// fetched once into .design-cache/fonts/ and checked against its SHA-256. Both are SIL OFL 1.1.
const FONTS = {
  inter: { path: "crates/adapters/text-parley/fonts/InterVariable.ttf" },
  mono: {
    path: ".design-cache/fonts/JetBrainsMono-wght.ttf",
    url: "https://raw.githubusercontent.com/JetBrains/JetBrainsMono/cd5227bd1f61dff3bbd6c814ceaf7ffd95e947d9/fonts/variable/JetBrainsMono%5Bwght%5D.ttf",
    sha256: "662a196d58f1183bf2d77428b6d5283fe3f45161ab021bea4036bc98e5cac016",
  },
};

const args = process.argv.slice(2);
const root = resolve(
  args.includes("--root")
    ? args[args.indexOf("--root") + 1]
    : join(dirname(fileURLToPath(import.meta.url)), ".."),
);
const at = (...p) => join(root, ...p);
const die = (message) => {
  console.error(message);
  process.exit(1);
};

const expectedPath = at("docs/design/snapshots/foundations/Tokens.png");
if (!existsSync(expectedPath))
  die(
    "docs/design/snapshots/foundations/Tokens.png is missing (gitignored). Run pnpm design:tokens to export it.",
  );

async function ensureFont({ path, url, sha256 }) {
  const file = at(path);
  if (!existsSync(file)) {
    if (!url) die(`${path} is missing.`);
    const response = await fetch(url);
    if (!response.ok) die(`could not fetch ${url}: ${response.status}`);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, Buffer.from(await response.arrayBuffer()));
  }
  if (sha256 && createHash("sha256").update(readFileSync(file)).digest("hex") !== sha256)
    die(`${path} does not match its pinned SHA-256; delete it and run again.`);
  return file;
}
const fonts = { inter: await ensureFont(FONTS.inter), mono: await ensureFont(FONTS.mono) };

const page = `<!doctype html><html><head><meta charset="utf-8">
<link rel="stylesheet" href="/packages/ui-tokens/src/generated/tokens.css">
<style>
@font-face{font-family:"Inter";src:url(/font/inter) format("truetype");font-weight:100 900}
@font-face{font-family:"JetBrains Mono";src:url(/font/mono) format("truetype");font-weight:100 800}
html,body{margin:0;background:#fff}
</style>
<script type="importmap">{"imports":{"@vellum/config":"/packages/config/src/index.ts"}}</script>
<script type="module">
import { renderTokenPage } from "/packages/ui-tokens/src/token-page.ts";
import { comparePixels } from "/scripts/renderer-golden-compare.mjs";
document.body.innerHTML = renderTokenPage();
await Promise.all([...document.fonts].map((f) => f.load()));
await document.fonts.ready;
const pixels = async (src) => {
  const img = new Image(); img.src = src; await img.decode();
  const c = new OffscreenCanvas(img.width, img.height), g = c.getContext("2d");
  g.drawImage(img, 0, 0);
  return { width: img.width, height: img.height, data: g.getImageData(0, 0, img.width, img.height).data };
};
window.compare = async (actualBase64, tolerance) => {
  const e = await pixels("/expected.png"), a = await pixels("data:image/png;base64," + actualBase64);
  if (e.width !== a.width || e.height !== a.height)
    return { pass: false, reason: "size " + a.width + "x" + a.height + " vs " + e.width + "x" + e.height };
  const { diff, ...result } = comparePixels(e.data, a.data, tolerance);
  const c = new OffscreenCanvas(e.width, e.height);
  c.getContext("2d").putImageData(new ImageData(new Uint8ClampedArray(diff), e.width, e.height), 0, 0);
  const blob = await c.convertToBlob({ type: "image/png" });
  const bytes = new Uint8Array(await blob.arrayBuffer());
  let s = ""; for (let i = 0; i < bytes.length; i += 32768) s += String.fromCharCode(...bytes.subarray(i, i + 32768));
  return { ...result, diffPng: btoa(s) };
};
const box = (el) => { const r = el.getBoundingClientRect(); return [el.className || el.tagName, Math.round(r.x * 100) / 100, Math.round(r.y * 100) / 100, Math.round(r.width * 100) / 100, Math.round(r.height * 100) / 100]; };
window.ready = {
  height: document.querySelector(".tp").getBoundingClientRect().height,
  boxes: [...document.querySelectorAll(".tp > *, .tp-foot p, .tp-foot h2, .tp-token:first-of-type > *")].map(box),
};
</script></head><body></body></html>`;

const SERVED = [
  "packages/ui-tokens/src/",
  "packages/config/src/",
  "scripts/renderer-golden-compare.mjs",
];
const server = createServer((req, res) => {
  const pathname = new URL(req.url, "http://localhost").pathname;
  const send = (type, data) => {
    res.setHeader("Content-Type", type);
    res.end(data);
  };
  if (pathname === "/") return send("text/html", page);
  if (pathname === "/expected.png") return send("image/png", readFileSync(expectedPath));
  if (pathname === "/font/inter") return send("font/ttf", readFileSync(fonts.inter));
  if (pathname === "/font/mono") return send("font/ttf", readFileSync(fonts.mono));
  let path = resolve(root, `.${pathname}`);
  if (!SERVED.some((p) => path === resolve(root, p) || path.startsWith(resolve(root, p) + sep)))
    return res.writeHead(404).end();
  if (!existsSync(path) && !extname(path)) path += ".ts";
  if (!existsSync(path)) return res.writeHead(404).end();
  const ext = extname(path);
  if (ext === ".css") return send("text/css", readFileSync(path));
  const text = readFileSync(path, "utf8");
  send(
    "text/javascript",
    ext === ".ts"
      ? ts.transpileModule(text, {
          compilerOptions: { target: ts.ScriptTarget.ES2023, module: ts.ModuleKind.ESNext },
        }).outputText
      : text,
  );
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));

let chrome;
let result;
try {
  chrome = await launchChrome({ width: WIDTH, height: 1000 });
  await chrome.send("Page.navigate", { url: `http://127.0.0.1:${server.address().port}/` });
  let info = null;
  for (let i = 0; i < 200 && !info; i++) {
    await new Promise((ok) => setTimeout(ok, 50));
    info = await chrome.evaluate("window.ready ?? null").catch(() => null);
  }
  if (!info) throw new Error("the token page did not render");
  if (args.includes("--boxes")) for (const b of info.boxes) console.log(JSON.stringify(b));
  const height = Math.round(info.height);
  await chrome.send("Emulation.setDeviceMetricsOverride", {
    width: WIDTH,
    height,
    deviceScaleFactor: 1,
    mobile: false,
  });
  const { data } = await chrome.send("Page.captureScreenshot", {
    format: "png",
    clip: { x: 0, y: 0, width: WIDTH, height, scale: 1 },
  });
  const out = at("target/token-page");
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "actual.png"), Buffer.from(data, "base64"));
  const evaluated = await chrome.send("Runtime.evaluate", {
    expression: `window.compare(${JSON.stringify(data)}, ${JSON.stringify(TOLERANCE)})`,
    awaitPromise: true,
    returnByValue: true,
  });
  if (evaluated.exceptionDetails)
    throw new Error(
      evaluated.exceptionDetails.exception?.description ?? evaluated.exceptionDetails.text,
    );
  result = evaluated.result.value;
  if (result.diffPng) writeFileSync(join(out, "diff.png"), Buffer.from(result.diffPng, "base64"));
  delete result.diffPng;
} finally {
  await chrome?.close();
  server.close();
}

const summary = result.reason
  ? result.reason
  : `${(result.fraction * 100).toFixed(3)} % of pixels differ (limit ${TOLERANCE.fraction * 100} %), max channel delta ${result.max}, p95 ${result.p95}`;
if (!result.pass)
  die(`✗ token page vs Tokens sheet: ${summary}. See target/token-page/{actual,diff}.png.`);
console.log(`✓ token page vs Tokens sheet: ${summary}`);
