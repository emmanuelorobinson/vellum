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
