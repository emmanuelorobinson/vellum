import { describe, expect, it, vi } from "vitest";
import type { DesktopEvent, DesktopRequest } from "./bridge";
import { createBridgeServices, parseRequest } from "./bridge";

describe("contextBridge Host services", () => {
  it("keeps the synchronous theme current even without a theme listener", () => {
    let receive: ((event: DesktopEvent) => void) | undefined;
    const unsubscribe = vi.fn();
    const bridge = {
      request: vi.fn(async () => ({ ok: true as const })),
      subscribe: (listener: (event: DesktopEvent) => void) => {
        receive = listener;
        return unsubscribe;
      },
    };
    const services = createBridgeServices(bridge, { appearance: "light" });
    const update = {
      type: "state" as const,
      state: {
        folderName: "",
        tabs: [],
        active: null,
        platform: "mac" as const,
        daemon: false,
        problem: "offline" as const,
        appearance: "dark" as const,
      },
    };
    receive?.(update);
    expect(services.theme()).toEqual({ appearance: "dark" });
    services.dispose();
    expect(unsubscribe).toHaveBeenCalledOnce();
  });
  it("maps opaque document reads and subscriptions without exposing a transport", async () => {
    const request = vi.fn(async () => ({ ok: true as const, value: [1, 2] }));
    const unsubscribe = vi.fn();
    const bridge = { request, subscribe: vi.fn(() => unsubscribe) };
    const services = createBridgeServices(bridge, { appearance: "dark" });
    expect(await services.readFile("document:tab1")).toEqual(new Uint8Array([1, 2]));
    expect(request).toHaveBeenCalledWith({ type: "readDocument", tab: "tab1" });
    await expect(services.readFile("file:///secret")).rejects.toThrow("invalid");
    await expect(services.writeFile("document:tab1", new Uint8Array())).rejects.toThrow(
      "read-only",
    );
    const subscription = services.watch("document:tab1", vi.fn());
    subscription.dispose();
    expect(unsubscribe).toHaveBeenCalledOnce();
  });
  it("sanitizes errors and exposes only a relayed daemon endpoint", async () => {
    const bridge = {
      request: vi.fn(async () => ({ ok: true as const, value: true })),
      subscribe: vi.fn(() => () => {}),
    };
    const services = createBridgeServices(bridge, { appearance: "light" });
    expect(await services.discoverDaemon()).toEqual({ kind: "relayed", protocol: 1 });
    bridge.request.mockRejectedValueOnce(Error("secret"));
    await expect(services.openExternal("https://example.org")).rejects.toThrow("failed");
    expect(services.theme()).toEqual({ appearance: "light" });
  });
});

it("accepts only fixed account actions, never renderer URLs, tokens or command arguments", async () => {
  const { parseRequest } = await import("./bridge");
  for (const type of [
    "accountLogin",
    "accountCancel",
    "accountRefresh",
    "accountLogout",
    "accountOpen",
  ]) {
    expect(parseRequest({ type })).toEqual({ type });
    expect(parseRequest({ type, token: "canary" })).toBeNull();
    expect(parseRequest({ type, url: "https://untrusted.example" })).toBeNull();
    expect(parseRequest({ type, args: ["--account-url", "https://untrusted.example"] })).toBeNull();
  }
});

it("starts a narrow live browser stream and stops it when disposed", async () => {
  const { createDesktopBrowser, parseRequest } = await import("./bridge");
  let receive: ((event: DesktopEvent) => void) | undefined;
  const unsubscribe = vi.fn();
  const request = vi.fn(async (_request: DesktopRequest) => ({ ok: true as const }));
  const port = createDesktopBrowser({
    request,
    subscribe: (listener) => {
      receive = listener;
      return unsubscribe;
    },
  });
  const frame = vi.fn();
  const stop = await port.watch?.(
    { url: "http://localhost:3000/", width: 375, height: 812 },
    frame,
    () => {},
  );
  const start = request.mock.calls[0]?.[0];
  expect(start).toMatchObject({ type: "browserWatch" });
  if (start?.type !== "browserWatch") throw Error("missing watch");
  receive?.({
    type: "browserFrame",
    id: start.id,
    capture: { png: "cG5n", width: 375, height: 812 },
  });
  expect(frame).toHaveBeenCalledOnce();
  stop?.dispose();
  expect(unsubscribe).toHaveBeenCalledOnce();
  expect(request).toHaveBeenLastCalledWith({ type: "browserStop", id: start.id });
  expect(
    parseRequest({
      type: "browserWatch",
      id: "valid",
      request: { url: "file:///etc/passwd", width: 1, height: 1 },
    }),
  ).toBeNull();
});

it("allows only explicit telemetry consent booleans at the native boundary", () => {
  expect(parseRequest({ type: "telemetry", enabled: true })).toEqual({
    type: "telemetry",
    enabled: true,
  });
  expect(parseRequest({ type: "telemetry", enabled: "true" })).toBeNull();
  expect(parseRequest({ type: "telemetry", enabled: true, prompt: "TEST DATA" })).toBeNull();
});
