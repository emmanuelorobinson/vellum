// The browser test app: the real editor workspace over a scripted engine and a fake daemon. TEST DATA throughout.
// Nothing here applies operations to a document: a test scripts the snapshot the engine answers with, and checks
// the transaction the UI sent and what the UI shows after the reply.
import {
  contextForSelection,
  createAgentController,
  createAuthoringController,
  createCommandController,
  createComposerController,
  createConflictController,
  createDesignController,
  createEdgeUi,
  createEditorHandlers,
  createEditorSession,
  createExportController,
  createHistoryController,
  createInspectController,
  createPaintEditor,
  createPaletteController,
  createPreferences,
  createPresenceController,
  createResourceController,
  createSettingsController,
  createTokenSyncController,
  createWorkspace,
  type EdgeHost,
  FakeFileChanges,
  FakeHistoryHost,
  FakePreferencesHost,
  fitViewport,
  type Mention,
  paletteCommands,
  paletteEntries,
  selectionLabel,
  union,
} from "@vellum/editor-core";
import { type EngineSnapshot, FakeEngineClient } from "@vellum/engine-client";
import { FakeDaemonClient } from "@vellum/host-api";
import { createBrowserHost } from "@vellum/host-browser";
import type { Document, Node } from "@vellum/types-ts";
import {
  COLLAB_NAMES,
  collaborationNodes,
  collaborationScenario,
} from "../../fixtures/collaboration-scenarios";
import { liveDocument, liveScenario } from "../../fixtures/live-scenarios";
import "@vellum/ui-tokens/tokens.css";
import { createRoot } from "react-dom/client";
import {
  AGENT_ACTORS,
  AGENT_LAYER_NAMES,
  AGENT_NOW,
  type AgentScenario,
  agentScenario,
  COMPOSER_HOST,
} from "../../fixtures/agent-scenarios";
import {
  DESIGN_DEFAULTS,
  type DesignScenario,
  designDocument,
  designSelection,
} from "../../fixtures/design-scenarios";
import {
  resourceDocument,
  resourceHost,
  resourcePasteConflicts,
} from "../../fixtures/resource-scenarios";
import { EDGE_RESET, type EdgeScenario, edgeSetup } from "../../fixtures/shell-scenarios";
import { type EdgeProps, EditorWorkspace } from "../../src/EditorWorkspace";
import { ShareDialog } from "../../src/share/ShareDialog";
import "./fonts.css";
import { authoringDocument, authoringHost } from "../../fixtures/authoring-scenarios";
import {
  JuniperHandoff,
  LEDGER_BASE,
  type LedgerScenario,
  ledgerChange,
  ledgerEntries,
  ledgerNodes,
} from "../../fixtures/ledger-scenarios";
import { sceneOf } from "./test-scene";

const q = new URLSearchParams(location.search);
const scenario = q.get("scenario");
const edgeName = q.get("edge") as EdgeScenario | null;
const edgeConf = edgeName ? edgeSetup(edgeName) : null;
const shellOn = q.has("shell") || !!edgeName;
const authoringName = q.get("authoring");
const collaborationName = q.get("collaboration");
const ledgerName = q.get("ledger") as LedgerScenario | null;
const collaboration = collaborationName ? collaborationScenario(collaborationName) : null;
const resourceName = q.get("resource") ?? (edgeConf?.library ? "missingLibrary" : null);
const designName = q.get("design") as DesignScenario | null;
const appearance = (q.get("appearance") ?? "light") as "light" | "dark" | "hc-light" | "hc-dark";

const frame = (
  id: string,
  name: string,
  x: number,
  y: number,
  extra: Record<string, unknown> = {},
) =>
  ({
    type: "frame",
    id,
    name,
    width: 200,
    height: 300,
    position: { type: "absolute", x, y },
    ...extra,
  }) as unknown as Node;
export const startNodes = (): Node[] => [
  frame("welcome", "Welcome", 440, 120),
  frame("step1", "Step 1", 680, 120, { layout: "column" }),
  frame("step2", "Step 2", 920, 120, {
    children: [
      {
        type: "rect",
        id: "card",
        name: "Habit card",
        width: 160,
        height: 60,
        position: { type: "absolute", x: 20, y: 20 },
      },
      {
        type: "text",
        id: "title",
        name: "Title",
        width: 160,
        height: 24,
        position: { type: "absolute", x: 20, y: 100 },
      },
    ],
  }),
];
const box = (type: string, id: string, name: string, extra: Record<string, unknown> = {}) =>
  ({
    type,
    id,
    name,
    width: 100,
    height: 40,
    position: { type: "absolute", x: 0, y: 0 },
    ...extra,
  }) as unknown as Node;
/** The Juniper layers of design screen C18: Step 2 holds a component and an instance. */
const juniperNodes = (): Node[] => [
  box("frame", "welcome", "Welcome"),
  box("frame", "step1", "Step 1", { layout: "column" }),
  box("frame", "step2", "Step 2", {
    layout: "column",
    children: [
      box("frame", "habit", "Habit card", {
        children: [
          box("text", "title", "Title"),
          box("frame", "choices", "Habit choices", { layout: "column" }),
        ],
      }),
      box("instance", "continue", "Continue", { componentId: "c1" }),
    ],
  }),
  box("frame", "step3", "Step 3"),
];
const documentOf = (children: Node[]): Document =>
  ({
    meta: { formatVersion: 1, name: "juniper.vellum", createdWith: "test" },
    components: {
      ...(edgeConf?.empty
        ? {}
        : {
            c1: {
              name: "Button / Primary",
              root: { type: "frame", id: "c1root", name: "Button", width: 120, height: 40 },
            },
          }),
      ...(scenario === "juniper"
        ? {
            c2: {
              name: "HabitRow",
              root: { type: "frame", id: "c2root", name: "HabitRow", width: 180, height: 32 },
            },
            c3: {
              name: "Preference toggle",
              root: { type: "frame", id: "c3root", name: "Toggle", width: 60, height: 28 },
            },
          }
        : {}),
    },
    pages: [
      { id: "p1", name: "Onboarding", children },
      { id: "p2", name: "Settings", children: [] },
      ...(scenario === "juniper"
        ? [
            { id: "p3", name: "Paywall", children: [] },
            { id: "p4", name: "Components", children: [] },
          ]
        : []),
    ],
  }) as unknown as Document;

let current = authoringName
  ? authoringDocument(authoringName)
  : resourceName
    ? resourceDocument(resourceName)
    : designName
      ? designDocument(designName)
      : documentOf(
          collaborationName
            ? collaborationNodes(collaborationName)
            : ledgerName
              ? ledgerNodes(ledgerName)
              : q.get("empty") || edgeConf?.empty
                ? []
                : scenario === "juniper"
                  ? juniperNodes()
                  : startNodes(),
        );
if (q.has("live")) current = liveDocument(current);
/** The document with other layers on its first page: the design scenarios keep their variables and components. */
const withChildren = (children: Node[]): Document =>
  designName
    ? ({
        ...current,
        pages: current.pages.map((p, i) => (i === 0 ? { ...p, children } : p)),
      } as Document)
    : documentOf(children);
let revision = 0;
const snapshotOf = (doc: Document, readOnly = false): EngineSnapshot => ({
  revision: ++revision,
  document: doc,
  readOnly,
});
const engine = new FakeEngineClient(
  { revision: 0, document: current, readOnly: q.has("readonly") || !!edgeConf?.readOnly },
  sceneOf(() => current),
);
// Every request the UI makes, applied or refused: the fake records only the ones it answers.
const attempts: unknown[] = [];
const realApply = engine.apply.bind(engine);
engine.apply = (transaction) => {
  attempts.push(JSON.parse(JSON.stringify(transaction)));
  return realApply(transaction);
};
const daemon = new FakeDaemonClient();
const host = createBrowserHost();
const device = "device-test";
let counter = 0;
const session = createEditorSession({
  engine,
  device,
  page: "p1",
  nextNodeId: () => `n${++counter}`,
  createNode: (kind, id, b) =>
    ({
      type: kind,
      id,
      name: kind === "rect" ? "Rectangle" : kind,
      width: b.width,
      height: b.height,
      position: { type: "absolute", x: b.x, y: b.y },
    }) as unknown as Node,
  viewportSize: () => ({ width: innerWidth, height: innerHeight }),
});
const workspace = createWorkspace();
workspace.resize({ width: innerWidth, height: innerHeight });
if (q.get("tab")) workspace.switchTab(q.get("tab") as "agent" | "layers" | "assets" | "history");
const zoom = Number(q.get("zoom") ?? 1);
const [panX = 0, panY = 0] = (q.get("pan") ?? "").split(",").map(Number);
if (zoom !== 1 || panX || panY) session.setViewport({ x: panX, y: panY, zoom });
if (q.get("select")) session.select((q.get("select") as string).split(","));
const fonts = ["Inter", "JetBrains Mono"];
const design = createDesignController({
  session,
  engine,
  defaults: DESIGN_DEFAULTS,
  text: { available: (family) => fonts.includes(family), fallback: "Inter" },
});
const paintEditor = createPaintEditor({ session, design });
const hostActions: { command: string; detail?: unknown }[] = [];
const authoringRequests: unknown[] = [];
const authoringFeatureHost = authoringName
  ? authoringHost(authoringName, () => engine.snapshot().revision, authoringRequests)
  : undefined;
const authoring = authoringName
  ? createAuthoringController({
      engine,
      device,
      nextNodeId: () => `n${++counter}`,
      host: authoringFeatureHost as import("@vellum/editor-core").AuthoringHost,
    })
  : undefined;
if (authoringName) {
  session.select([
    authoringName.startsWith("c28")
      ? "script"
      : authoringName === "c29"
        ? "slot"
        : authoringName === "c29-filled"
          ? "instance"
          : authoringName.startsWith("c30")
            ? "photo"
            : "frame",
  ]);
}
const resources = createResourceController({
  engine,
  device,
  nextVariableId: () => `variable${++counter}`,
  host: {
    ...resourceHost,
    libraries:
      resourceName === "missingLibrary" || resourceName === "missingDetails"
        ? [{ alias: "core", name: "Juniper Core", state: "missing", cached: true }]
        : resourceHost.libraries,
    chooseLibrary: async (alias) => ({
      alias: alias ?? "imported",
      path: "../libraries/located.lib.vellum",
    }),
    chooseFont: async () => ({ asset: "b".repeat(64), face: { family: "Imported Font" } }),
    save: async () => {
      hostActions.push({ command: "document.save" });
      return true;
    },
    action: (command, detail) => hostActions.push({ command, detail }),
    openLibrary: (alias) => hostActions.push({ command: "library.open", detail: { alias } }),
    planPaste: async (_request, choices) => {
      hostActions.push({ command: "paste.plan", detail: choices });
      return [
        {
          type: "insert",
          to: { parent: { type: "page", id: "p1" }, index: 0 },
          node: { type: "rect", id: "pasted", name: "Pasted", width: 10, height: 10 },
        },
      ];
    },
  },
});
if (
  resourceName &&
  ["variables", "themes", "libraries", "fonts", "manageFonts"].includes(resourceName)
)
  resources.open(resourceName as "variables");
if (q.get("resource") === "missingLibrary") resources.open("libraries");
if (resourceName === "convert") resources.requestConversion();
if (resourceName === "paste" || resourceName === "conflicts")
  resources.requestPaste({
    id: "clip1",
    ...(resourceName === "paste" ? { libraryPath: "../libraries/juniper.lib.vellum" } : {}),
    conflicts: resourceName === "conflicts" ? resourcePasteConflicts : [],
    omissions:
      resourceName === "conflicts"
        ? []
        : ["Juniper Sans (custom font)", "images/hero.png (missing)"],
  });
if (resourceName) session.select([resourceName === "vector" ? "vector" : "card"]);
if (resourceName === "scale") {
  session.dispatch({ type: "tool", tool: "scale" });
  session.dispatch({ type: "scaleFactor", factor: "200%" });
  session.dispatch({ type: "scaleAnchor", anchor: { x: 0, y: 0 } });
}
if (resourceName === "vector") {
  session.dispatch({ type: "tool", tool: "path" });
  if (q.has("point")) {
    session.dispatch({
      type: "pointerDown",
      point: { x: 770, y: 410 },
      target: { type: "point", contour: 0, index: 1 },
    });
    session.dispatch({ type: "pointerUp", point: { x: 770, y: 410 } });
  }
}

if (designName && !q.get("select")) session.select([designSelection(designName)]);
if (q.get("edit")) {
  // Open the selected text layer for typing, as a double-click on it does.
  const at = session.sceneBounds(designSelection(designName ?? "c14"));
  const v = session.state.viewport;
  if (at)
    session.dispatch({
      type: "doubleClick",
      point: { x: (at.x + 4) * v.zoom + v.x, y: (at.y + 4) * v.zoom + v.y },
    });
}

// The Agent tab over a scripted session feed (fixtures/agent-scenarios.ts). `hold` keeps the scripted updates
// back until a test sends them with `agentEmit`.
const agentName = (q.get("agent") ?? edgeConf?.agent ?? null) as AgentScenario | null;
const agentSetup = collaboration ?? (agentName ? agentScenario(agentName) : null);
let agentClock = AGENT_NOW;
const agent = agentSetup
  ? createAgentController({
      feed: agentSetup.feed,
      now: () => agentClock,
      timeZone: "UTC",
      pageName: (id) => COLLAB_NAMES[id] ?? id,
      nameOf: (id) => AGENT_LAYER_NAMES[id] ?? null,
      onZoomTo: (node) => {
        hostActions.push({ command: "agent.zoomTo", detail: { node } });
        if (session.sceneItem(node)) session.select([node]);
      },
      onReviewChanges: (s, turn) =>
        hostActions.push({ command: "history.reviewTurn", detail: { session: s, turn } }),
    })
  : null;
// The composer over the same feed: the selection and page come from the editor session.
const composer =
  agent && agentSetup
    ? createComposerController({
        agent,
        feed: agentSetup.feed,
        timeZone: "UTC",
        defaultEngine: {
          engine: "builtin",
          provider: "openrouter",
          model: "deepseek/deepseek-v4.1-flash",
        },
        scope: () => {
          const ids = session.state.selection;
          const page = current.pages.find((p) => p.id === session.page) ?? null;
          const label = ids.length ? selectionLabel(current, session.page, ids) : null;
          return {
            selection: label ? { ids, label } : null,
            page: page ? { id: page.id, name: page.name } : null,
            context: contextForSelection(current, session.page, ids),
          };
        },
        documentMentions: (query) => {
          const q = query.toLowerCase();
          const frames: Mention[] = (current.pages[0]?.children ?? [])
            .filter((n) => n.type === "frame")
            .map((n) => ({ kind: "frame", label: n.name ?? n.id, value: n.id }));
          const components: Mention[] = Object.entries(current.components ?? {}).map(([id, c]) => ({
            kind: "component",
            label: c.name,
            value: id,
          }));
          return [...frames, ...components].filter((m) => m.label.toLowerCase().includes(q));
        },
      })
    : null;
const presence = collaboration
  ? createPresenceController({
      feed: collaboration.presenceFeed,
      engine,
      now: () => agentClock,
      onFollow: (v) => session.setViewport(v),
      onOpenSession: (id) => {
        void agent?.select(id);
        workspace.switchTab("agent", { reveal: true });
      },
      onRejected: (frame, agent) =>
        hostActions.push({ command: "history.rejected", detail: { frame, agent } }),
      contains: (frame, node) => {
        let item = engine.scene.get(node);
        while (item?.parent) {
          if (item.parent === frame) return true;
          item = engine.scene.get(item.parent);
        }
        return false;
      },
    })
  : null;
if (collaborationName === "c35") session.select(["step2"]);
const live = q.has("live") ? liveScenario(engine, session, q.get("live")!) : null;
const handoffHost = new JuniperHandoff();
const ledger = ledgerName
  ? (() => {
      const historyHost = new FakeHistoryHost(edgeConf?.empty ? [] : ledgerEntries(ledgerName));
      historyHost.attach(
        engine,
        { id: "sam", kind: "person", name: "Sam" },
        () => LEDGER_BASE + 46 * 60_000,
      );
      const history = createHistoryController({
        engine,
        device,
        host: historyHost,
        select: (ids) => session.select(ids),
        zoom: (ids) => {
          const target = union(ids.flatMap((id) => session.sceneBounds(id) ?? []));
          if (target)
            session.setViewport(
              fitViewport(target, workspace.state, workspace.layout().canvasInsets, { maxZoom: 1 }),
            );
        },
      });
      const feed = new FakeFileChanges(ledgerChange(ledgerName));
      const conflicts = createConflictController({
        engine,
        device,
        feed,
        fileName: "juniper.vellum",
        nextNodeId: () => `n${++counter}`,
        askAgent: (node) => hostActions.push({ command: "conflict.askAgent", detail: { node } }),
        openLedger: (entry) => {
          workspace.switchTab("history", { reveal: true });
          history.open(entry);
        },
      });
      return {
        history,
        historyHost,
        conflicts,
        feed,
        inspect: createInspectController({ engine, host: handoffHost }),
        tokens: createTokenSyncController({ engine, device, host: handoffHost }),
        exporter: createExportController({ engine, host: handoffHost }),
        timeZone: "UTC",
      };
    })()
  : null;
// The palette lists what the controller holds; it is built first because ⌘/ needs it.
const palette = shellOn
  ? createPaletteController({
      commands: () => paletteCommands(controller.commands),
      entries: () => paletteEntries(engine.snapshot().document, agent?.state.sessions ?? []),
    })
  : null;
const editorHandlers = createEditorHandlers({
  workspace,
  session,
  design,
  ...(agent ? { agent } : {}),
  ...(composer ? { composer } : {}),
  ...(ledger ? { exporter: ledger.exporter } : {}),
  ...(palette ? { palette } : {}),
  ...(shellOn ? { pauseAll: () => hostActions.push({ command: "agents.pauseAll" }) } : {}),
});
// The document edit commands belong to a later prompt: here they only record that the menu or palette ran them.
if (shellOn)
  for (const id of [
    "edit.copy",
    "edit.paste",
    "edit.group",
    "edit.wrap",
    "edit.autoLayout",
    "component.toggle",
    "arrange.forward",
    "edit.copyPng",
    "edit.delete",
  ])
    editorHandlers[id] ??= () => void hostActions.push({ command: id });
const controller = createCommandController(editorHandlers);
const preferencesHost = new FakePreferencesHost();
const preferences = createPreferences(preferencesHost);
const settings = shellOn
  ? createSettingsController({
      preferences,
      keys: controller.keys,
      context: {
        host: "browser",
        platform: navigator.platform.startsWith("Mac") ? "mac" : "linux",
        focus: "panel",
      },
    })
  : null;
const edgeUi = createEdgeUi();
if (edgeConf?.openFonts) edgeUi.openFonts();
const edgeHost: EdgeHost = {
  action: (id, detail) =>
    hostActions.push({ command: `edge.${id}`, ...(detail ? { detail } : {}) }),
  closeChoice: (choice) => hostActions.push({ command: "edge.close", detail: { choice } }),
};
const edgeProps: EdgeProps | null = edgeConf
  ? {
      host: edgeHost,
      ui: edgeUi,
      ...(edgeConf.newerFormat ? { newerFormat: edgeConf.newerFormat } : {}),
      ...(edgeConf.unreadable ? { unreadable: edgeConf.unreadable } : {}),
      ...(edgeConf.fonts ? { fonts: edgeConf.fonts } : {}),
      ...(edgeConf.largeFile ? { largeFile: true, layers: edgeConf.largeFile.layers } : {}),
      ...(edgeConf.rejection ? { rejection: edgeConf.rejection } : {}),
      ...(edgeConf.closing ? { closing: edgeConf.closing } : {}),
      ...(edgeConf.loading
        ? { loading: { agent: true, layers: true, assets: true, history: true, design: true } }
        : {}),
    }
  : null;
const pendingAgentUpdates = [...(agentSetup?.updates ?? [])];
const agentReady = (async () => {
  if (!agent || !agentSetup) return;
  await presence?.load();
  await agent.load();
  const initialSession = "current" in agentSetup ? agentSetup.current : null;
  if (initialSession) await agent.select(initialSession);
  if (edgeConf?.engine && agentSetup) {
    const catalog = agentSetup.feed.catalog;
    const claude = catalog.installed.find((e) => e.id === "claude-code");
    if (edgeConf.engine === "key-failed") {
      const provider = catalog.builtin.find((p) => p.id === "openrouter");
      if (provider) provider.status = "failed";
    } else if (claude) {
      claude.state = edgeConf.engine === "signed-out" ? "signed_out" : "usage_limit";
      if (edgeConf.engine === "usage-limit") claude.resetsAt = EDGE_RESET;
    }
  }
  await composer?.load();
  if (edgeConf?.engine && edgeConf.engine !== "key-failed")
    composer?.choose({ engine: "claude-code", model: "claude-code" });
  if (!q.has("hold")) for (const u of pendingAgentUpdates.splice(0)) agentSetup.feed.emit(u);
  agentClock += ("elapse" in agentSetup ? agentSetup.elapse : 0) ?? 0;
  render();
  for (const key of ("expanded" in agentSetup ? agentSetup.expanded : []) ?? []) agent.toggle(key);
  if (q.has("switcher")) agent.toggleSwitcher(true);
  if (q.has("undo")) agent.askUndo(agent.turns().at(-1)?.id ?? null);
  const surface = q.get("composer");
  if (composer && surface)
    composer.open(
      surface === "setup"
        ? { type: "setup", provider: "openrouter" }
        : ({ type: surface } as never),
    );
  if (composer && q.has("inline")) composer.openInline();
  const paletteQuery = q.get("palette");
  if (palette && paletteQuery !== null) {
    palette.open();
    palette.setQuery(paletteQuery);
  }
})();
if (settings && q.get("settings") === "open") settings.open();

const clock = 1_000_000;
const recent: Record<string, { agent: string; at: number }> = {};
if (scenario === "juniper") recent.habit = { agent: "teal", at: clock - 2000 };
const connection =
  q.get("connection") === "headless" || edgeConf?.connection === "headless"
    ? ({ mode: "headless" } as const)
    : q.get("connection") === "offline" || edgeConf?.connection === "offline"
      ? ({ mode: "offline", note: "Editing and saving available" } as const)
      : ({ mode: "live", agents: 1 } as const);
type Hooks = {
  /** The next write is answered with this document. */
  reply(children: Node[], operation?: "apply" | "undo" | "redo"): void;
  /** The next write is answered with this whole document (variables and components included). */
  replyDocument(doc: Document, operation?: "apply" | "undo" | "redo"): void;
  /** The engine now holds this document and says so: the panel shows it (no transaction is recorded). */
  publish(doc: Document): void;
  /** The commands the panel asked the host for: Add export, Choose image, and so on. */
  hostActions(): { command: string; detail?: unknown }[];
  document(): Document;
  transactions(): unknown[];
  historyActors(): unknown[];
  mark(id: string, agent: string, ageMs: number): void;
  selection(): readonly string[];
  tool(): string;
  viewport(): unknown;
  authoringRequests(): unknown[];
  authoringEmit(view: import("@vellum/editor-core").ScriptView): void;
  nodes(): Node[];
  /** Resolves once the Agent tab's scenario has loaded. */
  agentReady(): Promise<void>;
  /** Sends the next `n` scripted session-feed updates (all when absent). */
  agentEmit(n?: number): number;
  /** Sends one feed update, as the daemon would. */
  agentSend(update: unknown): void;
  /** What the panel asked the daemon for: undo turn, retry, stop... */
  agentRequests(): unknown[];
  agentTick(ms: number): void;
  presenceRequests(): unknown[];
  presencePaused(on: boolean): void;
  /** The composer's state, as JSON (to prove what it holds and what it does not). */
  composerState(): string;
  /** Requests the composer sent, with their sessions. */
  composerSent(): {
    session: string;
    request: import("@vellum/editor-core").PromptRequest & Record<string, unknown>;
  }[];
  /** Keys the fake keychain holds, by provider. */
  keychain(): Record<string, string>;
  /** The change ledger's state, the entries the host holds and what the handoff host recorded. */
  ledgerState(): unknown;
  /** The next write is answered with this document (a revert, a token import, a conflict pick). */
  ledgerReply(children: Node[]): void;
  /** The saved preferences, newest last, and the palette's state. */
  preferences(): unknown;
  preferencesSaved(): unknown[];
  palette(): unknown;
  edgeUi(): unknown;
  liveRole(role: import("@vellum/types-ts").Role): void;
  liveConnection(state: import("@vellum/editor-core").CollaborationLink["state"]): void;
  liveRequests(): unknown[];
};
declare global {
  interface Window {
    __vellum: Hooks;
  }
}
window.__vellum = {
  reply(children, operation = "apply") {
    current = withChildren(children);
    engine.script(snapshotOf(current), operation);
  },
  publish(doc) {
    current = doc;
    engine.script(snapshotOf(current));
    realApply({
      origin: { actor: { kind: "person", device } },
      ops: [{ type: "update", id: "none", patch: {} }],
    });
  },
  replyDocument(doc, operation = "apply") {
    current = doc;
    engine.script(snapshotOf(current), operation);
  },
  authoringRequests: () => authoringRequests,
  authoringEmit: (view) => authoringFeatureHost?.emitScript(view),
  hostActions: () => hostActions,
  document: () => current,
  transactions: () => attempts,
  historyActors: () => engine.historyActors,
  mark(id, agent, ageMs) {
    recent[id] = { agent, at: clock - ageMs };
    render();
  },
  selection: () => session.state.selection,
  tool: () => session.state.tool,
  viewport: () => session.state.viewport,
  nodes: () => current.pages[0]?.children ?? [],
  agentReady: () => agentReady,
  agentEmit(n) {
    const batch = pendingAgentUpdates.splice(0, n ?? pendingAgentUpdates.length);
    for (const u of batch) agentSetup?.feed.emit(u);
    return pendingAgentUpdates.length;
  },
  agentSend: (update) => agentSetup?.feed.emit(update as never),
  agentRequests: () => agentSetup?.feed.requests ?? [],
  presenceRequests: () => collaboration?.presenceFeed.requests ?? [],
  presencePaused(on) {
    if (collaboration) collaboration.presenceFeed.emit({ ...collaboration.snapshot, paused: on });
  },
  agentTick(ms) {
    agentClock += ms;
    render();
  },
  composerState: () => JSON.stringify(composer?.state ?? null),
  composerSent: () =>
    (agentSetup?.feed.sent ?? []) as unknown as {
      session: string;
      request: import("@vellum/editor-core").PromptRequest & Record<string, unknown>;
    }[],
  keychain: () => agentSetup?.feed.keys ?? {},
  ledgerState: () =>
    ledger
      ? {
          history: ledger.history.state,
          entries: ledger.historyHost.entries(),
          undone: ledger.historyHost.undone,
          copied: ledger.historyHost.copied,
          tokens: ledger.tokens.state,
          exporter: ledger.exporter.state,
          inspect: ledger.inspect.state,
          conflict: ledger.conflicts.state.node?.id ?? null,
          clipboard: handoffHost.clipboard,
          written: handoffHost.written,
          exported: handoffHost.exported,
          copiedAs: handoffHost.copiedAs,
          implemented: handoffHost.implemented,
          html: handoffHost.htmlRequests,
          hostActions,
        }
      : null,
  ledgerReply(children) {
    current = withChildren(children);
    engine.script(snapshotOf(current));
  },
  preferences: () => preferences.state,
  preferencesSaved: () => preferencesHost.saved,
  palette: () => (palette ? { ...palette.state, results: palette.results() } : null),
  edgeUi: () => edgeUi.state,
  liveRole: (role) => live?.role(role),
  liveConnection: (state) => live?.connection(state),
  liveRequests: () => live?.requests ?? [],
};

const root = createRoot(document.getElementById("root") as HTMLElement);
let liveShareOpen = q.get("live") === "share" || q.get("live") === "selfhost";
function render() {
  root.render(
    <>
      <EditorWorkspace
        workspace={workspace}
        {...(presence ? { presence } : {})}
        {...(live ? { collaboration: live.controller } : {})}
        session={session}
        engine={engine}
        controller={controller}
        device={device}
        host={"browser"}
        platform={navigator.platform.startsWith("Mac") ? "mac" : "linux"}
        appearance={appearance}
        fileName="juniper.vellum"
        saveState={(q.get("save") as "saved" | "saving" | "unsaved" | null) ?? "saved"}
        branch="feat/onboarding"
        connection={connection}
        agents={[{ id: "teal", kind: "agent", name: "Teal", hue: "teal" }]}
        actors={{ teal: { id: "teal", kind: "agent", name: "Teal", hue: "teal" }, ...AGENT_ACTORS }}
        onShare={() => {
          if (live) {
            liveShareOpen = true;
            render();
          }
        }}
        onPreview={() => {}}
        {...(settings ? {} : { onSettings: () => {} })}
        onAction={(command) => {
          if (shellOn) hostActions.push({ command });
        }}
        basicRenderer={!!edgeConf?.basicRenderer}
        {...(shellOn
          ? {
              shell: {
                ...(palette ? { palette } : {}),
                ...(settings ? { settings } : {}),
                settingsHost: {
                  providerKeySet: true,
                  stockProviders: [{ id: "pixabay", name: "Pixabay" }],
                  changeProviderKey: () => hostActions.push({ command: "stock.key" }),
                },
              },
            }
          : {})}
        {...(edgeProps ? { edge: edgeProps } : {})}
        recent={recent}
        componentRoots={new Set(scenario === "juniper" ? ["habit"] : [])}
        now={() => clock}
        nextNodeId={() => `a${++counter}`}
        assets={{
          libraries: edgeConf?.empty
            ? []
            : [{ alias: "Juniper Core", state: "synced", components: 8 }],
          iconSets: [
            { id: "lucide", label: "Lucide" },
            { id: "material", label: "Material Symbols" },
            { id: "phosphor", label: "Phosphor" },
            { id: "feather", label: "Feather" },
          ],
          projectIconFolder: { path: "design/icons" },
          visualStyles: [
            { id: "minimal", name: "Minimal" },
            { id: "editorial", name: "Editorial" },
            { id: "technical", name: "Technical" },
          ],
          designSystems:
            scenario === "juniper"
              ? [
                  { id: "starter-minimal", name: "Starter Minimal" },
                  { id: "starter-editorial", name: "Starter Editorial" },
                  { id: "starter-technical", name: "Starter Technical" },
                ]
              : [{ id: "starter-minimal", name: "Starter Minimal" }],
          onAction: () => {},
        }}
        {...(resourceName || edgeConf?.fonts ? { resources } : {})}
        {...(authoring ? { authoring } : {})}
        {...(ledger
          ? {
              ledger: {
                history: ledger.history,
                conflicts: ledger.conflicts,
                inspect: ledger.inspect,
                tokens: ledger.tokens,
                exporter: ledger.exporter,
                timeZone: ledger.timeZone,
              },
            }
          : {})}
        {...(agent ? { agent } : {})}
        {...(composer && !q.has("slot")
          ? {
              composer,
              composerHost: {
                ...COMPOSER_HOST,
                timeZone: "UTC",
                onConnectAgents: () => hostActions.push({ command: "agents.connect" }),
                attach: async () => {
                  hostActions.push({ command: "composer.attach" });
                  return [
                    { kind: "attachment" as const, label: "hero.png", value: "attachment:1" },
                  ];
                },
                ...(connection.mode === "offline"
                  ? {
                      offline: true,
                      onRestart: () => hostActions.push({ command: "daemon.restart" }),
                    }
                  : {}),
              },
            }
          : {})}
        {...(collaborationName === "c35" ? {} : { design, paintEditor })}
        designHost={{
          fonts,
          onAction: (command, detail) => {
            hostActions.push({ command, detail });
          },
        }}
        slots={{ agent: <div data-testid="agent-slot" /> }}
      />
      {liveShareOpen && live && (
        <div data-appearance={appearance}>
          <ShareDialog
            name="juniper.vellum"
            collaboration={live.controller}
            onClose={() => {
              liveShareOpen = false;
              render();
            }}
            port={{
              request: async () => ({
                state: "shared",
                id: "TEST-DATA-shared-snapshot",
                url: "https://share.vellum.design/share/TEST-DATA-shared-snapshot",
                snapshot: { at: 1791729120000, incomplete: false },
              }),
              copy: async () => {},
              open: async () => {},
            }}
          />
        </div>
      )}
    </>,
  );
}
render();
void daemon;
void host;

if (ledger && ledgerName) {
  const name = ledgerName;
  if (name.startsWith("c10")) {
    workspace.switchTab("history");
    ledger.history.filter({ session: "s1" });
    ledger.history.open("teal");
    if (name === "c10-revert" || name === "c10-reverted") ledger.history.askRevert("teal");
    if (name === "c10-reverted") {
      engine.script(snapshotOf(current));
      ledger.history.confirmRevert();
    }
    if (name === "c10-undone") void ledger.history.undo("teal");
  }
  if (name === "c19" || name === "c19-copied") session.select(["step2"]);
  if (name === "c19-copied")
    void ledger.inspect.load("step2").then(() => ledger.inspect.copyPrompt());
  if (name === "c20" || name === "c20-applied") ledger.tokens.openImport();
  if (name === "c20-export") ledger.tokens.openExport();
  if (name === "c20-applied") {
    void ledger.tokens.preview().then(() => {
      engine.script(snapshotOf(current));
      void ledger.tokens.apply();
    });
  }
  if (name === "c21") void ledger.exporter.open(["step1", "step2", "step3"]);
  if (name === "c22-conflict") ledger.conflicts.open("step2");
  if (name === "c22") workspace.switchTab("agent");
}
if (authoringName === "c12-guidelines") void authoring?.openGuidelines();
if (authoringName?.startsWith("c30")) authoring?.openImage("photo", "fill", 0);
