import type { BrowserWindow, BrowserWindowConstructorOptions } from "electron";
import { expect, it, vi } from "vitest";
import { createElectronBrowser } from "./browser";

it("asks before creating any native page and releases captures", async () => {
  const authorize = vi.fn(async () => {
    throw Error("denied");
  });
  const create = vi.fn<(options: BrowserWindowConstructorOptions) => BrowserWindow>();
  const port = createElectronBrowser({ authorize, create });
  await expect(
    port.capture({ url: "http://localhost:3000/", width: 390, height: 844 }),
  ).rejects.toThrow("denied");
  expect(create).not.toHaveBeenCalled();
});
it("captures only in an isolated window and always closes it", async () => {
  const close = vi.fn();
  const sendCommand = vi.fn(async (method: string) =>
    method === "Runtime.evaluate"
      ? { result: { value: { clip: { x: 0, y: 0, width: 390, height: 844, scale: 1 } } } }
      : { data: "cG5n" },
  );
  const native = {
    webContents: {
      debugger: { attach: vi.fn(), sendCommand },
      session: {
        setPermissionRequestHandler: vi.fn(),
        setPermissionCheckHandler: vi.fn(),
        webRequest: { onBeforeRequest: vi.fn() },
      },
      setWindowOpenHandler: vi.fn(),
      on: vi.fn(),
    },
    loadURL: vi.fn(async () => {}),
    destroy: close,
    isDestroyed: () => false,
  };
  const create = vi.fn(
    (_options: BrowserWindowConstructorOptions) => native as unknown as BrowserWindow,
  );
  const authorize = vi.fn(async () => {});
  const port = createElectronBrowser({ create, authorize });
  const result = await port.capture({ url: "http://localhost:3000/", width: 390, height: 844 });
  expect(authorize).toHaveBeenCalledWith("http://localhost:3000");
  expect(result.width).toBe(390);
  expect(close).toHaveBeenCalledOnce();
  expect(create.mock.calls[0]?.[0].webPreferences).toMatchObject({
    sandbox: true,
    contextIsolation: true,
    nodeIntegration: false,
  });
});
it("keeps a live page until its watch is disposed", async () => {
  const destroy = vi.fn();
  const native = {
    webContents: {
      debugger: {
        attach: vi.fn(),
        sendCommand: vi.fn(async (method: string) =>
          method === "Runtime.evaluate"
            ? { result: { value: { clip: { x: 0, y: 0, width: 375, height: 812, scale: 1 } } } }
            : { data: "cG5n" },
        ),
      },
      session: {
        setPermissionRequestHandler: vi.fn(),
        setPermissionCheckHandler: vi.fn(),
        webRequest: { onBeforeRequest: vi.fn() },
      },
      setWindowOpenHandler: vi.fn(),
      on: vi.fn(),
    },
    loadURL: vi.fn(async () => {}),
    destroy,
    isDestroyed: () => false,
  };
  const port = createElectronBrowser({
    authorize: async () => {},
    create: () => native as unknown as BrowserWindow,
  });
  const frame = vi.fn();
  const stop = await port.watch?.(
    { url: "http://localhost:3000/", width: 375, height: 812 },
    frame,
    () => {},
  );
  expect(frame).toHaveBeenCalledOnce();
  expect(destroy).not.toHaveBeenCalled();
  stop?.dispose();
  expect(destroy).toHaveBeenCalledOnce();
});
