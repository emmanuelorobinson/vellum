// TEST DATA: Juniper pages, layers, components, variables and sessions.
import { type EngineSnapshot, FakeEngineClient, type ToolScene } from "@vellum/engine-client";
import type { Document, Node } from "@vellum/types-ts";
import { describe, expect, it, vi } from "vitest";
import type { SessionInfo } from "../agent/sessions";
import { createCommandController } from "../command-controller";
import { createEditorHandlers } from "../handlers";
import { createEditorSession } from "../session";
import { createWorkspace } from "../workspace";
import {
  createPaletteController,
  createShellActions,
  paletteCommands,
  paletteEntries,
} from "./index";

const doc = {
  meta: {},
  components: { c1: { name: "Button / Primary", root: { id: "r", type: "frame" } } },
  variables: { v1: { name: "color/primary", type: "color" } },
  pages: [
    {
      id: "p1",
      name: "Onboarding",
      children: [
        {
          type: "frame",
          id: "step2",
          name: "Step 2 · Onboarding",
          children: [{ type: "text", id: "title", name: "Title" }],
        },
      ],
    },
    { id: "p2", name: "Settings", children: [] },
  ],
} as unknown as Document;
const sessions = [
  {
    id: "s1",
    title: "Onboarding screens",
    kind: { type: "engine", engine: "builtin" },
    agent: "a",
    createdAt: 0,
    updatedAt: 1,
    state: "done",
    agents: 1,
  },
] as unknown as SessionInfo[];

const scene: ToolScene = {
  get: (id) =>
    id === "step2"
      ? {
          node: { id, type: "frame" } as unknown as Node,
          bounds: { x: 200, y: 100, width: 300, height: 400 },
          children: [],
          parent: null,
        }
      : undefined,
  roots: () => ["step2"],
  hitPoint: () => null,
  hitRect: () => [],
  snap: () => ({ offset: { x: 0, y: 0 }, guides: [] }),
  toParent: (_a, p) => p,
  toLocal: (_a, p) => p,
  path: () => null,
};
function fixture() {
  const snapshot: EngineSnapshot = { revision: 0, document: doc, readOnly: false };
  const engine = new FakeEngineClient(snapshot, scene);
  const session = createEditorSession({
    engine,
    device: "dev",
    page: "p1",
    nextNodeId: () => "n",
    createNode: () => ({}) as Node,
    viewportSize: () => ({ width: 1000, height: 600 }),
  });
  const workspace = createWorkspace();
  workspace.resize({ width: 1000, height: 600 });
  return { session, workspace };
}

describe("palette entries", () => {
  it("lists pages, every layer, components, variables and sessions", () => {
    const entries = paletteEntries(doc, sessions);
    expect(entries.map((e) => `${e.kind}:${e.label}`)).toEqual([
      "page:Onboarding",
      "layer:Step 2 · Onboarding",
      "layer:Title",
      "page:Settings",
      "component:Button / Primary",
      "variable:color/primary",
      "session:Onboarding screens",
    ]);
    expect(paletteEntries(null)).toEqual([]);
  });
  it("lists the registry's commands with whether they can run", () => {
    const { session, workspace } = fixture();
    const controller = createCommandController(createEditorHandlers({ workspace, session }));
    const list = paletteCommands(controller.commands);
    expect(list.find((c) => c.id === "panel.layers")?.enabled).toBe(true);
    expect(list.find((c) => c.id === "edit.group")?.enabled).toBe(false);
  });
});

describe("shell actions", () => {
  it("opening a layer selects it and brings it into view", () => {
    const { session, workspace } = fixture();
    const actions = createShellActions({ workspace, session, document: () => doc });
    actions.open({ kind: "layer", id: "step2", label: "Step 2 · Onboarding" });
    expect(session.state.selection).toEqual(["step2"]);
    expect(session.state.viewport.zoom).toBeGreaterThan(0);
  });
  it("opening a page changes the page; a component opens Assets; a variable opens Variables", () => {
    const { session, workspace } = fixture();
    const open = vi.fn();
    const actions = createShellActions({
      workspace,
      session,
      document: () => doc,
      resources: { open } as never,
    });
    actions.open({ kind: "page", id: "p2", label: "Settings" });
    expect(session.page).toBe("p2");
    actions.open({ kind: "component", id: "c1", label: "Button" });
    expect(workspace.state.tab).toBe("assets");
    actions.open({ kind: "variable", id: "v1", label: "color/primary" });
    expect(open).toHaveBeenCalledWith("variables");
  });
  it("Ask the agent shows the Agent tab and prefills the composer; it never sends", () => {
    const { session, workspace } = fixture();
    workspace.switchTab("layers");
    const composer = { prefill: vi.fn(), requestFocus: vi.fn() };
    const actions = createShellActions({
      workspace,
      session,
      document: () => doc,
      composer: composer as never,
    });
    actions.ask("tighten the spacing");
    expect(workspace.state.tab).toBe("agent");
    expect(composer.prefill).toHaveBeenCalledWith("tighten the spacing");
    actions.askAbout();
    expect(composer.requestFocus).toHaveBeenCalled();
    actions.redesign();
    expect(composer.prefill).toHaveBeenLastCalledWith("Redesign ");
  });
  it("a palette bound to the actions runs, opens and asks through them", () => {
    const { session, workspace } = fixture();
    const composer = { prefill: vi.fn(), requestFocus: vi.fn() };
    const actions = createShellActions({
      workspace,
      session,
      document: () => doc,
      composer: composer as never,
    });
    const run = vi.fn();
    const palette = createPaletteController({
      commands: () => [
        { id: "panel.layers", title: "Layers panel", category: "Agents", enabled: true },
      ],
      entries: () => paletteEntries(doc, sessions),
    });
    palette.open();
    palette.setQuery("layers");
    palette.choose(1);
    expect(workspace.state.tab).not.toBe("layers");
    palette.bind({ run, open: actions.open, ask: actions.ask });
    palette.open();
    palette.setQuery("layers");
    palette.choose(1);
    expect(run).toHaveBeenCalledWith("panel.layers");
    palette.open();
    palette.setQuery("make it calmer");
    palette.choose(0);
    expect(composer.prefill).toHaveBeenCalledWith("make it calmer");
  });
});
