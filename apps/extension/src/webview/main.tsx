/** The product editor bundle in a VS Code webview. No socket, file path or credential enters this module. */
import {
  createCommandController,
  createEditorHandlers,
  createEditorSession,
  createWorkspace,
} from "@vellum/editor-core";
import { EditorWorkspace } from "@vellum/editor-react";
import type { EngineClient, EngineSnapshot } from "@vellum/engine-client";
import type { Appearance } from "@vellum/host-api";
import { createRoot } from "react-dom/client";
import "@vellum/ui-tokens/tokens.css";
import "./theme.css";

declare function acquireVsCodeApi(): { postMessage(value: unknown): void };
const api = acquireVsCodeApi();
const rootElement = document.getElementById("root");
if (!rootElement) throw Error("missing root");
const root = createRoot(rootElement);
const send = (command: string) => api.postMessage({ type: "command", command });
// Fail closed while the compiled document ABI and live-editor IPC are unavailable. This is an unavailable
// engine, not a TypeScript DocStore. Never present source JSON as validated engine state or invent geometry.
const snapshot: EngineSnapshot = { revision: 0, document: null, readOnly: true };
const engine: EngineClient = {
  state: "ready",
  snapshot: () => snapshot,
  scene: {
    get: () => undefined,
    roots: () => [],
    hitPoint: () => null,
    hitRect: () => [],
    snap: () => ({ offset: { x: 0, y: 0 }, guides: [] }),
    toParent: (_address, point) => point,
    toLocal: (_address, point) => point,
    path: () => null,
  },
  apply: () => ({ status: "rejected", reason: "unavailable" }),
  undo: () => ({ status: "rejected", reason: "unavailable" }),
  redo: () => ({ status: "rejected", reason: "unavailable" }),
  subscribe: () => ({ dispose() {} }),
  dispose() {},
};
let nextId = 0;
const device = "vscode";
const session = createEditorSession({
  engine,
  device,
  page: "unavailable",
  nextNodeId: () => `unavailable${++nextId}`,
  createNode: () => {
    throw Error("document engine unavailable");
  },
  viewportSize: () => ({ width: innerWidth, height: innerHeight }),
});
const workspace = createWorkspace();
const controller = createCommandController(
  createEditorHandlers({ workspace, session, pauseAll: () => send("pauseAll") }),
);
interface State {
  type: "state";
  fileName: string;
  appearance: Appearance;
  matchEditorColours: boolean;
  dirty: boolean;
  daemon: boolean;
  problem: string;
}
let state: State | null = null;
const platform = /Mac/.test(navigator.userAgent)
  ? "mac"
  : /Win/.test(navigator.userAgent)
    ? "windows"
    : "linux";
function render() {
  if (!state) return;
  const current = state;
  workspace.resize({ width: innerWidth, height: innerHeight });
  root.render(
    <EditorWorkspace
      workspace={workspace}
      session={session}
      engine={engine}
      controller={controller}
      device={device}
      host="vscode"
      platform={platform}
      appearance={current.appearance}
      fileName={current.fileName}
      dirty={current.dirty}
      saveState={current.dirty ? "unsaved" : "saved"}
      branch={null}
      connection={current.daemon ? { mode: "headless" } : { mode: "offline" }}
      agents={[]}
      actors={{}}
      recent={{}}
      now={() => Date.now()}
      nextNodeId={() => `unavailable${++nextId}`}
      assets={{
        libraries: [],
        iconSets: [],
        projectIconFolder: null,
        visualStyles: [],
        designSystems: [],
      }}
      slots={{
        agent: (
          <div className="vellum-host-problem" role="status">
            <p>⊘ Document engine unavailable</p>
            <p>
              This build opens files as read-only. The compiled document engine and live editor
              connection are not available.
            </p>
            <button type="button" onClick={() => send("openAsText")}>
              Open as text
            </button>
            {!current.daemon && (
              <button type="button" onClick={() => send("restartDaemon")}>
                Restart daemon
              </button>
            )}
            {current.problem !== "document-runtime-unavailable" && (
              <p>File or host state changed. Reopen the file as text to inspect it.</p>
            )}
          </div>
        ),
      }}
      onPauseAll={() => send("pauseAll")}
    />,
  );
}
window.addEventListener("message", (event: MessageEvent<unknown>) => {
  if (!event.data || typeof event.data !== "object") return;
  const value = event.data as State;
  if (
    value.type !== "state" ||
    !["light", "dark", "hc-light", "hc-dark"].includes(value.appearance) ||
    typeof value.fileName !== "string"
  )
    return;
  state = value;
  document.documentElement.dataset.matchEditorColours = String(value.matchEditorColours === true);
  document.documentElement.dataset.appearance = value.appearance;
  render();
});
window.addEventListener("resize", render);
api.postMessage({ type: "ready" });
