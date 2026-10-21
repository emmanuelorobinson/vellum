import {
  createBrowserImport,
  createCommandController,
  createEditorHandlers,
  createEditorSession,
  createFigmaImport,
  createWorkspace,
} from "@vellum/editor-core";
import {
  BrowserImportPanel,
  EditorWorkspace,
  FigmaImportPanel,
  ShareDialog,
} from "@vellum/editor-react";
import type { EngineClient, EngineSnapshot } from "@vellum/engine-client";
import { type SharePort, shareState } from "@vellum/host-api";
import { createElectronHost } from "@vellum/host-electron";
import {
  createBridgeServices,
  createDesktopBrowser,
  type DesktopState,
} from "@vellum/host-electron/bridge";
import { useEffect, useRef, useState } from "react";
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
  const [importing, setImporting] = useState(false);
  const [importKind, setImportKind] = useState<"browser" | "figma">("browser");
  const figmaImport = useRef(createFigmaImport({}));
  const browserImport = useRef(createBrowserImport({ browser: createDesktopBrowser(bridge) }));
  const [sharing, setSharing] = useState(false);
  const sharePorts = useRef(new Map<string, SharePort>());

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
      if (event.type === "command" && current?.active) {
        if (event.command === "file.importBrowser") setImporting(true);
        else void editors.get(current.active)?.controller.execute(event.command);
      }
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
  if (!sharePorts.current.has(tab.id))
    sharePorts.current.set(tab.id, {
      request: async (request) => {
        const reply = await bridge.request({ type: "share", tab: tab.id, request });
        if (!reply.ok) throw Error("Sharing unavailable");
        return shareState(reply.value);
      },
      copy: async (text) => {
        const r = await bridge.request({ type: "clipboardWrite", text });
        if (!r.ok) throw Error();
      },
      open: async (url) => {
        await bridge.request({ type: "external", url });
      },
    });
  const sharePort = sharePorts.current.get(tab.id);
  if (!sharePort) throw Error("Share port unavailable");
  return (
    <main
      id={`document-${tab.id}`}
      data-platform={state.platform}
      role="tabpanel"
      aria-labelledby={`tab-${tab.id}`}
    >
      {importing ? (
        <div className="desktop-browser-panel">
          <nav aria-label="Import source">
            <button
              type="button"
              onClick={() => setImportKind("browser")}
              aria-pressed={importKind === "browser"}
            >
              Website
            </button>
            <button
              type="button"
              onClick={() => setImportKind("figma")}
              aria-pressed={importKind === "figma"}
            >
              Figma
            </button>
          </nav>
          {importKind === "browser" ? (
            <BrowserImportPanel
              controller={browserImport.current}
              readOnly
              onClose={() => setImporting(false)}
            />
          ) : (
            <>
              <FigmaImportPanel controller={figmaImport.current} readOnly />
              <button type="button" onClick={() => setImporting(false)}>
                Close import
              </button>
            </>
          )}
        </div>
      ) : null}
      <EditorWorkspace
        workspace={editor.workspace}
        session={editor.session}
        controller={editor.controller}
        engine={engine}
        browser={browserImport.current.host.browser}
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
        onShare={() => setSharing(true)}
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
      {sharing && (
        <ShareDialog
          key={tab.id}
          name={tab.name}
          port={sharePort}
          onClose={() => setSharing(false)}
        />
      )}
    </main>
  );
}
root.render(<App />);
