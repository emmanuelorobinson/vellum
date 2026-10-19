import { describe, expect, it, vi } from "vitest";
import type { DesktopEvent } from "./bridge";
import { createBridgeServices } from "./bridge";

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
