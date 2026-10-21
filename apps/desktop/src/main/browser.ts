/** Electron's own Chromium implements BrowserPort. Remote pages have no preload or native bridge. */
import { randomUUID } from "node:crypto";
import {
  type BrowserCapture,
  type BrowserPort,
  type BrowserRequest,
  browserOrigins,
  browserRequest,
} from "@vellum/host-api";
import type { BrowserWindow, BrowserWindowConstructorOptions } from "electron";
import collector from "../../../../packages/host-api/src/browser-snapshot.json";
import { WEB_PREFERENCES } from "./index";
export interface BrowserServices {
  authorize(origin: string): Promise<void>;
  create(options: BrowserWindowConstructorOptions): BrowserWindow;
}
async function bounded<T>(work: Promise<T>): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      work,
      new Promise<never>((_ok, no) => {
        timer = setTimeout(() => no(Error("Browser timed out.")), 30000);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}
export function createElectronBrowser(services: BrowserServices): BrowserPort {
  async function open(input: BrowserRequest) {
    const request = browserRequest(input),
      origins = browserOrigins(request);
    for (const origin of origins) await services.authorize(origin);
    const win = services.create({
      show: false,
      width: request.width,
      height: request.height,
      useContentSize: true,
      webPreferences: {
        ...WEB_PREFERENCES,
        partition: `vellum-browser-${randomUUID()}`,
        offscreen: true,
        backgroundThrottling: false,
        webSecurity: true,
      },
    });
    const contents = win.webContents;
    const dispose = () => {
      if (!win.isDestroyed()) win.destroy();
    };
    contents.session.setPermissionRequestHandler((_contents, _permission, reply) => reply(false));
    contents.session.setPermissionCheckHandler(() => false);
    contents.setWindowOpenHandler(() => ({ action: "deny" }));
    contents.on("will-attach-webview", (event) => event.preventDefault());
    contents.session.on?.("will-download", (event) => event.preventDefault());
    contents.session.webRequest.onBeforeRequest((details, reply) => {
      let allowed = details.url === "about:blank" || details.url.startsWith("data:");
      try {
        const url = new URL(details.url);
        allowed ||=
          !url.username &&
          !url.password &&
          origins.includes(url.origin) &&
          ["http:", "https:"].includes(url.protocol);
      } catch {}
      reply({ cancel: !allowed });
    });
    const cdp = (method: string, params: Record<string, unknown> = {}) =>
      contents.debugger.sendCommand(method, params);
    try {
      contents.debugger.attach("1.3");
      await bounded(
        (async () => {
          await cdp("Emulation.setDeviceMetricsOverride", {
            width: request.width,
            height: request.height,
            deviceScaleFactor: 1,
            mobile: false,
          });
          await win.loadURL(request.url);
          await cdp("Emulation.setPageScaleFactor", { pageScaleFactor: request.zoom });
        })(),
      );
    } catch {
      dispose();
      throw Error("The browser could not load this page.");
    }
    return {
      dispose,
      async capture(): Promise<BrowserCapture> {
        return bounded(
          (async () => {
            const result = await cdp("Runtime.evaluate", {
              expression: `(${collector.script})(${JSON.stringify({ selector: request.selector, scroll: request.scroll, snapshot: request.snapshot })})`,
              returnByValue: true,
              awaitPromise: true,
            });
            const data = result.result?.value;
            if (!data || data.error || !data.clip)
              throw Error("The selected page or element could not be captured.");
            const shot = await cdp("Page.captureScreenshot", {
              format: "png",
              clip: data.clip,
              captureBeyondViewport: true,
            });
            if (
              typeof shot.data !== "string" ||
              shot.data.length > 44 * 1024 * 1024 ||
              JSON.stringify(data).length > 32 * 1024 * 1024
            )
              throw Error("Browser capture exceeds limits.");
            return {
              png: shot.data,
              width: Math.ceil(data.clip.width),
              height: Math.ceil(data.clip.height),
              ...(request.snapshot ? { dom: data.dom } : {}),
            };
          })(),
        );
      },
    };
  }
  return {
    async capture(request) {
      const page = await open(request);
      try {
        return await page.capture();
      } finally {
        page.dispose();
      }
    },
    async watch(request, frame, failed) {
      const page = await open({ ...request, snapshot: false });
      let timer: ReturnType<typeof setTimeout> | undefined;
      let disposed = false;
      const dispose = () => {
        if (disposed) return;
        disposed = true;
        clearTimeout(timer);
        page.dispose();
      };
      const tick = async () => {
        try {
          const capture = await page.capture();
          if (disposed) return;
          frame(capture);
          timer = setTimeout(() => void tick(), 1000);
        } catch {
          dispose();
          failed();
        }
      };
      try {
        const initial = await page.capture();
        frame(initial);
        timer = setTimeout(() => void tick(), 1000);
      } catch {
        dispose();
        throw Error("The browser could not capture this page.");
      }
      return { dispose };
    },
  };
}
