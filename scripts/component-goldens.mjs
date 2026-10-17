#!/usr/bin/env node
// Chrome + design lane. Needs a built React Storybook, Chrome and local component board PNGs.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { launchChrome } from "./lib/chrome.mjs";
import { COMPONENT_TOLERANCE, compareComponentPixels } from "./lib/component-goldens.mjs";
import { comparePixels } from "./renderer-golden-compare.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const staticRoot = resolve(root, "packages/editor-react/storybook-static");
if (!existsSync(staticRoot + "/iframe.html"))
  throw Error("Run pnpm --filter @vellum/editor-react build-storybook first");
const fixture = JSON.parse(
  readFileSync(root + "packages/editor-react/fixtures/components.json", "utf8"),
);
const inventory = JSON.parse(readFileSync(root + "docs/design/snapshots/inventory.json", "utf8"));
if (fixture.source_sha256 !== inventory.source_sha256)
  throw Error("Component design source is stale");
const index = JSON.parse(readFileSync(staticRoot + "/index.json", "utf8"));
const stories = Object.values(index.entries).filter(
  (e) => e.type === "story" && e.title === "Harness/Components",
);
if (stories.length !== fixture.cases.length)
  throw Error("Built component stories are stale; rebuild Storybook");
const types = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".json": "application/json",
  ".ttf": "font/ttf",
  ".png": "image/png",
  ".svg": "image/svg+xml",
};
const server = createServer((req, res) => {
  let path;
  try {
    path = resolve(
      staticRoot,
      "." + decodeURIComponent(new URL(req.url, "http://localhost").pathname),
    );
  } catch {
    return res.writeHead(400).end();
  }
  if (!path.startsWith(staticRoot + sep) || !existsSync(path)) return res.writeHead(404).end();
  try {
    res.setHeader("Content-Type", types[extname(path)] ?? "application/octet-stream");
    res.end(readFileSync(path));
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const results = { passed: [], pending: [], failed: [] };
let chrome;
try {
  chrome = await launchChrome({ width: 3200, height: 2000 });
  for (const [i, specimen] of fixture.cases.entries()) {
    if (
      process.argv.includes("--case") &&
      process.argv[process.argv.indexOf("--case") + 1] !== specimen.id
    )
      continue;
    const story = stories.find((e) => e.exportName === `Case${String(i + 1).padStart(3, "0")}`)?.id;
    if (!story) throw Error("Missing component story export");
    await chrome.send("Page.navigate", {
      url: `http://127.0.0.1:${server.address().port}/iframe.html?id=${story}&viewMode=story`,
    });
    let element;
    for (let poll = 0; poll < 200; poll++) {
      element = await chrome.evaluate(
        `(() => { const el = document.querySelector('[data-golden-status]'); if (!el) return null; const b = el.getBoundingClientRect(); return { status: el.dataset.goldenStatus, id: el.closest("[data-component-case]")?.dataset.componentCase, x: b.x, y: b.y, width: b.width, height: b.height }; })()`,
      );
      if (element?.id === specimen.id) break;
      element = null;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    if (!element) {
      results.failed.push({
        id: specimen.id,
        reason: "Story did not render",
        detail: await chrome.evaluate("document.body.innerText.slice(0,2000)"),
      });
      break;
    }
    if (element.status === "pending") {
      results.pending.push(specimen.id);
      continue;
    }
    if (element.status !== "ready" || element.id !== specimen.id) {
      results.failed.push({ id: specimen.id, reason: "Wrong component rendered" });
      continue;
    }
    const board = fixture.boards.find((board) => board.id === specimen.frameId);
    if (!board) {
      results.failed.push({ id: specimen.id, reason: "No design reference for this appearance" });
      continue;
    }
    const expectedPath = root + "docs/design/snapshots/" + board.snapshot;
    if (!existsSync(expectedPath)) {
      results.failed.push({
        id: specimen.id,
        reason: "Missing design snapshot: " + board.snapshot,
      });
      continue;
    }
    await chrome.send("Runtime.evaluate", {
      expression: "document.fonts.ready.then(() => true)",
      awaitPromise: true,
      returnByValue: true,
    });
    const actual = await chrome.send("Page.captureScreenshot", {
      format: "png",
      clip: {
        x: element.x,
        y: element.y,
        width: Math.round(element.width),
        height: Math.round(element.height),
        scale: 1,
      },
      captureBeyondViewport: true,
    });
    const expected = readFileSync(expectedPath).toString("base64");
    const evaluated = await chrome.send("Runtime.evaluate", {
      awaitPromise: true,
      returnByValue: true,
      expression: `(async () => {
      const comparePixels = ${comparePixels.toString()};
      const COMPONENT_TOLERANCE = ${JSON.stringify(COMPONENT_TOLERANCE)};
      const compareComponentPixels = ${compareComponentPixels.toString()};
      async function pixels(base64, box) {
        const image = new Image(); image.src = 'data:image/png;base64,' + base64; await image.decode();
        const width = Math.round(box?.width ?? image.width), height = Math.round(box?.height ?? image.height);
        const canvas = new OffscreenCanvas(width, height), context = canvas.getContext('2d');
        if (box) context.drawImage(image, box.x, box.y, box.width, box.height, 0, 0, width, height); else context.drawImage(image, 0, 0);
        return { width, height, data: context.getImageData(0, 0, width, height).data };
      }
      return compareComponentPixels(await pixels(${JSON.stringify(expected)}, ${JSON.stringify(specimen.box)}), await pixels(${JSON.stringify(actual.data)}));
    })()`,
    });
    if (evaluated.exceptionDetails) throw Error("Component pixel comparison failed");
    const comparison = evaluated.result.value;
    if (comparison.pass) results.passed.push(specimen.id);
    else results.failed.push({ id: specimen.id, ...comparison });
  }
} finally {
  await chrome?.close();
  await new Promise((resolve) => server.close(resolve));
}
mkdirSync(root + "target/component-goldens", { recursive: true });
writeFileSync(
  root + "target/component-goldens/results.json",
  JSON.stringify(results, null, 2) + "\n",
);
console.log(
  `Component goldens: ${results.passed.length} passed, ${results.pending.length} pending, ${results.failed.length} failed`,
);
if (results.failed.length) {
  console.error(JSON.stringify(results.failed));
  process.exitCode = 1;
}
