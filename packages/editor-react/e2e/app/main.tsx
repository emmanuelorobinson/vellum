// The browser test app: the real editor workspace over a scripted engine and a fake daemon. TEST DATA throughout.
// Nothing here applies operations to a document: a test scripts the snapshot the engine answers with, and checks
// the transaction the UI sent and what the UI shows after the reply.
import {
  createCommandController,
  createEditorHandlers,
  createEditorSession,
  createWorkspace,
} from "@vellum/editor-core";
import { type EngineSnapshot, FakeEngineClient } from "@vellum/engine-client";
import { FakeDaemonClient } from "@vellum/host-api";
import { createBrowserHost } from "@vellum/host-browser";
import type { Document, Node } from "@vellum/types-ts";
import "@vellum/ui-tokens/tokens.css";
import { createRoot } from "react-dom/client";
import { EditorWorkspace } from "../../src/EditorWorkspace";
import "./fonts.css";
import { sceneOf } from "./test-scene";

const q = new URLSearchParams(location.search);
const scenario = q.get("scenario");
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

let current = documentOf(
  q.get("empty") ? [] : scenario === "juniper" ? juniperNodes() : startNodes(),
);
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
if (zoom !== 1) session.setViewport({ x: 0, y: 0, zoom });
if (q.get("select")) session.select((q.get("select") as string).split(","));
const controller = createCommandController(createEditorHandlers({ workspace, session }));

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
  transactions(): unknown[];
  historyActors(): unknown[];
  mark(id: string, agent: string, ageMs: number): void;
  selection(): readonly string[];
  tool(): string;
  viewport(): unknown;
  nodes(): Node[];
};
declare global {
  interface Window {
    __vellum: Hooks;
  }
}
window.__vellum = {
  reply(children, operation = "apply") {
    current = documentOf(children);
    engine.script(snapshotOf(current), operation);
  },
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
      actors={{ teal: { id: "teal", kind: "agent", name: "Teal", hue: "teal" } }}
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
      slots={{ agent: <div data-testid="agent-slot" />, right: <div data-testid="right-slot" /> }}
    />,
  );
}
render();
void daemon;
void host;
