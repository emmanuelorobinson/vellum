# Shaders and examples

Shader fills and effects are local v1.1 features, with no account or network dependency. The native
`vellum-shader-sksl::SkslCompiler` implements ShaderCompiler. `vellum-render-skia::Resources.shaders`
and `Frame.shaders` in the TS renderer API carry host-authorized source bytes; no renderer opens files.
The `.vellum` model continues to store a relative `src` plus literal or variable-bound `uniforms`.
See ADR-008 for the limits and GLSL decision.

## Dialect v1

```c
// @color tint
uniform float4 tint;
uniform float strength;
uniform float2 size;
half4 main(float2 p) {
    float2 uv = p / size;
    return half4(tint.rgb * strength, tint.a);
}
```

One `half4 main(float2 p)` entry, uniform declarations, initialized local declarations and a final
return are allowed. Supported scalar/vector constructors are float/half, float2–4/half2–4 and int;
scalar int uniforms represent booleans. `// @color name` marks a float4 as a colour. Unannotated float4
is a vector. Exposed uniforms must all have values in the document. `size` is an injected float2
and cannot be overridden. There is no automatic clock, time, randomness, image fetch or animation.

The expression-call allowlist is abs, min, max, clamp, mix, step, smoothstep, floor, ceil, fract,
mod, sin, cos, sqrt, dot, length, normalize, saturate and the constructors above. Operators and
swizzles are type-checked by Skia. No arrays, preprocessors, other functions, control-flow statements
or assignments are accepted. Statements cannot hide additional entry points. Shader values must
be finite and within ±1000000. Output colours are premultiplied, as required by Skia.

An effect declares `uniform shader content;` and can sample `content.eval(p)` exactly once without
an offset. A fill cannot declare a child shader. The host supplies the geometric node size for an
effect and paint bounds for a fill. Content is the layer's rendering including earlier effects;
other layers and destination pixels are inaccessible. Literal vectors have two, three or four values.
Only number, boolean and colour uniforms bind to variables because the document has no vector variables.

## Editor host

Create `createShaderController` with authorized `inspect`, `choose`, `importExample` and `openExample`
callbacks and pass it as `designHost.shaders` to EditorWorkspace. `inspect` compiles through the Rust
port and returns a typed ready/error result and uniform controls. Keep reserved uniforms out of the
control list. Host callbacks perform file selection, source read authorization and copying; absent
callbacks disable actions with an explicit unavailable state. Do not put source reads in UI code.
Source changes call `refresh`; epochs reject obsolete async results. Supply defaults in ShaderChoice.

Controls send one person-origin transaction through the existing DesignController. Effects use the
same controls and the ordinary add/update-effect Ops. The source chooser rejects stale selections.
Compilation errors appear on the properties controls and on affected nodes. Runtime renderer errors
are available via `Scene.shader_errors()` and `Renderer.shaderErrors()`; host inspection should include
source and uniform validation, and surface those errors using the same controller result shape.

Installed desktop/extension source selection, library opening and shader compiler callbacks await
Prompt 55's writable runtime. They are not claimed attached by this prompt. The daemon export and
screenshot tools load named shader sources and use the full native renderer. In-memory `execute` scene
reads lack a file-resource provider and report missing shader sources explicitly. Identical source
paths from distinct libraries are refused until display resources carry per-occurrence source identity.
Existing filesystem/PermissionBroker policy still applies; symlink confinement is inherited from the
host filesystem, not provided by ShaderCompiler.

## Examples and validation

`examples/catalog.json` is the gallery manifest. Sources and four analytic 16 × 16 RGBA references
are beside it. Shader imports copy dependencies before committing their source references. Opening
scripts and libraries uses the existing script/library paths and must preserve the current document.
Browsing the gallery does not commit any transaction.

With Node 24, pnpm 12, telemetry disabled and the source Skia cache:

```sh
node scripts/skia-build.mjs test -p vellum-shader-sksl --test compiler
node scripts/skia-build.mjs test -p vellum-render-skia --test shaders --test appearance --test cache
node scripts/skia-build.mjs test -p vellum --test shaders --test exports --test tools
node scripts/skia-build.mjs build --release -p vellum-wasm --bin vellum-editor --target wasm32-unknown-emscripten
node scripts/shader-goldens.mjs
pnpm --filter @vellum/editor-react exec playwright test --config e2e/playwright.config.ts shaders.spec.ts --workers=1
pnpm checksum
```

The browser renderer lane requires Chrome/WebGL2 and emsdk 4.0.6; `--software` selects SwiftShader
explicitly. It compares both WASM CPU and GPU pixels exactly with the shader references and checks
hostile source diagnostics. Native tests need no GPU/network. UI goldens require the exported C33
snapshots and retain the unchanged 1% threshold. Artifacts live in `target/shader-examples/` and
`target/screen-goldens/p67-*`. Do not regenerate analytic references from renderer output.
