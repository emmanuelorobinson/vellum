// TEST DATA. Production C30/C31 components over scripted ports; no external network or document logic.
import "./fonts.css";
import "@vellum/ui-tokens/tokens.css";
import "../../src/authoring/authoring.css";
import "../../src/editor.css";
import { createAuthoringController, createFigmaImport } from "@vellum/editor-core";
import { FakeEngineClient } from "@vellum/engine-client";
import { createRoot } from "react-dom/client";
import { AuthoringDialogs } from "../../src/authoring/AuthoringDialogs";
import { ImageActions } from "../../src/authoring/ImageActions";
import { FigmaImportPanel } from "../../src/browser/FigmaImport";
import { PortalContainer } from "../../src/chrome/portal";

const params = new URLSearchParams(location.search);
document.documentElement.dataset.appearance = params.get("mode") ?? "light";
const node = {
  type: "rect" as const,
  id: "photo",
  name: "TEST DATA photo",
  width: 440,
  height: 260,
  fills: [{ type: "image" as const, asset: "a".repeat(64) }],
};
const documentFixture = {
  meta: { formatVersion: 1, name: "TEST DATA", createdWith: "Vellum" },
  pages: [{ id: "page", name: "TEST DATA", children: [node] }],
};
const engine = new FakeEngineClient({ document: documentFixture, revision: 0, readOnly: false }, {
  get: () => ({
    node,
    bounds: { x: 0, y: 0, width: 440, height: 260 },
    children: [],
    parent: null,
  }),
} as never);
const requests: unknown[] = [];
const authoring = createAuthoringController({
  engine,
  device: "test",
  nextNodeId: () => "testnew",
  host: {
    imageModels: [{ provider: "configured", id: "fixture", name: "Configured image model" }],
    generateImage: async (r) => {
      requests.push(r);
      engine.script({
        document: documentFixture,
        revision: engine.snapshot().revision + 1,
        readOnly: false,
      });
      return { asset: "b".repeat(64) };
    },
    processImage: async (r) => {
      requests.push(r);
    },
  },
});
authoring.openImage("photo", "fill", 0);
const figma = createFigmaImport({
  importFile: async () => ({
    frames: 12,
    nodes: 284,
    notes: [
      "Smart Animate transitions weren't converted.",
      "2 third-party fonts use Inter until installed.",
      "A complex mask was imported as an image.",
    ],
  }),
});
figma.configure("fixture");
if (params.has("report")) await figma.importFile();
Object.assign(window, { imageTest: { authoring, figma, engine, requests } });
const root = document.getElementById("root");
if (!root) throw Error("missing root");
root.className = "vc-workspace";
createRoot(root).render(
  params.has("figma") ? (
    <div style={{ position: "absolute", left: 480, top: 180 }}>
      <FigmaImportPanel controller={figma} onReview={() => requests.push("review")} />
    </div>
  ) : params.has("ops") ? (
    <div
      style={{
        position: "absolute",
        left: 944,
        top: 144,
        width: 320,
        background: "var(--chrome-raised)",
        boxShadow: "inset 0 0 0 1px var(--chrome-border)",
        borderRadius: "var(--radius-lg)",
      }}
    >
      <ImageActions authoring={authoring} address="photo" />
    </div>
  ) : (
    <PortalContainer.Provider value={root}>
      <AuthoringDialogs authoring={authoring} readOnly={false} />
    </PortalContainer.Provider>
  ),
);
