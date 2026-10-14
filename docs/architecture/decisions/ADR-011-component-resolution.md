# ADR-011 — Component props, resolved occurrences and library scope

Date: 2026-10-14. Status: implemented for Prompt 17; the person selected explicit prop bindings in this session.

## Context

ARCHITECTURE §4.1 declares props without specifying how they drive a node; ADR-010 left the choice open.
ComponentId is an opaque id and cannot encode an import alias. Repeated instances also cannot share a
variable-resolution cache keyed only by the definition's NodeId. Library data must cross an injected
boundary because vellum-core cannot read files.

## Decisions

1. Each boolean/text/number prop has optional `bindings: [{node: NodeIdPath, property: string}]`.
   Existing declarations without bindings still decode. Default and chosen values are type-checked and
   applied to the named properties. Identity and children cannot be prop targets. A text prop targeting
   `content` replaces the characters with one run using the first existing run's style; an empty text
   node has no style to inherit and reports an error. Two props cannot drive the same target property.
2. Expansion order is definition, default/chosen props, selected variant, instance overrides, slot
   content, then sparse instance common fields. Replacement roots retain their inherited logical id,
   so later overrides keep the same address. Paths can cross nested instances by including their
   definition root. Nested links survive detach. Suggestions remain available on filled slots and
   never restrict content.
3. An instance may carry `library: <import alias>` alongside `componentId`. Without it, the component
   is local. No component-id syntax or variable-id syntax changes. Definitions resolve their variables,
   theme defaults and nested components in their source document. Instance patches and owned slot,
   replacement and children payloads use the importing document's scope, including deep overrides.
4. `resolver::Resolver` returns `components::ResolvedNode`: resolved own fields, occurrence address,
   source path, instance choices, slot suggestions, problems, children and missing-library paths.
   Repeated definitions have separate occurrence addresses. Variable/theme revisions invalidate
   values per source document; effective own-field comparisons detect component and override edits.
   Unchanged own values and subtrees reuse Arcs. Expansion still visits and materializes the tree;
   this is not a measured large-document performance claim.
5. `LibrarySource` supplies decoded files at normalized paths. Imports must be relative `.lib.vellum`
   paths; sibling `..` paths are valid. Normalization is lexical, without filesystem access. Each path
   refreshes once per resolve pass. Missing, unreadable or non-library data can use a last-known valid
   copy, always flagged. An unavailable copy produces a problem. Hosts can seed and persist the cache;
   disk storage and live file watching belong to Prompts 24/28. Updating the import implements the
   core side of Locate. Starter systems use the same library interface; no default system is chosen here.
6. Paste returns ordinary ops, never applies a document or fabricates an Origin. Copies remap owned ids,
   prop/override paths, component dependencies including suggestions, variables and alias chains, and
   used styles. Variable name clashes require Use existing (same type) or Add renamed (unused name).
   Theme values travel while existing destination defaults stay first. Imports are rebased, existing
   nested external links stay linked, and library Link mode adds an import. Ordinary documents always
   copy. Font definitions travel; asset/font/script bytes are explicit host dependencies to carry or
   report before a UI completes the paste.
7. Local prop/variant override bindings are checked after typed application, once per transaction.
   Reference checks and expanded-binding checks remain separate, preserving Prompt 16's precise
   property paths. Imported-instance ops validate the alias and owned bindings; deeper library
   validation happens in the resolver, where the source is available. Existing detach/scale/replace
   operations without library input explicitly return MissingInput for imported instances, never use
   a same-id local definition. Supplying these operations with library data is a later integration.

## Evidence and limits

`resolver::components` and `resolver::libraries` contain 27 tests, including 128 generated sequences
comparing cached and fresh output after mixed edits. The canonical TEST DATA files in `fixtures/resolver/`
cover the component/slot/library parity rows. Existing component ops, variable binding-path assertions,
codec fixtures, diff/merge and store contracts remain covered by the workspace tests.

No vendor dependency or reference code was added. Guide §3.2 assigns no reference-repository row to
Prompt 17. A sibling-project search found no reusable Rust component/library resolver; this implementation
shares the existing typed NodePatch and component op machinery. UI screenshot tests do not apply to this
engine prompt. Disk caching, copying dependency bytes, starter-system distribution and editor dialogs
remain in their later prompts. No large-document timing or GPU test was performed.
