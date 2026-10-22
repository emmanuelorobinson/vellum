# Accessibility, language and direction (Prompt 70)

`@vellum/i18n` owns bundled i18next resources. Each editor workspace creates an independent, synchronous
instance. No remote backend, account, language detection, prompt rewriting or translation service is used.
The owner approved English, Arabic, Simplified and Traditional Chinese, French, German, Hindi, Italian,
Japanese, Korean, Portuguese, Russian and Spanish. Settings validates the language against this list.

The current catalogs cover 127 common chrome, Settings, first-open and accessibility messages in every
language. Shared labels are wired across the chrome, dialogs and agent controls. This is **not complete
translation coverage**: advanced panel descriptions, dynamic step wording, permission/error diagnostics,
desktop Home, and standalone account/web surfaces still have English source/fallback copy. Catalog key
and interpolation parity tests do not establish full product coverage or linguistic acceptance. These
remaining strings need extraction, translation and human language review before a fully localized release.
Generated conversation text, user layer/page names, file names, code, model ids and paths are retained.

Desktop persists preferences through its local storage host. VS Code uses the webview's persisted state;
it does not put preferences in the document. Untrusted/corrupt stored data falls back to validated defaults.
The native editors still have Prompt 55's unavailable document runtime; these settings do not create a
writable runtime or implement missing host services. Appearance overrides apply to chrome and its default canvas appearance. An explicit host-supplied
canvas appearance remains independent.

## Keyboard and announcements

Tab to “Keyboard canvas mode” and activate it. On the canvas:

- Tab / Shift+Tab cycle siblings in the selected layer's parent.
- Enter selects the first child; Escape selects the parent (or clears a root selection).
- Arrow keys retain the existing Person nudge/reorder commands and write guards.
- F6 exits and returns focus to the entry button; Tab can then leave it normally.

Selection/navigation is local view state. Layers remains the accessible tree, with level, position, size,
selection and expansion semantics. Its expand/collapse arrows and panel tabs follow the chrome direction.
Ordinary canvas shortcuts retain their existing behavior outside this mode.

One persistent polite, atomic live region announces completed steps/turns even when Agent is not the open
tab. It baselines stored history and session changes quietly; streaming prose and unrelated rerenders do
not announce. Step failures/refusals and stopped/failed turns retain explicit state words.

Arabic sets the workspace's `lang` and `dir` and Base UI's DirectionProvider. Chrome positions use logical
insets; fit-to-selection swaps the physical covered insets. The canvas coordinate system and document
space overlays remain LTR. Exact values, code, paths and mono fields use isolated LTR runs. Conversation
layout mirrors, while user/model prose uses its own text direction. IBM Plex Sans Arabic is bundled with
its OFL license and asset hashes; it never requests a font service.

Both OS reduced motion and Settings → Canvas → Animations and effects off suppress descendant animations,
transitions and smooth scrolling. Forced-color mode supplies system focus/selection outlines and borders.

## Verification

Needs Chromium for the browser commands. No provider, network fixture or native document engine is needed.

```sh
pnpm --filter @vellum/editor-core test
pnpm --filter @vellum/editor-react test
pnpm --filter @vellum/i18n test
pnpm --filter @vellum/editor-react exec playwright test --config e2e/playwright.config.ts accessibility.spec.ts wcag.spec.ts --workers=1
pnpm --filter @vellum/editor-react exec playwright test --config e2e/playwright.config.ts rtl-goldens.spec.ts --workers=1
```

The axe suite covers Layers, Agent and Settings in English/Arabic × light/dark/high-contrast light/dark,
with WCAG 2 A/AA and 2.1 A/AA tags, no excluded rules. This is automated evidence for these states, not
screen-reader certification or an audit of every surface. Manual VoiceOver/NVDA, text scaling, forced-color
and installed Linux/Windows/macOS acceptance remain required.

Arabic goldens compare the immutable Fixture-RTL light/dark snapshots at 1280×800, channel delta 48,
maximum differing fraction 1%. Output and side-by-side sheets are in `target/screen-goldens/p70-rtl-*`.
They currently fail (13.19% light / 30.55% dark): Fixture-RTL's docked panels/horizontal tool group differ
from DESIGN.md's floating editor, and the fixture's full artwork is not the unavailable native runtime.
No tolerance or source image was changed. DESIGN-GAPS.md records the layout disagreement.

`pnpm tokens:lint` still fails its 20 pre-existing design-export inverse-text contrast pairs. Those failures
are retained independently of axe's passing browser checks. Full WCAG/visual release acceptance is open.

## Prior implementation reference

Reviewed `cutesycal/apps/mobile/src/i18n/index.ts` for bundled resources, synchronous first render and fallback.
Vellum keeps offline initialization and catalog parity, with one instance per workspace rather than a global.
The translator is behind a headless wrapper; React consumes an editor-local context instead of an RN binding.
No sibling source, external reference code, or design-tool implementation was ported.
