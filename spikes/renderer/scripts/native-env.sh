# Sourced by scripts/build.sh. Environment for building the native path (spike-skia-native).
#
# The emscripten build rasterizes glyphs with FreeType through Skia's "custom empty" font manager.
# rust-skia's prebuilt macOS binaries have neither (CoreText only; `embed-freetype` is dropped as
# redundant on macOS), and CoreText rasterizes glyphs differently and cannot read the CBDT colour emoji
# font. So the native build compiles Skia from source with the same font backend as the browser
# (Prompt 5). Needs ninja and python3; the first build takes a few minutes and is cached in target/.
export FORCE_SKIA_BUILD=1
export SKIA_GN_ARGS="skia_use_freetype=true skia_use_system_freetype2=false skia_enable_fontmgr_custom_empty=true"
