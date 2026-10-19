// TEST DATA: native webview and client-filesystem ports are injected; no real configuration is changed.
import { join } from "node:path";
import { afterEach, expect, it, vi } from "vitest";
import type { ExtensionContext } from "vscode";

const mock = vi.hoisted(() => ({
  receive: undefined as undefined | ((v: unknown) => Promise<void>),
  close: () => {},
  post: vi.fn(),
}));
vi.mock("vscode", () => ({
  Uri: { joinPath: (_uri: unknown, ...paths: string[]) => ({ toString: () => paths.join("/") }) },
  ViewColumn: { Active: 1 },
  window: {
    activeColorTheme: { kind: 1 },
    onDidChangeActiveColorTheme: () => ({ dispose() {} }),
    createWebviewPanel: () => ({
      reveal() {},
      dispose() {
        mock.close();
      },
      onDidDispose(fn: () => void) {
        mock.close = fn;
        return { dispose() {} };
      },
      webview: {
        cspSource: "local",
        asWebviewUri: (uri: unknown) => uri,
        postMessage: mock.post,
        onDidReceiveMessage(fn: typeof mock.receive) {
          mock.receive = fn;
          return { dispose() {} };
        },
        html: "",
      },
    }),
  },
  env: { clipboard: { writeText: vi.fn() } },
}));

import { AgentConnections } from "./connect";
import { ConnectView } from "./connect-view";
import type { DaemonProcess } from "./relay";

afterEach(() => {
  mock.close();
  vi.clearAllMocks();
});
function setup() {
  const read = vi.fn(async () => null);
  const write = vi.fn(async () => {});
  const service = new AgentConnections({
    home: "/test-home",
    platform: "darwin",
    configHome: "/test-home/.config",
    appData: "/test-home/appdata",
    binary: "/stable/vellum",
    read,
    write,
    exists: async () => false,
    executable: async () => false,
  });
  const daemon = {
    hostState: vi.fn(async () => ({
      clients: [],
      toolCount: 27,
      sessions: [],
      sessionsAvailable: true,
    })),
  };
  const view = new ConnectView(
    { extensionUri: {} } as ExtensionContext,
    service,
    daemon as unknown as DaemonProcess,
  );
  view.open();
  return { view, service, read, write, daemon };
}
it("ignores raw RPC, unknown clients, paths, extra fields and caller-supplied clipboard text", async () => {
  const { write, read } = setup();
  for (const value of [
    { type: "rpc", method: "tools/call" },
    { type: "change", id: "other", action: "connect" },
    { type: "change", id: "codex", action: "connect", path: "/outside" },
    { type: "copy", id: "codex", text: "injected" },
    { type: "change", id: "codex", action: "delete" },
  ])
    await mock.receive?.(value);
  expect(read).not.toHaveBeenCalled();
  expect(write).not.toHaveBeenCalled();
});
it("writes only the native client's fixed path and waits for rendered acknowledgement", async () => {
  const { view, write } = setup();
  await mock.receive?.({ type: "ready" });
  expect(view.ready).toBe(false);
  expect(mock.post).toHaveBeenCalledWith(
    expect.objectContaining({ connection: "○ 27 tools available · waiting for a client" }),
  );
  await mock.receive?.({ type: "rendered" });
  expect(view.ready).toBe(true);
  await mock.receive?.({ type: "change", id: "codex", action: "connect" });
  expect(write).toHaveBeenCalledWith(
    join("/test-home/.codex", "config.toml"),
    expect.stringContaining('command = "/stable/vellum"'),
    null,
  );
});
