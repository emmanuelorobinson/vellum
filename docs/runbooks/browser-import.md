# Browser import and embeds (Prompt 65)

BrowserPort is local and signed-out. The daemon uses Chromiumoxide 0.9.1 and an installed Chromium;
set `VELLUM_CHROME` to its executable. Desktop uses Electron's own Chromium. Neither uses the
person's browser profile. Network origins require permission; additional asset origins must be explicit.

The shared `browser` tool accepts:

```json
{"action":"screenshot","url":"http://localhost:3000/","width":1280,"height":800}
{"action":"import","url":"http://localhost:3000/","parent":"page-id"}
{"action":"import_element","url":"http://localhost:3000/","selector":"#card","parent":"page-id"}
{"action":"embed","url":"http://localhost:3000/","device":"phone","parent":"page-id","zoom":0.8}
```

Call it through MCP, `vellum tool` or a headless `vellum shell` session (see cli.md). A background daemon records unattended denials for network calls. For terminal consent, run
`vellum daemon` in a terminal (stop an existing background daemon first), keep it open, then use MCP or
`vellum shell` from another terminal. Open a document before calling browser, so a session exists for
the decision. Answer the exact-origin question in the daemon terminal; each network call asks again.
Hosts can supply their trusted PermissionPrompt through the same broker. Imports are
attributed transactions and use the target's existing save behavior (headless targets auto-save). `embed` creates only the node;
opening a document never automatically loads its URLs. Phone means 375 × 812; desktop means 1280 × 800.

File → Import opens C31 on desktop. Open page asks for network permission; its preview stays outside the
editor DOM. Pick an element by clicking its measured box or using the keyboard-accessible element list.
The installed editor still lacks Prompt 55's writable document ABI: previews work at the host boundary,
but Import as layers and Screenshot are disabled with an explanation until a host supplies the person-origin
import callbacks. No parallel document store was added. The daemon's imports use its existing asset/Op path.

The importer accepts a version-1 computed DOM snapshot (`import`, format `html`), not arbitrary HTML source.
It preserves geometry, solid colours, editable text line fragments and readable image pixels. Unsupported
background images, transforms, filters, shadows, generated/live/vector content and unreadable images are
reported. Non-bundled fonts require a separate authorized font import; a font family name is not font bytes.

Embeds retain URL/device/zoom/scroll in the existing file format. Electron's watch sessions emit bounded
preview frames once a second without reloading the page. Stop, unmount, changing inputs and closing the
window release the native page. Other hosts use an explicitly labeled snapshot. Direct page input is not
exposed. Core's `build_with_embeds` consumes derived preview asset ids and clips to the embed radius; it
never fetches or stores previews in a document. Missing preview data is an explicit render problem.

Validation (telemetry disabled):

```sh
cargo test -p vellum-import-html
cargo test -p vellum-browser-cdp --test policy
cargo test -p vellum-contracts --test browser
cargo test -p vellum-index-rstar --test engine_output
node scripts/skia-build.mjs test -p vellum --test browser_tools --test asset_tools --test mcp --test tools --test exports
VELLUM_CHROME=/path/to/chrome cargo test -p vellum-browser-cdp --test chrome -- --ignored
VELLUM_CHROME=/path/to/chrome node scripts/skia-build.mjs test -p vellum --test browser_fidelity -- --ignored
node scripts/browser-fidelity.mjs
pnpm --filter @vellum/editor-react exec playwright test --config e2e/playwright.config.ts browser.spec.ts --workers=1
node apps/desktop/test/browser-native.mjs
```

The browser fixture, pixel comparison, Playwright and native commands are tagged Chrome/Electron lanes and need process/loopback permission. Fidelity
artifacts live in `target/browser-fidelity/`; C31 design results are in `target/screen-goldens/p65-*`.
The native Electron harness has a 90-second failure timeout. Do not count its unit fake as native acceptance.

Foreground terminal consent uses the existing synchronous prompt. GUI permission-card integration and
Stop latency while awaiting terminal consent still need host acceptance; a model cannot approve itself
through a tool argument or private RPC.
