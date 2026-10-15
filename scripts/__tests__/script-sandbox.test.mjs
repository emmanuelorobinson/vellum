// Tagged wasm lane. Requires emsdk clang and the Rust wasm32 target; see docs/runbooks/scripts.md.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { repo } from "./helpers.mjs";

const enabled = (process.env.VELLUM_LANES ?? "").split(",").includes("wasm");
test("production ScriptRuntime native/WASM parity and escape corpus", {
  skip: enabled ? false : "wasm lane: needs Rust wasm32 and emsdk (VELLUM_LANES=wasm)",
}, () => {
  const result = spawnSync(process.execPath, ["scripts/script-sandbox.mjs"], {
    cwd: repo,
    encoding: "utf8",
    timeout: 600000,
  });
  assert.equal(result.status, 0, result.stderr || result.error?.message);
  assert.match(result.stdout, /deterministic fixtures passed/);
});
