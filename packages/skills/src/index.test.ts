import { describe, expect, it } from "vitest";
import { PACKAGE_NAME } from "./index";

describe("@vellum/skills", () => {
  it("identifies itself", () => {
    expect(PACKAGE_NAME).toBe("@vellum/skills");
  });
});
