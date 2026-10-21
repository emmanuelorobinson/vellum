import type { EngineClient, EngineCommit, ToolOp, ToolTransaction } from "@vellum/engine-client";
import type { DeviceId, Node, NodeId, Page, PageId } from "@vellum/types-ts";

/** Only top-level frames are slides. Notes, prompts and context remain editing annotations. */
export const slideFrames = (page: Page | null | undefined) =>
  page?.kind === "slides" ? (page.children ?? []).filter((n) => n.type === "frame") : [];

/** Reorder uses the core's final child index, not a slide ordinal. Annotations retain their relative order. */
export function reorderSlide(
  page: Page,
  id: NodeId,
  target: NodeId,
  side: "before" | "after",
): ToolOp | null {
  const slides = slideFrames(page);
  if (id === target || !slides.some((s) => s.id === id) || !slides.some((s) => s.id === target))
    return null;
  const children = page.children ?? [];
  const from = children.findIndex((n) => n.id === id);
  const remaining = children.filter((n) => n.id !== id);
  const index = remaining.findIndex((n) => n.id === target) + (side === "after" ? 1 : 0);
  // Adjacent slides separated only by annotations already have the requested slide order.
  const ordered = slides.filter((n) => n.id !== id);
  ordered.splice(
    ordered.findIndex((n) => n.id === target) + (side === "after" ? 1 : 0),
    0,
    slides.find((n) => n.id === id)!,
  );
  if (ordered.every((n, i) => n.id === slides[i]?.id) || index === from) return null;
  return { type: "reorder", id, index };
}
export interface SlidesHost {
  /** Isolated frame PNG from the shared export renderer at this exact live document revision. */
  image?(id: NodeId, revision: number): Promise<string>;
  /** One PDF, in the supplied order, from the live revision; host owns destination UI and saving. */
  pdf?(ids: readonly NodeId[], revision: number): Promise<void>;
}
export function createSlidesController(deps: {
  engine: EngineClient;
  device: DeviceId;
  page(): PageId;
  nextPageId(): PageId;
  onPage(id: PageId): void;
  host?: SlidesHost;
  commit?(tx: ToolTransaction): EngineCommit;
}) {
  const { engine } = deps;
  let state = {
    presenting: false,
    page: null as PageId | null,
    current: null as NodeId | null,
    error: null as string | null,
    busy: false,
  };
  const listeners = new Set<() => void>();
  const change = (patch: Partial<typeof state>) => {
    state = { ...state, ...patch };
    for (const l of [...listeners]) l();
  };
  const page = () => engine.snapshot().document?.pages.find((p) => p.id === deps.page()) ?? null;
  const frames = () => slideFrames(page());
  const commit = (op: ToolOp) => {
    if (engine.snapshot().readOnly) {
      change({ error: "This file is read-only" });
      return false;
    }
    const result = (deps.commit ?? engine.apply.bind(engine))({
      origin: { actor: { kind: "person", device: deps.device } },
      ops: [op],
    });
    change({ error: result.status === "rejected" ? `Change refused: ${result.reason}` : null });
    return result.status === "applied";
  };
  const subscription = engine.subscribe(() => {
    if (state.presenting) {
      const all = frames();
      if (state.page !== deps.page() || !all.length)
        change({ presenting: false, current: null, page: null });
      else if (!all.some((n) => n.id === state.current)) change({ current: all[0]!.id });
    }
    change({});
  });
  const c = {
    get state() {
      return state;
    },
    page,
    frames,
    pages: () => engine.snapshot().document?.pages ?? [],
    changePage(id: PageId) {
      c.stop();
      deps.onPage(id);
      change({});
    },
    subscribe(l: () => void) {
      listeners.add(l);
      return { dispose: () => void listeners.delete(l) };
    },
    canPresent: () => !!deps.host?.image && frames().length > 0,
    canExport: () => !!deps.host?.pdf && frames().length > 0 && !state.busy,
    newPage() {
      if (state.presenting) return;
      const doc = engine.snapshot().document;
      if (!doc) return;
      const id = deps.nextPageId();
      if (
        commit({
          type: "insertPage",
          index: doc.pages.length,
          page: { id, name: "Slides", kind: "slides", children: [] },
        })
      )
        deps.onPage(id);
    },
    reorder(id: NodeId, target: NodeId, side: "before" | "after") {
      if (state.presenting) return;
      const p = page();
      const op = p ? reorderSlide(p, id, target, side) : null;
      if (op) commit(op);
    },
    start(id?: NodeId) {
      if (!c.canPresent()) {
        change({ error: "Presentation rendering is unavailable" });
        return false;
      }
      const all = frames();
      const selectedRoot = id?.split("/")[0];
      const contains = (node: Node): boolean =>
        node.id === selectedRoot || ("children" in node && (node.children ?? []).some(contains));
      change({
        presenting: true,
        page: deps.page(),
        current: all.find(contains)?.id ?? all[0]!.id,
        error: null,
      });
      return true;
    },
    current(): Node | null {
      return state.presenting ? (frames().find((n) => n.id === state.current) ?? null) : null;
    },
    next() {
      c.move(1);
    },
    previous() {
      c.move(-1);
    },
    move(delta: number) {
      if (!state.presenting) return;
      const all = frames();
      const next =
        all[
          Math.max(
            0,
            Math.min(all.length - 1, all.findIndex((n) => n.id === state.current) + delta),
          )
        ];
      if (next) change({ current: next.id });
    },
    stop() {
      change({ presenting: false, page: null, current: null });
    },
    async image(id: NodeId) {
      if (!deps.host?.image) throw Error("Presentation rendering is unavailable");
      const revision = engine.snapshot().revision;
      const result = await deps.host.image(id, revision);
      if (engine.snapshot().revision !== revision)
        throw Error("The document changed while rendering this slide");
      return result;
    },
    async exportPdf(selected?: readonly NodeId[]) {
      if (state.busy) return;
      const all = frames();
      const ids = all
        .filter((n) => selected === undefined || selected.includes(n.id))
        .map((n) => n.id);
      if (!deps.host?.pdf) return change({ error: "PDF export is unavailable" });
      if (!ids.length || (selected && selected.some((id) => !all.some((n) => n.id === id))))
        return change({ error: "Select slides from this page" });
      change({ busy: true, error: null });
      try {
        await deps.host.pdf(ids, engine.snapshot().revision);
        change({ busy: false });
      } catch (e) {
        change({ busy: false, error: e instanceof Error ? e.message : "PDF export failed" });
      }
    },
    dispose() {
      subscription.dispose();
      listeners.clear();
    },
  };
  return c;
}
export type SlidesController = ReturnType<typeof createSlidesController>;

export interface SlideExportFile {
  name: string;
  mime: string;
  base64: string;
}
/** Bridge the existing shared `export` tool to the presenter and native destination UI. No second renderer. */
export function slidesHostFromExport(deps: {
  revision(): number;
  render(request: {
    nodes: readonly NodeId[];
    format: "png" | "pdf";
    scale: 1;
  }): Promise<{ files: readonly SlideExportFile[] }>;
  savePdf(file: SlideExportFile): Promise<void>;
}): SlidesHost {
  const render = async (nodes: readonly NodeId[], format: "png" | "pdf", revision: number) => {
    const check = () => {
      if (deps.revision() !== revision) throw Error("The document changed while exporting slides");
    };
    check();
    const result = await deps.render({ nodes, format, scale: 1 });
    check();
    const file = result.files[0];
    if (
      result.files.length !== 1 ||
      !file ||
      file.mime !== (format === "png" ? "image/png" : "application/pdf") ||
      file.base64.length > 45 * 1024 * 1024 ||
      !/^[A-Za-z0-9+/]+={0,2}$/.test(file.base64)
    )
      throw Error("The exporter returned an invalid slide bundle");
    return file;
  };
  return {
    async image(id, revision) {
      const f = await render([id], "png", revision);
      return `data:image/png;base64,${f.base64}`;
    },
    async pdf(ids, revision) {
      await deps.savePdf(await render(ids, "pdf", revision));
    },
  };
}
