---
title: "Open questions"
description: "The questions the architecture set to answer early: which ones are settled and by what decision, and which remain open."
---

The architecture ends with a list of questions to settle early. Some are settled, and each settled one points to the decision that settled it. The
rest are open. Listing them here means that you can see what is still undecided, and that nobody builds on an assumption that was never checked.

## Settled

- **Text in the display list.** Settled by the layout and renderer decisions. The text engine lays out the text, and the renderer draws the positioned
  glyphs. The renderer does no paragraph layout of its own.
- **The document store's granularity.** Settled by the store decision. Transactions are over the project's own types. The store is behind a contract suite,
  and undo lives in the core rather than in the store.
- **The document store at scale.** Settled by the store and file decisions. A document of 10,000 nodes opens in about 29 ms and uses about 4 MB of heap.
  The folder form is suggested at 10,000 nodes or 4 MB, and it is never required.
- **Script engine in the editor.** Settled by the store and script decisions. The script runtime adds about 0.4 MB compressed to the editor, and a script of
  500 operations takes about 0.2 ms. The same corpus gives the same results natively and in WebAssembly.
- **The renderer.** Settled by the renderer decision: Skia inside the WebAssembly module, with no CanvasKit. Vello stays on a list of conditions for revisiting.
- **The daemon's lifecycle.** Settled by the daemon decision: one daemon for each private folder, started or reused through a start lock by every host and by
  `vellum mcp`.

## Open

1. **`execute` or typed tools.** Which should the skills recommend by default, for each model size? This needs a measurement of token cost, error rate and design
   quality across two or three models. The same question applies to the compact spec syntax against JSON.
2. **Stock and generated images.** A stock provider needs a licence that allows use in a shipped product. Generated images may need a visible marker in the file.
3. **Installed engine contracts.** Claude Code and Codex change their headless modes and permission systems between releases. The plan pins the tested versions
   for each release, and detects unsupported ones at start-up. The engine pages show which versions were verified.
4. **Interoperability with other tools' formats.** Figma import through its API, and SVG paste, are settled. Reading other tools' proprietary formats needs a legal
   review first. Matching another product's capabilities is allowed. Copying its script API, prompts, skills, documentation or interface is not.
5. **Licence.** The extension is Apache-2.0, as the owner selected. The architecture proposes Apache-2.0 for the core, the command line, the format and the
   ports, and AGPL for the sync server, which protects a hosted service. That proposal is still open.

The [decision records](/docs/reference/decisions) hold the settled answers, and the [build plan](/docs/development/build-plan) page explains how open questions are
tracked during development.
