import type { EngineClient, EngineCommit } from "@vellum/engine-client";
import type { DeviceId, NodeId, NodeIdPath, PageId, Point } from "@vellum/types-ts";
import { world } from "./geometry";
import type { Disposable } from "./registries";
import { invertPathMapping } from "./resources/path-map";
import type {
  Place,
  SnapOptions,
  ToolContext,
  ToolEvent,
  ToolResult,
  ToolState,
  ToolTransaction,
  Viewport,
} from "./tool-types";
import { initialToolState, transitionTool } from "./tools";

export type SessionNotice =
  | { kind: "rejected"; reason: Extract<EngineCommit, { status: "rejected" }>["reason"] }
  | { kind: "read-only" }
  | { kind: "error"; error: NonNullable<ToolResult["error"]> };

export interface SessionDeps {
  engine: EngineClient;
  device: DeviceId;
  /** The page the person is on: new layers go to the end of its children. */
  page: PageId;
  nextNodeId(): NodeId;
  createNode: ToolContext["createNode"];
  viewportSize(): { width: number; height: number };
  dragThreshold?: number;
  snap?: Omit<SnapOptions, "zoom">;
}

const DEFAULT_SNAP: Omit<SnapOptions, "zoom"> = {
  search: { x: -100_000, y: -100_000, width: 200_000, height: 200_000 },
  tolerance: 6,
  edges: true,
  centres: true,
  equalSpacing: true,
  grid: null,
};

/**
 * One person's editing session over an EngineClient: it feeds events to the tool controller, hands the Person
 * transactions it produces to the engine, and puts the controller back to a safe state when the engine says no.
 * It holds no document: the engine owns that. Undo and redo are always the person's own.
 */
export function createEditorSession(deps: SessionDeps) {
  const { engine, device } = deps;
  let page = deps.page;
  let state: ToolState = initialToolState();
  let notice: SessionNotice | null = null;
  let hovered: NodeIdPath | null = null;
  let snapOptions: Omit<SnapOptions, "zoom"> = deps.snap ?? DEFAULT_SNAP;
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const listener of [...listeners]) listener();
  };
  const context = (): ToolContext => {
    const snapshot = engine.snapshot();
    const current = snapshot.document?.pages.find((p) => p.id === page);
    const place: Place = {
      parent: { type: "page", id: page },
      index: current?.children?.length ?? 0,
    };
    return {
      device,
      readOnly: snapshot.readOnly,
      nextNodeId: deps.nextNodeId,
      scene: engine.scene,
      placement: place,
      createNode: deps.createNode,
      viewportSize: deps.viewportSize(),
      dragThreshold: deps.dragThreshold ?? 3,
      snap: snapOptions,
    };
  };
  const prune = () => {
    const kept = state.selection.filter((id) => engine.scene.get(id));
    if (kept.length !== state.selection.length) {
      state = { ...state, selection: kept };
      return true;
    }
    return false;
  };
  const subscription = engine.subscribe(() => {
    prune();
    notify();
  });
  let lastOverlay: ToolResult["overlay"] = transitionTool(
    state,
    { type: "cancel" },
    context(),
  ).overlay;
  return {
    get state(): Readonly<ToolState> {
      return state;
    },
    get overlay() {
      return lastOverlay;
    },
    get notice() {
      return notice;
    },
    get hovered() {
      return hovered;
    },
    get page(): PageId {
      return page;
    },
    get device(): DeviceId {
      return device;
    },
    /** Another page: the selection belongs to the old one, so it is cleared. */
    setPage(id: PageId) {
      if (id === page) return;
      page = id;
      state = { ...state, selection: [] };
      notify();
    },
    /**
     * For changes made outside the tools (properties, layers, assets): the engine applies them as this person,
     * and a refusal is reported the same way a tool's is.
     */
    commit(transaction: ToolTransaction): EngineCommit {
      const commit = engine.apply(transaction);
      if (commit.status === "rejected") notice = { kind: "rejected", reason: commit.reason };
      else notice = null;
      notify();
      return commit;
    },
    /** For commits made outside the tools (layers, assets) that the engine refused. */
    reportRejection(reason: Extract<EngineCommit, { status: "rejected" }>["reason"]) {
      notice = { kind: "rejected", reason };
      notify();
    },
    clearNotice() {
      if (notice) {
        notice = null;
        notify();
      }
    },
    subscribe(listener: () => void): Disposable {
      listeners.add(listener);
      return {
        dispose() {
          listeners.delete(listener);
        },
      };
    },
    dispatch(event: ToolEvent): ToolResult {
      const ctx = context();
      const before = state;
      const result = transitionTool(before, event, ctx);
      let next = result.state;
      if (result.error === "read-only") notice = { kind: "read-only" };
      else if (result.error) notice = { kind: "error", error: result.error };
      for (const transaction of result.transactions) {
        const commit = engine.apply(transaction);
        if (commit.status === "rejected") {
          notice = { kind: "rejected", reason: commit.reason };
          next = transitionTool(before, { type: "cancel" }, ctx).state;
          break;
        }
        notice = null;
      }
      state = next;
      prune();
      lastOverlay = transitionTool(state, { type: "cancel" }, ctx).overlay;
      notify();
      return result;
    },
    /** Layers and Assets use this: selecting is local state, never a document change. */
    select(ids: readonly NodeIdPath[]) {
      state = { ...state, selection: ids.filter((id) => engine.scene.get(id)) };
      lastOverlay = transitionTool(state, { type: "cancel" }, context()).overlay;
      notify();
    },
    /** Settings → Canvas: snap to objects and to the pixel grid. Takes effect on the next drag. */
    setSnap(options: { objects: boolean; grid: number | null }) {
      snapOptions = {
        ...snapOptions,
        edges: options.objects,
        centres: options.objects,
        equalSpacing: options.objects,
        grid: options.grid,
      };
    },
    setViewport(viewport: Viewport) {
      if (!Number.isFinite(viewport.zoom) || viewport.zoom <= 0)
        throw Error("Viewport zoom must be a positive number");
      state = { ...state, viewport: { ...viewport } };
      notify();
    },
    /** `point` is in screen pixels. */
    hover(point: Point) {
      const hit = engine.scene.hitPoint(world(point, state.viewport), "deepest");
      if (hit !== hovered) {
        hovered = hit;
        notify();
      }
    },
    /** Read-only scene lookups for view commands and panels. */
    roots(): NodeIdPath[] {
      return engine.scene.roots();
    },
    sceneItem(id: NodeIdPath) {
      return engine.scene.get(id) ?? null;
    },
    pathHit(point: Point) {
      const mode = state.mode;
      return mode.type === "path"
        ? (engine.scene.hitPath?.(
            mode.address,
            world(point, state.viewport),
            6 / state.viewport.zoom,
          ) ?? null)
        : null;
    },
    pathMapping(id: NodeIdPath) {
      return invertPathMapping((point) => engine.scene.toLocal(id, point));
    },
    sceneBounds(id: NodeIdPath) {
      return engine.scene.get(id)?.bounds ?? null;
    },
    undo() {
      return engine.undo({ kind: "person", device });
    },
    redo() {
      return engine.redo({ kind: "person", device });
    },
    dispose() {
      subscription.dispose();
      listeners.clear();
    },
  };
}
export type EditorSession = ReturnType<typeof createEditorSession>;
