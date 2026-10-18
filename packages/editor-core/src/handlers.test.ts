// TEST DATA: synthetic scene and scripted engine.
import { type EngineSnapshot, FakeEngineClient, type ToolScene } from "@vellum/engine-client";
import type { Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import { createCommandController } from "./command-controller";
import { createDesignController } from "./design/controller";
import { createEditorHandlers } from "./handlers";
import { createEditorSession } from "./session";
import { createWorkspace } from "./workspace";

const box = (id: string, x: number, y: number, w: number, h: number) => ({
  id,
  bounds: { x, y, width: w, height: h },
});
const items = [box("a", 0, 0, 100, 100), box("b", 300, 0, 100, 100)];
const scene: ToolScene = {
  get: (id) => {
    const i = items.find((x) => x.id === id);
    return i
      ? {
          node: { id, type: "rect" } as unknown as Node,
          bounds: i.bounds,
          children: [],
          parent: null,
        }
      : undefined;
  },
  roots: () => items.map((i) => i.id),
  hitPoint: () => null,
  hitRect: () => [],
  snap: () => ({ offset: { x: 0, y: 0 }, guides: [] }),
  toParent: (_a, p) => p,
  toLocal: (_a, p) => p,
  path: () => null,
};
const snap = (revision: number): EngineSnapshot => ({ revision, document: null, readOnly: false });
function fixture() {
  const engine = new FakeEngineClient(snap(0), scene);
  const session = createEditorSession({
    engine,
    device: "dev",
    page: "p",
    nextNodeId: () => "n",
    createNode: () => ({}) as Node,
    viewportSize: () => ({ width: 1000, height: 600 }),
  });
  const workspace = createWorkspace();
  workspace.resize({ width: 1000, height: 600 });
  const handlers = createEditorHandlers({ workspace, session });
  const controller = createCommandController(handlers);
  return { engine, session, workspace, handlers, controller };
}
const ctx = { host: "desktop", platform: "mac", focus: "canvas" } as const;
const editor = { host: "desktop", platform: "mac", focus: "panel" } as const;

describe("palette and pause-all handlers", () => {
  it("⌘/ opens the palette and ⇧⌘. pauses every agent, only when something can do it", async () => {
    const { workspace, session } = fixture();
    const calls: string[] = [];
    const none = createEditorHandlers({ workspace, session });
    expect(none["palette.open"]).toBeUndefined();
    expect(none["agent.pauseAll"]).toBeUndefined();
    const handlers = createEditorHandlers({
      workspace,
      session,
      palette: { open: () => calls.push("palette") } as never,
      pauseAll: () => calls.push("pause"),
    });
    const controller = createCommandController(handlers);
    await controller.execute("palette.open");
    await controller.execute("agent.pauseAll");
    expect(calls).toEqual(["palette", "pause"]);
  });
});

describe("export from the keyboard", () => {
  it("⇧⌘E opens the Export dialog for the selection", async () => {
    const { workspace, session } = fixture();
    session.select(["a", "b"]);
    const opened: (readonly string[])[] = [];
    const exporter = {
      open: async (ids: readonly string[]) => void opened.push(ids),
      copyPng: async () => {},
      copyHtml: async () => {},
    };
    const controller = createCommandController(
      createEditorHandlers({ workspace, session, exporter: exporter as never }),
    );
    const result = await controller.keyDown(
      { code: "KeyE", metaKey: true, shiftKey: true },
      { host: "desktop", platform: "mac", focus: "canvas" },
    );
    expect(result.status).toBe("executed");
    expect(opened).toEqual([["a", "b"]]);
    const none = createCommandController(createEditorHandlers({ workspace, session }));
    expect(none.commands.get("edit.export")?.enabled?.()).toBe(false);
  });
});

describe("editor handlers", () => {
  it("handles every panel, tool, history and view command the chrome shows", () => {
    const { handlers } = fixture();
    for (const id of [
      "panel.agent",
      "panel.layers",
      "panel.assets",
      "panel.history",
      "view.hidePanels",
      "view.togglePanels",
      "tool.move",
      "tool.scale",
      "tool.frame",
      "tool.rect",
      "tool.ellipse",
      "tool.line",
      "tool.pen",
      "tool.text",
      "tool.note",
      "tool.hand",
      "tool.holdHand",
      "tool.releaseHand",
      "tool.holdZoom",
      "tool.releaseZoom",
      "edit.undo",
      "edit.redo",
      "select.clear",
      "select.children",
      "select.parent",
      "view.zoomIn",
      "view.zoomOut",
      "view.actualSize",
      "view.fitAll",
      "view.fitSelection",
    ])
      expect(handlers[id], id).toBeTypeOf("function");
  });
  it("⌥1–⌥4 switch the panel tab and reveal a hidden panel", async () => {
    const { controller, workspace } = fixture();
    workspace.togglePanels();
    await controller.keyDown({ code: "Digit2", altKey: true }, editor);
    expect(workspace.state.tab).toBe("layers");
    expect(workspace.layout().agent.mode).toBe("open");
    await controller.keyDown({ code: "Digit4", altKey: true }, editor);
    expect(workspace.state.tab).toBe("history");
  });
  it("Tab and ⌘\\ reach the workspace", async () => {
    const { controller, workspace } = fixture();
    await controller.keyDown({ code: "Tab" }, editor);
    expect(workspace.state.chromeHidden).toBe(true);
    await controller.keyDown({ code: "Tab" }, editor);
    expect(workspace.state.chromeHidden).toBe(false);
    await controller.keyDown({ code: "Backslash", metaKey: true }, editor);
    expect(workspace.state.panelsHidden).toBe(true);
  });
  it("tool keys change the tool", async () => {
    const { controller, session } = fixture();
    await controller.keyDown({ code: "KeyR" }, ctx);
    expect(session.state.tool).toBe("rect");
    await controller.keyDown({ code: "KeyV" }, ctx);
    expect(session.state.tool).toBe("move");
  });
  it("holding Space pans and releasing it restores the tool", async () => {
    const { controller, session } = fixture();
    await controller.keyDown({ code: "KeyR" }, ctx);
    await controller.keyDown({ code: "Space" }, ctx);
    expect(session.state.tool).toBe("hand");
    await controller.keyUp({ code: "Space" });
    expect(session.state.tool).toBe("rect");
  });
  it("zooms by stops about the middle of the canvas", () => {
    const { handlers, session } = fixture();
    handlers["view.zoomIn"]?.();
    expect(session.state.viewport.zoom).toBe(2);
    handlers["view.zoomOut"]?.();
    handlers["view.zoomOut"]?.();
    expect(session.state.viewport.zoom).toBe(0.5);
    handlers["view.actualSize"]?.();
    expect(session.state.viewport.zoom).toBe(1);
  });
  it("fits everything, and the selection when there is one", () => {
    const { handlers, session, workspace } = fixture();
    workspace.resize({ width: 500, height: 300 });
    handlers["view.fitAll"]?.();
    const all = session.state.viewport.zoom;
    session.select(["a"]);
    handlers["view.fitSelection"]?.();
    expect(session.state.viewport.zoom).toBeGreaterThan(all);
  });
  it("leaves the viewport alone when fitting an empty selection", () => {
    const { handlers, session } = fixture();
    const before = { ...session.state.viewport };
    handlers["view.fitSelection"]?.();
    expect(session.state.viewport).toEqual(before);
  });
  it("undo and redo are the person's own", () => {
    const { handlers, engine } = fixture();
    engine.script(snap(1), "undo");
    handlers["edit.undo"]?.();
    expect(engine.historyActors).toEqual([{ kind: "person", device: "dev" }]);
  });
});

describe("component commands", () => {
  const instanceScene: ToolScene = {
    ...scene,
    get: (id) =>
      id === "inst"
        ? {
            node: {
              id,
              type: "instance",
              name: "Button",
              componentId: "c1",
              width: 10,
              height: 10,
            } as unknown as Node,
            bounds: { x: 0, y: 0, width: 10, height: 10 },
            children: [],
            parent: null,
          }
        : id === "nested"
          ? {
              node: {
                id,
                type: "instance",
                name: "Icon",
                componentId: "c2",
                width: 10,
                height: 10,
              } as unknown as Node,
              bounds: { x: 0, y: 0, width: 10, height: 10 },
              children: [],
              parent: null,
              instance: { id: "inst", path: "inst/nested" },
            }
          : scene.get(id),
  };
  function withDesign() {
    const engine = new FakeEngineClient(snap(0), instanceScene);
    const session = createEditorSession({
      engine,
      device: "dev",
      page: "p",
      nextNodeId: () => "n",
      createNode: () => ({}) as Node,
      viewportSize: () => ({ width: 1000, height: 600 }),
    });
    const workspace = createWorkspace();
    const design = createDesignController({
      session,
      engine,
      defaults: { fill: "#D9D9D9", stroke: "#1F1F1F", shadow: "#00000040" },
      text: { fallback: "Inter" },
    });
    const handlers = createEditorHandlers({ workspace, session, design });
    return { engine, session, design, handlers, controller: createCommandController(handlers) };
  }
  it("has no handler for them without the properties panel, so the commands are disabled", () => {
    const { controller } = fixture();
    expect(controller.commands.get("component.detach")?.enabled?.()).toBe(false);
    expect(controller.commands.get("component.replace")?.enabled?.()).toBe(false);
  });
  it("detaches the selected instances in one transaction", async () => {
    const { engine, session, handlers, controller } = withDesign();
    expect(controller.commands.get("component.detach")?.enabled?.()).toBe(true);
    session.select(["inst"]);
    engine.script(snap(1));
    handlers["component.detach"]?.();
    expect(engine.transactions).toHaveLength(1);
    expect(engine.transactions[0]?.ops).toEqual([{ type: "detach", id: "inst" }]);
  });
  it("detaches nothing for layers that are not instances, or are inside another instance", () => {
    const { engine, session, handlers } = withDesign();
    session.select(["a", "nested"]);
    engine.script(snap(1));
    handlers["component.detach"]?.();
    expect(engine.transactions).toHaveLength(0);
  });
  it("asks the panel to open the replace picker", () => {
    const { design, handlers } = withDesign();
    let seen = 0;
    design.subscribe(() => {
      seen++;
    });
    handlers["component.replace"]?.();
    expect(seen).toBe(1);
    expect(design.takeRequest()).toBe("replace");
    expect(design.takeRequest()).toBeNull();
  });
});
