# Skills, styles and parity

```sh
vellum tool list_skills
vellum tool read_skill --path execute.md
vellum tool read_skill --path vellum-doc.d.ts
vellum tool get_style
vellum tool get_style --name quiet
cargo test -p vellum --test skills_styles
cargo test -p vellum-contracts --test content
pnpm doc-api:check
pnpm parity:check
```

These content tools work before a document is open. Read returns `{path, content}`; list returns metadata
under `skills` with `builtin`, `design_system` and `user` groups. Styles return `styles` metadata or a
`style` recipe with palette, typography, radius and density. Loading a style changes nothing.

Builtin instructions live in `packages/skills/builtin/`; `vellum-doc.d.ts` remains generated from core's
runtime registry. Edit the Markdown, never generated declarations. `skills_styles` executes every fenced
JavaScript construction example through real QuickJS and core and checks the declaration bytes match.
The six optional canvas recipes live in `packages/styles/builtin/`; they are not chrome tokens.

Hosts supply a `StackedSkills` to `Registry::with_sources`: builtin first, design-system guidelines next,
user folders last. `FileSkills::guidelines` and `FileSkills::folders` take an explicitly authorized rooted
filesystem and a list of folder paths. Each folder serves only its guidelines.md or SKILL.md. Folder styles
take explicit .json files. Source constructors grant no permission; do not expose arbitrary file reads to
agents or discover folders from their cwd. Host configuration and its permission UI arrive later.

SKILL.md requires opening/closing `---` lines, nonempty name and description, and a body. Metadata accepts
plain strings, JSON double-quoted strings, single-quoted strings (doubled quote escaping), and | or > block
scalars, optionally with -. This is a documented subset, not a general YAML loader. Bodies are returned
unchanged. Missing files, invalid metadata/recipes, invalid UTF-8 and bodies above 128 KiB fail explicitly.

`fixtures/parity/coverage.json` maps all architecture §7.4 v1 rows to named Rust tests. The harness checks
test declarations and their ignore state; CI's Rust lanes run the tests. It is an inventory, not a test
runner. Mixed release rows cover their v1 portion only. A mapped active test is evidence for that behavior,
not a promise that the complete user experience or scripted-agent E2E already exists.

When completing a later prompt, replace the row's pending test with relevant behavior tests and remove its
`pending` object. Remove the now-unused placeholder from `crates/vellum/tests/parity_pending.rs`. Every
pending test is ignored with its required prompt and fails with `not implemented yet` when explicitly run;
do not run the whole pending file with --ignored as an acceptance check. `pnpm parity:check` reports active
and pending counts, and `node scripts/parity.mjs --json` emits the inventory. New v1 rows, removed tests,
or mappings to helper functions fail `scripts/__tests__/parity.test.mjs` and the standalone command.
