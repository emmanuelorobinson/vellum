---
title: "Documenting the site"
description: "How this documentation site is built, which pages are written by hand and which are copied from the repository, and how to write for it."
---

This site is a Fumadocs application in `apps/docs`. It is built with Next.js, and the pages are Markdown. Most pages are written by hand. Some are
copied from the repository on every build, so that the reference never drifts from the source it describes.

## Run it

```sh
pnpm --filter @vellum/docs dev
```

Open `http://localhost:3002`. For a production build, run `pnpm --filter @vellum/docs build`, then `pnpm --filter @vellum/docs start`. The search
endpoint needs the Next.js server, so the site is not a static export.

## Where content lives

| Content | Where to edit it |
| --- | --- |
| Hand-written guides and references | `apps/docs/content/docs`, in the matching section folder |
| Navigation | The `meta.json` file in each folder |
| Runbooks | The runbook in `docs/runbooks`. Edit it there. The page is copied from it |
| Decision records | The record in `docs/architecture/decisions`. Edit it there |
| The MCP tool reference | The tool's schema snapshot, and the tool's own description. The groups are set in the generator |
| The script API, the file format and the keyboard | Their repository sources: the generated declarations, the format document and the keybinding table |
| Layout, theme and fonts | `apps/docs/app` and `apps/docs/lib` |

Never edit a generated page. The page says where it came from, and the next build overwrites it. The generator is
`apps/docs/scripts/docs-content.mjs`, and the generated folders are listed in the header of that file.

## How a build works

`pnpm docs:prepare` runs the generator, which writes the generated pages and the index for agents, and then checks every page. The checks fail the build
when a page has no title or description, when a link points nowhere, when a navigation entry has no page, when a placeholder is left, or when a
hand-written page breaks the copy rules. The Fumadocs compiler then builds the pages. The `dev`, `build` and `type-check` commands run this step for you.

## The product's name

Write `{{brand}}` wherever the product's name would appear in prose, headings, frontmatter and navigation titles. The build replaces it with the
name from the shared configuration, so a rename is one edit there. Commands, file names, environment variables and code keep their literal names,
such as `vellum`, `.vellum` and `VELLUM_CHROME`.

## Page rules

- Every page has a `title` and a `description` in its frontmatter.
- Links inside the site are absolute, starting with `/docs/`, and they point to a page that exists.
- Navigation lives in `meta.json`. A new page is not in the sidebar until it is listed there.
- Write plainly. Do not use exclamation marks, and do not make claims of taste or the vocabulary of marketing.
- Say what a feature does today. Mark a feature that is planned or partly built as planned or partly built, and link to the
  [availability](/docs/getting-started/availability) page.
- Describe a limit with its number, so a reader can check it.

## Tests

`pnpm --filter @vellum/docs test` regenerates the pages, runs the checks, and runs the site's unit tests. The tests check that every runbook and
decision record has a page, that every MCP tool is documented in a group, that the keyboard table is complete, and that the generated pages match
their sources.

## Search and the index for agents

Search is served by the site itself, from its own index. The site also serves `/llms.txt`, a plain list of every page with its description, so an
agent can find the right page without crawling the site.
