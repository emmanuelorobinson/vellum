# Display-list version 1

`vellum_core::display_list::build` takes resolved page roots, the completed LayoutResult, TextEngine,
FontSource, a mutable Scene with an injected SpatialIndex, and a viewport. It returns the typed list,
painted occurrence addresses, used immutable font bytes and explicit output problems. It is read-only
with respect to document state. `DisplayList::to_bytes()` supplies the host buffer; `from_bytes()` is
the validated native reader. Types are also generated into `@vellum/types-ts`.

The viewport bounds use model pixels. Scale is zoom times device pixel ratio. The viewport's top-left
maps to device (0,0). Every `Transform` **replaces** the matrix; it does not concatenate. Established
clips stay in device space. Primitive coordinates remain local under that device transform. Save and
Layer each push state; Restore closes one. A layer's effects configure its offscreen composition in
order. Frame clips are installed after drawing the frame itself and apply to descendants.

## Header and commands

Every integer is little endian. There is no native struct padding or alignment requirement.

| Offset | Data |
| --- | --- |
| 0 | Four bytes `VDL` followed by NUL |
| 4 | u32 DISPLAY_LIST_VERSION (1) |
| 8 | u32 command count |
| 12 | u32 resource count |
| 16 | Command records, followed by resource records |

Each command starts with a u8 opcode; the following fields have no padding. Resource references are
u32 indices. Optional stroke uses `0xFFFFFFFF` for none. Geometry scalars are finite IEEE-754 f32.

| Opcode | Command | Payload |
| --- | --- | --- |
| 0 | Save | none |
| 1 | Restore | none |
| 2 | Transform | six f32, `[a,b,c,d,e,f]` |
| 3 | Clip | shape u32 |
| 4 | Path | shape, paint, optional stroke u32 |
| 5 | GlyphRun | glyphs, paint, optional stroke u32 |
| 6 | Image | image u32; destination x,y,width,height f32 |
| 7 | Effect | effect u32; immediately inside the configured Layer |
| 8 | Layer | opacity f32; blend u32 (index in BlendMode::ALL) |

The renderer must keep Layer's logical transform independent of the offscreen surface's coordinate
origin. DrawCmd::Transform always refers to the target viewport's device coordinate system.

## Resources

Each resource starts with a u32 byte length, then one binary value representing the externally tagged
Resource enum and its typed fields. The schema is the generated Resource/Shape/Render* types. Resources
are in first-use order, without hash iteration. Object keys are lexically ordered. Strings are UTF-8.

| Tag u8 | Following data |
| --- | --- |
| 0 | null |
| 1 | false |
| 2 | true |
| 3 | u64 |
| 4 | i64 |
| 5 | finite f32 |
| 6 | u32 UTF-8 byte length, then bytes |
| 7 | u32 item count, then values |
| 8 | u32 entry count, then string-key/value pairs |

Integers must never pass through Float32Array. In particular, font handles are u64 (BigInt in a JS
reader), glyph/cluster ids are u32, and variable font coordinates are signed integers. Floating zero
is normalized to positive zero. Null represents optional fields only where the typed schema allows it.
A consumer must reject unknown versions before dispatching commands. The native decoder validates
state balance and resource kinds and refuses malformed/trailing data. Maximum buffer size is 64 MiB;
maximum nested resource value depth is 128. Bump DISPLAY_LIST_VERSION for any semantic/schema change.
Font bytes are side resources keyed by their exact handle, not embedded in every display-list buffer.
An asset id refers to host-registered image bytes; this module does not load assets or access files.

## Checks

No GPU, browser, installed font, network or other external runtime is required:

```sh
cargo test --offline -p vellum-index-rstar --test engine_output
cargo test --offline -p vellum-types --test display_list
cargo test --offline -- layout_check:: display_list::
```

The guide's `cargo test layout_check:: display_list::` supplies two positional filters and Cargo
rejects it. The final command above forwards both filters to the test harness instead.

Tests cover the five layout diagnostic classes, complete clipping, transformed ancestors, occurrence
identities, problem-only projection, culling, effects outside layout bounds, large pan precision,
ordered paints/strokes/images, boolean operands, glyph metadata/font bytes, deterministic buffers,
exact large integer identities, version rejection and malformed/mutated data. Pixel output and
renderer parity are verified by the later renderer prompts.
