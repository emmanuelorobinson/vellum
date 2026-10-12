# Sourced by scripts/build.sh. Environment for building Renderer B (skia-safe on wasm32-unknown-emscripten).
#
# Rust 1.99's emscripten target links with -fwasm-exceptions, so Skia must use wasm setjmp/longjmp as well.
# rust-skia's prebuilt emscripten binaries for 0.153.3 use the JS-based longjmp and fail to link
# ("invoke_ functions exported but exceptions and longjmp are both disabled"), so Skia is built from
# source here (FORCE_SKIA_BUILD=1, about 1 GB of sources and a long first build; cached in target/).
#
# Needs: emsdk 4.0.6 at $EMSDK (default ~/emsdk), ninja, python3, and libclang (Xcode command line tools).
export EMSDK="${EMSDK:-$HOME/emsdk}"
export EMSDK_QUIET=1
. "$EMSDK/emsdk_env.sh"
export FORCE_SKIA_BUILD=1
export AR_wasm32_unknown_emscripten=emar
export EMCC_CFLAGS="-fwasm-exceptions -sSUPPORT_LONGJMP=wasm \
  -sERROR_ON_UNDEFINED_SYMBOLS=0 -sMAX_WEBGL_VERSION=2 -sMIN_WEBGL_VERSION=2 \
  --no-entry -sINVOKE_RUN=0 -sMODULARIZE=1 -sEXPORT_ES6=1 -sEXPORT_NAME=createSkiaSpike \
  -sALLOW_MEMORY_GROWTH=1 -sFILESYSTEM=0 -sENVIRONMENT=web \
  -sEXPORTED_RUNTIME_METHODS=GL,HEAPU8"
