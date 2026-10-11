import { describe, expect, it } from "vitest";
import { WEB_PREFERENCES } from "./index";

describe("@vellum/desktop main process", () => {
  it("never relaxes the renderer security defaults", () => {
    expect(WEB_PREFERENCES).toEqual({
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
    });
  });
});
