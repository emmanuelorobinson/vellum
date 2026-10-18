import type { EngineClient, ToolOp } from "@vellum/engine-client";
import type { DeviceId, NodeId } from "@vellum/types-ts";
import { personTransaction } from "../design/patch";

const lastSegments = (path: string, n = 2) => path.split("/").filter(Boolean).slice(-n).join("/");
const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

// ---------------------------------------------------------------- Inspect (C19)

export type CodeStack = "css" | "tailwind" | "shadcn" | "react-native";
export const CODE_STACKS: readonly { id: CodeStack; label: string }[] = [
  { id: "css", label: "CSS" },
  { id: "tailwind", label: "Tailwind" },
  { id: "shadcn", label: "shadcn" },
  { id: "react-native", label: "React Native" },
];
export interface InspectToken {
  name: string;
  /** "Light #234D3C · Dark #A8C8AD", or the resolved value for a single-valued token. */
  value: string;
}
/** vellum_core's get_spec output, shaped for the panel. */
export interface InspectSpec {
  summary: readonly string[];
  tokens: readonly InspectToken[];
  components: readonly string[];
  code: Readonly<Record<CodeStack, string>>;
}
export interface HtmlSettings {
  flavour: "tailwind" | "css";
  scaffold: boolean;
  assets: "files" | "inline";
}
export interface ImplementEngine {
  id: string;
  name: string;
  /** "Installed · signed in". */
  status: string;
  ready: boolean;
}
export interface InspectHost {
  spec(node: NodeId): Promise<InspectSpec | null>;
  /** "Implement design/juniper.vellum · Onboarding / Step 2 using get_spec." */
  prompt(node: NodeId): Promise<string>;
  engines(): Promise<readonly ImplementEngine[]>;
  /** Starts a session turn on an installed engine; every write still goes through the PermissionBroker. */
  implement(engine: string, node: NodeId): Promise<void>;
  html(node: NodeId, settings: HtmlSettings): Promise<string>;
  exportHtml(node: NodeId, settings: HtmlSettings): Promise<string>;
  copy(text: string): void;
}
export interface InspectState {
  node: NodeId | null;
  spec: InspectSpec | null;
  loading: boolean;
  stack: CodeStack;
  engines: readonly ImplementEngine[];
  settings: HtmlSettings;
  notice: { title: string; body: string } | null;
  error: string | null;
}
export function createInspectController(deps: { engine: EngineClient; host: InspectHost }) {
  const { host } = deps;
  let ticket = 0;
  let state: InspectState = {
    node: null,
    spec: null,
    loading: false,
    stack: "css",
    engines: [],
    settings: { flavour: "tailwind", scaffold: true, assets: "files" },
    notice: null,
    error: null,
  };
  const listeners = new Set<() => void>();
  const change = (patch: Partial<InspectState>) => {
    state = { ...state, ...patch };
    for (const l of [...listeners]) l();
  };
  const guard = async (job: () => Promise<void>) => {
    try {
      await job();
    } catch (e) {
      change({ error: (e as Error).message });
    }
  };
  const c = {
    get state() {
      return state;
    },
    subscribe(l: () => void) {
      listeners.add(l);
      return { dispose: () => void listeners.delete(l) };
    },
    /** A spec that arrives after the selection moved is dropped. */
    async load(node: NodeId | null) {
      const mine = ++ticket;
      // The same layer keeps its spec on screen while a newer one loads, so an edit does not flash the panel.
      const keep = node !== null && node === state.node ? state.spec : null;
      change({
        node,
        spec: keep,
        loading: !!node && !keep,
        error: null,
        ...(node === state.node ? {} : { notice: null }),
      });
      if (!node) return;
      try {
        const [spec, engines] = await Promise.all([host.spec(node), host.engines()]);
        if (mine === ticket) change({ spec, engines, loading: false });
      } catch (e) {
        if (mine === ticket) change({ loading: false, error: (e as Error).message });
      }
    },
    set(patch: Partial<Pick<InspectState, "stack">> & Partial<HtmlSettings>) {
      const { stack, ...settings } = patch;
      change({
        ...(stack ? { stack } : {}),
        settings: { ...state.settings, ...settings },
      });
    },
    code: () => state.spec?.code[state.stack] ?? "",
    copyCode() {
      const text = c.code();
      if (text) host.copy(text);
    },
    settingsLine: () =>
      `Scaffold: ${state.settings.scaffold ? "On" : "Off"} · Assets: ${state.settings.assets === "files" ? "Write as files" : "Inline"}`,
    /** The first engine that is installed and signed in, or the first installed one. */
    defaultEngine: () => state.engines.find((e) => e.ready) ?? state.engines[0] ?? null,
    engineLine() {
      const e = c.defaultEngine();
      return e ? `${e.name} · ${e.status}` : null;
    },
    copyPrompt: () =>
      guard(async () => {
        if (!state.node) return;
        const body = await host.prompt(state.node);
        host.copy(body);
        change({ notice: { title: "Copied prompt for your agent", body }, error: null });
      }),
    implementWith: (engine: string) =>
      guard(async () => {
        if (state.node) await host.implement(engine, state.node);
      }),
    copyHtml: () =>
      guard(async () => {
        if (!state.node) return;
        host.copy(await host.html(state.node, state.settings));
        change({ notice: { title: "Copied HTML", body: c.settingsLine() }, error: null });
      }),
    exportHtml: () =>
      guard(async () => {
        if (!state.node) return;
        const path = await host.exportHtml(state.node, state.settings);
        change({ notice: { title: "Exported HTML", body: path }, error: null });
      }),
    dismiss() {
      change({ notice: null, error: null });
    },
  };
  return c;
}
export type InspectController = ReturnType<typeof createInspectController>;

// ---------------------------------------------------------------- Token sync (C20)

export type TokenFormat = "css" | "tailwind" | "json";
export const TOKEN_FORMATS: readonly { id: TokenFormat; label: string }[] = [
  { id: "css", label: "CSS variables" },
  { id: "tailwind", label: "Tailwind v4" },
  { id: "json", label: "Design Tokens JSON" },
];
export interface TokenTarget {
  path: string;
  format: TokenFormat;
  /** The selector that holds light values (":root"). */
  light: string;
  dark: string;
}
export interface TokenChange {
  kind: "added" | "changed" | "removed";
  name: string;
  from: string | null;
  to: string | null;
}
/** What vellum_core::tokens::plan produced for a file: the diff to show and the ops that apply it. */
export interface TokenPlan {
  changes: readonly TokenChange[];
  ops: readonly ToolOp[];
  errors: readonly string[];
  /** The path as the dialog's notice names it ("src/tokens.css"). */
  source?: string;
}
export interface TokenSyncHost {
  defaults(): TokenTarget;
  chooseFile?(): Promise<string | null>;
  preview(source: TokenTarget): Promise<TokenPlan>;
  exportPreview(target: TokenTarget): Promise<string>;
  write(target: TokenTarget): Promise<void>;
}
export interface TokenSyncState {
  dialog: "import" | "export" | null;
  target: TokenTarget;
  plan: TokenPlan | null;
  /** The document revision the plan was made against; an apply on another revision is refused. */
  revision: number;
  exportText: string;
  busy: boolean;
  error: string | null;
  notice: { title: string; body: string } | null;
}
export function createTokenSyncController(deps: {
  engine: EngineClient;
  device: DeviceId;
  host: TokenSyncHost;
}) {
  const { engine, host } = deps;
  let ticket = 0;
  let state: TokenSyncState = {
    dialog: null,
    target: host.defaults(),
    plan: null,
    revision: -1,
    exportText: "",
    busy: false,
    error: null,
    notice: null,
  };
  const listeners = new Set<() => void>();
  const change = (patch: Partial<TokenSyncState>) => {
    state = { ...state, ...patch };
    for (const l of [...listeners]) l();
  };
  const open = (dialog: "import" | "export") =>
    change({
      dialog,
      target: host.defaults(),
      plan: null,
      exportText: "",
      error: null,
      notice: null,
    });
  const c = {
    get state() {
      return state;
    },
    subscribe(l: () => void) {
      listeners.add(l);
      return { dispose: () => void listeners.delete(l) };
    },
    openImport: () => open("import"),
    openExport: () => open("export"),
    close() {
      ticket++;
      change({ dialog: null, busy: false, error: null });
    },
    set(patch: Partial<TokenTarget>) {
      change({ target: { ...state.target, ...patch }, plan: null, exportText: "", error: null });
    },
    async choose() {
      const path = await host.chooseFile?.();
      if (path) c.set({ path });
    },
    /** Reads the file (import) or renders the variables (export). Nothing in the document changes. */
    async preview() {
      const mine = ++ticket;
      const revision = engine.snapshot().revision;
      change({ busy: true, error: null });
      try {
        if (state.dialog === "export") {
          const exportText = await host.exportPreview(state.target);
          if (mine === ticket) change({ exportText, busy: false });
        } else {
          const plan = await host.preview(state.target);
          if (mine === ticket) change({ plan, revision, busy: false });
        }
      } catch (e) {
        if (mine === ticket) change({ busy: false, error: (e as Error).message });
      }
    },
    summary: () => `${plural(state.plan?.changes.length ?? 0, "change")} to review`,
    canApply: () =>
      state.dialog === "import" &&
      !state.busy &&
      !engine.snapshot().readOnly &&
      !!state.plan &&
      state.plan.changes.length > 0 &&
      state.plan.errors.length === 0,
    /** One person transaction, so the import is one ledger entry and one undo. */
    async apply() {
      const plan = state.plan;
      if (!c.canApply() || !plan) return;
      if (engine.snapshot().revision !== state.revision)
        return change({ error: "The file changed since this preview · Preview again" });
      const tx = personTransaction(deps.device, [...plan.ops]);
      const result = tx ? engine.apply(tx) : null;
      if (result?.status !== "applied")
        return change({
          error: `Change refused: ${result?.status === "rejected" ? result.reason : "nothing to apply"}`,
        });
      change({
        dialog: null,
        plan: null,
        error: null,
        notice: {
          title: `Applied ${plural(plan.changes.length, "token change")}`,
          body: `Imported from ${plan.source ?? lastSegments(state.target.path)}. Review the batch in Activity.`,
        },
      });
    },
    async write() {
      change({ busy: true, error: null });
      try {
        await host.write(state.target);
        change({
          dialog: null,
          busy: false,
          notice: { title: "Wrote design tokens", body: lastSegments(state.target.path) },
        });
      } catch (e) {
        change({ busy: false, error: (e as Error).message });
      }
    },
    dismiss() {
      change({ notice: null, error: null });
    },
  };
  return c;
}
export type TokenSyncController = ReturnType<typeof createTokenSyncController>;

// ---------------------------------------------------------------- Export (C21)

export type ExportFormat = "png" | "jpeg" | "webp" | "svg" | "pdf";
export const EXPORT_FORMATS: readonly ExportFormat[] = ["png", "jpeg", "webp", "svg", "pdf"];
export const EXPORT_SCALES: readonly number[] = [1, 2, 3];
export interface ExportRow {
  id: NodeId;
  name: string;
  format: ExportFormat;
  scale: number;
  suffix: string;
}
export interface ExportFrame {
  id: NodeId;
  name: string;
  format?: ExportFormat;
  scale?: number;
  suffix?: string;
}
export interface ExportRequest {
  destination: string;
  rows: readonly (ExportRow & { file: string })[];
}
export interface ExportHost {
  frames(ids: readonly NodeId[]): Promise<readonly ExportFrame[]> | readonly ExportFrame[];
  destination(): Promise<string>;
  chooseDestination?(): Promise<string | null>;
  /** A thumbnail URL for the frame as exported; null while it cannot be drawn. */
  thumbnail?(row: ExportRow): Promise<string | null>;
  writeFiles(request: ExportRequest): Promise<void>;
  copyAs(kind: "png" | "html", ids: readonly NodeId[]): Promise<void>;
}
/** Raster formats default to 2× with an @2x suffix; vector formats to 1× and no suffix (the design's rows). */
export const exportDefaults = (format: ExportFormat) =>
  format === "svg" || format === "pdf"
    ? { scale: 1, suffix: "" }
    : { scale: format === "png" ? 2 : 1, suffix: format === "png" ? "@2x" : "" };
const slug = (name: string) =>
  name
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/-{2,}/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80) || "frame";
/** `step-1@2x.png`. A name already taken gets `-2`, `-3`… before the suffix, so two frames never collide. */
export function exportFileName(
  name: string,
  format: ExportFormat,
  suffix: string,
  taken: readonly string[],
): string {
  const base = slug(name);
  const ext = format === "jpeg" ? "jpg" : format;
  const clean = suffix.replace(/[^A-Za-z0-9@_.-]/g, "");
  for (let n = 1; ; n++) {
    const file = `${n === 1 ? base : `${base}-${n}`}${clean}.${ext}`;
    if (!taken.includes(file)) return file;
  }
}
export interface ExportState {
  open: boolean;
  rows: readonly ExportRow[];
  destination: string;
  thumbnails: Readonly<Record<NodeId, string | null>>;
  busy: boolean;
  error: string | null;
  notice: string | null;
}
export function createExportController(deps: { engine: EngineClient; host: ExportHost }) {
  const { host, engine } = deps;
  let ticket = 0;
  let state: ExportState = {
    open: false,
    rows: [],
    destination: "",
    thumbnails: {},
    busy: false,
    error: null,
    notice: null,
  };
  const listeners = new Set<() => void>();
  const change = (patch: Partial<ExportState>) => {
    state = { ...state, ...patch };
    for (const l of [...listeners]) l();
  };
  const loadThumbnails = async (rows: readonly ExportRow[], mine: number) => {
    if (!host.thumbnail) return;
    for (const row of rows) {
      const url = await host.thumbnail(row).catch(() => null);
      if (mine !== ticket) return;
      change({ thumbnails: { ...state.thumbnails, [row.id]: url } });
    }
  };
  const c = {
    get state() {
      return state;
    },
    subscribe(l: () => void) {
      listeners.add(l);
      return { dispose: () => void listeners.delete(l) };
    },
    async open(ids: readonly NodeId[]) {
      const mine = ++ticket;
      change({ open: true, busy: true, error: null, notice: null, thumbnails: {} });
      try {
        const [frames, destination] = await Promise.all([host.frames(ids), host.destination()]);
        if (mine !== ticket) return;
        const rows = frames.map((f): ExportRow => {
          const format = f.format ?? "png";
          const d = exportDefaults(format);
          return {
            id: f.id,
            name: f.name,
            format,
            scale: f.scale ?? d.scale,
            suffix: f.suffix ?? d.suffix,
          };
        });
        change({ rows, destination, busy: false });
        void loadThumbnails(rows, mine);
      } catch (e) {
        if (mine === ticket) change({ busy: false, error: (e as Error).message });
      }
    },
    close() {
      ticket++;
      change({ open: false, busy: false, error: null });
    },
    setRow(id: NodeId, patch: Partial<Pick<ExportRow, "format" | "scale" | "suffix">>) {
      const mine = ticket;
      change({
        rows: state.rows.map((r) =>
          r.id === id
            ? { ...r, ...(patch.format ? exportDefaults(patch.format) : {}), ...patch }
            : r,
        ),
      });
      const row = state.rows.find((r) => r.id === id);
      if (row && host.thumbnail)
        void host
          .thumbnail(row)
          .catch(() => null)
          .then((url) => {
            if (mine === ticket) change({ thumbnails: { ...state.thumbnails, [id]: url } });
          });
    },
    async choose() {
      const destination = await host.chooseDestination?.();
      if (destination) change({ destination });
    },
    files(): string[] {
      const taken: string[] = [];
      for (const r of state.rows) taken.push(exportFileName(r.name, r.format, r.suffix, taken));
      return taken;
    },
    title: () => `${plural(state.rows.length, "frame")} will be written to your chosen folder.`,
    action: () => `Export ${plural(state.rows.length, "frame")}`,
    canWrite: () => state.open && !state.busy && state.rows.length > 0 && !!state.destination,
    async write() {
      if (!c.canWrite()) return;
      const files = c.files();
      change({ busy: true, error: null });
      try {
        await host.writeFiles({
          destination: state.destination,
          rows: state.rows.map((r, i) => ({ ...r, file: files[i] as string })),
        });
        change({
          open: false,
          busy: false,
          notice: `Wrote ${plural(files.length, "file")} to ${state.destination}`,
        });
      } catch (e) {
        change({ busy: false, error: (e as Error).message });
      }
    },
    async copyPng(ids: readonly NodeId[]) {
      await c.copy("png", ids);
    },
    async copyHtml(ids: readonly NodeId[]) {
      await c.copy("html", ids);
    },
    async copy(kind: "png" | "html", ids: readonly NodeId[]) {
      if (!ids.length || !engine.snapshot().document) return;
      try {
        await host.copyAs(kind, ids);
        change({ notice: kind === "png" ? "Copied as PNG" : "Copied as HTML", error: null });
      } catch (e) {
        change({ error: (e as Error).message });
      }
    },
    dismiss() {
      change({ notice: null, error: null });
    },
  };
  return c;
}
export type ExportController = ReturnType<typeof createExportController>;
