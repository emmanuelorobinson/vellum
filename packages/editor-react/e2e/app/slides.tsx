// TEST DATA. Real workspace over scripted snapshots; PNG service is a recorded renderer boundary.
import "./fonts.css";
import "@vellum/ui-tokens/tokens.css";
import {
  createCommandController,
  createEditorHandlers,
  createEditorSession,
  createWorkspace,
} from "@vellum/editor-core";
import { FakeEngineClient } from "@vellum/engine-client";
import type { Document, Node } from "@vellum/types-ts";
import { createRoot } from "react-dom/client";
import { EditorWorkspace } from "../../src/EditorWorkspace";
import { sceneOf } from "./test-scene";

const params = new URLSearchParams(location.search);
const mode = params.get("mode") === "dark" ? "dark" : "light";
const names = [
  "The Juniper idea",
  "Small habits",
  "Your weekly progress",
  "A daily rhythm",
  "Next steps",
];
const doc: Document = {
  meta: { formatVersion: 1, name: "TEST DATA", createdWith: "fixture" },
  pages: [
    {
      id: "page000001",
      kind: "slides",
      name: "Slides",
      children: names.map((name, i) => ({
        type: "frame",
        id: `slide0000${i + 1}`,
        name,
        width: 800,
        height: 450,
        position: { type: "absolute", x: i * 1000, y: 0 },
      })),
    },
  ],
};
const engine: FakeEngineClient = new FakeEngineClient(
  { document: doc, revision: 0, readOnly: params.has("readonly") },
  sceneOf(() => engine.snapshot().document),
);
const session = createEditorSession({
  engine,
  device: "device0001",
  page: "page000001",
  nextNodeId: () => "node000001",
  createNode: () => ({}) as Node,
  viewportSize: () => ({ width: 1280, height: 800 }),
});
session.select(params.has("noselection") ? [] : ["slide00003"]);
const workspace = createWorkspace();
workspace.resize({ width: 1280, height: 800 });
workspace.switchTab("layers");
const controller = createCommandController(createEditorHandlers({ workspace, session }));
const exports: { ids: readonly string[]; revision: number }[] = [];
const requests: string[] = [];
// A transparent one-pixel PNG: presentation tests verify the renderer contract and isolation, not content fidelity.
const pixels =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg==";
const pending = new Map<string, (url: string) => void>();
const slidesHost = params.has("unavailable")
  ? undefined
  : {
      image: async (id: string) => {
        requests.push(id);
        if (params.has("slow")) return new Promise<string>((resolve) => pending.set(id, resolve));
        return pixels;
      },
      pdf: async (ids: readonly string[], revision: number) => {
        exports.push({ ids, revision });
      },
    };
Object.assign(window, {
  slidesTest: { engine, session, workspace, exports, requests, pending, pixels, doc },
});
const nextNodeId = () => "node000002";
createRoot(document.getElementById("root")!).render(
  <EditorWorkspace
    engine={engine}
    session={session}
    workspace={workspace}
    controller={controller}
    device="device0001"
    host="browser"
    platform="mac"
    appearance={mode}
    fileName="juniper.vellum"
    saveState="saved"
    branch="feat/onboarding"
    connection={{ mode: "headless" }}
    agents={[]}
    actors={{}}
    recent={{}}
    now={() => 0}
    nextNodeId={nextNodeId}
    nextPageId={() => "page000002"}
    assets={{
      libraries: [],
      iconSets: [],
      projectIconFolder: null,
      visualStyles: [],
      designSystems: [],
    }}
    {...(slidesHost ? { slidesHost } : {})}
  />,
);
