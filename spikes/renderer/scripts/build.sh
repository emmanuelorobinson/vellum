#!/usr/bin/env bash
# Builds everything the renderer spike page and measurements need into web/dist/ (gitignored):
#   spike_scene_wasm.wasm   Renderer A's Rust half (wasm32-unknown-unknown)
#   skia_wasm.{js,wasm}     Renderer B (wasm32-unknown-emscripten; see scripts/emscripten-env.sh)
#   textlayout/skia_wasm.*  Renderer B with Skia's textlayout feature, for the size measurement only
#   spike-skia-native       the native PNG path (prebuilt Skia binaries from rust-skia)
set -euo pipefail
cd "$(dirname "$0")/.."
dist=web/dist
mkdir -p "$dist"

cargo build --release -p spike-skia-native
cargo build --release -p spike-scene-wasm --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/spike_scene_wasm.wasm "$dist/"

out=target/wasm32-unknown-emscripten/release
(
  . scripts/emscripten-env.sh
  cargo build --release -p spike-skia-wasm --target wasm32-unknown-emscripten --features textlayout
  mkdir -p "$dist/textlayout"
  cp "$out/skia_wasm.js" "$out/skia_wasm.wasm" "$dist/textlayout/"
  cargo build --release -p spike-skia-wasm --target wasm32-unknown-emscripten
)
cp "$out/skia_wasm.js" "$out/skia_wasm.wasm" "$dist/"
ls -l "$dist"
