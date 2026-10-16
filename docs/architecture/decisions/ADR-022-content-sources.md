# ADR-022: Skills, styles and parity inventory (Prompt 31)

Status: Accepted for implementation; architecture synchronization needs a person.

## Decision

`SkillSource` and `StyleLibrary` live in core's pure ports. Their metadata and recipe structs are separate
from the document format: reading content never changes a document. Both list operations return `Result`,
rather than §3.4's `SkillSource::list -> Vec`, so missing or malformed configured folders are explicit.
`ContentError` distinguishes missing registrations, invalid content and unavailable files.

The native composition root embeds versioned Markdown from `packages/skills/builtin`, the generated
`vellum-doc.d.ts`, and six JSON recipes from `packages/styles/builtin`. Instructions are written from
scratch. Recipe values apply to canvas work only, not editor chrome or a starter design system. They are
optional starting points; existing variables, guidelines and the person's decisions take precedence.
Fonts in recipes are preferences, never claims of availability or instructions to download fonts.

`StackedSkills` receives sources in builtin → design-system → user order. Later sources own duplicate
paths, moving that entry to their position; reads never fall back to an older body after an error. Styles
stack by name in the same way. `FileSkills` loads exactly the configured `guidelines.md` or `SKILL.md`
files through an injected rooted `FileSystem`; `FolderStyles` loads explicitly registered JSON files.
There is no recursive scan, ambient home-directory discovery or unrestricted file-reading tool. Sources
refresh each call and reject empty, non-UTF-8 or over-128-KiB bodies. SKILL.md metadata supports name and
description as plain, JSON double-quoted, YAML single-quoted or block scalars, not arbitrary YAML. Other
metadata grants no permissions. Unsupported metadata forms fail explicitly.

`Registry::standard` embeds builtins. Hosts can inject configured sources with `Registry::with_sources`.
User settings, Add skill UI and PermissionBroker authorizations belong to Prompts 35 and 50; this prompt
does not create a side door for an agent to authorize a folder. The same registry serves MCP and CLI.
`read_skill` without path lists skills, as does `list_skills`; `format` and `execute` aliases retain the
paths recommended by existing tool descriptions. `get_style` without name lists metadata.

The parity inventory reads §7.4 directly, including mixed v1/v1.1 rows, excluding v1.1-only/later rows. Every
v1 row must map to existing named Rust tests. Missing, stale, duplicate or empty mappings fail. Future
capabilities map to named ignored tests that panic if explicitly run. The manifest gives their next prompt
and reason. Counts distinguish active behavior-test mappings from pending work; they do not establish
scripted-agent E2E coverage, which needs the later model and installed-engine lanes.

## Reference study (no ported code)

Codex's skills entry point separates discovery metadata from loaded content and embeds its system assets;
Vellum keeps that separation and embeds its own files without installing copies into a home directory.
Penpot's prompting guides emphasize explicit scope, actual tokens and components, bounded context and
visual review. Vellum uses those ideas with its own API and wording, document notes, atomic edits and
typed failure reporting. The sibling outpost baseline reconciles mutable skill installations with receipts;
Vellum needs no installation writes or receipt store, because builtins are embedded and folders read live.

## Revisit

At Prompts 35/50, connect the source constructors to authorized host settings and the Add skill UI. If full
YAML or recursive skill packages become a requirement, add a bounded adapter and conformance tests; do not
silently broaden the filesystem capability. Later prompts replace pending parity mappings with behavior
tests and add scripted-agent coverage using the stub model endpoint and recorded engine transcripts.
