import { describe, expect, it } from "vitest";
import { PACKAGE_NAME } from "./index";

describe("@vellum/host-electron", () => {
  it("identifies itself", () => {
    expect(PACKAGE_NAME).toBe("@vellum/host-electron");
  });
});
