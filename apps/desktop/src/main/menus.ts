import { MENU_CATEGORIES, type MenuCategory } from "@vellum/host-electron/bridge";
export interface MenuAction {
  id: string;
  label: string;
  enabled: boolean;
  accelerator?: string;
  command?: string;
}
/** Unsupported runtime actions stay visible and disabled. No menu supplies document bytes or Ops. */
export function menuModel(state: {
  hasFolder: boolean;
  hasDocument: boolean;
  daemon: boolean;
}): { label: MenuCategory; items: MenuAction[] }[] {
  const file: MenuAction[] = [
    {
      id: "folder.open",
      label: "Open a folder…",
      accelerator: "CommandOrControl+Shift+O",
      enabled: true,
    },
    {
      id: "file.open",
      label: "Open files…",
      accelerator: "CommandOrControl+O",
      enabled: state.hasFolder,
    },
    {
      id: "file.new",
      label: "New Design File…",
      accelerator: "CommandOrControl+N",
      enabled: state.hasFolder && state.daemon,
    },
    { id: "file.save", label: "Save", accelerator: "CommandOrControl+S", enabled: false },
    {
      id: "file.saveAs",
      label: "Save As…",
      accelerator: "CommandOrControl+Shift+S",
      enabled: false,
    },
    {
      id: "file.close",
      label: "Close tab",
      accelerator: "CommandOrControl+W",
      enabled: state.hasDocument,
    },
    {
      id: "window.close",
      label: "Close window",
      accelerator: "CommandOrControl+Shift+W",
      enabled: true,
    },
  ];
  const action = (id: string, label: string, accelerator?: string): MenuAction => ({
    id,
    label,
    enabled: false,
    accelerator,
  });
  const view = (id: string, label: string, command: string, accelerator?: string): MenuAction => ({
    id,
    label,
    command,
    accelerator,
    enabled: state.hasDocument,
  });
  const groups: Record<MenuCategory, MenuAction[]> = {
    File: file,
    Edit: [
      action("edit.undo", "Undo your changes", "CommandOrControl+Z"),
      action("edit.redo", "Redo your changes", "CommandOrControl+Shift+Z"),
      action("edit.cut", "Cut"),
      action("edit.copy", "Copy"),
      action("edit.paste", "Paste"),
      action("edit.duplicate", "Duplicate"),
      action("edit.delete", "Delete"),
    ],
    View: [
      view("view.agent", "Agent", "panel.agent", "Alt+1"),
      view("view.layers", "Layers", "panel.layers", "Alt+2"),
      view("view.assets", "Assets", "panel.assets", "Alt+3"),
      view("view.history", "History", "panel.history", "Alt+4"),
      view("view.panels", "Toggle panels", "view.togglePanels", "CommandOrControl+\\"),
      view("view.zoomIn", "Zoom in", "view.zoomIn", "CommandOrControl+Plus"),
      view("view.zoomOut", "Zoom out", "view.zoomOut", "CommandOrControl+-"),
      view("view.zoom100", "Actual size", "view.actualSize", "CommandOrControl+0"),
    ],
    Object: [
      action("edit.group", "Group"),
      action("edit.ungroup", "Ungroup"),
      action("edit.wrap", "Wrap in frame"),
      action("edit.autoLayout", "Add auto-layout"),
      action("component.toggle", "Make component"),
      action("component.detach", "Detach instance"),
    ],
    Arrange: [
      action("arrange.forward", "Bring forward"),
      action("arrange.backward", "Send backward"),
      action("arrange.front", "Bring to front"),
      action("arrange.back", "Send to back"),
      action("arrange.left", "Align left"),
      action("arrange.right", "Align right"),
      action("arrange.top", "Align top"),
      action("arrange.bottom", "Align bottom"),
    ],
    Agents: [
      action("agent.focus", "Focus composer", "CommandOrControl+L"),
      action("agent.stop", "Stop current session", "CommandOrControl+."),
      {
        id: "agent.pauseAll",
        label: "Pause all agents",
        accelerator: "CommandOrControl+Shift+.",
        enabled: state.daemon,
      },
      { id: "daemon.restart", label: "Restart daemon connection", enabled: state.hasFolder },
    ],
    Help: [
      {
        id: "help.shortcuts",
        label: "Keyboard shortcuts",
        enabled: false,
      },
      { id: "help.about", label: "About", enabled: true },
    ],
  };
  return MENU_CATEGORIES.map((label) => ({ label, items: groups[label] }));
}
