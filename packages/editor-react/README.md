# Editor component harness

Prompt 45 scaffolds the React Storybook; product components arrive in Prompts 46–54. The token page
stays in `@vellum/ui-tokens` (HTML Storybook on port 6006); this React harness uses port 6007.

- `pnpm --filter @vellum/editor-react storybook`
- `pnpm --filter @vellum/editor-react harness:check` validates the design hash, component IDs and
  all 50 Components and Library boards, and checks the generated CSF stories.
- `pnpm --filter @vellum/editor-react build-storybook`
- `pnpm --filter @vellum/editor-react test:storybook` renders all 1,008 stories in headless Chromium.
  This is a **chrome** lane: install the pinned Playwright Chromium first.
- `pnpm --filter @vellum/editor-react test:goldens` starts a local static server over the built
  Storybook, opens each story in Chrome, and records passed/pending/failed cases in
  `target/component-goldens/results.json`. **chrome + design** lane, needs Chrome (`VELLUM_CHROME`)
  and locally exported board PNGs for implemented cases.

`fixtures/components.json` is TEST DATA read from the read-only design through Pencil MCP. Its
252 reusable component variants × four appearances identify their original reusable component, containing
board and crop in board coordinates. Instance names fall back to the reusable component's name.
No design HTML or application implementation is copied. `scripts/component-harness.mjs` generates
the explicit named CSF exports; run it after updating that fixture from a fresh read-only design
read. The generator does not read or decode the design file.

Future component prompts register a React component through `registerHarnessComponent(componentId,
component)` in a module imported by the stories. The component receives its specimen and isolated
Host, EngineClient, DaemonClient, SyncClient and Telemetry fakes. Default engine writes are refused
until a snapshot/result is scripted; there is no TypeScript document engine. Daemon/sync start
offline, telemetry is off. Release these services on unmount.

Unregistered specimens visibly say **Pending component implementation**. Their goldens are reported
as pending, never passing. Registered specimens render at their design size; the runner compares
their pixels with the crop from `docs/design/snapshots/components/<board-id>.png`. The existing
decision is 1% changed pixels, channel threshold 48. Wrong sizes, absent images, stale sources and
missing renders fail. The PNGs are generated locally through Pencil MCP and gitignored like the
other design snapshots. CI needs an exported snapshot artifact when components become ready.

Each of the 50 board IDs and its appearance is in the fixture. Export those frames as PNG at scale 1
from `.design-cache/vellum.readonly.pen` into `docs/design/snapshots/components/` after following the
repo's design-read procedure. No product component or brand choice is implemented by this prompt.

303 appearance cases have no matching reference in the design. They still have a story, marked
with `reference: "missing"`; their sizing box comes from another appearance, never a fabricated
golden. Once built, that appearance fails the golden runner until the design supplies its reference.

# Editor chrome (Prompt 46)

`EditorWorkspace` composes the floating chrome over a full-bleed canvas: document bar, tool rail, top-right bar, bottom strip,
zoom pill, the agent panel with its four tabs (Layers and Assets are built; Agent, History and the right panel are slots the
host mounts, for Prompts 47–52) and the collapsed strip. `CanvasHost` is a content canvas (the injected `Renderer` draws the
engine's display list; with none it says `data-content="unavailable"`) under an overlay canvas that draws the tool controller's
selection, handles, guides and measurements from the tokens. Menus and tooltips are Base UI; styling is `src/editor.css` over the
token variables; icons are `icons/lucide.generated.ts` (`pnpm ui-icons:gen`).

- `pnpm --filter @vellum/editor-react test` runs the state tests (server-rendered markup, no DOM).
- `pnpm --filter @vellum/editor-react test:e2e` (chrome lane; `playwright install chromium` first) runs `e2e/editor.spec.ts`
  in a Vite app (`e2e/app`) with the real workspace over a `FakeEngineClient` and a scene helper that only answers geometry
  questions. Each test says what the engine answers next and checks the transaction the UI sent. Nothing applies operations
  to a document in TypeScript. `e2e/goldens.spec.ts` compares the chrome with C03, C18 and S13 (`e2e/screens.json`) at 1280, 960
  and 760 px, light, dark and, for C03, high contrast; it skips when `docs/design/snapshots/*.png` is missing and writes
  comparison sheets to `target/screen-goldens/` when it fails.
- The harness's component specimens stay pending: every design component this prompt covers lacks its high-contrast reference
  (see the Prompt 45 note), and a built case without a reference fails its golden.

# Properties panel (Prompt 47)

`EditorWorkspace` builds the right panel when it is given a `DesignController` and a `PaintEditor` (from `@vellum/editor-core`):
`RightPanel` (Design, Inspect, Activity; the last two are slots) around `DesignTab`, which lists the sections the selection has.
Spacing is dense for frames, roomy for text and components, wide for fill, stroke and effects detail (see DESIGN-GAPS). The
panel never writes the document: every control calls `ctx.edit(...)`, which is one Person transaction through the controller.

- Controls (`design/controls.tsx`): `ValueField` (sums, arrows, scrubbing the label, Mixed, bound, disabled with its reason, a brief
  message beside the field when input is refused; Escape reverts without writing), `Segmented` (real radios in a fieldset),
  `SelectField`, `ToggleRow`, `TextField`, `VariableChip` (menu: another variable of the type, or Detach), `Section`, `PropertyRow`.
- `PaintPopover` is the fill editor (C33): Solid with Color and Variables, Gradient (linear, radial, angular with stops and angle;
  mesh with a preview, grid, point and points that are dragged on the canvas), Image (fit, crop on canvas; choosing or replacing
  a file is the host's), Shader (v1.1, read-only). It stays open while the canvas is used.
- `DesignHost` is what the app around the editor provides: `fonts`, `onGoToMain`, and `onAction` for what the panel cannot do
  (`export.add`, `image.choose`, `image.replace`, `component.goToMain`). A control that needs one is disabled without it.
- Menus, selects and popovers mount in the workspace element (`chrome/portal.tsx`), so they take its appearance.
- The canvas types text in place (`canvas/TextEditor.tsx`): a double-click on text opens it; the Text section edits the draft while
  it is open, so typing and restyling are one change; a press on the canvas, ⌘ Enter or leaving the panel ends it; Escape cancels.
- Tests: `design.test.tsx` (every control state and the panel for several selections, static markup), `layout-oracle.test.ts` (all
  187 oracle fixtures read into the panel and written back with its edits give the same layout properties), `e2e/design.spec.ts`
  (typing, scrubbing, bindings, layout and sizing, fills, the fill editor, dragging mesh points, text, components and overrides,
  effects, read-only), and goldens for C13, C14, C16, C16-instance, C33 and C33-effects in `e2e/screens.json`. Text-dense regions
  carry their own limits, with the reason, because the two engines draw glyph edges differently. Test colours live in
  `fixtures/design-scenarios.ts` (TEST DATA), which the token lint exempts.

## Prompt 48: variables and resources

Pass `resources: createResourceController({engine, device, host, nextVariableId})` to `EditorWorkspace`.
It supplies the Assets routes and registered Variables, Themes, Libraries and Manage fonts commands;
`ResourceController.open` also opens them directly. The host owns the controller and disposes it when the
editor closes. Scale and the vector inspector use the existing `EditorSession`.

`ResourceHost` is deliberately injected. `chooseLibrary` opens and validates a library through the core,
returning its alias and a path relative to this document. `chooseFont` accepts TTF/OTF/WOFF/WOFF2, stores
its original bytes through AssetStore, and returns its SHA-256 asset id and inspected `FontFace`.
`save`, `openLibrary`, installed/cached font catalogs and library/cache status come from the host.
Absent file/save actions are disabled. No UI code reads a file, downloads a font or maintains a library cache.
Google font installation, bundled-library Add/Guidelines, token import/export and Ask agent are explicit
`action` callbacks. A font catalog should contain families the host's FontSource can supply.

For clipboard paste, the host first asks the core for dependencies, conflicting variable names/types and
missing asset/font/script bytes; it presents that data with `requestPaste`. After Link/Copies and each
Existing/Renamed choice, `planPaste` invokes the Rust paste planner against the supplied latest snapshot.
It returns the ordinary `setShell`, `setVariables`, `setComponent` and structural requests; the controller
commits them in one Person transaction. No JavaScript paste planner or document store exists here.
A document revision change or dialog dismissal discards a pending picker/planner result. A successful
Locate offers Save; a refused change or failed save retains an error and the retryable state.

The Variables editor uses stable ids, same-type aliases (names or ids at input), collections by the first
name segment and grouped combinations of all theme axes (explicit error beyond 128 columns). Frame Theme
edits are patches through the existing instance-aware path. Numeric sizing menus and Bind variable chips
can create bindings. Font usage counts editable source text, including local component definitions;
it does not count expanded occurrences, slot payloads or text replacements inside instance overrides.
Replacement changes matching source text runs; it does not rewrite library definitions or instance content.

Path coordinates use the engine Scene's inverse mapping, inverted for the overlay; point/handle dragging,
point coordinates and Done are wired. The current path model has no per-point radius, so that field is
disabled with a reason. Segment hits (bend/cut/add-point on the canvas) come through `ToolScene.hitPath` in world coordinates,
including the segment parameter. Older bindings may omit it, making those gestures unavailable; point
handles still work. The compiled host must supply that method; there is no second path hit-testing engine in the UI.

`e2e/resources.spec.ts` uses scripted engine replies and host results; Rust scale/theme/library regression
suites verify the underlying operations separately. The browser tests do not prove a native font/file
picker, disk save/cache or compiled WASM content rendering. `resources.stories.tsx` covers 15 states.
The new screen goldens retain the design decision's 1% limit; remaining mismatches are reported in reports.md,
with comparison sheets under `target/screen-goldens/` when a comparison fails.
