// TEST DATA: builds the properties panel's controller over the Juniper scenarios, for the stories and the unit tests.
// The engine is scripted and the scene is a read-only view of the document: nothing here applies an operation.
import {
  createDesignController,
  createEditorSession,
  createPaintEditor,
  type SceneItem,
} from "@vellum/editor-core";
import { type EngineSnapshot, FakeEngineClient, type ToolScene } from "@vellum/engine-client";
import type { Document, Node } from "@vellum/types-ts";
import {
  DESIGN_DEFAULTS,
  type DesignScenario,
  designDocument,
  designSelection,
} from "../fixtures/design-scenarios";

export function designFixture(
  scenario: DesignScenario,
  options: {
    change?: (doc: Document) => void;
    selection?: string[];
    readOnly?: boolean;
    fonts?: readonly string[];
  } = {},
) {
  const doc = designDocument(scenario);
  options.change?.(doc);
  const items = new Map<string, SceneItem>();
  const walk = (list: Node[], parent: string | null) => {
    for (const n of list) {
      const kids = (n as unknown as { children?: Node[] }).children ?? [];
      const test = (
        n as unknown as { metadata?: { testBounds?: { width: number; height: number } } }
      ).metadata?.testBounds;
      const w = (n as unknown as { width: unknown }).width;
      const h = (n as unknown as { height: unknown }).height;
      const position = (n as unknown as { position?: { x?: number; y?: number } }).position;
      items.set(n.id, {
        node: n,
        bounds: {
          x: position?.x ?? 0,
          y: position?.y ?? 0,
          width: typeof w === "number" ? w : (test?.width ?? 0),
          height: typeof h === "number" ? h : (test?.height ?? 0),
        },
        children: kids.map((k) => k.id),
        parent,
      });
      walk(kids, n.id);
    }
  };
  walk(doc.pages[0]?.children ?? [], null);
  const scene: ToolScene = {
    get: (id) => items.get(id),
    roots: () => [...items.values()].filter((i) => i.parent === null).map((i) => i.node.id),
    hitPoint: () => null,
    hitRect: () => [],
    snap: () => ({ offset: { x: 0, y: 0 }, guides: [] }),
    toParent: (_a, p) => p,
    toLocal: (_a, p) => p,
    path: () => null,
  };
  const snapshot: EngineSnapshot = {
    revision: 0,
    document: doc,
    readOnly: options.readOnly ?? false,
  };
  const engine = new FakeEngineClient(snapshot, scene);
  const session = createEditorSession({
    engine,
    device: "story-device",
    page: "p1",
    nextNodeId: () => "n",
    createNode: () => ({}) as Node,
    viewportSize: () => ({ width: 800, height: 600 }),
  });
  session.select(options.selection ?? [designSelection(scenario)]);
  const fonts = options.fonts ?? ["Inter", "JetBrains Mono"];
  const design = createDesignController({
    session,
    engine,
    defaults: DESIGN_DEFAULTS,
    text: { available: (family) => fonts.includes(family), fallback: "Inter" },
  });
  const paintEditor = createPaintEditor({ session, design });
  return {
    doc,
    engine,
    session,
    design,
    paintEditor,
    readOnly: options.readOnly ?? false,
    fonts,
  };
}
