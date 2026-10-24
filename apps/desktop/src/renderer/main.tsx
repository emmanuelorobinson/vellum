import { brand } from "@vellum/config";
import {
  createBrowserImport,
  createComposerController,
  createFigmaImport,
  createLivePresence,
  createPreferences,
  createPresenceController,
  createSettingsController,
  DaemonAgentFeed,
} from "@vellum/editor-core";
import {
  BrowserImportPanel,
  createLiveEditor,
  EditorWorkspace,
  FigmaImportPanel,
  type LiveEditor,
  ShareDialog,
} from "@vellum/editor-react";
import { EditorReplica, openReplica, type SharePort, shareState } from "@vellum/host-api";
import { createElectronHost } from "@vellum/host-electron";
import {
  createBridgeServices,
  createDesktopBrowser,
  type DesktopState,
} from "@vellum/host-electron/bridge";
import { type EngineModule, isEngineModule, WasmDocumentEngine } from "@vellum/renderer-skia";
import createEditor from "@vellum/renderer-skia/editor-module";
import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { Home } from "./Home";
import { DocumentTabs } from "./Tabs";
import "@vellum/ui-tokens/tokens.css";
import "./style.css";

const bridge = window.vellumDesktop;
const preferences = createPreferences({
  async load() {
    return JSON.parse(localStorage.getItem(`${brand.desktopAppId}.preferences`) ?? "null");
  },
  async save(values) {
    localStorage.setItem(`${brand.desktopAppId}.preferences`, JSON.stringify(values));
  },
});
const syncTelemetry = () => {
  void bridge.request({ type: "telemetry", enabled: preferences.get("telemetry") });
};
preferences.subscribe(syncTelemetry);
void preferences.load();

interface EngineLoad {
  live: boolean;
  device: string;
  path: string;
  file: Uint8Array | null;
}
interface Editor {
  live: LiveEditor;
  replica: EditorReplica | null;
  settings: ReturnType<typeof createSettingsController>;
  composer?: ReturnType<typeof createComposerController>;
  /** Where an agent is and what it is doing, read off the running turn and the changes it makes. */
  presence?: ReturnType<typeof createPresenceController>;
  actors: ReturnType<typeof createLivePresence>["actors"];
  disposeFeed(): void;
}
/** One editor module for the window; each tab is an engine in it (ADR-033). */
let moduleLoad: Promise<{ module: EngineModule; fonts: Uint8Array[] }> | undefined;
function loadModule() {
  moduleLoad ??= bridge.request({ type: "engineModule" }).then(async (reply) => {
    if (!reply.ok || !reply.value) throw Error("The document engine could not start");
    const { wasm, fonts } = reply.value as { wasm: Uint8Array; fonts: Uint8Array[] };
    const module = await createEditor({
      canvas: document.createElement("canvas"),
      noInitialRun: true,
      wasmBinary: wasm,
      locateFile: (path) => path,
    });
    if (!isEngineModule(module)) throw Error("The editor module has no document engine");
    return { module, fonts };
  });
  moduleLoad.catch(() => {
    moduleLoad = undefined;
  });
  return moduleLoad;
}
async function makeEditor(tab: string, platform: DesktopState["platform"]): Promise<Editor> {
  const reply = await bridge.request({ type: "engineLoad", tab });
  if (!reply.ok || !reply.value) throw Error("The document engine could not start");
  const load = reply.value as EngineLoad;
  const { module, fonts } = await loadModule();
  let replica: EditorReplica | null = null;
  let engine: WasmDocumentEngine;
  if (load.live) {
    const wire = async (
      action: "open" | "push" | "pull" | "close",
      params: Record<string, string>,
    ) => {
      const answer = await bridge.request({ type: "engine", tab, action, params });
      if (!answer.ok) throw Error("The daemon refused the request");
      return answer.value as Record<string, unknown>;
    };
    const start = await openReplica(wire);
    engine = WasmDocumentEngine.open(module, {
      file: start.genesis,
      updates: start.updates,
      path: load.path,
      readOnly: false,
      fonts,
    });
    const live = engine;
    replica = new EditorReplica(engine, wire, {
      // Edits the daemon did not take stay on screen; no more are made until the file is reopened.
      onFailure: () => live.setReadOnly(true),
    });
    engine.subscribe(() => void replica?.push());
    replica.start();
  } else {
    if (!load.file) throw Error("No file to show");
    engine = WasmDocumentEngine.open(module, {
      file: load.file,
      path: load.path,
      readOnly: true,
      fonts,
    });
  }
  const feed = new DaemonAgentFeed(
    async (action, params) => {
      const reply = await bridge.request({
        type: "agentFeed",
        tab,
        action: action as never,
        params,
      });
      if (!reply.ok)
        throw Error("The agent request failed. Check the daemon and installed engine.");
      return reply.value;
    },
    { device: load.device as never },
  );
  const unsubscribeFeed = bridge.subscribe((event) => {
    if (event.type === "agentText" && event.tab === tab)
      feed.live(event.session as never, event.turn as never, { type: "text", text: event.text });
  });
  const live = createLiveEditor({
    module,
    engine,
    device: load.device || "01JA0000000000000000000000",
    pauseAll: () => void bridge.request({ type: "pauseAll" }),
    ...(load.live ? { agentFeed: feed } : {}),
    preferences,
  });
  const settings = createSettingsController({
    preferences,
    keys: live.controller.keys,
    context: { host: "desktop", platform, focus: "outside" },
  });
  const composer = live.agent
    ? createComposerController({
        agent: live.agent,
        feed,
        defaultEngine: { engine: "codex", model: "default" },
        scope: () => {
          const ids = [...live.session.state.selection];
          const doc = live.client.snapshot().document;
          const page = doc?.pages.find((p) => p.id === live.session.page);
          return {
            selection: ids.length ? { ids, label: `${ids.length} layers` } : null,
            page: page ? { id: page.id, name: page.name } : null,
          };
        },
        documentMentions: () => [],
      })
    : undefined;
  if (composer) {
    live.connectComposer(composer);
    void composer.load();
  }
  const livePresence =
    live.agent && composer && load.live
      ? createLivePresence({
          agent: live.agent,
          client: live.client,
          now: () => Date.now(),
          page: () => live.session.page,
          engineName: () => composer.modelLine()?.split(" · ")[0] ?? "Agent",
          engineId: () => composer.state.choice?.engine ?? "agent",
          connected: () => true,
          timers: { set: (fn, ms) => setTimeout(fn, ms), clear: (h) => clearTimeout(h as number) },
          onAction: async (action) => {
            if (action.type === "pauseAll") void bridge.request({ type: "pauseAll" });
            else if (action.type === "stopAgent") await composer.stopAgent(action.agent);
            else if (action.type === "stopFrame") await composer.stopAll();
          },
        })
      : undefined;
  const presence = livePresence
    ? createPresenceController({
        feed: livePresence.feed,
        now: () => Date.now(),
        engine: live.client,
        onFollow: () => {},
      })
    : undefined;
  void presence?.load();
  return {
    live,
    replica,
    settings,
    ...(composer ? { composer } : {}),
    ...(presence ? { presence } : {}),
    actors: livePresence?.actors ?? {},
    disposeFeed() {
      unsubscribeFeed();
      presence?.dispose();
      livePresence?.dispose();
      composer?.dispose();
      live.agent?.dispose();
    },
  };
}
const editors = new Map<string, Editor>();
const loading = new Set<string>();
const failed = new Set<string>();
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
          void editor.live.controller.blur();
          void editor.replica?.close();
          editor.live.dispose();
          editor.disposeFeed();
          editors.delete(id);
        }
      // A tab's engine starts when the tab is first shown, not when it opens.
      for (const id of state.active ? [state.active] : [])
        if (!editors.has(id) && !loading.has(id) && !failed.has(id)) {
          loading.add(id);
          makeEditor(id, state.platform).then(
            (editor) => {
              loading.delete(id);
              if (!current?.tabs.some((t) => t.id === id)) {
                void editor.replica?.close();
                editor.live.dispose();
                return;
              }
              editors.set(id, editor);
              setState((s) => (s ? { ...s } : s));
            },
            () => {
              loading.delete(id);
              failed.add(id);
              setState((s) => (s ? { ...s } : s));
            },
          );
        }
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
        else void editors.get(current.active)?.live.controller.execute(event.command);
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
  if (!tab || (state.page && state.page !== "editor") || state.recovery)
    return <Home state={state} bridge={bridge} />;
  if (!editor)
    return (
      <main
        className="desktop-problem"
        data-platform={state.platform}
        role="status"
        aria-busy={!failed.has(tab.id)}
      >
        {state.platform === "mac" && <div className="desktop-titlebar-drag" aria-hidden="true" />}
        <DocumentTabs state={state} bridge={bridge} />
        {failed.has(tab.id) && <p>⊘ The document engine could not start</p>}
      </main>
    );
  const live = editor.live;
  const readOnly = live.client.snapshot().readOnly;
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
        shell={{ settings: editor.settings }}
        workspace={live.workspace}
        session={live.session}
        controller={live.controller}
        engine={live.client}
        design={live.design}
        paintEditor={live.paintEditor}
        {...(live.agent ? { agent: live.agent } : {})}
        {...(editor.composer ? { composer: editor.composer } : {})}
        {...(editor.presence ? { presence: editor.presence } : {})}
        content={live.content}
        createRenderer={live.createRenderer}
        browser={browserImport.current.host.browser}
        device={live.device}
        host="desktop"
        platform={state.platform}
        appearance={state.appearance}
        fileName={tab.name}
        saveState={tab.dirty ? "unsaved" : "saved"}
        dirty={tab.dirty}
        branch={null}
        connection={state.daemon && !readOnly ? { mode: "headless" } : { mode: "offline" }}
        agents={[]}
        actors={editor.actors}
        recent={{}}
        now={() => Date.now()}
        nextNodeId={live.nextNodeId}
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
          agent: readOnly ? (
            <div className="desktop-problem" role="status">
              <p>⊘ Read-only</p>
              <p>The daemon is not holding this file, so it opens read-only.</p>
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
          ) : undefined,
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
