// Loads the spike-script wasm32-unknown-unknown module in Node (no browser, no wasm-bindgen).

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const engine = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const repo = resolve(engine, "../..");
export const scriptWasm = join(engine, "target/wasm32-unknown-unknown/release/spike_script.wasm");
export const vellumWasm = join(repo, "target/wasm32-unknown-unknown/release/vellum_wasm.wasm");

export function instantiate(path = scriptWasm) {
  const module = new WebAssembly.Module(readFileSync(path));
  const imports = WebAssembly.Module.imports(module).map((i) => `${i.module}.${i.name}`);
  const instance = new WebAssembly.Instance(module, {
    // rquickjs's one host import (Date.now and friends). The spike script never reads the clock.
    env: { __rquickjs_host_now_us: () => Date.now() * 1000 },
  });
  return { imports, exports: instance.exports };
}
