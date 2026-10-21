# ADR-008: Bounded shaders on the shared Skia renderer

Status: Accepted for the local v1.1 implementation; native host and visual acceptance remain open.
Date: 2026-10-21

## Decision

Vellum shader dialect v1 is a documented straight-line subset of SkSL. ShaderCompiler admits the
source in pure core and the `shader-sksl` adapter invokes the pinned Skia compiler for syntax and
type validation. The renderer repeats admission before instantiating host-supplied source text;
adapters do not depend on one another. Native and browser WASM use the same renderer implementation.
Compiled/runtime objects and diagnostics are derived resources, never `.vellum` content.

Automatic GLSL ES import is deferred. A GLSL entry point, preprocessor, array, loop, helper function
or recursive call is an Unsupported/Invalid error rather than a best-effort rewrite. No naga or
new vendor dependency is needed. Files with another suffix still must contain the Vellum dialect.
A future importer must preserve the same admission and uniform contracts and get its own fixtures.

The parser caps source at 16 KiB, tokens at 2048, parentheses at 16, declarations at 32 uniforms,
statements at 64 and content sampling at one exact `content.eval(p)` per effect. No loops or helpers
can reach Skia, so per-pixel execution has a finite static bound. A render frame accepts at most
64 source resources and 128 shader draw/effect uses. These are structural work bounds, not a GPU
preemption guarantee or a claimed production latency measurement.

The host supplies source bytes; the renderer performs no filesystem or network access. Export loads
only the visible document's named dependencies through its rooted filesystem, using ShaderCompiler.
Conflicting identical relative paths from different libraries are refused. Bindings resolve through
the existing variable resolver, then typed renderer uniform validation. Missing/type-mismatched
uniforms fail explicitly. `size` is reserved and injected; effects also require the reserved `content`
child. Colours are unpremultiplied sRGB control values; shader output follows Skia's premultiplied contract.

Invalid fills become transparent in the editable scene, invalid effects retain their input, and typed
source diagnostics are returned. The editor maps compilation errors onto every affected scene node
with a symbol and label. Export and screenshots refuse incomplete shader pixels. Source bytes enter
frame-cache keys. Canvas2D retains its explicit shader/effect simplification reports.

## Evidence and limits

The compiler contract, hostile-source rejection, uniform typing, cache invalidation, source-backed
export/screenshot parity, person-origin undo and four original example pixel references run locally.
The shader lane also exercises the source-built WASM CPU and WebGL paths; commands and results live
in reports.md. UI comparisons retain the existing 1% limit. Installed native editors still depend on
Prompt 55's authoring/runtime integration; a port or a scripted UI fixture is not that acceptance.

No guide §3.2 row prescribes a shader reference implementation. Read the pinned rust-skia runtime
shader/builder/image-filter wrappers and consulted Context7's official rust-skia documentation.
A sibling search found no reusable implementation outside excluded extracted/vendor content. No
reference code was ported. Primary references: [rust-skia](https://github.com/rust-skia/rust-skia)
and [Skia runtime effects](https://skia.org/docs/user/sksl/).
