import { copy } from "@vellum/config";
import type { EngineClient, ToolOp } from "@vellum/engine-client";
import type { DeviceId, NodeId, PageId } from "@vellum/types-ts";
import { personTransaction } from "../design/patch";

/** Who made a batch. Agents are rounded squares, people circles, the file a document glyph (AGENTS.md). */
export interface LedgerActor {
  id: string;
  kind: "agent" | "person" | "file";
  /** "Teal", "Claude Code", a person's name, or "File". */
  name: string;
  /** An agent hue name from the tokens; ignored for people and the file. */
  hue?: string;
}
export interface LedgerOpView {
  id: string;
  /** "Update Continue · #cta-02". */
  label: string;
  /** "fill: #F3F3F1 → {color/surface}". */
  detail?: string;
  /** The op as the engine recorded it; Copy JSON writes these. */
  json: unknown;
}
/** One accepted transaction (or a rejected write) in the change ledger, newest last as the host lists them. */
export interface LedgerEntry {
  id: string;
  at: number;
  actor: LedgerActor;
  summary: string;
  count: number;
  page: PageId | null;
  session: string | null;
  /** The session's title, for the session filter ("Onboarding screens"). */
  sessionTitle?: string;
  nodes: readonly NodeId[];
  ops: readonly LedgerOpView[];
  /** The ops that undo this batch; absent when the host cannot compute them (Revert is then unavailable). */
  inverse?: readonly ToolOp[] | undefined;
  /** This entry reverts that one. */
  revertOf?: string;
  /** Plain Undo applies only to the newest batch that nothing later depends on. */
  undoable?: boolean;
  /** A write the Guard refused; it changed nothing. */
  rejected?: { frame: NodeId; by: string };
  /** "git checkout" for a change that arrived from disk. */
  source?: string;
}
/** The ledger as the daemon's session log serves it. The UI never derives entries itself. */
export interface HistoryHost {
  entries(): readonly LedgerEntry[];
  subscribe(listener: () => void): { dispose(): void };
  /** Undo one agent batch (vellum_core::ops::undo_turn); the host answers when it is applied. */
  undo?(entry: LedgerEntry): Promise<void>;
  copy?(text: string): void;
}
export type ActorFilter = "all" | "you" | "agents" | "file";
export interface HistoryNotice {
  title: string;
  body: string;
}
export interface HistoryState {
  expanded: string | null;
  actor: ActorFilter;
  page: PageId | "all";
  session: string | "all";
  confirm: string | null;
  notice: HistoryNotice | null;
  error: string | null;
}
export interface HistoryDeps {
  engine: EngineClient;
  device: DeviceId;
  host: HistoryHost;
  select?(ids: readonly NodeId[]): void;
  zoom?(ids: readonly NodeId[]): void;
}

export function createHistoryController(deps: HistoryDeps) {
  const { engine, host } = deps;
  let state: HistoryState = {
    expanded: null,
    actor: "all",
    page: "all",
    session: "all",
    confirm: null,
    notice: null,
    error: null,
  };
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const l of [...listeners]) l();
  };
  const change = (patch: Partial<HistoryState>) => {
    state = { ...state, ...patch };
    notify();
  };
  const hostSub = host.subscribe(notify);
  const find = (id: string) => host.entries().find((e) => e.id === id);
  const live = (ids: readonly NodeId[]) => ids.filter((id) => engine.scene.get(id));
  const matches = (e: LedgerEntry) =>
    (state.actor === "all" ||
      (state.actor === "you" && e.actor.kind === "person") ||
      (state.actor === "agents" && e.actor.kind === "agent") ||
      (state.actor === "file" && e.actor.kind === "file")) &&
    (state.page === "all" || e.page === state.page) &&
    (state.session === "all" || e.session === state.session);
  const c = {
    get state() {
      return state;
    },
    footer: copy.historyFooter,
    emptyText: copy.historyEmpty,
    subscribe(l: () => void) {
      listeners.add(l);
      return { dispose: () => void listeners.delete(l) };
    },
    /** Newest first; entries with the same time keep the host's order, latest first. */
    view(): LedgerEntry[] {
      return host
        .entries()
        .map((entry, index) => ({ entry, index }))
        .filter((x) => matches(x.entry))
        .sort((a, b) => b.entry.at - a.entry.at || b.index - a.index)
        .map((x) => x.entry);
    },
    /** True when there is nothing at all, as opposed to nothing under the current filter. */
    empty: () => host.entries().length === 0,
    pages(): { id: PageId; name: string }[] {
      return (engine.snapshot().document?.pages ?? []).map((p) => ({ id: p.id, name: p.name }));
    },
    sessions(): { id: string; title: string }[] {
      const seen = new Map<string, string>();
      for (const e of host.entries())
        if (e.session && !seen.has(e.session)) seen.set(e.session, e.sessionTitle ?? e.session);
      return [...seen].map(([id, title]) => ({ id, title }));
    },
    filter(patch: Partial<Pick<HistoryState, "actor" | "page" | "session">>) {
      change(patch);
    },
    toggle(id: string) {
      change({ expanded: state.expanded === id ? null : id });
    },
    open(id: string) {
      change({ expanded: id });
    },
    selectLayers(id: string) {
      const e = find(id);
      if (e) deps.select?.(live(e.nodes));
    },
    zoomTo(id: string) {
      const e = find(id);
      if (!e) return;
      deps.zoom?.(live(e.nodes));
    },
    copyJson(id: string) {
      const e = find(id);
      if (!e) return null;
      const text = JSON.stringify(
        e.ops.map((o) => o.json),
        null,
        2,
      );
      host.copy?.(text);
      return text;
    },
    canUndo: (e: LedgerEntry) => !!host.undo && !!e.undoable,
    askRevert(id: string) {
      const e = find(id);
      if (!e) return;
      if (engine.snapshot().readOnly) return change({ error: "This file is read-only" });
      if (!e.inverse?.length) return change({ error: "This entry cannot be reverted" });
      change({ confirm: id, error: null, notice: null });
    },
    cancelRevert() {
      change({ confirm: null });
    },
    /** The inverse goes in as ONE new person transaction, so the revert is its own ledger entry. */
    confirmRevert() {
      const e = state.confirm ? find(state.confirm) : undefined;
      if (!e?.inverse?.length) return change({ confirm: null });
      const tx = personTransaction(deps.device, [...e.inverse]);
      const result = tx ? engine.apply(tx) : null;
      if (result?.status !== "applied")
        return change({
          confirm: null,
          error: `Revert refused: ${result?.status === "rejected" ? result.reason : "nothing to apply"}`,
        });
      change({
        confirm: null,
        error: null,
        notice: {
          title: "Reverted by you",
          body: "Applied inverse changes as a new batch. Later work remains in the ledger.",
        },
      });
    },
    async undo(id: string) {
      const e = find(id);
      if (!e) return;
      if (!host.undo) return change({ error: "Undo is not available for this entry" });
      try {
        await host.undo(e);
        change({
          error: null,
          notice: {
            title: "Undone by you",
            body: `${e.actor.name}’s batch was undone. Your edits are unchanged.`,
          },
        });
      } catch (err) {
        change({ error: (err as Error).message });
      }
    },
    dismiss() {
      change({ notice: null, error: null });
    },
    dispose() {
      hostSub.dispose();
      listeners.clear();
    },
  };
  return c;
}
export type HistoryController = ReturnType<typeof createHistoryController>;
