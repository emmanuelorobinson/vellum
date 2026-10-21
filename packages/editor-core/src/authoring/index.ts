import type { EngineClient, SceneItem, ToolOp } from "@vellum/engine-client";
import type { DeviceId, Document, Node, Paint, Scalar, Value } from "@vellum/types-ts";
import { normalizeHex } from "../design/colour";
import { patchOps, personTransaction } from "../design/patch";

/** Parsed by vellum-core's Header parser. UI never evaluates or reparses JavaScript. */
export interface ScriptInput {
  name: string;
  label?: string;
  kind: "number" | "string" | "enum" | "color" | "boolean" | "ref";
  min?: number;
  max?: number;
  step?: number;
  options?: readonly string[];
  default?: Scalar;
  multiline?: boolean;
  refKind?: "node" | "page" | "component" | "variable";
}
export type ScriptView =
  | { status: "empty" | "loading" | "unavailable"; inputs: readonly ScriptInput[] }
  | { status: "ready"; inputs: readonly ScriptInput[] }
  | {
      status: "error";
      inputs: readonly ScriptInput[];
      message: string;
      line?: number;
      excerpt?: string;
    };
/** Derived output is rendered by the existing ContentSource; only its diagnostics and header reach this port. */
export interface ScriptSnapshot {
  revision: number;
  entries: Readonly<Record<string, ScriptView>>;
}
export interface Guidelines {
  name: string;
  path: string;
  text: string;
  version: string;
  writable: boolean;
}
export interface Attribution {
  provider: string;
  author: string;
  authorUrl: string;
  sourceUrl: string;
  licence: string;
  licenceUrl: string;
}
export interface StockPhoto {
  id: string;
  description: string;
  previewUrl: string;
  attribution: Attribution;
}
export interface ImportedPicture {
  asset: string;
  attribution?: Attribution;
}
export interface AuthoringHost {
  scriptSnapshot?(): ScriptSnapshot;
  subscribeScripts?(listener: () => void): { dispose(): void };
  /** Authorized rooted file access, and optimistic version comparison, belong to the host. */
  loadGuidelines?(): Promise<Guidelines>;
  saveGuidelines?(
    request: Pick<Guidelines, "path" | "text" | "version">,
  ): Promise<{ version: string }>;
  openScript?(src: string): void;
  chooseScript?(): Promise<string | null>;
  askScript?(address: string, intent: "write" | "fix"): void;
  /** ImageSource and AssetStore import original bytes before returning a validated content hash. */
  chooseImage?(): Promise<ImportedPicture | null>;
  uploadImage?(file: unknown): Promise<ImportedPicture | null>;
  searchStock?(
    query: string,
    page: number,
  ): Promise<{ photos: readonly StockPhoto[]; total: number }>;
  importStock?(photo: StockPhoto): Promise<ImportedPicture>;
  imageModels?: readonly { provider: string; id: string; name: string }[];
  generateImage?(request: {
    prompt: string;
    provider: string;
    model: string;
  }): Promise<ImportedPicture>;
  /** Host computes pixels and applies one person-origin transaction after checking expectedRevision. */
  processImage?(request: {
    address: string;
    action: "remove_background" | "trace";
    expectedRevision: number;
  }): Promise<void>;
}
export function scriptView(
  snapshot: ScriptSnapshot | undefined,
  revision: number,
  address: string,
  node: Node,
): ScriptView {
  if (node.type !== "script" || !node.src) return { status: "empty", inputs: [] };
  return snapshot?.revision === revision
    ? (snapshot.entries[address] ?? { status: "unavailable", inputs: [] })
    : { status: "unavailable", inputs: [] };
}
export function parseScriptInput(input: ScriptInput, raw: string | boolean): Scalar {
  if (input.kind === "boolean") {
    if (typeof raw !== "boolean") throw Error("Choose on or off");
    return raw;
  }
  const text = String(raw);
  if (input.kind === "number") {
    const n = Number(text);
    if (
      !text.trim() ||
      !Number.isFinite(n) ||
      (input.min !== undefined && n < input.min) ||
      (input.max !== undefined && n > input.max)
    )
      throw Error("Enter a number within the input's limits");
    return n;
  }
  if (input.kind === "enum" && !input.options?.includes(text))
    throw Error("Choose a declared option");
  if (input.kind === "color") {
    const hex = normalizeHex(text);
    if (!hex) throw Error("Enter a hex colour");
    return hex;
  }
  return text;
}
export interface SlotTarget {
  id: string;
  name: string;
  component: string;
  componentName: string;
  instance: string | null;
  library: string | null;
  suggested: { id: string; name: string; available: boolean }[];
}
const children = (node: Node): Node[] => (node as { children?: Node[] }).children ?? [];
function slots(node: Node): Extract<Node, { type: "frame" }>[] {
  return [...(node.type === "frame" && node.slot ? [node] : []), ...children(node).flatMap(slots)];
}
function contains(node: Node, id: string): boolean {
  return node.id === id || children(node).some((n) => contains(n, id));
}
export function canMakeSlot(item: SceneItem, doc: Document): boolean {
  return (
    !item.instance &&
    item.node.type === "frame" &&
    !item.node.slot &&
    !children(item.node).length &&
    Object.values(doc.components ?? {}).some(
      (c) => c.root.id !== item.node.id && contains(c.root as Node, item.node.id),
    )
  );
}
export function slotTargets(item: SceneItem, doc: Document): SlotTarget[] {
  const node = item.node;
  const owner =
    node.type === "instance" ? node : item.instance ? findNode(doc, item.instance.id) : node;
  const instance = owner?.type === "instance" ? owner : null;
  const component = instance
    ? instance.componentId
    : Object.entries(doc.components ?? {}).find(([, c]) => contains(c.root as Node, node.id))?.[0];
  if (!component || instance?.library) return [];
  const definition = doc.components?.[component];
  if (!definition) return [];
  const candidates = node.type === "instance" ? slots(definition.root as Node) : slots(node);
  return candidates.map((slot) => ({
    id: slot.id,
    name: slot.name,
    component,
    componentName: definition.name,
    instance: instance?.id ?? null,
    library: instance?.library ?? null,
    suggested: (slot.slot?.suggested ?? []).map((id) => ({
      id,
      name: doc.components?.[id]?.name ?? id,
      available: !!doc.components?.[id],
    })),
  }));
}
export function findNode(doc: Document, id: string): Node | undefined {
  const walk = (node: Node): Node | undefined =>
    node.id === id ? node : children(node).map(walk).find(Boolean);
  for (const page of doc.pages)
    for (const node of page.children ?? []) {
      const found = walk(node);
      if (found) return found;
    }
  for (const c of Object.values(doc.components ?? {})) {
    const found = walk(c.root as Node);
    if (found) return found;
  }
  return undefined;
}
export function insertSlotOp(
  slot: SlotTarget,
  componentId: string,
  id: string,
  doc: Document,
  item: SceneItem,
): ToolOp {
  if (!slot.instance) throw Error("Select an instance to fill this slot");
  const owner = item.node.id === slot.instance ? item.node : findNode(doc, slot.instance);
  const definition = doc.components?.[componentId];
  if (
    owner?.type !== "instance" ||
    !definition ||
    owner.componentId !== slot.component ||
    !doc.components?.[slot.component] ||
    !slots(doc.components[slot.component]!.root as Node).some((n) => n.id === slot.id)
  )
    throw Error("The slot or component is unavailable");
  const patch = {
    slotContent: {
      ...owner.slotContent,
      [slot.id]: [
        ...(owner.slotContent?.[slot.id] ?? []),
        {
          type: "instance",
          id,
          name: definition.name,
          componentId,
          width: definition.root.width,
          height: definition.root.height,
        },
      ],
    },
  };
  return item.node.id === owner.id
    ? (patchOps([item], () => patch)[0] as ToolOp)
    : { type: "update", id: owner.id, patch };
}
const relativeScript = (src: string) =>
  !!src &&
  !src.includes("\\") &&
  !src.includes("\0") &&
  !/^(?:\/|[a-z][a-z\d+.-]*:)/i.test(src) &&
  src.endsWith(".js");
export function createAuthoringController(deps: {
  engine: EngineClient;
  device: DeviceId;
  host: AuthoringHost;
  nextNodeId(): string;
}) {
  const { engine, host } = deps;
  let disposed = false,
    ticket = 0,
    searchTicket = 0;
  let state: {
    dialog: "guidelines" | "image" | null;
    guidelines: Guidelines | null;
    savedText: string;
    busy: boolean;
    error: string | null;
    image: { address: string; scope: "fill" | "stroke"; index: number } | null;
    photos: readonly StockPhoto[];
    total: number;
    query: string;
    page: number;
    searching: boolean;
    selected: string | null;
  } = {
    dialog: null,
    guidelines: null,
    savedText: "",
    busy: false,
    error: null,
    image: null,
    photos: [],
    total: 0,
    query: "",
    page: 1,
    searching: false,
    selected: null,
  };
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const l of [...listeners]) l();
  };
  const change = (patch: Partial<typeof state>) => {
    state = { ...state, ...patch };
    notify();
  };
  const writable = () => {
    if (disposed) throw Error("Authoring controller disposed");
    const s = engine.snapshot();
    if (s.readOnly) throw Error("This file is read-only");
    if (!s.document) throw Error("No document is open");
    return s.document;
  };
  const commit = (ops: ToolOp[]) => {
    writable();
    const tx = personTransaction(deps.device, ops);
    if (!tx) return null;
    const r = engine.apply(tx);
    if (r.status === "rejected") throw Error(`Change refused: ${r.reason}`);
    return r;
  };
  const attempt = (fn: () => unknown) => {
    try {
      return fn();
    } catch (e) {
      if (!disposed) change({ error: (e as Error).message });
      return null;
    }
  };
  const run = async (job: () => Promise<unknown>) => {
    if (disposed || state.busy) return;
    const t = ++ticket;
    change({ busy: true, error: null });
    try {
      await job();
    } catch (e) {
      if (!disposed && ticket === t) change({ error: (e as Error).message });
    } finally {
      if (!disposed && ticket === t) change({ busy: false });
    }
  };
  const eSub = engine.subscribe(notify),
    sSub = host.subscribeScripts?.(notify);
  const c = {
    host,
    get state() {
      return state;
    },
    subscribe(l: () => void) {
      listeners.add(l);
      return { dispose: () => void listeners.delete(l) };
    },
    view(address: string, node: Node) {
      return scriptView(host.scriptSnapshot?.(), engine.snapshot().revision, address, node);
    },
    attempt,
    close() {
      if (state.busy && state.dialog === "guidelines" && state.guidelines) return;
      ticket++;
      searchTicket++;
      change({
        dialog: null,
        guidelines: null,
        image: null,
        busy: false,
        error: null,
        searching: false,
      });
    },
    async openGuidelines() {
      if (!host.loadGuidelines) return;
      ticket++;
      change({ dialog: "guidelines", guidelines: null, error: null, busy: false });
      const t = ticket + 1;
      await run(async () => {
        const g = await host.loadGuidelines!();
        if (!disposed && ticket === t && state.dialog === "guidelines")
          change({ guidelines: { ...g }, savedText: g.text });
      });
    },
    editGuidelines(text: string) {
      if (!state.busy && state.guidelines) change({ guidelines: { ...state.guidelines, text } });
    },
    async saveGuidelines() {
      if (
        !host.saveGuidelines ||
        !state.guidelines?.writable ||
        state.guidelines.text === state.savedText
      )
        return;
      await run(async () => {
        writable();
        const g = state.guidelines!;
        const result = await host.saveGuidelines!({
          path: g.path,
          text: g.text,
          version: g.version,
        });
        if (!disposed) change({ guidelines: { ...g, version: result.version }, savedText: g.text });
      });
    },
    convertScript(address: string) {
      return attempt(() => {
        const item = engine.scene.get(address);
        if (
          !item ||
          item.instance ||
          item.node.type !== "script" ||
          c.view(address, item.node).status !== "ready"
        )
          throw Error("Current script output is unavailable");
        return commit([{ type: "convertScript", id: item.node.id }]);
      });
    },
    setInput(address: string, input: ScriptInput, value: Value<Scalar>) {
      return attempt(() => {
        const item = engine.scene.get(address);
        if (!item || item.node.type !== "script") throw Error("Script unavailable");
        const current = c.view(address, item.node).inputs.find((i) => i.name === input.name);
        if (!current || JSON.stringify(current) !== JSON.stringify(input))
          throw Error("Script header changed; try again");
        const normalized =
          typeof value === "object"
            ? value
            : parseScriptInput(current, typeof value === "boolean" ? value : String(value));
        const inputs = item.node.inputs;
        return commit(
          patchOps([item], () => ({ inputs: { ...inputs, [input.name]: normalized } })),
        );
      });
    },
    chooseScript(address: string) {
      if (!host.chooseScript) return Promise.resolve();
      return run(async () => {
        writable();
        const revision = engine.snapshot().revision,
          t = ticket;
        const src = await host.chooseScript!();
        if (!src || disposed || ticket !== t) return;
        if (revision !== engine.snapshot().revision) throw Error("The document changed; try again");
        if (!relativeScript(src)) throw Error("Choose a relative .js file");
        const item = engine.scene.get(address);
        if (!item || item.node.type !== "script") throw Error("Script unavailable");
        commit(patchOps([item], () => ({ src, inputs: {} })));
      });
    },
    insertSlot(address: string, slotId: string, componentId: string) {
      return attempt(() => {
        const doc = writable(),
          item = engine.scene.get(address);
        if (!item) throw Error("Slot unavailable");
        const slot = slotTargets(item, doc).find((s) => s.id === slotId);
        if (!slot) throw Error("Slot unavailable");
        return commit([insertSlotOp(slot, componentId, deps.nextNodeId(), doc, item)]);
      });
    },
    setSuggestions(address: string, slotId: string, ids: string[]) {
      return attempt(() => {
        const doc = writable(),
          item = engine.scene.get(address);
        if (!item) throw Error("Slot unavailable");
        const slot = slotTargets(item, doc).find((s) => s.id === slotId);
        if (!slot || slot.instance) throw Error("Edit suggestions in the component");
        if (ids.some((id) => !doc.components?.[id])) throw Error("Component unavailable");
        return commit([{ type: "makeSlot", id: slotId, suggested: [...new Set(ids)] }]);
      });
    },
    makeSlot(address: string) {
      return attempt(() => {
        const doc = writable(),
          item = engine.scene.get(address);
        if (
          !item ||
          item.instance ||
          item.node.type !== "frame" ||
          children(item.node).length ||
          !Object.values(doc.components ?? {}).some(
            (c) => contains(c.root as Node, item.node.id) && c.root.id !== item.node.id,
          )
        )
          throw Error("Select an empty frame inside a component");
        return commit([{ type: "makeSlot", id: item.node.id, suggested: [] }]);
      });
    },
    imagePaint(): Paint | null {
      const target = state.image;
      if (!target) return null;
      const node = engine.scene.get(target.address)?.node as
        | { fills?: Paint[]; stroke?: { paints: Paint[] } }
        | undefined;
      return (target.scope === "fill" ? node?.fills : node?.stroke?.paints)?.[target.index] ?? null;
    },
    editImage(edit: (paint: Paint) => Paint) {
      return attempt(() => {
        const target = state.image;
        if (!target) throw Error("Image picker is closed");
        const item = engine.scene.get(target.address);
        if (!item) throw Error("Layer unavailable");
        return commit(
          patchOps([item], () => {
            const node = item.node as { fills?: Paint[]; stroke?: { paints: Paint[] } };
            const before = target.scope === "fill" ? node.fills : node.stroke?.paints;
            if (!before?.[target.index]) throw Error("Fill unavailable");
            const paints = before.map((p, i) => (i === target.index ? edit(p) : p));
            return target.scope === "fill"
              ? { fills: paints }
              : { stroke: { ...node.stroke, paints } };
          }),
        );
      });
    },
    openImage(address: string, scope: "fill" | "stroke", index: number) {
      ticket++;
      searchTicket++;
      change({
        dialog: "image",
        image: { address, scope, index },
        error: null,
        busy: false,
        photos: [],
        selected: null,
        query: "",
        total: 0,
        page: 1,
        searching: false,
      });
    },
    selectPhoto(id: string) {
      if (state.photos.some((p) => p.id === id)) change({ selected: id });
    },
    async search(query: string, page = 1) {
      if (!host.searchStock) return;
      const t = ++searchTicket;
      change({ query, page, searching: true, error: null, selected: null, photos: [], total: 0 });
      try {
        const r = await host.searchStock(query, page);
        if (!disposed && t === searchTicket) change({ photos: r.photos, total: r.total });
      } catch (e) {
        if (!disposed && t === searchTicket) change({ error: (e as Error).message });
      } finally {
        if (!disposed && t === searchTicket) change({ searching: false });
      }
    },
    async processImage(address: string, action: "remove_background" | "trace") {
      if (!host.processImage) return;
      await run(async () => {
        writable();
        await host.processImage!({ address, action, expectedRevision: engine.snapshot().revision });
      });
    },
    generateImage(prompt: string, provider: string, model: string) {
      if (!prompt.trim() || prompt.length > 32000 || !provider || !model) {
        change({ error: "Describe the image and choose a configured model" });
        return Promise.resolve();
      }
      return c.importImage("generate", { prompt, provider, model });
    },
    async importImage(source: "choose" | "upload" | "stock" | "generate", file?: unknown) {
      const target = state.image;
      if (!target) return;
      const photo = state.photos.find((p) => p.id === state.selected);
      if (
        (source === "choose" && !host.chooseImage) ||
        (source === "upload" && !host.uploadImage) ||
        (source === "stock" && (!host.importStock || !photo)) ||
        (source === "generate" && !host.generateImage)
      )
        return;
      await run(async () => {
        writable();
        const rev = engine.snapshot().revision,
          t = ticket;
        const request = file as { prompt: string; provider: string; model: string };
        const result =
          source === "generate"
            ? await host.generateImage!(request)
            : source === "choose"
              ? await host.chooseImage!()
              : source === "upload"
                ? await host.uploadImage!(file)
                : await host.importStock!(photo!);
        if (!result || disposed || ticket !== t) return;
        if (engine.snapshot().revision !== rev) throw Error("The document changed; try again");
        if (!/^[a-f\d]{64}$/.test(result.asset))
          throw Error("Image import returned an invalid asset");
        const item = engine.scene.get(target.address);
        if (!item) throw Error("Layer unavailable");
        commit(
          patchOps([item], () => {
            const n = item.node as Node & { fills?: Paint[]; stroke?: { paints: Paint[] } };
            const before = target.scope === "fill" ? n.fills : n.stroke?.paints;
            if (!before || !before[target.index]) throw Error("Fill unavailable");
            const old = before[target.index]!;
            const paints = before.map((p, i) =>
              i === target.index
                ? {
                    type: "image" as const,
                    asset: result.asset,
                    ...(old.type === "image"
                      ? { fit: old.fit, opacity: old.opacity, visible: old.visible }
                      : {}),
                  }
                : p,
            );
            const metadata: Record<string, import("@vellum/types-ts").JsonValue> = {
              ...n.metadata,
            };
            delete metadata.attribution;
            delete metadata.generated;
            if (source === "generate")
              metadata.generated = { provider: request.provider, model: request.model };
            if (result.attribution) metadata.attribution = { ...result.attribution };
            return {
              ...(target.scope === "fill"
                ? { fills: paints }
                : { stroke: { ...n.stroke, paints } }),
              metadata,
            };
          }),
        );
        change({ dialog: null, image: null });
      });
    },
    dispose() {
      disposed = true;
      ticket++;
      searchTicket++;
      eSub.dispose();
      sSub?.dispose();
      listeners.clear();
    },
  };
  return c;
}
export type AuthoringController = ReturnType<typeof createAuthoringController>;
