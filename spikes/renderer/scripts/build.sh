#!/usr/bin/env bash
# Builds everything the renderer spike page and measurements need into web/dist/ (gitignored):
#   spike_scene_wasm.wasm   Renderer A's Rust half (wasm32-unknown-unknown)
#   skia_wasm.{js,wasm}     Renderer B (wasm32-unknown-emscripten; see scripts/emscripten-env.sh)
#   textlayout/skia_wasm.*  Renderer B with Skia's textlayout feature, for the size measurement only
#   quickjs/skia_wasm.*     Renderer B with rquickjs (Prompt 5), for the size measurement only
#   spike-skia-native       the native PNG path (Skia from source with FreeType, scripts/build-native.sh)
set -euo pipefail
cd "$(dirname "$0")/.."
dist=web/dist
mkdir -p "$dist"

bash scripts/build-native.sh
cargo build --release -p spike-scene-wasm --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/spike_scene_wasm.wasm "$dist/"

out=target/wasm32-unknown-emscripten/release
(
  . scripts/emscripten-env.sh
  cargo build --release -p spike-skia-wasm --target wasm32-unknown-emscripten --features textlayout
  mkdir -p "$dist/textlayout"
  cp "$out/skia_wasm.js" "$out/skia_wasm.wasm" "$dist/textlayout/"
  # rquickjs has no shipped bindings for emscripten; bindgen needs emscripten's sysroot.
  BINDGEN_EXTRA_CLANG_ARGS_wasm32_unknown_emscripten="--sysroot=$EMSDK/upstream/emscripten/cache/sysroot" \
    cargo build --release -p spike-skia-wasm --target wasm32-unknown-emscripten --features quickjs
  mkdir -p "$dist/quickjs"
  cp "$out/skia_wasm.js" "$out/skia_wasm.wasm" "$dist/quickjs/"
  cargo build --release -p spike-skia-wasm --target wasm32-unknown-emscripten
)
cp "$out/skia_wasm.js" "$out/skia_wasm.wasm" "$dist/"
ls -l "$dist"
