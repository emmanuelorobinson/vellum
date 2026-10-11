// needs: rust target wasm32-unknown-unknown (installed by rust-toolchain.toml) and Node's WebAssembly.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

test("vellum-wasm builds for the browser target with a placeholder export", () => {
  const build = spawnSync(
    "cargo",
    ["build", "-p", "vellum-wasm", "--target", "wasm32-unknown-unknown"],
    { cwd: repo, encoding: "utf8" },
  );
  assert.equal(build.status, 0, build.stderr);

  const bytes = readFileSync(
    join(repo, "target", "wasm32-unknown-unknown", "debug", "vellum_wasm.wasm"),
  );
  const module = new WebAssembly.Module(bytes);
  assert.ok(
    WebAssembly.Module.exports(module).some((e) => e.name === "vellum_format_version"),
    "wasm module must export vellum_format_version",
  );
  assert.deepEqual(WebAssembly.Module.imports(module), [], "the placeholder needs no host imports");

  const instance = new WebAssembly.Instance(module, {});
  assert.equal(instance.exports.vellum_format_version(), 1);
});
