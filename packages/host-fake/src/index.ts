import {
  type ClipboardData,
  cloneClipboard,
  type DaemonEndpoint,
  type Disposable,
  externalUrl,
  type Host,
  PortError,
  type ThemeInfo,
} from "@vellum/host-api";
export const PACKAGE_NAME = "@vellum/host-fake";
export class FakeHost implements Host {
  readonly kind = "fake";
  private files = new Map<string, Uint8Array>();
  private watchers = new Map<string, Set<() => void>>();
  private themeListeners = new Set<(theme: ThemeInfo) => void>();
  private currentTheme: ThemeInfo = { appearance: "light" };
  private data: ClipboardData = { items: {} };
  private failWrite = false;
  private disposed = false;
  daemon: DaemonEndpoint | null = null;
  readonly external: string[] = [];
  private live() {
    if (this.disposed) throw new PortError("disposed");
  }
  failNextWrite() {
    this.failWrite = true;
  }
  async readFile(uri: string) {
    this.live();
    const data = this.files.get(uri);
    if (!data) throw new PortError("missing");
    return data.slice();
  }
  async writeFile(uri: string, data: Uint8Array) {
    this.live();
    if (this.failWrite) {
      this.failWrite = false;
      throw new PortError("failed");
    }
    this.files.set(uri, data.slice());
    for (const listener of [...(this.watchers.get(uri) ?? [])]) listener();
  }
  watch(uri: string, listener: () => void): Disposable {
    this.live();
    const set = this.watchers.get(uri) ?? new Set();
    set.add(listener);
    this.watchers.set(uri, set);
    return {
      dispose: () => {
        set.delete(listener);
        if (!set.size) this.watchers.delete(uri);
      },
    };
  }
  async discoverDaemon() {
    this.live();
    return this.daemon ? { ...this.daemon } : null;
  }
  async openExternal(url: string) {
    this.live();
    this.external.push(externalUrl(url));
  }
  clipboard = {
    read: async () => {
      this.live();
      return cloneClipboard(this.data);
    },
    write: async (data: ClipboardData) => {
      this.live();
      this.data = cloneClipboard(data);
    },
  };
  theme() {
    this.live();
    return { ...this.currentTheme };
  }
  setTheme(theme: ThemeInfo) {
    this.live();
    this.currentTheme = { ...theme };
    for (const listener of [...this.themeListeners]) listener({ ...theme });
  }
  onThemeChange(listener: (theme: ThemeInfo) => void): Disposable {
    this.live();
    this.themeListeners.add(listener);
    return {
      dispose: () => {
        this.themeListeners.delete(listener);
      },
    };
  }
  dispose() {
    this.disposed = true;
    this.watchers.clear();
    this.themeListeners.clear();
  }
}
