import { describe, expect, it } from "vitest";
import { createLocale, LANGUAGES, resources } from "./index";

describe("offline locale boundary", () => {
  it("resolves thirteen independently scoped languages with exact source fallback", () => {
    for (const { id } of LANGUAGES) {
      const locale = createLocale(id);
      expect(locale.language).toBe(id);
      expect(locale.t("Layers")).toBe(resources[id].Layers);
      expect(locale.t("untranslated.diagnostic")).toBe("untranslated.diagnostic");
    }
    expect(createLocale("ar").direction).toBe("rtl");
    expect(createLocale("en").direction).toBe("ltr");
    expect(createLocale("../ar").language).toBe("en");
  });
});
