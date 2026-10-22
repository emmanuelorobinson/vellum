// needs: Chrome/WebGL2 and the shared editor WASM module. Explicit test-only loopback server.
import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";
import { gzipSync } from "node:zlib";
import ts from "typescript";
import { launchChrome } from "../spikes/renderer/web/lib/chrome.mjs";
import { assess, summary } from "./performance-budget.mjs";
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
  const drags=await Promise.all(Array.from({length:30},(_,i)=>bytes('drag-'+i+'.bin')));
  const batches=await Promise.all(Array.from({length:30},(_,i)=>bytes('batch200-'+i+'.bin')));
  const samples={};basic.resize({width:1000,height:600,dpr:1});
  for(const kind of ['skia-webgl2','skia-wasm-cpu','canvas2d']) {
   const draw=frame=>{
    if(kind==='canvas2d'){basic.draw(frame);document.getElementById('basic').getContext('2d').getImageData(0,0,1,1);}
    else if(kind==='skia-wasm-cpu'){
      module._vellum_resources_clear();const upload=module._vellum_alloc(frame.displayList.length);
      if(!upload)throw Error('CPU upload failed');module.HEAPU8.set(frame.displayList,upload);
      try{if(module._vellum_scene(upload)!==1)throw Error('CPU scene failed');}finally{module._vellum_free(upload);}
      const ptr=module._vellum_raster_pixels();if(!ptr)throw Error('CPU paint failed');module._vellum_free(ptr);
    }else{gpu.draw(frame);gl.finish();}
   };
   for(const [metric,inputs] of Object.entries({panZoom:frames,drag:drags,batch200:batches})){
    // Complete real paints first, not fixed sleeps. Asset fetch is outside this draw-only measurement.
    for(let i=0;i<3;i++)draw({displayList:inputs[i],fonts:[],images:[]});
    const values=[];
    for(const displayList of inputs){const start=performance.now();draw({displayList,fonts:[],images:[]});values.push(performance.now()-start);}
    samples[kind+'.'+metric]=values;
   }
  }
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
  await chrome.cdp.evaluate("document.fonts.ready.then(()=>true)");
  if (!software) assert.doesNotMatch(info.device, /SwiftShader|llvmpipe/i);
  for (const name of manifest) {
    const actual = await chrome.cdp.evaluate(`window.fixture(${JSON.stringify(name)})`),
      native = readFileSync(resolve(output, `${name}.rgba`)),
      basic = readFileSync(resolve(output, `${name}.basic-native.rgba`));
    const repeated = await chrome.cdp.evaluate(`window.fixture(${JSON.stringify(name)})`);
    for (const renderer of ["cpu", "gpu", "basic"])
      assert.deepEqual(
        actual[renderer],
        repeated[renderer],
        `${name}/${renderer}: nondeterministic paint`,
      );
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
  writeFileSync(
    resolve(output, software ? "browser-software-results.json" : "browser-results.json"),
    JSON.stringify({ device: info.device, results, failures }, null, 2),
  );
  const samples = await chrome.cdp.evaluate("window.benchmark()");
  await chrome.cdp.evaluate("window.contract()");
  const performance = Object.fromEntries(
    Object.entries(samples).map(([name, values]) => [name, summary(values)]),
  );
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
  const budgetsReport = {
    machine: process.env.VELLUM_PERF_MACHINE ?? info.device,
    lane: "browser",
    adapters: ["skia-webgl2", "skia-wasm-cpu", "canvas2d"],
    metrics: { ...performance, wasmGzip: summary([bundle.gzipBytes]) },
  };
  writeFileSync(resolve(output, "performance.json"), JSON.stringify(budgetsReport, null, 2));
  if (process.argv.includes("--budget")) failures.push(...assess(budgetsReport));
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
