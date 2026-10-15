# macOS 27 bindings workaround: retired before production

The spike rewrote `std___hash_table___node_allocator = std___rebind_alloc<_Traits>;` in
bindgen output. With the production data-only font configuration (CoreText/fontconfig/DirectWrite
managers disabled), the unpatched skia-bindings 0.153.3 output compiles on macOS 27.0.1.
There is no production patch and no upstream issue filed for a failure we no longer reproduce.
`scripts/skia-build.mjs` refuses this alias in generated output on every build, before success is
reported. Reintroducing a patch requires a fresh reproduction and an upstream issue URL here.
ADR-016 records the evidence and the difference from the guide. This directory's complete SHA-256
is part of the build identity, including this retirement record.
