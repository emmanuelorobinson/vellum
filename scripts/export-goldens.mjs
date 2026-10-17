// needs: Chrome (chrome lane). No server, listener, or network. Native goldens come from export-goldens.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { launchChrome } from "./lib/chrome.mjs";
import { comparePixels } from "./renderer-golden-compare.mjs";

const root = resolve(import.meta.dirname, "..");
const cases = JSON.parse(readFileSync(resolve(root, "target/export-goldens/manifest.json")));
const chrome = await launchChrome({ width: 160, height: 100 });
const decode = (data) =>
  `new Promise((resolve,reject)=>{const img=new Image();img.onload=()=>{const c=document.createElement('canvas');c.width=img.width;c.height=img.height;const ctx=c.getContext('2d');ctx.drawImage(img,0,0);resolve(Array.from(ctx.getImageData(0,0,c.width,c.height).data));};img.onerror=reject;img.src='data:image/png;base64,${data}';})`;
const evaluate = async (expression) => {
  const result = await chrome.send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  assert.ok(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
  return result.result.value;
};
try {
  await chrome.send("Emulation.setDefaultBackgroundColorOverride", {
    color: { r: 0, g: 0, b: 0, a: 0 },
  });
  await chrome.send("Network.enable");
  await chrome.send("Network.setBlockedURLs", { urls: ["http://*", "https://*"] });
  for (const item of cases) {
    const path = resolve(root, "target/export-goldens", item.case, "index.html");
    // Embed bundle-owned files to load through data: rather than give the test a file origin or listener.
    const html = readFileSync(path, "utf8").replaceAll(/(?:assets\/[A-Za-z0-9.-]+)/g, (name) => {
      const mime = name.endsWith(".png")
        ? "image/png"
        : name.endsWith(".svg")
          ? "image/svg+xml"
          : name.endsWith(".otf")
            ? "font/otf"
            : "font/ttf";
      return `data:${mime};base64,${readFileSync(resolve(dirname(path), name)).toString("base64")}`;
    });
    await chrome.setContent(html);
    await evaluate(
      "document.fonts.ready.then(()=>Promise.all(Array.from(document.images,img=>img.decode()))).then(()=>new Promise(ok=>requestAnimationFrame(()=>requestAnimationFrame(ok))))",
    );
    const { data } = await chrome.send("Page.captureScreenshot", {
      format: "png",
      clip: { x: 0, y: 0, width: item.width, height: item.height, scale: 1 },
    });
    const actual = await evaluate(decode(data));
    const expected = await evaluate(
      decode(
        readFileSync(resolve(root, "fixtures/exports", `${item.fixture}.png`)).toString("base64"),
      ),
    );
    const tolerance =
      item.fixture === "text"
        ? { channel: 16, fraction: 0.02, p95: 2 }
        : { channel: 8, fraction: 0.01, p95: 1 };
    const { diff, ...result } = comparePixels(expected, actual, tolerance);
    writeFileSync(resolve(dirname(path), "chrome.png"), Buffer.from(data, "base64"));
    assert.ok(result.pass, `${item.case}: ${JSON.stringify(result)}`);
    console.log(`${item.case}: ${JSON.stringify(result)}`);
  }
} finally {
  await chrome.close();
}
