// TEST DATA.

import { createBrowserImport } from "@vellum/editor-core";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { BrowserImportPanel, EmbedProperties } from "./BrowserImport";

it("makes unavailable document writes explicit and keeps pages outside the editor DOM", () => {
  const controller = createBrowserImport({
    browser: {
      capture: async () => {
        throw Error("offline");
      },
    },
  });
  const html = renderToStaticMarkup(<BrowserImportPanel controller={controller} readOnly />);
  expect(html).toContain("Import from a local page");
  expect(html).toContain("disabled");
  expect(html).not.toContain("<iframe");
  expect(html).toContain("localhost");
});
it("shows URL, device and zoom controls for an embed", () => {
  const html = renderToStaticMarkup(
    <EmbedProperties
      node={{ url: "http://localhost:3000", device: "phone", zoom: 0.8 }}
      onChange={() => {}}
    />,
  );
  for (const label of ["URL", "Device", "Zoom", "375 × 812", "80"]) expect(html).toContain(label);
});
