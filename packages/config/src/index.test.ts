import { describe, expect, it } from "vitest";
import { brand, PACKAGE_NAME } from "./index";

describe("@vellum/config", () => {
  it("identifies itself", () => {
    expect(PACKAGE_NAME).toBe("@vellum/config");
  });

  it("exports the canonical brand", () => {
    expect(brand.name).toBe("Vellum");
    expect(brand.tagline).toBe("Design where your code lives.");
  });
});
