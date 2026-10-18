/**
 * Editor settings (DESIGN.md C25): the open/closed state of the dialog and the keyboard shortcuts section, which is
 * the one place every shortcut is listed, searched and rebound, with host conflicts marked. Everything else in the
 * dialog is a typed preference or an Agent feed setting, read and written by the view.
 */
import { type BindingContext, type Keybindings, keyboardBindings } from "../keybindings";
import type { PreferencesController } from "./preferences";

export interface ShortcutRow {
  command: string;
  title: string;
  category: string;
  chord: string;
  /** The host keeps this chord (⌘P, ⌘S ...) or the command is a desktop-only file command. */
  hostConflict: boolean;
  scope: string;
}

export function createSettingsController(deps: {
  preferences: PreferencesController;
  keys: Keybindings;
  context: BindingContext;
}) {
  const { keys, context } = deps;
  let open = false;
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const l of [...listeners]) l();
  };
  return {
    get state() {
      return { open };
    },
    preferences: deps.preferences,
    open() {
      open = true;
      notify();
    },
    close() {
      open = false;
      notify();
    },
    toggle() {
      open = !open;
      notify();
    },
    /** Every binding once, filtered by the query (command, title, category or chord). */
    shortcuts(query: string): ShortcutRow[] {
      const rows = query.trim() ? keys.search(query, context) : keys.list(context);
      return rows.map((b) => ({
        command: b.command,
        title: b.title,
        category: b.category,
        chord: b.chord,
        hostConflict: b.hostConflict,
        scope: b.scope,
      }));
    },
    /** What a command is called in the shortcut list, for a conflict message. */
    titleOf(command: string): string {
      return keyboardBindings.find((b) => b.command === command)?.title ?? command;
    },
    rebind(command: string, chord: string | null) {
      const result = keys.rebind(command, chord, context);
      if (result.ok) notify();
      return result;
    },
    resetShortcut(command: string) {
      keys.reset(command, context);
      notify();
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return { dispose: () => void listeners.delete(listener) };
    },
  };
}
export type SettingsController = ReturnType<typeof createSettingsController>;
