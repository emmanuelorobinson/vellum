#!/usr/bin/env bash
# Builds the native path (spike-skia-native) with Skia compiled from source with FreeType
# (scripts/native-env.sh explains why).
#
# Workaround: bindgen, run against the macOS 27 SDK's libc++, emits one alias with an unbound type
# parameter (`std___hash_table___node_allocator = std___rebind_alloc<_Traits>`) and skia-bindings fails
# to compile. The alias names libc++'s hash-table node allocator, an empty type, so it is rewritten to a
# zero-sized array and the build re-run (the build script does not re-run; only rustc does).
set -euo pipefail
cd "$(dirname "$0")/.."
. scripts/native-env.sh
if cargo build --release -p spike-skia-native; then exit 0; fi
patched=0
for f in target/release/build/skia-bindings-*/out/skia/bindings.rs; do
  if grep -q 'std___hash_table___node_allocator = std___rebind_alloc<_Traits>;' "$f"; then
    sed -i.orig 's/std___hash_table___node_allocator = std___rebind_alloc<_Traits>;/std___hash_table___node_allocator = [u8; 0];/' "$f"
    echo "build-native: patched $f" >&2
    patched=1
  fi
done
[ "$patched" = 1 ] || exit 1
cargo build --release -p spike-skia-native
