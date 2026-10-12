#!/usr/bin/env bash
# Builds the wasm32 artifacts the script measurements use:
#   spike_script.wasm   vellum-wasm plus rquickjs and the toy doc API (this workspace)
#   vellum_wasm.wasm    the product composition root alone, the size baseline (root workspace)
#   loro_wasm.{js,wasm} the Loro measurements on wasm32-unknown-emscripten, the editor module's target
#                       (ADR-001), as a Node ES module
# rquickjs compiles QuickJS's C for wasm32 and needs a clang with the WebAssembly backend; Apple's
# clang has none, so this uses emsdk's (EMSDK, default ~/emsdk; the renderer spike needs it anyway).
set -euo pipefail
cd "$(dirname "$0")/.."
emsdk="${EMSDK:-$HOME/emsdk}"
export CC_wasm32_unknown_unknown="$emsdk/upstream/bin/clang"
export AR_wasm32_unknown_unknown="$emsdk/upstream/bin/llvm-ar"
cargo build --release -p spike-script --target wasm32-unknown-unknown
(
  export EMSDK="$emsdk" EMSDK_QUIET=1
  . "$emsdk/emsdk_env.sh"
  export EMCC_CFLAGS="-fwasm-exceptions -sSUPPORT_LONGJMP=wasm -sERROR_ON_UNDEFINED_SYMBOLS=0 --no-entry -sINVOKE_RUN=0 \
    -sMODULARIZE=1 -sEXPORT_ES6=1 -sEXPORT_NAME=createLoroSpike -sALLOW_MEMORY_GROWTH=1 \
    -sFILESYSTEM=0 -sENVIRONMENT=node -sEXPORTED_RUNTIME_METHODS=HEAPU8"
  cargo build --release -p spike-loro-wasm --target wasm32-unknown-emscripten
)
(cd ../.. && cargo build --release -p vellum-wasm --target wasm32-unknown-unknown)
