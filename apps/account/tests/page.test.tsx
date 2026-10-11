import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import Page from "../app/page";

describe("home page", () => {
  it("renders the brand name and tagline", () => {
    const html = renderToStaticMarkup(<Page />);
    expect(html).toContain("Vellum");
    expect(html).toContain("Design where your code lives.");
  });
});
