// The browser test app: the real editor workspace over a scripted engine and a fake daemon. TEST DATA throughout.
// Nothing here applies operations to a document: a test scripts the snapshot the engine answers with, and checks
// the transaction the UI sent and what the UI shows after the reply.
import {
  createAgentController,
  createCommandController,
  createDesignController,
  createEditorHandlers,
  createEditorSession,
  createPaintEditor,
  createResourceController,
  createWorkspace,
} from "@vellum/editor-core";
import { type EngineSnapshot, FakeEngineClient } from "@vellum/engine-client";
import { FakeDaemonClient } from "@vellum/host-api";
import { createBrowserHost } from "@vellum/host-browser";
import type { Document, Node } from "@vellum/types-ts";
import "@vellum/ui-tokens/tokens.css";
import { createRoot } from "react-dom/client";
import {
  AGENT_ACTORS,
  AGENT_LAYER_NAMES,
  AGENT_NOW,
  type AgentScenario,
  agentScenario,
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
import { EditorWorkspace } from "../../src/EditorWorkspace";
import "./fonts.css";
import { sceneOf } from "./test-scene";

const q = new URLSearchParams(location.search);
const scenario = q.get("scenario");
const resourceName = q.get("resource");
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
      c1: {
        name: "Button / Primary",
        root: { type: "frame", id: "c1root", name: "Button", width: 120, height: 40 },
      },
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

let current = resourceName
  ? resourceDocument(resourceName)
  : designName
    ? designDocument(designName)
    : documentOf(q.get("empty") ? [] : scenario === "juniper" ? juniperNodes() : startNodes());
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
  { revision: 0, document: current, readOnly: q.has("readonly") },
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
if (resourceName === "missingLibrary") resources.open("libraries");
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
const controller = createCommandController(createEditorHandlers({ workspace, session, design }));
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
const agentName = q.get("agent") as AgentScenario | null;
const agentSetup = agentName ? agentScenario(agentName) : null;
let agentClock = AGENT_NOW;
const agent = agentSetup
  ? createAgentController({
      feed: agentSetup.feed,
      now: () => agentClock,
      timeZone: "UTC",
      nameOf: (id) => AGENT_LAYER_NAMES[id] ?? null,
      onZoomTo: (node) => {
        hostActions.push({ command: "agent.zoomTo", detail: { node } });
        if (session.sceneItem(node)) session.select([node]);
      },
      onReviewChanges: (s, turn) =>
        hostActions.push({ command: "history.reviewTurn", detail: { session: s, turn } }),
    })
  : null;
const pendingAgentUpdates = [...(agentSetup?.updates ?? [])];
const agentReady = (async () => {
  if (!agent || !agentSetup) return;
  await agent.load();
  if (agentSetup.current) await agent.select(agentSetup.current);
  if (!q.has("hold")) for (const u of pendingAgentUpdates.splice(0)) agentSetup.feed.emit(u);
  agentClock += agentSetup.elapse ?? 0;
  render();
  for (const key of agentSetup.expanded ?? []) agent.toggle(key);
  if (q.has("switcher")) agent.toggleSwitcher(true);
  if (q.has("undo")) agent.askUndo(agent.turns().at(-1)?.id ?? null);
})();

const clock = 1_000_000;
const recent: Record<string, { agent: string; at: number }> = {};
if (scenario === "juniper") recent.habit = { agent: "teal", at: clock - 2000 };
const connection =
  q.get("connection") === "headless"
    ? ({ mode: "headless" } as const)
    : q.get("connection") === "offline"
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
  agentTick(ms) {
    agentClock += ms;
    render();
  },
};

const root = createRoot(document.getElementById("root") as HTMLElement);
function render() {
  root.render(
    <EditorWorkspace
      workspace={workspace}
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
      onShare={() => {}}
      onPreview={() => {}}
      onSettings={() => {}}
      onAction={() => {}}
      recent={recent}
      componentRoots={new Set(scenario === "juniper" ? ["habit"] : [])}
      now={() => clock}
      nextNodeId={() => `a${++counter}`}
      assets={{
        libraries: [{ alias: "Juniper Core", state: "synced", components: 8 }],
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
      {...(resourceName ? { resources } : {})}
      {...(agent ? { agent } : {})}
      design={design}
      paintEditor={paintEditor}
      designHost={{
        fonts,
        onAction: (command, detail) => {
          hostActions.push({ command, detail });
        },
      }}
      slots={{ agent: <div data-testid="agent-slot" /> }}
    />,
  );
}
render();
void daemon;
void host;
