import { brand } from "@vellum/config";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import Page from "../app/page";

describe("home page", () => {
  it("renders the brand name and tagline", () => {
    const html = renderToStaticMarkup(<Page />);
    expect(html).toContain(brand.name);
    expect(html).toContain(brand.tagline);
  });
});
