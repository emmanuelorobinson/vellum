export interface Disposable {
  dispose(): void;
}
export type Appearance = "light" | "dark" | "hc-light" | "hc-dark";
export interface ThemeInfo {
  appearance: Appearance;
}
export interface ClipboardData {
  items: Readonly<Record<string, Uint8Array>>;
}
/** A host-owned relay handle; never a socket path, TCP address or authentication token. */
export interface DaemonEndpoint {
  kind: "relayed";
  protocol: 1;
}
export interface Host {
  readonly kind: "fake" | "browser" | "electron" | "vscode";
  readFile(uri: string): Promise<Uint8Array>;
  /** The host must perform atomic replacement. A rejected write leaves the original bytes intact. */
  writeFile(uri: string, data: Uint8Array): Promise<void>;
  watch(uri: string, onChange: () => void): Disposable;
  discoverDaemon(): Promise<DaemonEndpoint | null>;
  openExternal(url: string): Promise<void>;
  clipboard: { read(): Promise<ClipboardData>; write(data: ClipboardData): Promise<void> };
  theme(): ThemeInfo;
  onThemeChange(listener: (theme: ThemeInfo) => void): Disposable;
  dispose(): void;
}
export class PortError extends Error {
  constructor(
    public readonly code:
      | "offline"
      | "read-only"
      | "missing"
      | "disposed"
      | "unsupported"
      | "invalid"
      | "failed",
  ) {
    // Never echo bridge payloads, paths, credentials or vendor errors into UI diagnostics.
    super(code);
    this.name = "PortError";
  }
}
export function externalUrl(url: string): string {
  if (
    !/^https?:\/\/[^\s]+$/i.test(url) ||
    [...url].some((c) => c.charCodeAt(0) < 32 || c.charCodeAt(0) === 127) ||
    /^https?:\/\/[^/]*@/i.test(url)
  )
    throw new PortError("invalid");
  return url;
}
export function cloneClipboard(data: ClipboardData): ClipboardData {
  return {
    items: Object.fromEntries(
      Object.entries(data.items).map(([mime, bytes]) => [mime, bytes.slice()]),
    ),
  };
}

/** Trusted host services injected by the VS Code extension / Electron preload / browser composition.
 * Those native compositions own I/O, permission checks and daemon authentication (Prompts 55–58).
 */
export type HostServices = Omit<Host, "kind" | "dispose">;
export function createHost(kind: Host["kind"], services: HostServices): Host {
  let disposed = false;
  const subscriptions = new Set<Disposable>();
  const live = () => {
    if (disposed) throw new PortError("disposed");
  };
  const subscribe = (subscribe: () => Disposable): Disposable => {
    live();
    const inner = subscribe();
    subscriptions.add(inner);
    return {
      dispose() {
        if (subscriptions.delete(inner)) inner.dispose();
      },
    };
  };
  return {
    kind,
    async readFile(uri) {
      live();
      const bytes = await services.readFile(uri);
      live();
      return bytes.slice();
    },
    async writeFile(uri, data) {
      live();
      await services.writeFile(uri, data.slice());
    },
    watch: (uri, listener) =>
      subscribe(() =>
        services.watch(uri, () => {
          if (!disposed) listener();
        }),
      ),
    async discoverDaemon() {
      live();
      const endpoint = await services.discoverDaemon();
      live();
      return endpoint ? { kind: "relayed", protocol: 1 } : null;
    },
    async openExternal(url) {
      live();
      await services.openExternal(externalUrl(url));
    },
    clipboard: {
      async read() {
        live();
        const data = await services.clipboard.read();
        live();
        return cloneClipboard(data);
      },
      async write(data) {
        live();
        await services.clipboard.write(cloneClipboard(data));
      },
    },
    theme() {
      live();
      return { ...services.theme() };
    },
    onThemeChange: (listener) =>
      subscribe(() =>
        services.onThemeChange((t) => {
          if (!disposed) listener({ ...t });
        }),
      ),
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const d of subscriptions) d.dispose();
      subscriptions.clear();
    },
  };
}
