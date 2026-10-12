// Tagged `wasm` (guide §11 rule 19). Needs: the wasm32 builds from `bash scripts/build-wasm.sh`, which
// needs emsdk's clang. Runs only when VELLUM_LANES contains "wasm"; otherwise reported as skipped.
//
// rquickjs inside a vellum-wasm-sized module: the 500-op script runs in wasm32, the module imports
// nothing but rquickjs's clock, and vellum-wasm's export is still there.

import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { test } from "node:test";

import { instantiate, scriptWasm } from "../scripts/lib.mjs";

const lanes = (process.env.VELLUM_LANES ?? "").split(",").map((s) => s.trim());
const skip = lanes.includes("wasm") ? false : "wasm lane not enabled (set VELLUM_LANES=wasm)";

test("the 500-op script runs inside the wasm32 module", { skip }, () => {
  assert.ok(existsSync(scriptWasm), `${scriptWasm} is missing; run bash scripts/build-wasm.sh`);
  const { imports, exports } = instantiate();
  assert.deepEqual(imports, ["env.__rquickjs_host_now_us"]);
  assert.equal(typeof exports.vellum_format_version, "function");
  assert.equal(exports.script_run_500_cold(), 500);
  assert.equal(exports.script_run_500_warm(), 500);
  assert.equal(exports.script_run_500_warm(), 500, "a warm engine runs the script again");
});
