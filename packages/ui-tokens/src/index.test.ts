import { describe, expect, it } from "vitest";
import { PACKAGE_NAME } from "./index";

describe("@vellum/ui-tokens", () => {
  it("identifies itself", () => {
    expect(PACKAGE_NAME).toBe("@vellum/ui-tokens");
  });
});
