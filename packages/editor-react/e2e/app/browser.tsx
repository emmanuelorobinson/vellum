import { BROWSER_FIXTURE_SVG } from "../../fixtures/browser-scenarios";
import "./fonts.css";
// TEST DATA. Real C31 controls over a scripted, permission-free BrowserPort.
import { createBrowserImport } from "@vellum/editor-core";
import type { BrowserRequest } from "@vellum/host-api";
import { useState } from "react";
import { createRoot } from "react-dom/client";
import { BrowserImportPanel, EmbedPreview, EmbedProperties } from "../../src/browser/BrowserImport";
import "@vellum/ui-tokens/tokens.css";

const q = new URLSearchParams(location.search);
document.documentElement.dataset.appearance = q.get("mode") ?? "light";
const svg = BROWSER_FIXTURE_SVG;
const png = await new Promise<string>((resolve) => {
  const image = new Image();
  image.onload = () => {
    const c = document.createElement("canvas");
    c.width = 1280;
    c.height = 800;
    c.getContext("2d")?.drawImage(image, 0, 0);
    resolve(c.toDataURL("image/png").split(",")[1] ?? "");
  };
  image.src = `data:image/svg+xml;base64,${btoa(svg)}`;
});
const requests: unknown[] = [];
const imports: unknown[] = [];
const browser = {
  capture: async (request: BrowserRequest) => {
    requests.push(request);
    if (request.url.includes("offline")) throw Error("offline");
    return {
      png,
      width: request.width,
      height: request.height,
      dom: {
        version: 1 as const,
        width: request.width,
        height: request.height,
        notes: [],
        nodes: [
          {
            kind: "frame" as const,
            name: "HabitCard",
            selector: "#card",
            parent: null,
            x: 24,
            y: 150,
            width: 680,
            height: 120,
            style: {},
          },
        ],
      },
    };
  },
};
const controller = createBrowserImport({
  browser,
  importLayers: async (dom) => {
    imports.push(dom);
  },
  importScreenshot: async (capture) => {
    imports.push(capture);
  },
});
if (!q.has("empty")) {
  await controller.load();
  controller.select("#card");
}
Object.assign(window, { browserTest: { controller, requests, imports } });
function App() {
  const [node, setNode] = useState({ url: "http://localhost:3000", device: "phone", zoom: 0.8 });
  return q.has("embed") ? (
    <>
      <div style={{ position: "absolute", left: 560, top: 96 }}>
        <EmbedPreview node={node} width={300} height={600} browser={browser} />
      </div>
      <aside style={{ position: "absolute", left: 992, top: 56, width: 280 }}>
        <EmbedProperties
          node={node}
          onOpen={() => {}}
          onChange={(patch) => setNode({ ...node, ...patch, device: patch.device ?? node.device })}
        />
      </aside>
    </>
  ) : (
    <div style={{ position: "absolute", left: 448, top: 80 }}>
      <BrowserImportPanel controller={controller} />
    </div>
  );
}
const root = document.getElementById("root");
if (root) createRoot(root).render(<App />);
