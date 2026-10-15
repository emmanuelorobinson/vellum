#!/usr/bin/env node
// Tagged wasm lane: needs Node 24, Rust wasm32-unknown-unknown and emsdk's clang/llvm-ar.
// Builds the same production adapter for native and WASM; compares data/API call transcripts.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const emsdk = process.env.EMSDK || join(homedir(), "emsdk");
const env = {
  ...process.env,
  CC_wasm32_unknown_unknown: join(emsdk, "upstream/bin/clang"),
  AR_wasm32_unknown_unknown: join(emsdk, "upstream/bin/llvm-ar"),
};
const offline = process.argv.includes("--offline") ? ["--offline"] : [];
function cargo(args) {
  const result = spawnSync("cargo", args, {
    cwd: root,
    env,
    encoding: "utf8",
    maxBuffer: 4 * 1024 * 1024,
  });
  if (result.status !== 0)
    throw new Error(result.stderr || result.error?.message || `cargo exit ${result.status}`);
  return result.stdout;
}
const native = cargo([
  "run",
  ...offline,
  "--locked",
  "--quiet",
  "-p",
  "vellum-script-quickjs",
  "--example",
  "script-native-corpus",
]);
cargo([
  "build",
  ...offline,
  "--locked",
  "--release",
  "-p",
  "vellum-script-quickjs",
  "--example",
  "script-corpus",
  "--target",
  "wasm32-unknown-unknown",
]);
const buildRoot = resolve(root, env.CARGO_TARGET_DIR || "target");
const module = new WebAssembly.Module(
  readFileSync(join(buildRoot, "wasm32-unknown-unknown/release/examples/script_corpus.wasm")),
);
const imports = WebAssembly.Module.imports(module)
  .map((x) => `${x.module}.${x.name}`)
  .sort();
assert.deepEqual(imports, ["env.__rquickjs_host_now_us", "env.vellum_script_now_ms"]);
const { exports } = new WebAssembly.Instance(module, {
  env: {
    // Internal QuickJS clock/seed: fixed and unobservable. Math.random uses our injected seed.
    __rquickjs_host_now_us: () => 0,
    // Only the interrupt handler sees this monotonic host clock.
    vellum_script_now_ms: () => performance.now(),
  },
});
exports.script_check();
const wasm = new TextDecoder().decode(
  new Uint8Array(exports.memory.buffer, exports.script_result_ptr(), exports.script_result_len()),
);
assert.equal(wasm, native, "native/WASM results and recorded API calls must be byte-identical");
const out = join(buildRoot, "script-sandbox");
mkdirSync(out, { recursive: true });
writeFileSync(join(out, "native.txt"), native);
writeFileSync(join(out, "wasm.txt"), wasm);
process.stdout.write(
  `ScriptRuntime native/WASM contracts, escape corpus, resource limits and ${native.trim().split("\n").length} deterministic fixtures passed.\n`,
);
