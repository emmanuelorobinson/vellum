# ADR-035: the documentation site is a Fumadocs application built from the repository's own sources

Status: accepted for the first version (2026-10-23, requested by the owner: "build a fumadocs page as detailed as
outpost did ... for vellum"). Hosting, the public address and whether the site is versioned are not decided; they are
listed under Consequences.

## Context

The product's documentation lives in the runbooks, the decision records, the MCP tool snapshot, the format
specification and the keybinding table. Nothing presents it as one site, so a reader cannot search it, follow the
navigation, or find the page for a feature, and an agent has no index of it.

The owner's sibling project Outpost (`/Users/emmanuelejakpomewhe/Documents/kitchen/outpost`) already runs a Fumadocs
site of 64 content pages. Its structure is the pattern for this one: the App Router, a Fumadocs loader, a brand token
and a build step that checks the pages. Outpost's words, pages and tokens are not used.

Three repository rules shape the site:

- A reference page must match the code it describes (`AGENTS.md`, "Read-only files"). Copies of repository sources
  are generated on every build and never edited by hand.
- The product's name comes from `@vellum/config` (guide Appendix B), and colours come from the design tokens
  (`pnpm tokens:lint`).
- No telemetry until the person opts in (`AGENTS.md`, "Security and privacy").

## Decision

1. **Framework.** Fumadocs (`fumadocs-core` and `fumadocs-ui` 16.16.2, `fumadocs-mdx` 15.4.6) on Next.js 16.4.2 with the
   App Router, in `apps/docs` (`@vellum/docs`). Pages are Markdown. The build produces static pages, a search route
   (`/api/search`, which serves its index from the site itself) and `/llms.txt`, a list of every page with its
   description for agents.
2. **Two kinds of page.** The hand-written pages live in `apps/docs/content/docs` (71 pages). The copies of repository
   sources are written on every build by `apps/docs/scripts/docs-content.mjs` into gitignored folders: 40 runbooks, 36
   decision records, the MCP tool reference (43 tools in 8 groups), the keyboard shortcuts, the script API
   declarations, the file format specification, the file layout and the threat model. The generator stops when a tool
   belongs to no group, so a new tool cannot go undocumented without a failing build.
3. **The product's name.** Prose, headings, frontmatter and navigation titles write `{{brand}}`, which the build replaces
   from `@vellum/config`. Commands, file names, environment variables and code keep their literal names.
4. **Checks.** The build fails when a page has no title or description, a link points to a page that does not exist, a
   navigation entry has no page, a placeholder is left over, or a hand-written page breaks the copy rules: no
   exclamation marks, no claims of taste, no marketing vocabulary (`AGENTS.md`, "Visual language and copy"). The site's
   unit tests check the generator and the checks.
5. **Look.** Tailwind 4 with Fumadocs' stylesheet. Fumadocs' colour variables read the chrome tokens of
   `@vellum/ui-tokens`, so light and dark follow `data-appearance` and the site uses the editor's palette. The site's
   own code and CSS contain no hex, `rgb()` or `hsl()` colour. Inter and JetBrains Mono load from font files in the repository, not from a font service.
6. **Tracking.** None. The site has no analytics or telemetry code.
7. **Lint.** The root Biome configuration enables Tailwind directives, so `apps/docs/app/globals.css` parses.

Not chosen:

- Markdown in the repository only, read on the host. It has no search, no navigation and no index for agents.
- A static export. The search route needs the Node server, so the site runs with `next start`.
- MkDocs or Docusaurus. The sibling site uses Fumadocs, and Fumadocs runs on the React stack the editor already uses.

## Consequences

- A build step runs before `dev`, `build` and `type-check` (`docs:prepare`). It writes `.source/` and `.generated/`, both
  gitignored, and the generated pages under `content/docs`, which are gitignored too.
- Next.js and Tailwind enter the repository's catalog. The Next.js pin moves from 16.4.0 to 16.4.2 because `pnpm audit`
  reports three advisories for 16.4.0 that 16.4.1 fixes. The account app and the web viewer share that pin and type-check
  on it. Their builds were not run for this ADR. The version record is in `docs/architecture/version-baseline.md`.
- `apps/docs/lib/content-cache.mjs` is adapted from the Outpost project's helper of the same name. Outpost has no licence
  file, and `AGENTS.md` allows ports only from MIT or Apache sources. The owner decides how to attribute it, or whether
  to rewrite it. This is open in `reports.md`.
- Open: where the site is hosted and under which address, whether the site is versioned, and whether its search and its
  agent index should cover more than this repository. None of these is built.
- The site has no end-to-end test in CI. Its pages were checked in a browser by hand; see `reports.md`.
