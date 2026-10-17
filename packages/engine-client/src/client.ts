import type { Actor, Document } from "@vellum/types-ts";
import { clone } from "./clone";
import type { ToolScene, ToolTransaction } from "./model";
import { sceneView } from "./scene";

export interface EngineSnapshot {
  revision: number;
  document: Document | null;
  readOnly: boolean;
}
export type EngineCommit =
  | { status: "applied"; revision: number }
  | { status: "rejected"; reason: "read-only" | "invalid" | "unavailable" };
export interface EngineClient {
  readonly state: "ready" | "disposed";
  readonly scene: ToolScene;
  snapshot(): EngineSnapshot;
  apply(transaction: ToolTransaction): EngineCommit;
  undo(actor: Actor): EngineCommit;
  redo(actor: Actor): EngineCommit;
  subscribe(listener: (snapshot: EngineSnapshot) => void): { dispose(): void };
  dispose(): void;
}
/** The WASM composition supplies this facade over its Rust engine/Scene. No WASM loader in the UI.
 * This is an in-process TS boundary, not a new daemon protocol or a serialisable Op format.
 */
export interface EngineBindings extends EngineClient {}
export class WasmEngineClient implements EngineClient {
  private disposed = false;
  private subscriptions = new Set<{ dispose(): void }>();
  get state() {
    return this.disposed ? ("disposed" as const) : ("ready" as const);
  }
  private live() {
    if (this.disposed) throw Error("EngineClient disposed");
  }
  private view: ToolScene;
  constructor(private readonly bindings: EngineBindings) {
    this.view = sceneView(() => {
      this.live();
      return bindings.scene;
    });
  }
  get scene(): ToolScene {
    this.live();
    return this.view;
  }
  snapshot(): EngineSnapshot {
    this.live();
    return clone(this.bindings.snapshot());
  }
  apply(transaction: ToolTransaction): EngineCommit {
    this.live();
    if (this.snapshot().readOnly) return { status: "rejected", reason: "read-only" };
    if (
      transaction.origin?.actor?.kind !== "person" ||
      !transaction.origin.actor.device ||
      transaction.ops.length === 0
    )
      return { status: "rejected", reason: "invalid" };
    return this.bindings.apply(clone(transaction));
  }
  undo(actor: Actor): EngineCommit {
    this.live();
    if (this.snapshot().readOnly) return { status: "rejected", reason: "read-only" };
    return this.bindings.undo({ ...actor });
  }
  redo(actor: Actor): EngineCommit {
    this.live();
    if (this.snapshot().readOnly) return { status: "rejected", reason: "read-only" };
    return this.bindings.redo({ ...actor });
  }
  subscribe(listener: (snapshot: EngineSnapshot) => void) {
    this.live();
    const inner = this.bindings.subscribe((snapshot) => {
      if (!this.disposed) listener(clone(snapshot));
    });
    this.subscriptions.add(inner);
    return {
      dispose: () => {
        if (this.subscriptions.delete(inner)) inner.dispose();
      },
    };
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    for (const subscription of this.subscriptions) subscription.dispose();
    this.subscriptions.clear();
    this.bindings.dispose();
  }
}
const emptyScene: ToolScene = {
  get: () => undefined,
  roots: () => [],
  hitPoint: () => null,
  hitRect: () => [],
  path: () => null,
  snap: () => ({ offset: { x: 0, y: 0 }, guides: [] }),
  toParent: (_address, point) => ({ ...point }),
  toLocal: (_address, point) => ({ ...point }),
};
/** Scripted snapshots/results, never a second document engine. Tests inject Rust-backed bindings separately. */
export class FakeEngineClient implements EngineClient {
  private disposed = false;
  private current: EngineSnapshot;
  private listeners = new Set<(snapshot: EngineSnapshot) => void>();
  private replies: { snapshot: EngineSnapshot; operation: "apply" | "undo" | "redo" }[] = [];
  readonly transactions: ToolTransaction[] = [];
  readonly historyActors: Actor[] = [];
  get state() {
    return this.disposed ? ("disposed" as const) : ("ready" as const);
  }
  constructor(
    snapshot: EngineSnapshot = { document: null, revision: 0, readOnly: false },
    readonly scene: ToolScene = emptyScene,
  ) {
    this.current = clone(snapshot);
  }
  private live() {
    if (this.disposed) throw Error("EngineClient disposed");
  }
  snapshot() {
    this.live();
    return clone(this.current);
  }
  script(snapshot: EngineSnapshot, operation: "apply" | "undo" | "redo" = "apply") {
    this.live();
    this.replies.push({ snapshot: clone(snapshot), operation });
  }
  private commit(operation: "apply" | "undo" | "redo"): EngineCommit {
    this.live();
    if (this.current.readOnly) return { status: "rejected", reason: "read-only" };
    const next = this.replies[0];
    if (!next || next.operation !== operation) return { status: "rejected", reason: "unavailable" };
    if (next.snapshot.revision <= this.current.revision)
      throw Error("FakeEngineClient script revision must increase");
    this.replies.shift();
    this.current = clone(next.snapshot);
    for (const listener of [...this.listeners]) listener(this.snapshot());
    return { status: "applied", revision: this.current.revision };
  }
  apply(transaction: ToolTransaction): EngineCommit {
    this.live();
    if (
      transaction.origin?.actor?.kind !== "person" ||
      !transaction.origin.actor.device ||
      transaction.ops.length === 0
    )
      return { status: "rejected", reason: "invalid" };
    const result = this.commit("apply");
    if (result.status === "applied") this.transactions.push(clone(transaction));
    return result;
  }
  undo(actor: Actor) {
    const result = this.commit("undo");
    if (result.status === "applied") this.historyActors.push({ ...actor });
    return result;
  }
  redo(actor: Actor) {
    const result = this.commit("redo");
    if (result.status === "applied") this.historyActors.push({ ...actor });
    return result;
  }
  subscribe(listener: (snapshot: EngineSnapshot) => void) {
    this.live();
    this.listeners.add(listener);
    return {
      dispose: () => {
        this.listeners.delete(listener);
      },
    };
  }
  dispose() {
    this.disposed = true;
    this.listeners.clear();
    this.replies = [];
  }
}
