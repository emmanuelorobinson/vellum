import { createLocale, LANGUAGES, resources } from "@vellum/i18n";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PanelFrame } from "../panels/PanelFrame";
import { LocaleContext } from "./locale";

describe("bundled editor languages", () => {
  it("ships thirteen complete catalogs with matching placeholders", () => {
    expect(LANGUAGES).toHaveLength(13);
    const keys = Object.keys(resources.en).sort();
    for (const { id } of LANGUAGES) {
      expect(Object.keys(resources[id]).sort(), id).toEqual(keys);
      for (const key of keys) {
        const text = resources[id][key];
        expect(text?.trim(), `${id}:${key}`).toBeTruthy();
        expect(text?.match(/\{\{\w+\}\}/g)?.sort() ?? [], `${id}:${key}`).toEqual(
          resources.en[key]?.match(/\{\{\w+\}\}/g)?.sort() ?? [],
        );
      }
    }
  });
  it("isolates editor instances and keeps interpolation as text", () => {
    const ar = createLocale("ar"),
      en = createLocale("en");
    expect(ar.direction).toBe("rtl");
    expect(ar.t("Layers")).toBe("الطبقات");
    expect(en.t("Layers")).toBe("Layers");
    expect(createLocale("unknown").language).toBe("en");
    expect(ar.t("Selected: {{name}}", { name: "<script>" })).toContain("<script>");
  });
  it("renders translated chrome through React without changing document names", () => {
    const markup = renderToStaticMarkup(
      <LocaleContext.Provider value={createLocale("ar")}>
        <PanelFrame
          layout={{ mode: "panel", rect: { x: 8, y: 56, width: 320, height: 700 } } as never}
          tab="layers"
          onTab={() => {}}
          onCloseOverlay={() => {}}
          shortcut={null}
        >
          {{ agent: null, layers: <span>design/example.vellum</span>, assets: null, history: null }}
        </PanelFrame>
      </LocaleContext.Provider>,
    );
    expect(markup).toContain("الطبقات");
    expect(markup).toContain("design/example.vellum");
  });
});
