import {
  type BrowserCapture,
  type BrowserPort,
  type BrowserRequest,
  browserRequest,
  type DomNode,
  type DomSnapshot,
} from "@vellum/host-api";
export const BROWSER_DEVICES = {
  desktop: { width: 1280, height: 800, label: "Desktop · 1280 × 800" },
  phone: { width: 375, height: 812, label: "Phone · 375 × 812" },
} as const;
export interface BrowserImportHost {
  browser: BrowserPort;
  /** Host converts through the native HTML importer and applies one person-origin transaction. */
  importLayers?(snapshot: DomSnapshot): Promise<void>;
  /** Host stores original image bytes and inserts one image through a person-origin transaction. */
  importScreenshot?(capture: BrowserCapture): Promise<void>;
}
export interface BrowserImportState {
  url: string;
  device: keyof typeof BROWSER_DEVICES;
  preview: BrowserCapture | null;
  selection: DomNode | null;
  busy: boolean;
  error: string | null;
  status: string | null;
}
export function createBrowserImport(host: BrowserImportHost) {
  let state: BrowserImportState = {
    url: "http://localhost:3000",
    device: "desktop",
    preview: null,
    selection: null,
    busy: false,
    error: null,
    status: null,
  };
  let revision = 0;
  const listeners = new Set<() => void>();
  const update = (patch: Partial<BrowserImportState>) => {
    state = { ...state, ...patch };
    for (const listener of listeners) listener();
  };
  const request = (selection = false): BrowserRequest =>
    browserRequest({
      url: state.url,
      width: BROWSER_DEVICES[state.device].width,
      height: BROWSER_DEVICES[state.device].height,
      ...(selection && state.selection?.selector ? { selector: state.selection.selector } : {}),
      snapshot: true,
    });
  async function action(kind: "layers" | "screenshot") {
    if (state.busy || !state.preview) return;
    const apply = kind === "layers" ? host.importLayers : host.importScreenshot;
    if (!apply) return;
    const version = revision;
    update({ busy: true, error: null, status: null });
    try {
      const capture = state.selection ? await host.browser.capture(request(true)) : state.preview;
      if (version !== revision) return;
      if (kind === "layers") {
        if (!capture.dom) throw Error("The browser returned no editable layers.");
        await host.importLayers?.(capture.dom);
      } else await host.importScreenshot?.(capture);
      if (version === revision)
        update({ status: kind === "layers" ? "Layers imported" : "Screenshot imported" });
    } catch {
      if (version === revision) update({ error: "The import could not be completed. Try again." });
    } finally {
      if (version === revision) update({ busy: false });
    }
  }
  return {
    host,
    get state() {
      return state;
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    configure(patch: Partial<Pick<BrowserImportState, "url" | "device">>) {
      revision++;
      update({ ...patch, preview: null, selection: null, busy: false, error: null, status: null });
    },
    async load() {
      if (state.busy) return;
      const version = ++revision;
      update({ busy: true, preview: null, selection: null, error: null, status: null });
      try {
        const preview = await host.browser.capture(request());
        if (version === revision) update({ preview });
      } catch {
        if (version === revision)
          update({
            error: "The page could not be loaded. Check the address and permission, then retry.",
          });
      } finally {
        if (version === revision) update({ busy: false });
      }
    },
    pick(x: number, y: number) {
      if (state.busy) return;
      const candidates = (state.preview?.dom?.nodes ?? []).filter(
        (n) =>
          n.selector &&
          n.width > 0 &&
          n.height > 0 &&
          x >= n.x &&
          y >= n.y &&
          x <= n.x + n.width &&
          y <= n.y + n.height,
      );
      update({ selection: candidates.at(-1) ?? null, status: null });
    },
    select(selector: string) {
      update({ selection: state.preview?.dom?.nodes.find((n) => n.selector === selector) ?? null });
    },
    clearSelection() {
      update({ selection: null });
    },
    importLayers: () => action("layers"),
    screenshot: () => action("screenshot"),
    dispose() {
      revision++;
      listeners.clear();
    },
  };
}
export type BrowserImportController = ReturnType<typeof createBrowserImport>;
export function embedRequest(
  node: { url: string; device?: string | null; zoom?: number; scroll?: { x: number; y: number } },
  bounds: { width: number; height: number },
): BrowserRequest {
  const preset =
    node.device === "phone"
      ? BROWSER_DEVICES.phone
      : node.device === "desktop"
        ? BROWSER_DEVICES.desktop
        : null;
  return browserRequest({
    url: node.url,
    width: preset?.width ?? Math.round(bounds.width),
    height: preset?.height ?? Math.round(bounds.height),
    zoom: node.zoom ?? 1,
    scroll: [node.scroll?.x ?? 0, node.scroll?.y ?? 0],
  });
}

export * from "./figma";
