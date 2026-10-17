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
