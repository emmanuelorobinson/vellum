import { describe, expect, it } from "vitest";
import { PACKAGE_NAME } from "./index";

describe("@vellum/design-systems", () => {
  it("identifies itself", () => {
    expect(PACKAGE_NAME).toBe("@vellum/design-systems");
  });
});
