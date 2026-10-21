# ADR-030 — Public .pen interchange import

Status: implemented; installed editor authoring retains ADR-022's existing runtime dependency.
Date: 2026-10-21

The owner confirmed in chat that legal review of .pen import is complete. This satisfies the review
condition in guide §6.3 and ARCHITECTURE §15 for this reader; it does not change the separate .fig policy.

The reader is independently authored against the public [format documentation](https://docs.pen.dev/for-developers/the-pen-format),
whose live Document declaration was verified as 2.21 on this date. No private SDK, application binary,
MCP implementation, schema source, design asset, exported HTML or foreign implementation was copied.
Tests use independently authored, marked fixtures. Existing sibling projects supplied no reusable reader.

The adapter returns ordinary Imported node specs/assets. The composition root assigns new ids, stores
assets and commits one attributed Op transaction. Source ids are provenance only; .vellum remains the
system of record. Source files are never rewritten. Variables resolve under inherited/default themes;
local component references and descendant replacements expand to editable layers. Bindings/component
links and external design libraries are not retained. Conversion limitations are explicit notes.

File input and each safe relative companion image pass the existing Outside/PermissionBroker read path.
The broker canonicalizes targets, so a symlink does not inherit permission granted to another path.
Inline input has no implicit filesystem access. Inline PNG/JPEG/WebP data can be stored after existing
image-header validation. HTTP/data libraries, script files and shader files are never fetched. Unsafe
image paths are omitted. Resource failures are reported without returning private source addresses.

Script/browser layers and shader paints/effects are omitted. Foreign prompt/context/note content is
inert text, not a new executable agent request. Foreign metadata/hyperlinks are not copied. Local
references are cycle/depth/expansion bounded, and expanded own-property bytes are limited separately
from node count to prevent repeated component text from amplifying a small file without bound.

The current reader explicitly supports version 2.21 only, including documented JSON comments. Unknown
versions fail instead of guessing compatibility. Input is capped at 32 MB, source/expanded nodes at
20,000, tree depth at 64, reference/variable chains at 32, individual paths at 1 MB, expansion work/properties at 64 MB and images at
128/64 MB total. Native Ops perform final model validation. This is a partial editable conversion,
not a pixel-fidelity or round-trip guarantee. Gradients and rotated layers include review notes.

The composition root lowers registered icons through its existing catalog/insert-icon vector conversion
and injects the existing SVG parser/bounds helper for paths without an explicit view box. The adapters remain independent; no adapter imports another. Existing SVG curve-bound sampling
precision is retained, not claimed as an exact analytic bound. No new vendor dependency is introduced.
