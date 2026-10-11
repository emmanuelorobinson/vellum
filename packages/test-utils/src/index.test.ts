import { describe, expect, it } from "vitest";
import { PACKAGE_NAME } from "./index";

describe("@vellum/test-utils", () => {
  it("identifies itself", () => {
    expect(PACKAGE_NAME).toBe("@vellum/test-utils");
  });
});
