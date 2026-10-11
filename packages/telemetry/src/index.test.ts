import { describe, expect, it } from "vitest";
import { PACKAGE_NAME } from "./index";

describe("@vellum/telemetry", () => {
  it("identifies itself", () => {
    expect(PACKAGE_NAME).toBe("@vellum/telemetry");
  });
});
