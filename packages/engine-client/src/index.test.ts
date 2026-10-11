import { describe, expect, it } from "vitest";
import { PACKAGE_NAME } from "./index";

describe("@vellum/engine-client", () => {
  it("identifies itself", () => {
    expect(PACKAGE_NAME).toBe("@vellum/engine-client");
  });
});
