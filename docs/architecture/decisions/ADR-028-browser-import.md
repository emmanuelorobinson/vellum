# ADR-028 — Browser capture and computed HTML import

Status: implemented boundaries; native editor integration remains subject to ADR-022's runtime boundary.
Date: 2026-10-21

BrowserPort returns PNG bytes and a versioned, vendor-independent computed-layout snapshot. Chromiumoxide
0.9.1 owns a managed headless Chromium in the daemon. Electron's own Chromium serves desktop captures in
sandboxed windows without a preload, using ephemeral partitions. No browser SDK enters core or types.
The native executable is configured by the composition root (`VELLUM_CHROME`), never a tool argument.
Chromiumoxide controls its child over a temporary loopback CDP endpoint; this is not the daemon transport
or an exposed Vellum API. Electron uses its in-process debugger. The daemon still binds only its user socket.

Every requested HTTP(S) origin requires an Outside/PermissionBroker decision before headless capture.
The production MCP server shares its session store with the existing broker. Foreground terminal
consent is available when stdin and stderr are terminals; background calls record unattended denials.
Hosts may inject a trusted PermissionPrompt. A document must be opened first to establish the session.
No RPC/tool argument can carry its own approval. Desktop captures ask the person in a native dialog. Capture permits localhost; credential-bearing URLs and
non-web navigations are refused. Redirects/subrequests outside the authorized origins are blocked on the
captured page; Electron also denies permissions, downloads and new windows. Browser isolation is not a
replacement for Prompt 69's browser escape/worker/WebSocket security corpus.

A single collector reads measured boxes, computed styles, text line fragments and readable image pixels.
The HTML importer consumes this snapshot without I/O, returns standard node specs/assets and reports
unsupported styling. The existing import tool assigns ids, stores assets and commits one transaction.
No DOM, vendor format or preview pixels become the document's system of record. Font families are retained;
font files are not silently fetched into a document. Non-bundled fonts need a separate authorized import.

Embeds retain URL/device/zoom/scroll in the existing format. `build_with_embeds` accepts host-supplied preview
asset ids and clips pixels to the node radius, without mutation or fetching. Missing preview data is explicit.
Electron retains a page for live embeds and emits bounded PNG frames over the trusted bridge once per
second. Opening, stopping, changing the URL/device and closing a window dispose the session. The editor
uses a labeled, manually refreshed snapshot on hosts without streaming. Direct pointer/keyboard input
into the remote page is not exposed; interactive browsing remains in the person's browser. Native
live-session acceptance remains unverified until the Electron lane passes.
