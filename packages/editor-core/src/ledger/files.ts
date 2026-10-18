import type { EngineClient, Place, ToolOp } from "@vellum/engine-client";
import type { ConflictNode, DeviceId, Document, Node, NodeId } from "@vellum/types-ts";
import { personTransaction } from "../design/patch";

export type FileChange =
  /** The file changed on disk and the engine applied it as ops (a git checkout, a pull, another tool). */
  | { kind: "applied"; source: string; count: number; entry?: string }
  /** A three-way merge left conflict nodes in the document. */
  | { kind: "conflict"; versions: number; nodes: readonly NodeId[] };
export interface FileChangeFeed {
  current(): FileChange | null;
  subscribe(listener: () => void): { dispose(): void };
}
export type ConflictChoice = "ours" | "theirs" | "both";

const kids = (node: Node): Node[] => (node as { children?: Node[] }).children ?? [];
const asConflict = (node: Node) => (node.type === "conflict" ? (node as ConflictNode) : null);
/** Every conflict node the merge left, in document order. */
export function conflictNodes(doc: Document | null): ConflictNode[] {
  const out: ConflictNode[] = [];
  const walk = (node: Node) => {
    const c = asConflict(node);
    if (c) out.push(c);
    for (const child of kids(node)) walk(child);
  };
  for (const page of doc?.pages ?? []) for (const node of page.children ?? []) walk(node);
  return out;
}
/** Where a node sits, so a second version can go in beside it. */
export function findPlace(doc: Document | null, id: NodeId): Place | null {
  const search = (list: readonly Node[], parent: Place["parent"]): Place | null => {
    for (const [index, node] of list.entries()) {
      if (node.id === id) return { parent, index };
      const inner = search(kids(node), { type: "node", id: node.id });
      if (inner) return inner;
    }
    return null;
  };
  for (const page of doc?.pages ?? []) {
    const found = search(page.children ?? [], { type: "page", id: page.id });
    if (found) return found;
  }
  return null;
}
/**
 * The ops that settle a conflict. The conflict's merged children stay whichever version wins. A removed
 * side is a version too: keeping it deletes the node. "Both" keeps ours in place and adds theirs beside it.
 */
export function resolveConflictOps(
  conflict: ConflictNode,
  choice: ConflictChoice,
  nextId: () => NodeId,
  place: Place,
): ToolOp[] {
  const withKids = (version: Node): Node =>
    ({
      ...version,
      id: conflict.id,
      ...(conflict.children?.length ? { children: conflict.children } : {}),
    }) as Node;
  const pick = (side: "ours" | "theirs"): ToolOp =>
    conflict[side]
      ? { type: "replace", id: conflict.id, node: withKids(conflict[side] as Node) }
      : { type: "delete", id: conflict.id };
  if (choice !== "both") return [pick(choice)];
  if (!conflict.ours || !conflict.theirs) return [pick(conflict.ours ? "ours" : "theirs")];
  const theirs = conflict.theirs as Node;
  const copy = {
    ...theirs,
    id: nextId(),
    name: theirs.name === conflict.ours.name ? `${theirs.name} (theirs)` : theirs.name,
  } as Node;
  return [
    pick("ours"),
    { type: "insert", to: { parent: place.parent, index: place.index + 1 }, node: copy },
  ];
}

export interface ConflictDeps {
  engine: EngineClient;
  device: DeviceId;
  feed: FileChangeFeed;
  fileName: string;
  nextNodeId(): NodeId;
  /** "Ask agent to merge": the host starts a session turn through the PermissionBroker. */
  askAgent?(node: NodeId): void;
  /** Opens the History tab on one entry. */
  openLedger?(entry: string): void;
}
export function createConflictController(deps: ConflictDeps) {
  const { engine, feed } = deps;
  let state: { node: ConflictNode | null; error: string | null } = { node: null, error: null };
  let dismissed: FileChange | null = null;
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const l of [...listeners]) l();
  };
  const change = (patch: Partial<typeof state>) => {
    state = { ...state, ...patch };
    notify();
  };
  const subs = [feed.subscribe(notify), engine.subscribe(notify)];
  const c = {
    get state() {
      return state;
    },
    subscribe(l: () => void) {
      listeners.add(l);
      return { dispose: () => void listeners.delete(l) };
    },
    conflicts: () => conflictNodes(engine.snapshot().document),
    banner(): { text: string; action: string } | null {
      const now = feed.current();
      if (!now || now === dismissed) return null;
      if (now.kind === "applied")
        return {
          text: `${deps.fileName} changed on disk (${now.source}) · ${now.count} change${now.count === 1 ? "" : "s"} applied`,
          action: "View in ledger",
        };
      if (!c.conflicts().length) return null;
      return {
        text: `${deps.fileName} has a merge conflict · ${now.versions} versions`,
        action: "Resolve",
      };
    },
    runBanner() {
      const now = feed.current();
      if (now?.kind === "applied") {
        if (now.entry) deps.openLedger?.(now.entry);
      } else {
        const first = c.conflicts()[0];
        if (first) c.open(first.id);
      }
    },
    dismiss() {
      dismissed = feed.current();
      notify();
    },
    open(id: NodeId) {
      const node = c.conflicts().find((n) => n.id === id) ?? null;
      change({ node, error: node ? null : "That conflict is already resolved" });
    },
    close() {
      change({ node: null, error: null });
    },
    versions() {
      const n = state.node;
      return n
        ? [
            { side: "ours" as const, label: "Ours", node: (n.ours ?? null) as Node | null },
            { side: "theirs" as const, label: "Theirs", node: (n.theirs ?? null) as Node | null },
          ]
        : [];
    },
    askAgent() {
      if (state.node) deps.askAgent?.(state.node.id);
    },
    pick(choice: ConflictChoice) {
      const node = state.node;
      const snapshot = engine.snapshot();
      if (!node) return;
      if (snapshot.readOnly) return change({ error: "This file is read-only" });
      const place = findPlace(snapshot.document, node.id);
      if (!place) return change({ error: "That conflict is already resolved" });
      const tx = personTransaction(
        deps.device,
        resolveConflictOps(node, choice, deps.nextNodeId, place),
      );
      const result = tx ? engine.apply(tx) : null;
      if (result?.status !== "applied")
        return change({
          error: `Change refused: ${result?.status === "rejected" ? result.reason : "nothing to apply"}`,
        });
      change({ node: null, error: null });
    },
    dispose() {
      for (const s of subs) s.dispose();
      listeners.clear();
    },
  };
  return c;
}
export type ConflictController = ReturnType<typeof createConflictController>;
