// needs: native toolchain; browser mode also needs emsdk, Chrome/WebGL2. No network collector is used.
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const [lane = "native", report = "target/performance/native.json"] = process.argv.slice(2);
const run = (script, args = []) => {
  const result = spawnSync(process.execPath, [script, ...args], {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, TURBO_TELEMETRY_DISABLED: "1", NEXT_TELEMETRY_DISABLED: "1" },
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw Error(`${script} failed (${result.status})`);
};
try {
  if (lane === "native") {
    run("scripts/skia-build.mjs", [
      "run",
      "--locked",
      "--release",
      "-p",
      "vellum",
      "--example",
      "renderer-budget",
      "--",
      report,
    ]);
    run("scripts/skia-build.mjs", [
      "build",
      "--locked",
      "--release",
      "-p",
      "vellum",
      "--bin",
      "vellum",
    ]);
    const binary = resolve(
      process.env.CARGO_TARGET_DIR ?? "target",
      "release",
      process.platform === "win32" ? "vellum.exe" : "vellum",
    );
    run("scripts/performance-idle.mjs", [binary, report]);
    run("scripts/performance-budget.mjs", [report]);
  } else if (lane === "browser") {
    run("scripts/skia-build.mjs", [
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
    run("scripts/skia-build.mjs", [
      "run",
      "--locked",
      "--release",
      "-p",
      "vellum",
      "--example",
      "renderer-goldens",
    ]);
    run("scripts/renderer-goldens.mjs", ["--budget", ...process.argv.slice(3)]);
  } else throw Error("Usage: performance.mjs native [REPORT] | browser [--software]");
} finally {
  run("scripts/spec-checksums.mjs", ["check"]);
}
