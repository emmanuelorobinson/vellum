/**
 * The layer context menu (DESIGN.md C24): Ask about this… and Redesign this… first, then Copy / Paste, Group, Frame
 * selection and Add auto-layout, Create component, arrange, the copy-as rows, and Delete at the bottom. Each row names
 * the command the controller runs, so a shortcut shown here is the shortcut that works.
 */
export interface ContextMenuItem {
  id: string;
  label: string;
  /** The command the controller (or the host, for commands it owns) runs. */
  command: string;
  enabled: boolean;
  destructive?: boolean;
  /** Rows of one group sit together; a rule separates groups. */
  group: number;
}

const ROWS: readonly (readonly [group: number, command: string, label: string])[] = [
  [0, "agent.askAbout", "Ask about this…"],
  [0, "agent.redesign", "Redesign this…"],
  [1, "edit.copy", "Copy"],
  [1, "edit.paste", "Paste"],
  [1, "edit.pasteHere", "Paste here"],
  [2, "edit.group", "Group"],
  [2, "edit.wrap", "Frame selection"],
  [2, "edit.autoLayout", "Add auto-layout"],
  [3, "component.toggle", "Create component"],
  [4, "arrange.forward", "Bring forward"],
  [5, "edit.copyCss", "Copy as CSS"],
  [5, "edit.copyPng", "Copy as PNG"],
  [5, "edit.copyNodeId", "Copy node ID"],
  [6, "edit.delete", "Delete"],
];

/** What changes the document: a read-only file keeps asking and copying and refuses the rest. */
const EDITS = new Set([
  "agent.redesign",
  "edit.paste",
  "edit.pasteHere",
  "edit.group",
  "edit.wrap",
  "edit.autoLayout",
  "component.toggle",
  "arrange.forward",
  "edit.delete",
]);

export function contextMenuItems(input: {
  /** How many layers the menu acts on. */
  count: number;
  enabled(command: string): boolean;
  readOnly?: boolean;
}): ContextMenuItem[] {
  return ROWS.map(([group, command, label]) => ({
    id: command,
    label,
    command,
    group,
    enabled:
      input.count > 0 &&
      input.enabled(command) &&
      !(input.readOnly && EDITS.has(command)) &&
      (command !== "edit.copyNodeId" || input.count === 1),
    ...(command === "edit.delete" ? { destructive: true } : {}),
  }));
}
