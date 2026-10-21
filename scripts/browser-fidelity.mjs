// Chrome lane. Run the tagged browser_fidelity Rust test first (see docs/runbooks/browser-import.md).
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { launchChrome } from "./lib/chrome.mjs";
import { comparePixels } from "./renderer-golden-compare.mjs";

const output = resolve(import.meta.dirname, "../target/browser-fidelity");
const browser = await launchChrome();
const results = [];
try {
  const decode = async (bytes) => {
    const response = await browser.send("Runtime.evaluate", {
      expression: `new Promise((ok,no)=>{const image=new Image();image.onload=()=>{const c=document.createElement('canvas');c.width=image.width;c.height=image.height;const ctx=c.getContext('2d');ctx.drawImage(image,0,0);ok({width:c.width,height:c.height,pixels:Array.from(ctx.getImageData(0,0,c.width,c.height).data)});};image.onerror=no;image.src='data:image/png;base64,${bytes.toString("base64")}';})`,
      awaitPromise: true,
      returnByValue: true,
    });
    assert.ok(!response.exceptionDetails);
    return response.result.value;
  };
  for (const name of ["phone", "desktop", "element"]) {
    const expected = await decode(readFileSync(resolve(output, `${name}-chrome.png`)));
    const actual = await decode(readFileSync(resolve(output, `${name}-vellum.png`)));
    assert.deepEqual([actual.width, actual.height], [expected.width, expected.height], name);
    const { diff, ...result } = comparePixels(expected.pixels, actual.pixels, {
      channel: 48,
      fraction: 0.01,
      p95: 2,
    });
    results.push({ name, ...result });
    console.log(`${name}: ${JSON.stringify(result)}`);
  }
  writeFileSync(resolve(output, "results.json"), `${JSON.stringify(results, null, 2)}\n`);
  assert.ok(
    results.every((r) => r.pass),
    "Imported layers exceed the unchanged 1% pixel tolerance.",
  );
} finally {
  await browser.close();
}
