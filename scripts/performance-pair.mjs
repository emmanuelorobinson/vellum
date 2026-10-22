// CI-only paired comparison: one runner, toolchain and workload against base and candidate implementations.
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, realpathSync } from "node:fs";
import { resolve, sep } from "node:path";

const root = resolve(import.meta.dirname, ".."),
  base = realpathSync(resolve(root, "target/performance-base"));
if (!base.startsWith(resolve(root, "target") + sep))
  throw Error("Baseline must be an isolated checkout under target");
const lane = process.argv[2];
if (!["native", "browser"].includes(lane)) throw Error("Choose native or browser");
// Overlay only the measurement harness. Product implementations, fixtures and dependency pins come from base.
for (const file of [
  "crates/vellum/examples/renderer-budget.rs",
  "scripts/performance-budget.mjs",
  "scripts/performance-idle.mjs",
  "scripts/renderer-goldens.mjs",
]) {
  copyFileSync(resolve(root, file), resolve(base, file));
}
const run = (cwd, script, args = []) => {
  const r = spawnSync(process.execPath, [resolve(cwd, script), ...args], {
    cwd,
    stdio: "inherit",
    env: process.env,
  });
  if (r.error) throw r.error;
  if (r.status !== 0) throw Error(`${script}: ${r.status}`);
};
for (const cwd of [base, root]) {
  if (lane === "native") {
    run(cwd, "scripts/skia-build.mjs", [
      "run",
      "--locked",
      "--release",
      "-p",
      "vellum",
      "--example",
      "renderer-budget",
    ]);
    run(cwd, "scripts/skia-build.mjs", [
      "build",
      "--locked",
      "--release",
      "-p",
      "vellum",
      "--bin",
      "vellum",
    ]);
    run(cwd, "scripts/performance-idle.mjs", [
      resolve(
        process.env.CARGO_TARGET_DIR ?? resolve(cwd, "target"),
        "release",
        process.platform === "win32" ? "vellum.exe" : "vellum",
      ),
      "target/performance/native.json",
    ]);
  } else {
    run(cwd, "scripts/skia-build.mjs", [
      "build",
      "--locked",
      "--release",
      "-p",
      "vellum-wasm",
      "--bin",
      "vellum-editor",
      "--target",
      "wasm32-unknown-emscripten",
    ]);
    run(cwd, "scripts/skia-build.mjs", [
      "build",
      "--locked",
      "--release",
      "-p",
      "vellum-wasm",
      "--no-default-features",
      "--bin",
      "vellum-viewer",
      "--target",
      "wasm32-unknown-emscripten",
    ]);
    run(cwd, "scripts/skia-build.mjs", [
      "run",
      "--locked",
      "--release",
      "-p",
      "vellum",
      "--example",
      "renderer-fixtures",
      "--",
      "target/renderer-fixtures",
    ]);
    run(cwd, "scripts/skia-build.mjs", [
      "run",
      "--locked",
      "--release",
      "-p",
      "vellum",
      "--example",
      "renderer-goldens",
    ]);
    run(cwd, "scripts/renderer-goldens.mjs", ["--software"]);
  }
  run(cwd, "scripts/spec-checksums.mjs", ["check"]);
}
const name =
  lane === "native" ? "target/performance/native.json" : "target/renderer-goldens/performance.json";
mkdirSync(resolve(root, "target/performance"), { recursive: true });
const baseline = resolve(root, `target/performance/base-${lane}.json`);
copyFileSync(resolve(base, name), baseline);
run(root, "scripts/performance-budget.mjs", [name, baseline]);
