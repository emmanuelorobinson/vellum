/**
 * A design document held by the daemon's engine (ADR-033), for a native host (the VS Code extension host, the
 * desktop main process). The host is one daemon connection; the
 * document registers its own editor there, and each webview panel registers another, all replicas of the one
 * engine. Save, revert, backup and recovery go to that engine; no document is built or changed here.
 */
import { fromBase64, toBase64 } from "./editor-replica";

/** The daemon's `vellum/editor` method (`DaemonProcess.editor`). */
export type EditorCall = (params: Record<string, string>) => Promise<Record<string, unknown>>;

/** What a webview panel may ask of the daemon for its own editor; the host adds path, name and device. */
export type PanelAction = "open" | "push" | "pull" | "close";
const PANEL_PARAMS: Record<PanelAction, readonly string[]> = {
  open: [],
  push: ["updates"],
  pull: ["cursor"],
  close: [],
};

export function panelRequest(
  action: unknown,
  params: unknown,
): { action: PanelAction; params: Record<string, string> } | null {
  if (typeof action !== "string" || !(action in PANEL_PARAMS)) return null;
  if (!params || typeof params !== "object" || Array.isArray(params)) return null;
  const allowed = PANEL_PARAMS[action as PanelAction];
  const entries = Object.entries(params as Record<string, unknown>);
  if (
    entries.length !== allowed.length ||
    entries.some(([k, v]) => !allowed.includes(k) || typeof v !== "string" || v.length > 700_000)
  )
    return null;
  return {
    action: action as PanelAction,
    params: Object.fromEntries(entries) as Record<string, string>,
  };
}

let sequence = 0;
const name = (kind: string) => `${kind}-${Date.now().toString(36)}-${++sequence}`;

export class DaemonRuntime {
  readonly editable = true;
  private listeners = new Set<(change: { revision: number }) => void>();
  private panels = new Set<string>();
  private disposed = false;

  private constructor(
    private readonly call: EditorCall,
    private readonly path: string,
    private readonly device: string,
    private readonly editor: string,
    public revision: number,
  ) {}

  /** Opens `path` at the daemon. Throws when the daemon cannot hold it (the host opens it read-only). */
  static async open(call: EditorCall, path: string, device: string): Promise<DaemonRuntime> {
    const editor = name("host");
    const reply = await call({ action: "open", path, editor, device });
    return new DaemonRuntime(call, path, device, editor, Number(reply.revision ?? 0));
  }

  private observe(reply: Record<string, unknown>) {
    const revision = reply.revision;
    if (typeof revision !== "number" || revision === this.revision) return;
    this.revision = revision;
    for (const listener of [...this.listeners]) listener({ revision });
  }

  private async host(action: string, extra: Record<string, string> = {}) {
    if (this.disposed) throw Error("disposed");
    const reply = await this.call({ action, path: this.path, editor: this.editor, ...extra });
    this.observe(reply);
    return reply;
  }

  async snapshot() {
    const reply = await this.host("snapshot");
    if (typeof reply.bytes !== "string") throw Error("invalid snapshot");
    return { bytes: fromBase64(reply.bytes), revision: Number(reply.revision) };
  }

  async replace(bytes: Uint8Array, reason: "revert" | "external" | "recovery") {
    const reply =
      reason === "revert"
        ? await this.host("revert")
        : reason === "external"
          ? await this.host("replace", { bytes: "", reason })
          : await this.host("replace", { bytes: toBase64(bytes), reason });
    return { revision: Number(reply.revision) };
  }

  /** The daemon writes the engine's document to the file (a host without its own save path). */
  async save() {
    return { revision: Number((await this.host("save")).revision) };
  }

  /** The host wrote the file (Save): the daemon reads it back as the saved state. */
  async written() {
    await this.host("saved");
  }

  /** A webview panel's own editor: a new name for `open`, then its requests relayed with the host's device. */
  newPanel(): string {
    const panel = name("panel");
    this.panels.add(panel);
    return panel;
  }

  async relay(panel: string, action: PanelAction, params: Record<string, string>) {
    if (!this.panels.has(panel) || this.disposed) throw Error("panel closed");
    const reply = await this.call({
      action,
      path: this.path,
      editor: panel,
      ...(action === "open" ? { device: this.device } : {}),
      ...params,
    });
    this.observe(reply);
    if (action === "close") this.panels.delete(panel);
    return reply;
  }

  subscribe(listener: (change: { revision: number }) => void) {
    this.listeners.add(listener);
    return { dispose: () => void this.listeners.delete(listener) };
  }

  /** Closes every editor this runtime opened, and resolves once the daemon has let go of them. */
  async close() {
    if (this.disposed) return;
    this.disposed = true;
    this.listeners.clear();
    const editors = [...this.panels, this.editor];
    this.panels.clear();
    await Promise.all(
      editors.map((editor) =>
        this.call({ action: "close", path: this.path, editor }).catch(() => {}),
      ),
    );
  }

  dispose() {
    void this.close();
  }
}
