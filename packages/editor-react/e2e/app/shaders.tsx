// TEST DATA: real shader controls over scripted host/compiler DTOs; no native runtime is simulated.
import "./fonts.css";
import "@vellum/ui-tokens/tokens.css";
import "../../src/editor.css";
import "../../src/design/design.css";
import "../../src/authoring/authoring.css";
import { createShaderController } from "@vellum/editor-core";
import type { Document, Paint } from "@vellum/types-ts";
import { useState } from "react";
import { createRoot } from "react-dom/client";
import { COLOURS } from "../../fixtures/design-scenarios";
import { PortalContainer } from "../../src/chrome/portal";
import { CanvasShaders } from "../../src/design/CanvasShaders";
import { ShaderControls } from "../../src/design/ShaderControls";
import { designFixture } from "../../src/design-fixture";

const params = new URLSearchParams(location.search);
document.documentElement.dataset.appearance = params.get("mode") ?? "light";
const shader = {
  src: "effects/grain.glsl",
  uniforms: { strength: 0.12, colour: { $var: "v2" }, speed: 0.5 },
};
const f = designFixture("c33", {
  change(doc) {
    (doc.pages[0]!.children![0] as any).fills = [{ type: "shader", ...shader }];
  },
});
const requests: string[] = [];
const c = createShaderController({
  inspect: async () =>
    params.has("error")
      ? { status: "error", kind: "invalid", message: "line 2: unknown value" }
      : {
          status: "ready",
          uniforms: [
            { name: "strength", kind: "number" },
            { name: "colour", label: "Colour", kind: "color" },
            { name: "speed", kind: "number" },
          ],
        },
  choose: async () => ({
    src: "effects/chosen.vsh",
    uniforms: { strength: 1, colour: COLOURS.primary, speed: 0 },
  }),
  importExample: async (id) => {
    requests.push(id);
    return { src: `effects/${id}.vsh`, uniforms: {} };
  },
  openExample: async (id) => {
    requests.push(id);
  },
});
const ctx = {
  ...f,
  model: f.design.model(),
  disabled: params.has("readonly"),
  host: { shaders: c },
  density: "dense" as const,
  numbers: [],
  colours: [],
  document: f.doc as Document,
  colour: (v: any) => f.design.colour(v),
  edit: f.design.edit,
};
Object.assign(window, { shaderTest: { engine: f.engine, requests } });
function App() {
  const [paint, setPaint] = useState(shader);
  return (
    <PortalContainer value={document.getElementById("root")}>
      <div style={{ width: 280, height: 736, background: "var(--chrome-panel)" }}>
        <ShaderControls
          ctx={ctx}
          shader={paint as any}
          apply={(v) => {
            f.engine.script({
              document: f.doc,
              revision: f.engine.snapshot().revision + 1,
              readOnly: false,
            });
            f.design.edit(() => ({ fills: [{ type: "shader", ...v }] as Paint[] }));
            setPaint(v as any);
          }}
        />
      </div>
      {params.has("error") ? <CanvasShaders controller={c} session={f.session} /> : null}
    </PortalContainer>
  );
}
createRoot(document.getElementById("root")!).render(
  <div className="vc-workspace">
    <App />
  </div>,
);
