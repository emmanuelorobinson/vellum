// needs: Chrome/WebGL2, emsdk-built editor and native parity fixtures. Explicit loopback test server.
import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";
import ts from "typescript";
import { launchChrome } from "../spikes/renderer/web/lib/chrome.mjs";
export async function runRendererGpu({ software = false } = {}) {
  const root = resolve(import.meta.dirname, "..");
  const output = resolve(root, "target/renderer-fixtures");
  for (const p of [
    "target/renderer-fixtures/native.rgba",
    "packages/renderer-skia/dist/vellum-editor.js",
  ])
    assert.ok(existsSync(resolve(root, p)), `Missing ${p}; see docs/runbooks/renderer.md`);
  const html = `<canvas id="canvas" width="256" height="256"></canvas>
<script type="importmap">{"imports":{"@vellum/renderer-api":"/packages/renderer-api/src/index.ts"}}</script>
<script type="module">
import createModule from '/packages/renderer-skia/dist/vellum-editor.js';
import {createRenderer} from '/packages/renderer-skia/src/index.ts';
import {rendererContract} from '/packages/contracts/src/renderer.ts';
window.ready=(async()=>{
const canvas=document.getElementById('canvas'); let M;
const renderer=await createRenderer(canvas,async options=>{ M=await createModule(options);return M; });
const bytes=async name=>new Uint8Array(await (await fetch('/target/renderer-fixtures/'+name)).arrayBuffer());
const fonts=[];
for (const handle of [0n,1n]) fonts.push({handle,index:0,bytes:await bytes('font-'+handle+'.ttf')});
const frame={displayList:await bytes('scene.bin'),fonts,images:[{id:"renderer-image",bytes:await bytes("image.png")}]};
renderer.draw(frame);
const gpu=renderer.readPixels();
if(M._vellum_alloc(64*1024*1024+1)!==0 || M._vellum_scene(1)!==0) throw new Error("Invalid ABI input accepted");M._vellum_free(1);
const ptr=M._vellum_raster_pixels(),len=M._vellum_buffer_len(ptr),cpu=M.HEAPU8.slice(ptr,ptr+len);M._vellum_free(ptr);
window.raster=()=>Array.from(cpu);window.gpu=()=>Array.from(gpu);
const gl=canvas.getContext('webgl2'),ext=gl.getExtension('WEBGL_debug_renderer_info');
const device=ext?gl.getParameter(ext.UNMASKED_RENDERER_WEBGL):gl.getParameter(gl.RENDERER);
window.contract=()=>rendererContract({renderer,frame});
window.recovery=async()=>{
 const extension=gl.getExtension('WEBGL_lose_context'); if(!extension) throw new Error('Context-loss extension unavailable');
 const lost=new Promise(ok=>canvas.addEventListener('webglcontextlost',ok,{once:true}));extension.loseContext();await lost;
 if(renderer.state!=='context-lost') throw new Error('Loss not observed');
 await new Promise(ok=>setTimeout(ok,50));
 const restored=new Promise(ok=>canvas.addEventListener('webglcontextrestored',ok,{once:true}));extension.restoreContext();await restored;
 if(renderer.state!=='ready') throw new Error('Restore failed');
 renderer.draw(frame);return Array.from(renderer.readPixels());
};
return {device,exports:Object.keys(M).filter(k=>k.startsWith('_vellum_'))};
})();
</script>`;
  const server = createServer((req, res) => {
    if (req.url === "/") {
      res.setHeader("Content-Type", "text/html");
      res.end(html);
      return;
    }
    const path = resolve(root, `.${new URL(req.url, "http://localhost").pathname}`);
    if (
      ![
        "packages/renderer-api/src/",
        "packages/renderer-skia/src/",
        "packages/renderer-skia/dist/",
        "packages/contracts/src/",
        "target/renderer-fixtures/",
      ].some((prefix) => path.startsWith(resolve(root, prefix) + sep))
    ) {
      res.writeHead(404).end();
      return;
    }
    try {
      let data = readFileSync(path);
      const ext = extname(path);
      if (ext === ".ts")
        data = Buffer.from(
          ts.transpileModule(data.toString(), {
            compilerOptions: { target: ts.ScriptTarget.ES2023, module: ts.ModuleKind.ESNext },
          }).outputText,
        );
      res.setHeader(
        "Content-Type",
        ext === ".ts" || ext === ".js"
          ? "text/javascript"
          : ext === ".wasm"
            ? "application/wasm"
            : "application/octet-stream",
      );
      res.end(data);
    } catch {
      res.writeHead(404).end();
    }
  });
  await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
  let chrome;
  try {
    chrome = await launchChrome({
      extraArgs: software ? ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] : [],
    });
    await chrome.cdp.send("Page.enable");
    await chrome.cdp.send("Runtime.enable");
    const loaded = chrome.cdp.once("Page.loadEventFired");
    await chrome.cdp.send("Page.navigate", { url: `http://127.0.0.1:${server.address().port}/` });
    await loaded;
    const info = await chrome.cdp.evaluate("window.ready");
    if (!software)
      assert.doesNotMatch(
        info.device,
        /SwiftShader|llvmpipe/i,
        "gpu lane requires hardware; use renderer:webgl for software integration",
      );
    const native = new Uint8Array(readFileSync(resolve(output, "native.rgba")));
    const cpu = new Uint8Array(await chrome.cdp.evaluate("window.raster()"));
    const gpu = new Uint8Array(await chrome.cdp.evaluate("window.gpu()"));
    const compare = (a, b, channel, fraction) => {
      assert.equal(a.length, b.length);
      let max = 0,
        different = 0;
      const deltas = [];
      for (let i = 0; i < a.length; i += 4) {
        let d = 0;
        for (let c = 0; c < 4; c++) d = Math.max(d, Math.abs(a[i + c] - b[i + c]));
        max = Math.max(max, d);
        deltas.push(d);
        if (d > channel) different++;
      }
      deltas.sort((a, b) => a - b);
      const p95 = deltas[Math.floor(deltas.length * 0.95)];
      assert.ok(
        different / deltas.length <= fraction && p95 <= 1,
        `${different} pixels differ; max ${max}, p95 ${p95}`,
      );
      return { max, p95, different, fraction: different / deltas.length };
    };
    writeFileSync(resolve(output, "browser-cpu.rgba"), cpu);
    writeFileSync(resolve(output, "browser-gpu.rgba"), gpu);
    const cpuDiff = compare(native, cpu, 2, 0);
    const gpuDiff = compare(native, gpu, 2, 0.02);
    writeFileSync(resolve(output, "browser-cpu.rgba"), cpu);
    writeFileSync(resolve(output, "browser-gpu.rgba"), gpu);
    const recovered = new Uint8Array(await chrome.cdp.evaluate("window.recovery()"));
    compare(gpu, recovered, 0, 0);
    await chrome.cdp.evaluate("window.contract()");
    const result = {
      device: info.device,
      cpu: cpuDiff,
      gpu: gpuDiff,
      recovery: true,
      contract: true,
    };
    writeFileSync(resolve(output, "browser-results.json"), JSON.stringify(result, null, 2));
    return result;
  } finally {
    if (chrome) await chrome.close();
    await new Promise((ok) => server.close(ok));
  }
}
if (process.argv[1] === import.meta.filename)
  console.log(
    JSON.stringify(
      await runRendererGpu({ software: process.argv.includes("--software") }),
      null,
      2,
    ),
  );
