import { describe, expect, it } from "vitest";
import { PACKAGE_NAME } from "./index";

describe("@vellum/prompt-packs", () => {
  it("identifies itself", () => {
    expect(PACKAGE_NAME).toBe("@vellum/prompt-packs");
  });
});
