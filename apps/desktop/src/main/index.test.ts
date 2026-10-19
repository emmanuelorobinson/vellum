import { describe, expect, it } from "vitest";
import { WEB_PREFERENCES, windowChrome } from "./index";

describe("@vellum/desktop main process", () => {
  it("never relaxes the renderer security defaults", () => {
    expect(WEB_PREFERENCES).toEqual({
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
    });
  });
  it("integrates the macOS controls into the renderer without a separate title bar", () => {
    expect(windowChrome("darwin")).toEqual({
      titleBarStyle: "hidden",
      trafficLightPosition: { x: 20, y: 20 },
    });
    expect(windowChrome("win32")).toEqual({});
    expect(windowChrome("linux")).toEqual({});
  });
});
