/**
 * The command palette (DESIGN.md C23): ⌘/ opens it, and it searches actions, layers, pages, components, variables and
 * sessions. The first result is always "Ask the agent: <your query>", so any search can become a prompt; the
 * highlight starts on the first real match so Enter runs what the person was looking for.
 */

export const ASK_PREFIX = "Ask the agent: ";

export type PaletteKind =
  | "ask"
  | "command"
  | "layer"
  | "page"
  | "component"
  | "variable"
  | "session";
export interface PaletteCommand {
  id: string;
  title: string;
  category: string;
  enabled: boolean;
}
export interface PaletteEntry {
  kind: Exclude<PaletteKind, "ask" | "command">;
  id: string;
  label: string;
}
export interface PaletteResult {
  key: string;
  kind: PaletteKind;
  label: string;
  command?: string;
  entry?: PaletteEntry;
}
/** What choosing a result does. The editor binds these once it exists; until then a choice only closes the palette. */
export interface PaletteActions {
  run(command: string): void;
  open(entry: PaletteEntry): void;
  /** Puts the text in the composer; the person sends it. */
  ask(text: string): void;
}
export interface PaletteDeps extends Partial<PaletteActions> {
  commands(): readonly PaletteCommand[];
  entries(): readonly PaletteEntry[];
  /** Results shown besides Ask. */
  limit?: number;
}
export interface PaletteState {
  open: boolean;
  query: string;
  active: number;
}

const ORDER: PaletteKind[] = ["command", "layer", "page", "component", "variable", "session"];
/** Per kind, so one kind of result cannot push the others out of the list. */
const PER_KIND = 4;

/** 0: starts with the first word, 1: a word starts with it, 2: contains it, null: some word is missing. */
function rank(label: string, words: readonly string[]): number | null {
  const text = label.toLocaleLowerCase();
  if (!words.every((w) => text.includes(w))) return null;
  const first = words[0] ?? "";
  if (text.startsWith(first)) return 0;
  if (text.split(/[^\p{L}\p{N}]+/u).some((part) => part.startsWith(first))) return 1;
  return 2;
}

export function createPaletteController(deps: PaletteDeps) {
  const limit = deps.limit ?? 8;
  let actions: Partial<PaletteActions> = { run: deps.run, open: deps.open, ask: deps.ask };
  let state: PaletteState = { open: false, query: "", active: 0 };
  const listeners = new Set<() => void>();
  const change = (patch: Partial<PaletteState>) => {
    state = { ...state, ...patch };
    for (const l of [...listeners]) l();
  };

  const results = (): PaletteResult[] => {
    const query = state.query.trim();
    const words = query.toLocaleLowerCase().split(/\s+/).filter(Boolean);
    const candidates: (PaletteResult & { rank: number })[] = [];
    for (const c of deps.commands()) {
      if (!c.enabled) continue;
      const r = words.length ? rank(c.title, words) : 0;
      if (r !== null)
        candidates.push({
          key: `command:${c.id}`,
          kind: "command",
          label: c.title,
          command: c.id,
          rank: r,
        });
    }
    // With nothing typed the palette offers actions only: the document has too many layers to list.
    if (words.length)
      for (const e of deps.entries()) {
        const r = rank(e.label, words);
        if (r !== null)
          candidates.push({
            key: `${e.kind}:${e.id}`,
            kind: e.kind,
            label: e.label,
            entry: e,
            rank: r,
          });
      }
    const taken: Record<string, number> = {};
    const sorted = candidates
      .sort(
        (a, b) =>
          a.rank - b.rank ||
          ORDER.indexOf(a.kind) - ORDER.indexOf(b.kind) ||
          a.label.localeCompare(b.label),
      )
      .map(({ rank: _rank, ...r }) => r);
    const shown = sorted
      .filter((r) => {
        const n = (taken[r.kind] ?? 0) + 1;
        taken[r.kind] = n;
        return n <= PER_KIND;
      })
      .slice(0, limit);
    return query ? [{ key: "ask", kind: "ask", label: `${ASK_PREFIX}${query}` }, ...shown] : shown;
  };
  /** The first real match, else the Ask row. */
  const firstMatch = () => (results().length > 1 ? 1 : 0);

  const controller = {
    get state() {
      return state;
    },
    results,
    /** Gives the palette its actions; the last binding wins. */
    bind(next: Partial<PaletteActions>) {
      actions = { ...actions, ...next };
    },
    open() {
      change({ open: true, query: "", active: 0 });
    },
    close() {
      change({ open: false });
    },
    toggle() {
      if (state.open) controller.close();
      else controller.open();
    },
    setQuery(query: string) {
      state = { ...state, query };
      change({ active: firstMatch() });
    },
    setActive(active: number) {
      if (active >= 0 && active < results().length) change({ active });
    },
    move(by: 1 | -1) {
      const n = results().length;
      if (n) change({ active: (state.active + by + n) % n });
    },
    choose(index = state.active) {
      const r = results()[index];
      if (!r) return;
      const query = state.query.trim();
      controller.close();
      if (r.kind === "ask") actions.ask?.(query);
      else if (r.command) actions.run?.(r.command);
      else if (r.entry) actions.open?.(r.entry);
    },
    /** Arrow keys, Enter and Escape; returns whether the key was the palette's. */
    key(name: string): boolean {
      if (!state.open) return false;
      if (name === "ArrowDown") controller.move(1);
      else if (name === "ArrowUp") controller.move(-1);
      else if (name === "Enter") controller.choose();
      else if (name === "Escape") controller.close();
      else return false;
      return true;
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return { dispose: () => void listeners.delete(listener) };
    },
  };
  return controller;
}
export type PaletteController = ReturnType<typeof createPaletteController>;
