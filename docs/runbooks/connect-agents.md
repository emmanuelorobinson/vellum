# Connect agents and extension distribution (Prompt 56)

The VS Code command **Vellum: Connect Agents** and the canvas Agents menu open C02. The native host detects
clients without executing them, reads only known configuration locations, and gives the webview generated Vellum
snippets and state. It never forwards the rest of a client file, environment variables, daemon discovery data or
raw RPC. Connect/Disconnect accept a known client id and action only. Copy copies the generated snippet.

Connect makes no changes until the person clicks its client button. It shows the written entry and destination.
Disconnect removes an exact entry pointing at this extension's stable daemon path; foreign or modified entries
produce a conflict and are preserved. Existing MCP connections may survive removal until their client exits.

`Configured` means a matching entry exists, with a restart hint. `Connected` means the daemon currently has a
connection with the client's self-reported initialize name (including known CLI aliases). This is discovery,
not authentication of a particular installed app. Tool count comes from the daemon registry; it is never a
constant. Offline and invalid-file states are explicit. Refresh polls every ten seconds while the dialog is open.
The reusable React dialog inherits the editor's portal container, or uses the document body when hosted alone.

## Client configuration baseline

User scope unless noted. `~` is the extension host's home directory; remote hosts configure the remote computer.

| Client | Path | Entry shape |
|---|---|---|
| Claude Code | `~/.claude.json` | `mcpServers.vellum`, type stdio, command, args |
| Codex, desktop and IDE | `$CODEX_HOME/config.toml` or `~/.codex/config.toml` | `[mcp_servers.vellum]`, command, args |
| Cursor | `~/.cursor/mcp.json` | `mcpServers.vellum`, command, args |
| Windsurf | existing `~/.codeium/windsurf/mcp_config.json`; current docs' `$XDG_CONFIG_HOME/devin/mcp_config.json` preferred if present, otherwise default | `mcpServers.vellum`, command, args |
| VS Code Copilot | workspace `.vscode/mcp.json`; without a folder, the default Code user `mcp.json` | `servers.vellum`, type stdio, command, args |
| Gemini CLI | `~/.gemini/settings.json` | `mcpServers.vellum`, command, args |
| Antigravity | existing `~/.gemini/antigravity/mcp_config.json`; current docs' `~/.gemini/config/mcp_config.json` preferred if present, otherwise default | `mcpServers.vellum`, command, args |
| OpenCode | `$XDG_CONFIG_HOME/opencode/opencode.jsonc` if present, otherwise `opencode.json` | current `mcp.servers.vellum`; preserve v1 `mcp.vellum` in an existing v1 file; type local, command array, enabled |
| Kiro | `~/.kiro/settings/mcp.json` | `mcpServers.vellum`, command, args |
| Claude Desktop | macOS `~/Library/Application Support/Claude/claude_desktop_config.json`; Windows `%APPDATA%/Claude/claude_desktop_config.json` | `mcpServers.vellum`, command, args |

`XDG_CONFIG_HOME` defaults to `~/.config`. Windows Windsurf uses `%APPDATA%/devin/mcp_config.json`.
`CLAUDE_CONFIG_DIR` changes Claude Code's directory. Codex uses one config for its CLI, IDE and desktop surfaces;
this does not register a remote ChatGPT web connector. Claude Desktop on Linux has no verified official path;
its conventional config-directory candidate is offered for manual review. VS Code profiles, Insiders and
fork-specific user-data paths need manual setup when no workspace is open; the displayed path is explicit.

Detection uses an existing configuration, a PATH executable or a known macOS application bundle. It does not
prove that a client version supports the displayed schema, and custom installation directories may require
manual setup. OpenCode's docs contain v1 and v2 schemas, so existing v1 files retain their layout. No installed
client integration was exercised in this prompt; fixtures cover syntax, preserving unrelated values, comments,
invalid data, conflicts and repeated actions for all ten clients. The CLI actually supports `vellum mcp`;
the design's illustrative `--workspace` flag does not exist, so it is not emitted.

JSONC uses syntax edits, preserving comments and unrelated values. Duplicate keys, invalid/oversized files,
non-object server maps, invalid UTF-8 and symlink file targets are refused. TOML is fully parsed; only our exact
managed block can be removed, avoiding table-looking text inside multiline strings. Native writes compare the
latest file with the read version, then use the existing fsync/rename writer. This is an optimistic check, not a
filesystem-wide lock against another editor writing in the small interval before rename. Failures retain the
original config and show a generic error without its contents.

The daemon copied to the extension's global-storage `bin/` has a stable path across extension updates. The copy
is replaced by rename, set executable on Unix, and contains no key/token. Opening the dialog prepares this copy;
it does not change client configs. `VELLUM_CLIENT_HOME` is an optional native environment override for isolated
host tests; it is not a workspace setting or a webview field.

Sources checked through Context7, the primary docs and pinned reference source:

- [VS Code MCP API](https://code.visualstudio.com/api/extension-guides/ai/mcp),
  [VS Code MCP configuration](https://code.visualstudio.com/docs/copilot/customization/mcp-servers),
  [publishing platform extensions](https://code.visualstudio.com/api/working-with-extensions/publishing-extension).
- [Claude Code MCP scopes](https://code.claude.com/docs/en/mcp),
  [Codex MCP configuration](https://developers.openai.com/codex/mcp),
  [Claude Desktop stdio setup](https://modelcontextprotocol.io/quickstart/user).
- [Cursor](https://cursor.com/docs/context/mcp),
  [Windsurf](https://docs.windsurf.com/windsurf/cascade/mcp),
  [Gemini configuration](https://geminicli.com/docs/get-started/configuration/),
  [Antigravity](https://antigravity.google/docs/mcp),
  [OpenCode source documentation](https://github.com/anomalyco/opencode/blob/dev/services/www/src/docs/content/mcp-servers.mdx),
  [Kiro](https://kiro.dev/docs/mcp/configuration/).

## Sidebar and metadata

The Activity Bar uses the existing bundled Lucide frame glyph, not a new logo. Recent designs are deduplicated,
bounded to twenty and stored in VS Code global state. Opening a custom document refreshes the list. The sidebar
asks `vellum/hostState` for recorded session metadata for recent files inside the current workspace. The daemon
requires initialize, bounds and scopes paths to the host connection directory, uses the same repository key as
session recording and returns at most one hundred rows. No conversations, tool arguments or document content
travel in that response; it is outside the agent tool list. Sessions use last-activity order. Offline or missing
session storage is shown explicitly. The sidebar refreshes every fifteen seconds and has a Refresh command.

Selecting a session opens its design document. Conversation/turn navigation awaits Prompt 55's runtime and the
unserved session feed from Prompt 49. Sessions for files never opened by this extension are not listed yet.
Multi-root windows currently query the first workspace root, matching Prompt 55's daemon connection scope.

## Build, package, inspect, publish

Pinned `vsce` and `ovsx` are development dependencies. Normal builds and packaging never publish. Build output
is recreated from an explicit set of bundles/assets, with bundled dependencies' full license texts. The package
contains the native daemon, static webviews, an unscoped extension manifest, README and notices; no node_modules,
config files or publishing credentials are included. Native config parsers use their ESM entries while bundling;
jsonc-parser's UMD dynamic requires failed inside the actual VS Code host.

Supported targets:

| VSIX target | Rust target |
|---|---|
| darwin-arm64 | aarch64-apple-darwin |
| darwin-x64 | x86_64-apple-darwin |
| linux-x64 | x86_64-unknown-linux-gnu |
| linux-arm64 | aarch64-unknown-linux-gnu |
| win32-x64 | x86_64-pc-windows-msvc |
| win32-arm64 | aarch64-pc-windows-msvc |

Build a source-built release daemon on the matching runner under ADR-016, then package that exact binary.
The packager validates Mach-O, ELF or PE format and machine type, refusing a universal or mismatched binary.
It does not prove binary provenance; use the source-build pipeline and keep its artifacts.

```sh
node scripts/skia-build.mjs build --release -p vellum --target aarch64-apple-darwin
node apps/extension/packaging.mjs package darwin-arm64 \
  target/aarch64-apple-darwin/release/vellum target/vsix/vellum-darwin-arm64.vsix
```

The owner selected **Apache-2.0**. Root `LICENSE` is copied into every extension build and the manifest declares
`Apache-2.0`; packaging now succeeds without a missing-license bypass. Bundled dependencies retain their own
license texts and attribution notices.

**Release remains blocked:** Prompt 55's editor runtime and C02 visual acceptance are incomplete. The publisher
namespace, release version, signed binaries and release acceptance still need the owner. The local preview uses
the debug daemon, not a release build. Other native targets were not built here.

Once these release decisions and checks are complete, publish the reviewed artifact explicitly:

```sh
node apps/extension/packaging.mjs publish marketplace path/to/reviewed.vsix
node apps/extension/packaging.mjs publish open-vsx path/to/reviewed.vsix
```

The script refuses version 0.0.0 or UNLICENSED and passes the exact VSIX path to the selected tool. Use the
registry's supported credential environment/keychain flow; never commit credentials or save them in configs.
No registry was contacted to publish during this prompt. CI/release orchestration remains Prompt 72.

## Verification

```sh
pnpm --filter @vellum/host-vscode test
pnpm --filter @vellum/extension test
pnpm --filter @vellum/editor-react test
pnpm --filter @vellum/editor-react test:e2e e2e/connect.spec.ts
node scripts/skia-build.mjs test -p vellum --test mcp --test sessions
VELLUM_VSCODE_EXECUTABLE='/Applications/Visual Studio Code - Insiders.app/Contents/MacOS/Code - Insiders' \
  pnpm --filter @vellum/extension test:vscode
```

The `vscode` lane uses private profile, storage, config and daemon directories, requires a rendered-dialog
acknowledgement under CSP and focuses both sidebar views. Setting the application's HOME to the test home
interrupted the host lane; only client configuration paths are overridden. The `chrome` lane checks interactive
Connect/Disconnect/Copy, Escape and small-window scrolling. C02 is compared at 1280×800 in light and dark;
remaining differences stay red at the recorded 1% threshold, with sheets in `target/screen-goldens/`.

Reference study (R19): the pinned vscode-samples MCP sample contributes a definition provider, refreshes through
an emitter and uses the native stdio definition API. Vellum keeps native ownership and stdio startup, uses
client-specific configuration edits for cross-editor setup and workspace mcp.json for Copilot, and never
fetches remote server definitions. Sibling auto-themer confirmed the native command/disposable registration
pattern; no reusable cross-client writer was found. No reference source was ported.
