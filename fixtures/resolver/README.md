# Resolver parity fixtures

All files are TEST DATA. `components.vellum` covers props and defaults, variants, the three override
forms, nested instances, per-instance slots and suggestions, and theme-bound component properties.
`imports.vellum` links to `library.lib.vellum`; the tests load it through an in-memory `LibrarySource`.

The core suite `resolver::components` covers component toggle/detach behavior through the existing ops,
including preserving nested links, plus variable-type validation and cached/fresh equality.
`resolver::libraries` covers relative and transitive imports, live refresh, flagged last-known copies,
missing and unreadable files, Locate via an updated import, Link/Copies paste, Use existing/Add renamed,
ID/binding/alias remapping and explicit asset/font dependencies. These cover ARCHITECTURE §7.4's
component, slot and library rows. UI and disk persistence belong to Prompts 47/48/52 and 24 respectively.

Canonicalize with `cargo run -p vellum-codec-json --example canonicalize -- fixtures/resolver/*.vellum`.
