// TEST DATA: static controls; interactions run in the Chrome lane.

import { createResourceController } from "@vellum/editor-core";
import { FakeEngineClient } from "@vellum/engine-client";
import type { Document } from "@vellum/types-ts";
import { renderToStaticMarkup as html } from "react-dom/server";
import { expect, it } from "vitest";
import { FontPickerBody, ManageFontsBody } from "./resources/FontsDialog";
import { LibrariesBody, PasteBody } from "./resources/LibrariesDialog";
import { VariablesBody } from "./resources/VariablesDialog";

const doc: Document = {
  meta: { formatVersion: 1, name: "Test", createdWith: "test" },
  themes: { Mode: ["Light", "Dark"], Device: ["Phone", "Tablet"] },
  variables: {
    s: {
      type: "number",
      name: "space/4",
      values: [{ value: 16 }, { value: 24, when: { Device: "Tablet" } }],
    },
  },
  imports: { core: "../core.lib.vellum" },
  pages: [],
};
const resources = createResourceController({
  engine: new FakeEngineClient({ revision: 0, document: doc, readOnly: false }),
  device: "test",
  host: {
    libraries: [{ alias: "core", name: "Core", state: "missing", cached: true }],
    systemFonts: ["Inter"],
  },
  nextVariableId: () => "new",
});
it("renders every combination as columns", () => {
  const out = html(<VariablesBody resources={resources} document={doc} disabled={false} axes />);
  for (const name of ["Light / Phone", "Dark / Phone", "Light / Tablet", "Dark / Tablet"])
    expect(out).toContain(name);
  expect(out).toContain('value="24"');
});
it("missing libraries retain a locate action and cache explanation", () => {
  const out = html(<LibrariesBody resources={resources} document={doc} disabled={false} />);
  expect(out).toContain("Missing");
  expect(out).toContain("Locate…");
  expect(out).toContain("cached");
});
it("font picker exposes every section and disabled import when host is absent", () => {
  const out = html(
    <FontPickerBody resources={resources} document={doc} disabled={false} onChoose={() => {}} />,
  );
  for (const label of [
    "Recently used",
    "This document",
    "Google Fonts",
    "System fonts",
    "Manage fonts",
    "Add font…",
  ])
    expect(out).toContain(label);
  expect(out).toContain("disabled");
});
it("font manager reminds the person to review wrapping", () =>
  expect(html(<ManageFontsBody resources={resources} document={doc} disabled={false} />)).toContain(
    "wrapping",
  ));
it("paste conflicts disallow using a different type and list missing bytes", () => {
  resources.requestPaste({
    id: "clip",
    conflicts: [{ id: "c", name: "space/4", existing: "16", pasted: "text", sameType: false }],
    omissions: ["image.png"],
  });
  const out = html(<PasteBody resources={resources} disabled={false} />);
  expect(out).toContain("image.png");
  expect(out).toContain("Use existing");
  expect(out).toContain("disabled");
});
