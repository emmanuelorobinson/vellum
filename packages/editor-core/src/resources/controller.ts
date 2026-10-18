import type { DocumentShell, EngineClient, EngineCommit, ToolOp } from "@vellum/engine-client";
import type {
  AssetId,
  DeviceId,
  Document,
  FontFace,
  Node,
  VariableId,
  VariableType,
} from "@vellum/types-ts";
import { personTransaction } from "../design/patch";
import { editVariable, parseVariableValue, themeColumns } from "./variables";

export type ResourceDialog =
  | "variables"
  | "themes"
  | "libraries"
  | "fonts"
  | "manageFonts"
  | "convert"
  | "paste";
export interface LibraryInfo {
  alias: string;
  name: string;
  state: "ready" | "missing";
  updated?: string;
  cached?: boolean;
}
export interface PasteConflict {
  id: string;
  name: string;
  existing: string;
  pasted: string;
  sameType: boolean;
}
/** Prepared by the core clipboard planner; payload bytes stay with the host. */
export interface PasteRequest {
  id: string;
  libraryPath?: string;
  conflicts: readonly PasteConflict[];
  omissions: readonly string[];
}
export interface PasteChoices {
  link: boolean;
  variables: Record<string, "existing" | "renamed">;
}
export interface ResourceHost {
  /** Opens a file picker, validates a library with the core, and returns its path relative to this document. */
  chooseLibrary?(alias?: string): Promise<{ alias: string; path: string } | null>;
  openLibrary?(alias: string): void;
  /** Stores the original supported font bytes through AssetStore; returns inspected metadata and content hash. */
  chooseFont?(): Promise<{ asset: AssetId; face: FontFace } | null>;
  save?(): Promise<boolean>;
  libraries?: readonly LibraryInfo[];
  bundled?: readonly { id: string; name: string }[];
  recentFonts?: readonly string[];
  googleFonts?: readonly string[];
  systemFonts?: readonly string[];
  availableFonts?: readonly string[];
  fallbackFont?: string;
  action?(command: string, detail?: Record<string, unknown>): void;
  /** Delegates dependency/ID remapping and link/copy choices to vellum-core's paste planner. Returns ordinary ops. */
  planPaste?(request: PasteRequest, choices: PasteChoices, document: Document): Promise<ToolOp[]>;
}
export function shellOf(doc: Document): DocumentShell {
  return {
    meta: { ...doc.meta },
    imports: { ...doc.imports },
    themes: { ...doc.themes },
    styles: { ...doc.styles },
    fonts: { ...doc.fonts },
  };
}
export function documentNodes(doc: Document): Node[] {
  const out: Node[] = [];
  const walk = (node: Node) => {
    out.push(node);
    for (const child of (node as { children?: Node[] }).children ?? []) walk(child);
  };
  for (const page of doc.pages) for (const node of page.children ?? []) walk(node);
  for (const component of Object.values(doc.components ?? {})) walk(component.root as Node);
  return out;
}
/** Counts editable document text and component definitions; expanded instance/library usage belongs to the host. */
export function fontUsage(doc: Document): { family: string; nodes: string[]; assets: string[] }[] {
  const uses = new Map<string, Set<string>>();
  for (const node of documentNodes(doc))
    if (node.type === "text")
      for (const run of node.content) {
        if (!uses.has(run.font)) uses.set(run.font, new Set());
        uses.get(run.font)?.add(node.id);
      }
  for (const face of Object.values(doc.fonts ?? {}))
    if (!uses.has(face.family)) uses.set(face.family, new Set());
  return [...uses].map(([family, nodes]) => ({
    family,
    nodes: [...nodes],
    assets: Object.entries(doc.fonts ?? {})
      .filter(([, face]) => face.family === family)
      .map(([asset]) => asset),
  }));
}
export function replaceFontOps(doc: Document, from: string, to: string): ToolOp[] {
  if (!to.trim() || from === to) return [];
  return documentNodes(doc).flatMap((node) =>
    node.type === "text" && node.content.some((r) => r.font === from)
      ? [
          {
            type: "update" as const,
            id: node.id,
            patch: {
              content: node.content.map((run) => (run.font === from ? { ...run, font: to } : run)),
            },
          },
        ]
      : [],
  );
}
const relativeLibrary = (path: string) =>
  !!path &&
  !/^(?:[\\/]|[a-z][a-z0-9+.-]*:)/i.test(path) &&
  !path.includes("\\") &&
  !path.includes("\0") &&
  path.endsWith(".lib.vellum");
export function createResourceController(deps: {
  engine: EngineClient;
  device: DeviceId;
  host: ResourceHost;
  nextVariableId(): VariableId;
}) {
  const { engine, device, host } = deps;
  let generation = 0;
  let disposed = false;
  let state: {
    dialog: ResourceDialog | null;
    error: string | null;
    busy: boolean;
    saveSuggested: boolean;
    paste: PasteRequest | null;
  } = { dialog: null, error: null, busy: false, saveSuggested: false, paste: null };
  const located = new Map<string, string>();
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const l of [...listeners]) l();
  };
  const change = (patch: Partial<typeof state>) => {
    state = { ...state, ...patch };
    notify();
  };
  const doc = (): Document => {
    if (disposed) throw Error("Resource controller disposed");
    const s = engine.snapshot();
    if (s.readOnly) throw Error("This file is read-only");
    if (!s.document) throw Error("No document is open");
    return s.document;
  };
  const commit = (ops: ToolOp[]): EngineCommit | null => {
    doc();
    const tx = personTransaction(device, ops);
    if (!tx) return null;
    const result = engine.apply(tx);
    change({ error: result.status === "rejected" ? `Change refused: ${result.reason}` : null });
    return result;
  };
  const shell = (patch: Partial<DocumentShell>) =>
    commit([{ type: "setShell", shell: { ...shellOf(doc()), ...patch } }]);
  const run = async <T>(job: () => Promise<T>, apply: (value: T) => void) => {
    if (state.busy || disposed) return;
    let revision: number;
    try {
      doc();
      revision = engine.snapshot().revision;
    } catch (e) {
      change({ error: String((e as Error).message) });
      return;
    }
    const ticket = ++generation;
    change({ busy: true, error: null });
    try {
      const value = await job();
      if (disposed || ticket !== generation) return;
      if (engine.snapshot().revision !== revision) throw Error("The document changed; try again");
      doc();
      apply(value);
    } catch (e) {
      if (!disposed && ticket === generation)
        change({ error: (e as Error).message || "That action failed" });
    } finally {
      if (!disposed && ticket === generation) change({ busy: false });
    }
  };
  const controller = {
    get state() {
      return state;
    },
    host,
    document: () => engine.snapshot().document,
    libraryInfo(alias: string): LibraryInfo | undefined {
      const info = host.libraries?.find((i) => i.alias === alias);
      return located.get(alias) === engine.snapshot().document?.imports?.[alias] &&
        located.has(alias)
        ? { alias, name: info?.name ?? alias, state: "ready" }
        : info;
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return { dispose: () => listeners.delete(listener) };
    },
    open(dialog: ResourceDialog) {
      if (disposed) return;
      generation++;
      change({ dialog, error: null, busy: false });
    },
    close() {
      generation++;
      change({ dialog: null, error: null, busy: false, paste: null });
    },
    clearError() {
      change({ error: null });
    },
    attempt(action: () => unknown) {
      try {
        action();
      } catch (e) {
        change({ error: (e as Error).message });
      }
    },
    setThemes(themes: Record<string, string[]>) {
      themeColumns({ ...doc(), themes });
      return shell({ themes });
    },
    editVariable(id: string, mode: Record<string, string>, text: string) {
      return commit([editVariable(doc(), id, mode, text)]);
    },
    createVariable(name: string, type: VariableType, text: string) {
      const d = doc();
      name = name.trim();
      if (!name || Object.values(d.variables ?? {}).some((v) => v.name === name))
        throw Error("Use a new variable name");
      const id = deps.nextVariableId();
      if (d.variables?.[id]) throw Error("Variable id already exists");
      const variable = { type, name, values: [{ value: parseVariableValue(type, text) }] };
      return commit([
        {
          type: "setVariables",
          set: { [id]: variable as NonNullable<Document["variables"]>[string] },
          remove: [],
        },
      ]);
    },
    renameVariable(id: string, name: string) {
      const d = doc(),
        v = d.variables?.[id];
      name = name.trim();
      if (
        !v ||
        !name ||
        Object.entries(d.variables ?? {}).some(
          ([other, value]) => other !== id && value.name === name,
        )
      )
        throw Error("Use a new variable name");
      return commit([{ type: "setVariables", set: { [id]: { ...v, name } }, remove: [] }]);
    },
    removeVariable(id: string) {
      return commit([{ type: "setVariables", set: {}, remove: [id] }]);
    },
    importLibrary() {
      return host.chooseLibrary
        ? run(
            async () => host.chooseLibrary?.(),
            (value) => {
              if (!value) return;
              const d = doc();
              if (!value.alias.trim() || Object.hasOwn(d.imports ?? {}, value.alias))
                throw Error("Use a new library alias");
              if (!relativeLibrary(value.path)) throw Error("Choose a relative .lib.vellum path");
              shell({ imports: { ...d.imports, [value.alias]: value.path } });
            },
          )
        : Promise.resolve();
    },
    locate(alias: string) {
      return host.chooseLibrary
        ? run(
            async () => host.chooseLibrary?.(alias),
            (value) => {
              if (!value) return;
              const d = doc();
              if (!Object.hasOwn(d.imports ?? {}, alias))
                throw Error("Library is no longer imported");
              if (!relativeLibrary(value.path)) throw Error("Choose a relative .lib.vellum path");
              const result = shell({ imports: { ...d.imports, [alias]: value.path } });
              if (result?.status === "applied") {
                located.set(alias, value.path);
                change({ saveSuggested: true });
              }
            },
          )
        : Promise.resolve();
    },
    removeLibrary(alias: string) {
      const imports = { ...doc().imports };
      delete imports[alias];
      return shell({ imports });
    },
    requestConversion() {
      doc();
      if (doc().meta.kind !== "library") controller.open("convert");
    },
    confirmConversion() {
      if (state.dialog !== "convert") return;
      const d = doc();
      if (d.meta.kind === "library") return;
      const result = shell({ meta: { ...d.meta, kind: "library" } });
      if (result?.status === "applied") controller.close();
    },
    importFont() {
      return host.chooseFont
        ? run(
            async () => host.chooseFont?.(),
            (value) => {
              if (!value) return;
              if (!/^[a-f0-9]{64}$/.test(value.asset) || !value.face.family.trim())
                throw Error("Font import returned invalid metadata");
              shell({ fonts: { ...doc().fonts, [value.asset]: value.face } });
            },
          )
        : Promise.resolve();
    },
    removeFont(asset: string) {
      const fonts = { ...doc().fonts };
      delete fonts[asset];
      return shell({ fonts });
    },
    replaceFont(from: string, to: string) {
      return commit(replaceFontOps(doc(), from, to));
    },
    requestPaste(request: PasteRequest) {
      doc();
      change({ paste: request, dialog: "paste", error: null });
    },
    paste(choices: PasteChoices) {
      const request = state.paste;
      if (!request || !host.planPaste) return Promise.resolve();
      for (const conflict of request.conflicts)
        if (
          !choices.variables[conflict.id] ||
          (!conflict.sameType && choices.variables[conflict.id] === "existing")
        ) {
          change({ error: "Resolve every variable name before pasting" });
          return Promise.resolve();
        }
      return run(
        async () => host.planPaste?.(request, choices, doc()),
        (ops) => {
          const result = commit(ops ?? []);
          if (result?.status === "applied") controller.close();
        },
      );
    },
    async save() {
      if (!host.save || !state.saveSuggested) return;
      await run(
        async () => host.save?.(),
        (saved) => {
          if (saved) change({ saveSuggested: false });
        },
      );
    },
    dispose() {
      disposed = true;
      generation++;
      listeners.clear();
    },
  };
  return controller;
}
export type ResourceController = ReturnType<typeof createResourceController>;
