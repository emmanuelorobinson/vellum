// TEST DATA: the editor over the real Rust engine in the editor WASM module (ADR-033). Nothing scripted: every
// edit goes through the engine's write path and the canvas draws the engine's display list.
import "./fonts.css";
import "@vellum/ui-tokens/tokens.css";
import "../../src/editor.css";
import "../../src/design/design.css";
import { createDefaultNode } from "@vellum/editor-core";
import { isEngineModule, WasmDocumentEngine } from "@vellum/renderer-skia";
import createEditor from "@vellum/renderer-skia/editor-module";
import { createRoot } from "react-dom/client";
import { ENGINE_DOCUMENT as DOC } from "../../fixtures/engine-document";
import { EditorWorkspace } from "../../src/EditorWorkspace";
import { createLiveEditor } from "../../src/live-editor";

const DEVICE = "01JA0000000000000000000A11";
const module = await createEditor({ canvas: document.createElement("canvas"), noInitialRun: true });
if (!isEngineModule(module)) throw Error("The editor module has no document engine");
const inter = new URL("../../../../crates/adapters/text-parley/fonts/InterVariable.ttf", import.meta.url);
const font = new Uint8Array(await (await fetch(inter)).arrayBuffer());
const engine = WasmDocumentEngine.open(module, {
  file: new TextEncoder().encode(JSON.stringify(DOC)),
  path: "/TEST DATA/test.vellum",
  readOnly: new URLSearchParams(location.search).has("readonly"),
  fonts: [font],
});
const live = createLiveEditor({ module, engine, device: DEVICE });
live.workspace.resize({ width: innerWidth, height: innerHeight });
Object.assign(window, {
  __engine: {
    snapshot: () => engine.snapshot(),
    get: (address: string) => engine.scene.get(address),
    encode: () => new TextDecoder().decode(engine.encode()),
    /** Inserts one layer of every creation kind through the engine, as a tool would. */
    insertEveryKind: () =>
      (
        ["frame", "rect", "ellipse", "line", "polygon", "star", "text", "note", "vector"] as const
      ).map(
        (kind) =>
          live.client.apply({
            origin: { actor: { kind: "person", device: DEVICE } },
            ops: [
              {
                type: "insert",
                to: { parent: { type: "page", id: "page000001" }, index: 1 },
                node: createDefaultNode(kind, engine.nextId(), {
                  x: 0,
                  y: 0,
                  width: 40,
                  height: 40,
                }),
              },
            ],
          }).status,
      ),
  },
});
createRoot(document.getElementById("root") as HTMLElement).render(
  <EditorWorkspace
    workspace={live.workspace}
    session={live.session}
    controller={live.controller}
    engine={live.client}
    design={live.design}
    paintEditor={live.paintEditor}
    content={live.content}
    createRenderer={live.createRenderer}
    device={DEVICE}
    host="browser"
    platform="mac"
    appearance="light"
    fileName="test.vellum"
    saveState="saved"
    branch={null}
    connection={{ mode: "offline" }}
    agents={[]}
    actors={{}}
    recent={{}}
    now={() => Date.now()}
    nextNodeId={live.nextNodeId}
    assets={{
      libraries: [],
      iconSets: [],
      projectIconFolder: null,
      visualStyles: [],
      designSystems: [],
    }}
  />,
);
