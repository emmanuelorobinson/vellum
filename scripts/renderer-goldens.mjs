// needs: Chrome/WebGL2 and the shared editor WASM module. Explicit test-only loopback server.
import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";
import { gzipSync } from "node:zlib";
import ts from "typescript";
import { launchChrome } from "../spikes/renderer/web/lib/chrome.mjs";
import { comparePixels } from "./renderer-golden-compare.mjs";

const root = resolve(import.meta.dirname, ".."),
  output = resolve(root, "target/renderer-goldens");
const manifest = JSON.parse(readFileSync(resolve(output, "manifest.json")));
const tolerances = JSON.parse(readFileSync(resolve(root, "fixtures/rendering/tolerances.json")));
assert.deepEqual(
  Object.keys(tolerances).sort(),
  [...manifest].sort(),
  "Every fixture needs explicit tolerances",
);
const html = `<canvas id="gpu" width="256" height="256"></canvas><canvas id="basic" width="256" height="256"></canvas>
<script type="importmap">{"imports":{"@vellum/renderer-api":"/packages/renderer-api/src/index.ts"}}</script>
<script type="module">
import createModule from '/packages/renderer-skia/dist/vellum-editor.js';
import {createRenderer} from '/packages/renderer-skia/src/index.ts';
import {createRenderer as createBasic,createCompiler} from '/packages/renderer-canvas2d/src/index.ts';
import {rendererContract} from '/packages/contracts/src/renderer.ts';
window.ready=(async()=>{
 const canvas=document.getElementById('gpu');let module;
 const gpu=await createRenderer(canvas,async options=>{module=await createModule(options);return module;});
 const basicModule=await createModule({noInitialRun:true});
 const basic=createBasic(document.getElementById('basic'),createCompiler(basicModule));
 const bytes=async name=>new Uint8Array(await(await fetch('/target/renderer-goldens/'+name)).arrayBuffer());
 const gl=canvas.getContext('webgl2'),ext=gl.getExtension('WEBGL_debug_renderer_info');
 const device=ext?gl.getParameter(ext.UNMASKED_RENDERER_WEBGL):gl.getParameter(gl.RENDERER);
 window.fixture=async name=>{
  const frame={displayList:await bytes(name+'.bin'),fonts:[],images:[]};
  if(name==='text-images-strokes'){for(const handle of [0n,1n])frame.fonts.push({handle,index:0,bytes:await bytes('font-'+handle+'.ttf')});frame.images.push({id:'renderer-image',bytes:await bytes('image.png')});}
  gpu.draw(frame);
  const ptr=module._vellum_raster_pixels(),cpu=module.HEAPU8.slice(ptr,ptr+module._vellum_buffer_len(ptr));module._vellum_free(ptr);
  basic.draw(frame);return {gpu:Array.from(gpu.readPixels()),cpu:Array.from(cpu),basic:Array.from(basic.readPixels()),simplifications:basic.simplifications};
 };
 window.png=pixels=>{const canvas=document.createElement('canvas');canvas.width=256;canvas.height=256;canvas.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(pixels),256,256),0,0);return canvas.toDataURL('image/png').split(',')[1];};
 window.contract=async()=>{rendererContract({renderer:basic,frame:{displayList:await bytes('solid.bin'),fonts:[],images:[]}});};
 window.benchmark=async()=>{
  gpu.resize({width:1000,height:600,dpr:1});
  const frames=await Promise.all(Array.from({length:60},(_,i)=>bytes('performance-'+i+'.bin')));
  const samples={panZoom:[],cachedTiles:[],drag:[],batch200:[]};
  for(let i=0;i<60;i++){const start=performance.now();gpu.draw({displayList:frames[i],fonts:[],images:[],zoom:0.4+i*0.001});gl.finish();samples.panZoom.push(performance.now()-start);}
  const frame={displayList:frames[59],fonts:[],images:[],zoom:0.459};
  for(let i=0;i<60;i++){const start=performance.now();gpu.draw(frame);gl.finish();samples.cachedTiles.push(performance.now()-start);}
  const drags=await Promise.all(Array.from({length:30},(_,i)=>bytes('drag-'+i+'.bin')));for(let i=0;i<30;i++){const start=performance.now();gpu.draw({displayList:drags[i],fonts:[],images:[]});gl.finish();samples.drag.push(performance.now()-start);}
  // Display-list upload, preparation and draw only. Fetch and core ops/layout are measured separately.
  const batches=await Promise.all(Array.from({length:30},(_,i)=>bytes('batch200-'+i+'.bin')));for(let i=0;i<30;i++){const start=performance.now();gpu.draw({displayList:batches[i],fonts:[],images:[]});gl.finish();samples.batch200.push(performance.now()-start);}
  return samples;
 };
 return {device};})();</script>`;
const server = createServer((req, res) => {
  if (req.url === "/") {
    res.setHeader("Content-Type", "text/html");
    res.end(html);
    return;
  }
  let path = resolve(root, `.${new URL(req.url, "http://localhost").pathname}`);
  if (
    ![
      "packages/renderer-api/src/",
      "packages/renderer-skia/src/",
      "packages/renderer-skia/dist/",
      "packages/renderer-canvas2d/src/",
      "packages/contracts/src/",
      "target/renderer-goldens/",
    ].some((p) => path.startsWith(resolve(root, p) + sep))
  ) {
    res.writeHead(404).end();
    return;
  }
  if (!existsSync(path) && !extname(path)) path += ".ts";
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
      [".ts", ".js"].includes(ext)
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
const results = {},
  failures = [];
try {
  const software = process.argv.includes("--software");
  chrome = await launchChrome({
    extraArgs: software ? ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] : [],
  });
  await chrome.cdp.send("Page.enable");
  await chrome.cdp.send("Runtime.enable");
  const loaded = chrome.cdp.once("Page.loadEventFired");
  await chrome.cdp.send("Page.navigate", { url: `http://127.0.0.1:${server.address().port}/` });
  await loaded;
  const info = await chrome.cdp.evaluate("window.ready");
  if (!software) assert.doesNotMatch(info.device, /SwiftShader|llvmpipe/i);
  for (const name of manifest) {
    const actual = await chrome.cdp.evaluate(`window.fixture(${JSON.stringify(name)})`),
      native = readFileSync(resolve(output, `${name}.rgba`)),
      basic = readFileSync(resolve(output, `${name}.basic-native.rgba`));
    results[name] = { simplifications: actual.simplifications };
    for (const renderer of ["cpu", "gpu", "basic"]) {
      const pixels = Uint8Array.from(actual[renderer]),
        expected = renderer === "basic" ? basic : native;
      const { diff, ...result } = comparePixels(expected, pixels, tolerances[name][renderer]);
      results[name][renderer] = result;
      writeFileSync(resolve(output, `${name}.${renderer}.rgba`), pixels);
      if (!result.pass) {
        failures.push(`${name}/${renderer}: ${JSON.stringify(result)}`);
        if (diff) {
          const png = await chrome.cdp.evaluate(`window.png(${JSON.stringify(Array.from(diff))})`);
          writeFileSync(
            resolve(output, `${name}.${renderer}.diff.png`),
            Buffer.from(png, "base64"),
          );
        }
      }
    }
  }
  await chrome.cdp.evaluate("window.contract()");
  writeFileSync(
    resolve(output, software ? "browser-software-results.json" : "browser-results.json"),
    JSON.stringify({ device: info.device, results, failures }, null, 2),
  );
  const samples = await chrome.cdp.evaluate("window.benchmark()");
  const performance = {};
  for (const [name, values] of Object.entries(samples)) {
    values.sort((a, b) => a - b);
    performance[name] = {
      p50: values[Math.floor(values.length * 0.5)],
      p95: values[Math.floor(values.length * 0.95)],
      max: Math.max(...values),
    };
  }
  const bundle = {
    bytes: readFileSync(resolve(root, "packages/renderer-skia/dist/vellum_editor.wasm")).length,
    gzipBytes: gzipSync(
      readFileSync(resolve(root, "packages/renderer-skia/dist/vellum_editor.wasm")),
    ).length,
  };
  bundle.loaderBytes = readFileSync(
    resolve(root, "packages/renderer-skia/dist/vellum-editor.js"),
  ).length;
  bundle.loaderGzipBytes = gzipSync(
    readFileSync(resolve(root, "packages/renderer-skia/dist/vellum-editor.js")),
  ).length;
  const report = { device: info.device, results, performance, bundle, failures };
  writeFileSync(
    resolve(output, software ? "browser-software-results.json" : "browser-results.json"),
    JSON.stringify(report, null, 2),
  );
  console.log(
    JSON.stringify(
      { device: info.device, fixtures: manifest.length, performance, bundle, failures },
      null,
      2,
    ),
  );
  assert.equal(failures.length, 0, "Golden suite failed; see target/renderer-goldens/*diff.png");
} finally {
  if (chrome) await chrome.close();
  await new Promise((ok) => server.close(ok));
}
