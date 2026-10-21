// needs: Chrome/WebGL2, source-built WASM and native example fixtures. Test-only loopback server.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";
import ts from "typescript";
import { launchChrome } from "../spikes/renderer/web/lib/chrome.mjs";

const root = resolve(import.meta.dirname, "..");
const html = `<canvas id="canvas" width="16" height="16"></canvas><script type="importmap">{"imports":{"@vellum/renderer-api":"/packages/renderer-api/src/index.ts"}}</script><script type="module">
import createModule from '/packages/renderer-skia/dist/vellum-editor.js';
import {createRenderer} from '/packages/renderer-skia/src/index.ts';
window.ready=(async()=>{let M;const renderer=await createRenderer(document.getElementById('canvas'),async options=>{M=await createModule(options);return M;});
window.fixture=async(id,invalid=false)=>{const displayList=new Uint8Array(await(await fetch('/target/shader-examples/'+id+'.bin')).arrayBuffer());const source=invalid?'half4 main(float2 p) { while(true){} return half4(1); }':await(await fetch('/examples/shaders/'+id+'.vsh')).text();renderer.draw({displayList,fonts:[],images:[],shaders:[{id,source}]});const p=M._vellum_raster_pixels();const cpu=Array.from(M.HEAPU8.slice(p,p+M._vellum_buffer_len(p)));M._vellum_free(p);return {cpu,gpu:Array.from(renderer.readPixels()),errors:renderer.shaderErrors()};};return true;})();</script>`;
const server = createServer((req, res) => {
  if (req.url === "/") {
    res.setHeader("Content-Type", "text/html");
    res.end(html);
    return;
  }
  const path = resolve(root, "." + new URL(req.url, "http://localhost").pathname);
  if (
    ![
      "packages/renderer-api/",
      "packages/renderer-skia/",
      "target/shader-examples/",
      "examples/shaders/",
    ].some((p) => path.startsWith(resolve(root, p) + sep))
  ) {
    res.writeHead(404).end();
    return;
  }
  try {
    let bytes = readFileSync(path);
    const ext = extname(path);
    if (ext === ".ts")
      bytes = Buffer.from(
        ts.transpileModule(bytes.toString(), {
          compilerOptions: { target: ts.ScriptTarget.ES2023, module: ts.ModuleKind.ESNext },
        }).outputText,
      );
    res.setHeader(
      "Content-Type",
      ext === ".wasm"
        ? "application/wasm"
        : [".js", ".ts"].includes(ext)
          ? "text/javascript"
          : "application/octet-stream",
    );
    res.end(bytes);
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
let chrome;
try {
  chrome = await launchChrome({
    extraArgs: process.argv.includes("--software")
      ? ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"]
      : [],
  });
  await chrome.cdp.send("Page.enable");
  await chrome.cdp.send("Runtime.enable");
  const loaded = chrome.cdp.once("Page.loadEventFired");
  await chrome.cdp.send("Page.navigate", { url: `http://127.0.0.1:${server.address().port}/` });
  await loaded;
  await chrome.cdp.evaluate("window.ready");
  const results = {};
  for (const id of ["checker", "tint-content"]) {
    const actual = await chrome.cdp.evaluate(`window.fixture(${JSON.stringify(id)})`);
    assert.deepEqual(actual.errors, []);
    const expected = readFileSync(resolve(root, `examples/goldens/${id}.rgba`));
    for (const backend of ["cpu", "gpu"])
      assert.deepEqual(Buffer.from(actual[backend]), expected, `${id}/${backend}`);
    results[id] = { cpu: "exact", gpu: "exact" };
  }
  const invalid = await chrome.cdp.evaluate("window.fixture('checker',true)");
  assert.equal(invalid.errors[0].kind, "unsupported");
  results.invalid = invalid.errors.map((e) => ({ src: e.src, kind: e.kind }));
  writeFileSync(
    resolve(root, "target/shader-examples/browser.json"),
    JSON.stringify(results, null, 2),
  );
  process.stdout.write(JSON.stringify(results) + "\n");
} finally {
  await chrome?.close();
  await new Promise((ok) => server.close(ok));
}
