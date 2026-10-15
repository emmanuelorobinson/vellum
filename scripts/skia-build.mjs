#!/usr/bin/env node
import { spawnSync } from "node:child_process";
// Source-only pipeline (ADR-016). No downloaded binary can enter the link step.
import { createHash } from "node:crypto";
import {
  appendFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const config = JSON.parse(readFileSync(join(root, "scripts/skia-config.json"), "utf8"));
export function buildKey(
  target,
  patchFiles = readdirSync(join(root, "patches/skia-bindings"))
    .sort()
    .map((n) => [n, readFileSync(join(root, "patches/skia-bindings", n), "utf8")]),
) {
  return createHash("sha256")
    .update(JSON.stringify({ target, ...config, patchFiles }))
    .digest("hex");
}
export function checkBindings(text) {
  if (text.includes("std___hash_table___node_allocator = std___rebind_alloc<_Traits>"))
    throw new Error(
      "Unpatched macOS 27 bindings regression: reproduce and file an upstream issue before adding a patch",
    );
}
function main() {
  const args = process.argv.slice(2);
  const targetAt = args.indexOf("--target");
  const host = spawnSync("rustc", ["-vV"], { encoding: "utf8" });
  if (host.status !== 0) throw new Error(host.stderr);
  const target = targetAt < 0 ? host.stdout.match(/^host: (.+)$/m)?.[1] : args[targetAt + 1];
  if (!target) throw new Error("Cannot determine target");
  const key = buildKey(target);
  if (args[0] === "--key") {
    process.stdout.write(key);
    return;
  }
  if (!args.length)
    throw new Error(
      "Usage: node scripts/skia-build.mjs <cargo arguments> | --key [--target triple]",
    );
  const env = { ...process.env, FORCE_SKIA_BUILD: "1", SKIA_GN_ARGS: config.gn };
  // VELLUM_SKIA_FROM_SOURCE=1 is supported explicitly; option A already forces this for every run.
  if (env.VELLUM_SKIA_FROM_SOURCE === "1") env.FORCE_SKIA_BUILD = "1";
  delete env.SKIA_BINARIES_URL;
  delete env.FORCE_SKIA_BINARIES_DOWNLOAD;
  // Reuse an already-synchronized source tree without git-sync-deps rewriting registry metadata.
  if (!env.SKIA_SOURCE_DIR) {
    const registry = join(process.env.CARGO_HOME || join(homedir(), ".cargo"), "registry/src");
    for (const index of existsSync(registry) ? readdirSync(registry) : []) {
      const source = join(registry, index, `skia-bindings-${config.version}`, "skia");
      if (
        existsSync(join(source, "bin/gn")) &&
        existsSync(join(source, "third_party/externals/freetype/include"))
      ) {
        env.SKIA_SOURCE_DIR = source;
        break;
      }
    }
  }
  if (target === "wasm32-unknown-emscripten") {
    const emsdk = env.EMSDK || join(homedir(), "emsdk");
    const emsdkEnv = spawnSync(
      process.platform === "win32" ? "cmd" : "bash",
      process.platform === "win32"
        ? ["/c", `"${join(emsdk, "emsdk_env.bat")}" >nul && node -p "JSON.stringify(process.env)"`]
        : [
            "-c",
            'source "$1/emsdk_env.sh" >/dev/null 2>&1 && node -p "JSON.stringify(process.env)"',
            "vellum",
            emsdk,
          ],
      { encoding: "utf8", env },
    );
    if (emsdkEnv.status !== 0)
      throw new Error(`emsdk ${config.emsdk} required: ${emsdkEnv.stderr}`);
    Object.assign(env, JSON.parse(emsdkEnv.stdout));
    env.EMCC_CFLAGS = config.emscriptenCflags;
    env.AR_wasm32_unknown_emscripten = "emar";
    // rquickjs has no bundled emscripten bindings. Bindgen needs the target headers and
    // visible declarations (emscripten otherwise hides JS_ToCStringLen2 from bindgen).
    env.BINDGEN_EXTRA_CLANG_ARGS_wasm32_unknown_emscripten = [
      env.BINDGEN_EXTRA_CLANG_ARGS_wasm32_unknown_emscripten || "",
      `--sysroot=${join(emsdk, "upstream/emscripten/cache/sysroot")}`,
      "-fvisibility=default",
    ].join(" ");
  }
  const start = performance.now();
  const result = spawnSync("cargo", args, { cwd: root, env, stdio: "inherit" });
  if (result.status !== 0) process.exit(result.status || 1);
  const buildRoot = resolve(root, env.CARGO_TARGET_DIR || "target");
  // Binding output must remain unpatched. The assertion also covers the pinned SDK in CI.
  for (const profile of ["debug", "release"]) {
    const dir = join(buildRoot, ...(targetAt < 0 ? [] : [target]), profile, "build");
    for (const entry of existsSync(dir) ? readdirSync(dir) : []) {
      const bindings = join(dir, entry, "out/skia/bindings.rs");
      if (entry.startsWith("skia-bindings-") && existsSync(bindings))
        checkBindings(readFileSync(bindings, "utf8"));
    }
  }
  mkdirSync(join(buildRoot, "skia-measurements"), { recursive: true });
  const measurement = {
    key,
    target,
    command: args,
    seconds: (performance.now() - start) / 1000,
    source: env.SKIA_SOURCE_DIR || "registry source build",
    skia: config,
  };
  writeFileSync(
    join(buildRoot, "skia-measurements", `${target}.json`),
    JSON.stringify(measurement, null, 2),
  );
  appendFileSync(
    join(buildRoot, "skia-measurements", `${target}.jsonl`),
    `${JSON.stringify(measurement)}\n`,
  );
  if (target === "wasm32-unknown-emscripten" && args[0] === "build") {
    const profile = args.includes("--release") ? "release" : "debug";
    const source = join(buildRoot, target, profile);
    const output = join(root, "packages/renderer-skia/dist");
    mkdirSync(output, { recursive: true });
    for (const file of ["vellum-editor.js", "vellum_editor.wasm"])
      writeFileSync(join(output, file), readFileSync(join(source, file)));
  }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
