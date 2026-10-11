import { describe, expect, it } from "vitest";
import { PACKAGE_NAME } from "./index";

describe("@vellum/auth", () => {
  it("identifies itself", () => {
    expect(PACKAGE_NAME).toBe("@vellum/auth");
  });
});
