import {
  createCommandController,
  createEditorHandlers,
  createEditorSession,
  createWorkspace,
} from "@vellum/editor-core";
import { EditorWorkspace } from "@vellum/editor-react";
import type { EngineClient, EngineSnapshot } from "@vellum/engine-client";
import { createElectronHost } from "@vellum/host-electron";
import { createBridgeServices, type DesktopState } from "@vellum/host-electron/bridge";
import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { Home } from "./Home";
import { DocumentTabs } from "./Tabs";
import "@vellum/ui-tokens/tokens.css";
import "./style.css";

const bridge = window.vellumDesktop;
const snapshot: EngineSnapshot = { revision: 0, document: null, readOnly: true };
/** Explicit unavailable runtime. There is no TS document store or synthetic scene in the host. */
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
function makeEditor() {
  let node = 0;
  const session = createEditorSession({
    engine,
    device: "desktop",
    page: "unavailable",
    nextNodeId: () => `unavailable${++node}`,
    createNode: () => {
      throw Error("document engine unavailable");
    },
    viewportSize: () => ({ width: innerWidth, height: innerHeight }),
  });
  const workspace = createWorkspace();
  const controller = createCommandController(
    createEditorHandlers({
      workspace,
      session,
      pauseAll: async () => {
        await bridge.request({ type: "pauseAll" });
      },
    }),
  );
  return { session, workspace, controller };
}
type Editor = ReturnType<typeof makeEditor>;
const editors = new Map<string, Editor>();
const element = document.getElementById("root");
if (!element) throw Error("missing root");
const root = createRoot(element);
let host: ReturnType<typeof createElectronHost> | undefined;
let services: ReturnType<typeof createBridgeServices> | undefined;

function App() {
  const [state, setState] = useState<DesktopState | null>(null);
  const [problem, setProblem] = useState(false);
  useEffect(() => {
    const update = (state: DesktopState) => {
      document.documentElement.dataset.appearance = state.appearance;
      document.documentElement.dataset.canvasAppearance = state.appearance;
      if (!host) {
        services = createBridgeServices(bridge, { appearance: state.appearance });
        host = createElectronHost(services);
      }
      const ids = new Set(state.tabs.map((t) => t.id));
      for (const [id, editor] of editors)
        if (!ids.has(id)) {
          void editor.controller.blur();
          editors.delete(id);
        }
      for (const id of ids) if (!editors.has(id)) editors.set(id, makeEditor());
      setState(state);
    };
    let current: DesktopState | null = null;
    const receive = bridge.subscribe((event) => {
      if (event.type === "state") {
        current = event.state;
        update(current);
      }
      if (event.type === "command" && current?.active)
        void editors.get(current.active)?.controller.execute(event.command);
    });
    void bridge.request({ type: "state" }).then((result) => {
      if (result.ok && result.value) {
        current = result.value as DesktopState;
        update(current);
      }
    });
    const resize = () => setState((state) => (state ? { ...state } : null));
    window.addEventListener("resize", resize);
    return () => {
      receive();
      window.removeEventListener("resize", resize);
      host?.dispose();
      services?.dispose();
    };
  }, []);
  const request = async (type: "openFolder" | "openFiles" | "restartDaemon") => {
    setProblem(false);
    const result = await bridge.request({ type });
    if (!result.ok) setProblem(true);
  };
  if (!state) return null;
  const tab = state.tabs.find((t) => t.id === state.active);
  const editor = tab ? editors.get(tab.id) : null;
  if (!tab || !editor || (state.page && state.page !== "editor") || state.recovery)
    return <Home state={state} bridge={bridge} />;
  return (
    <main
      id={`document-${tab.id}`}
      data-platform={state.platform}
      role="tabpanel"
      aria-labelledby={`tab-${tab.id}`}
    >
      <EditorWorkspace
        workspace={editor.workspace}
        session={editor.session}
        controller={editor.controller}
        engine={engine}
        device="desktop"
        host="desktop"
        platform={state.platform}
        appearance={state.appearance}
        fileName={tab.name}
        saveState={tab.dirty ? "unsaved" : "saved"}
        dirty={tab.dirty}
        branch={null}
        connection={state.daemon ? { mode: "headless" } : { mode: "offline" }}
        agents={[]}
        actors={{}}
        recent={{}}
        now={() => Date.now()}
        nextNodeId={() => "unavailable"}
        assets={{
          libraries: [],
          iconSets: [],
          projectIconFolder: null,
          visualStyles: [],
          designSystems: [],
        }}
        onPauseAll={() => {
          void bridge.request({ type: "pauseAll" });
        }}
        slots={{
          documentBar: (
            <>
              {state.platform === "mac" && (
                <div className="desktop-titlebar-drag" aria-hidden="true" />
              )}
              <DocumentTabs state={state} bridge={bridge} />
            </>
          ),
          agent: (
            <div className="desktop-problem" role="status">
              <p>⊘ Document engine unavailable</p>
              <p>
                This build opens files as read-only. The compiled document engine and live editor
                connection are not available.
              </p>
              <button type="button" onClick={() => void request("openFiles")}>
                Open files…
              </button>
              {!state.daemon && (
                <button type="button" onClick={() => void request("restartDaemon")}>
                  Restart daemon
                </button>
              )}
              {problem && <p role="alert">The action could not be completed.</p>}
            </div>
          ),
        }}
      />
    </main>
  );
}
root.render(<App />);
