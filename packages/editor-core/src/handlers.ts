import { detachOps } from "./design/component";
import type { DesignController } from "./design/controller";
import { union } from "./geometry";
import type { EditorSession } from "./session";
import type { Box, ToolId } from "./tool-types";
import { fitViewport, nextZoomStep, zoomAbout } from "./viewport";
import { PANEL_TABS, type Workspace } from "./workspace";

type Handler = () => void;

/**
 * The handlers the chrome needs, for `createCommandController`: panels, tools, your own undo and redo, selection
 * keys and view commands. Commands that need a feature that does not exist yet have no handler, so the
 * controller reports them disabled instead of pretending.
 */
export function createEditorHandlers(deps: {
  workspace: Workspace;
  session: EditorSession;
  /** The properties panel's controller: with it the component commands work. */
  design?: DesignController;
}): Record<string, Handler> {
  const { workspace, session, design } = deps;
  const handlers: Record<string, Handler> = {};
  for (const tab of PANEL_TABS)
    handlers[`panel.${tab}`] = () => workspace.switchTab(tab, { reveal: true });
  handlers["view.hidePanels"] = () => workspace.hideAllPanels();
  handlers["view.togglePanels"] = () => workspace.togglePanels();
  const tools: ToolId[] = [
    "move",
    "scale",
    "frame",
    "rect",
    "ellipse",
    "line",
    "pen",
    "text",
    "note",
    "hand",
  ];
  for (const tool of tools)
    handlers[`tool.${tool}`] = () => session.dispatch({ type: "tool", tool });
  handlers["tool.holdHand"] = () =>
    session.dispatch({ type: "keyDown", key: " ", canvasFocus: true });
  handlers["tool.releaseHand"] = () => session.dispatch({ type: "keyUp", key: " " });
  handlers["tool.holdZoom"] = () =>
    session.dispatch({ type: "keyDown", key: "z", canvasFocus: true });
  handlers["tool.releaseZoom"] = () => session.dispatch({ type: "keyUp", key: "z" });
  for (const [id, key, mods] of [
    ["path.finish", "Enter", {}],
    ["path.cut", "x", {}],
    ["path.connect", "j", { command: true }],
    ["path.delete", "Delete", {}],
    ["path.selectAll", "a", { command: true }],
    ...["Up", "Down", "Left", "Right"].flatMap((d) => [
      [`path.nudge${d}`, `Arrow${d}`, {}],
      [`path.bigNudge${d}`, `Arrow${d}`, { shift: true }],
    ]),
  ] as [string, string, { command?: boolean; shift?: boolean }][])
    handlers[id] = () => session.dispatch({ type: "keyDown", key, canvasFocus: true, ...mods });
  handlers["path.pen"] = () => session.dispatch({ type: "tool", tool: "pen" });
  handlers["path.select"] = () => session.dispatch({ type: "tool", tool: "move" });
  handlers["path.toggleSmooth"] = () =>
    session.dispatch({ type: "pathAction", action: "toggleSmooth" });
  handlers["edit.undo"] = () => void session.undo();
  handlers["edit.redo"] = () => void session.redo();
  handlers["select.clear"] = () =>
    session.dispatch({ type: "keyDown", key: "Escape", canvasFocus: true });
  handlers["select.children"] = () =>
    session.dispatch({ type: "keyDown", key: "Enter", canvasFocus: true });
  handlers["select.parent"] = () =>
    session.dispatch({ type: "keyDown", key: "Enter", shift: true, canvasFocus: true });

  const size = () => {
    const { width, height } = workspace.state;
    return { width, height };
  };
  const centre = () => {
    const insets = workspace.layout().canvasInsets;
    const s = size();
    return { x: insets.left + (s.width - insets.left - insets.right) / 2, y: s.height / 2 };
  };
  const zoomTo = (zoom: number) =>
    session.setViewport(zoomAbout(session.state.viewport, centre(), zoom));
  handlers["view.zoomIn"] = () => zoomTo(nextZoomStep(session.state.viewport.zoom, "in"));
  handlers["view.zoomOut"] = () => zoomTo(nextZoomStep(session.state.viewport.zoom, "out"));
  handlers["view.actualSize"] = () => zoomTo(1);
  const fit = (boxes: Box[]) => {
    const target = union(boxes);
    if (!target) return;
    session.setViewport(
      fitViewport(target, size(), workspace.layout().canvasInsets, { maxZoom: 1 }),
    );
  };
  const boxesOf = (ids: readonly string[]) =>
    ids.flatMap((id) => {
      const item = session.sceneBounds(id);
      return item ? [item] : [];
    });
  if (design) {
    handlers["component.detach"] = () => {
      design.ops(design.targets().flatMap((t) => detachOps(t.item)));
    };
    handlers["component.replace"] = () => design.requestReplace();
  }
  handlers["view.fitAll"] = () => fit(boxesOf(session.roots()));
  handlers["view.fitSelection"] = () => fit(boxesOf(session.state.selection));
  return handlers;
}
