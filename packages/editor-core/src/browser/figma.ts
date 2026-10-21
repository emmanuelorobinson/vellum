/** The native host imports REST content and commits one person-origin transaction before reporting. */
export interface FigmaReport {
  nodes: number;
  frames: number;
  notes: readonly string[];
}
export interface ImportSignal {
  readonly aborted: boolean;
  subscribe(listener: () => void): { dispose(): void };
}
export interface FigmaImportHost {
  /** Capture the destination/revision; refuse to commit after cancellation or a document switch. */
  importFile?(urlOrKey: string, options: { signal: ImportSignal }): Promise<FigmaReport>;
}
export function createFigmaImport(host: FigmaImportHost) {
  let state: { source: string; busy: boolean; report: FigmaReport | null; error: string | null } = {
    source: "",
    busy: false,
    report: null,
    error: null,
  };
  let ticket = 0,
    disposed = false;
  let active: { aborted: boolean; listeners: Set<() => void> } | null = null;
  const cancel = () => {
    if (active) {
      active.aborted = true;
      for (const listener of [...active.listeners]) listener();
      active.listeners.clear();
      active = null;
    }
  };
  const listeners = new Set<() => void>();
  const update = (patch: Partial<typeof state>) => {
    state = { ...state, ...patch };
    for (const l of listeners) l();
  };
  return {
    host,
    get state() {
      return state;
    },
    subscribe(l: () => void) {
      listeners.add(l);
      return {
        dispose: () => {
          listeners.delete(l);
        },
      };
    },
    configure(source: string) {
      if (state.busy) return;
      ticket++;
      update({ source, report: null, error: null });
    },
    close() {
      cancel();
      ticket++;
      update({ busy: false, report: null, error: null });
    },
    async importFile() {
      if (disposed || state.busy || !host.importFile || !state.source.trim()) return;
      const t = ++ticket;
      const request = { aborted: false, listeners: new Set<() => void>() };
      active = request;
      const signal: ImportSignal = {
        get aborted() {
          return request.aborted;
        },
        subscribe(listener) {
          if (request.aborted) listener();
          else request.listeners.add(listener);
          return {
            dispose: () => {
              request.listeners.delete(listener);
            },
          };
        },
      };
      update({ busy: true, error: null, report: null });
      try {
        const report = await host.importFile(state.source, { signal });
        if (!disposed && ticket === t) update({ report });
      } catch {
        if (!disposed && ticket === t)
          update({
            error: "Figma import failed. Check the file access and key in Settings, then retry.",
          });
      } finally {
        if (!disposed && ticket === t) update({ busy: false });
      }
    },
    dispose() {
      disposed = true;
      cancel();
      ticket++;
      listeners.clear();
    },
  };
}
export type FigmaImportController = ReturnType<typeof createFigmaImport>;
