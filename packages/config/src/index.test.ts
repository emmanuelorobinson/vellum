import { describe, expect, it } from "vitest";
import { brand, copy, PACKAGE_NAME } from "./index";

describe("@vellum/config", () => {
  it("identifies itself", () => {
    expect(PACKAGE_NAME).toBe("@vellum/config");
  });

  it("exports the canonical brand", () => {
    expect(brand.name).toBe("Vellum");
    expect(brand.tagline).toBe("Design where your code lives.");
  });

  it("exports the Appendix B copy, and the brand tagline is taken from it", () => {
    // scripts/copy-lint.mjs checks every value against guide Appendix B; this checks the shape.
    expect(Object.keys(copy)).toHaveLength(31);
    expect(brand.tagline).toBe(copy.tagline);
    expect(copy.firstOpenHeading).toBe("What are we designing?");
    expect(copy.syncScopeA05).toBe("Copies of files you choose to share, never your whole repo");
  });
});
