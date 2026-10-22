/** The product editor bundle in a VS Code webview. No socket, file path or credential enters this module. */
import {
  createCommandController,
  createEditorHandlers,
  createEditorSession,
  createPreferences,
  createSettingsController,
  createWorkspace,
} from "@vellum/editor-core";
import { EditorWorkspace, ShareDialog } from "@vellum/editor-react";
import type { EngineClient, EngineSnapshot } from "@vellum/engine-client";
import type { Appearance } from "@vellum/host-api";
import { type SharePort, type ShareState, shareState } from "@vellum/host-api";
import { createRoot } from "react-dom/client";
import "@vellum/ui-tokens/tokens.css";
import "./theme.css";

declare function acquireVsCodeApi(): {
  postMessage(value: unknown): void;
  getState(): unknown;
  setState(value: unknown): void;
};
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
let sharing = false,
  shareSequence = 0;
const sharePending = new Map<
  number,
  { resolve: (s: ShareState) => void; timer: ReturnType<typeof setTimeout> }
>();
const sharePort: SharePort = {
  request: (request) =>
    new Promise((resolve) => {
      const requestId = ++shareSequence;
      const timer = setTimeout(() => {
        sharePending.delete(requestId);
        resolve({ state: "failed" });
      }, 30000);
      sharePending.set(requestId, { resolve, timer });
      api.postMessage({ type: "share", requestId, request });
    }),
  copy: async () => {
    api.postMessage({ type: "shareCopy" });
  },
  open: async () => {
    api.postMessage({ type: "shareOpen" });
  },
};
window.addEventListener("message", (event) => {
  const value = event.data;
  if (value?.type !== "share") return;
  const p = sharePending.get(value.requestId);
  if (!p) return;
  sharePending.delete(value.requestId);
  clearTimeout(p.timer);
  try {
    p.resolve(shareState(value.state));
  } catch {
    p.resolve({ state: "failed" });
  }
});

const platform = /Mac/.test(navigator.userAgent)
  ? "mac"
  : /Win/.test(navigator.userAgent)
    ? "windows"
    : "linux";
const preferences = createPreferences({
  async load() {
    const saved = api.getState();
    return saved && typeof saved === "object"
      ? (((saved as Record<string, unknown>).preferences as Record<string, unknown>) ?? null)
      : null;
  },
  async save(values) {
    const saved = api.getState();
    api.setState({ ...(saved && typeof saved === "object" ? saved : {}), preferences: values });
  },
});
preferences.subscribe(() =>
  api.postMessage({ type: "telemetry", enabled: preferences.get("telemetry") }),
);
void preferences.load();
const settings = createSettingsController({
  preferences,
  keys: controller.keys,
  context: { host: "vscode", platform, focus: "outside" },
});
function render() {
  if (!state) return;
  const current = state;
  workspace.resize({ width: innerWidth, height: innerHeight });
  root.render(
    <>
      <EditorWorkspace
        shell={{ settings }}
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
        onShare={() => {
          sharing = true;
          render();
        }}
        onPauseAll={() => send("pauseAll")}
        onConnectAgents={() => send("connectAgents")}
      />
      {sharing && (
        <ShareDialog
          name={current.fileName}
          port={sharePort}
          onClose={() => {
            sharing = false;
            render();
          }}
        />
      )}
    </>,
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
