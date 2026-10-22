/**
 * The editor over the Rust engine in the editor WASM module (ADR-033): what every host composes before it
 * mounts `EditorWorkspace`. The engine owns the document; this wires the session, commands, properties panel
 * and canvas to it. Host-specific services (daemon sync, files, agents) are added by the host.
 */
import {
  type AgentFeed,
  createAgentController,
  createCommandController,
  createDefaultNode,
  createDesignController,
  createEditorHandlers,
  createEditorSession,
  createPaintEditor,
  createWorkspace,
  DOCUMENT_DEFAULTS,
} from "@vellum/editor-core";
import { WasmEngineClient } from "@vellum/engine-client";
import {
  createRendererWith,
  type EngineModule,
  type WasmDocumentEngine,
} from "@vellum/renderer-skia";
import type { DeviceId, PageId } from "@vellum/types-ts";
import type { RendererFactory } from "./canvas/CanvasHost";

export interface LiveEditorOptions {
  module: EngineModule;
  engine: WasmDocumentEngine;
  device: DeviceId;
  /** The families the font picker offers (the engine has their faces registered). */
  fonts?: readonly string[];
  viewportSize?: () => { width: number; height: number };
  pauseAll?: () => void;
  /** The Agent tab's sessions (the daemon's session store); without it the tab is the host's slot. */
  agentFeed?: AgentFeed;
  /** The person's preferences, for the canvas shortcuts that change them (pixel grid). */
  preferences?: Parameters<typeof createEditorHandlers>[0]["preferences"];
}

export function createLiveEditor(options: LiveEditorOptions) {
  const { engine, module, device } = options;
  const client = new WasmEngineClient(engine);
  const first = client.snapshot().document?.pages[0]?.id ?? ("page" as PageId);
  engine.setPage(first);
  const session = createEditorSession({
    engine: client,
    device,
    page: first,
    nextNodeId: () => engine.nextId(),
    createNode: createDefaultNode,
    viewportSize:
      options.viewportSize ??
      (() => ({ width: globalThis.innerWidth, height: globalThis.innerHeight })),
  });
  const fonts = options.fonts ?? ["Inter"];
  const design = createDesignController({
    session,
    engine: client,
    defaults: DOCUMENT_DEFAULTS,
    text: { available: (family) => fonts.includes(family), fallback: "Inter" },
  });
  const paintEditor = createPaintEditor({ session, design });
  const workspace = createWorkspace();
  const controller = createCommandController(
    createEditorHandlers({
      workspace,
      session,
      design,
      pauseAll: options.pauseAll,
      preferences: options.preferences,
    }),
  );
  const agent = options.agentFeed
    ? createAgentController({
        feed: options.agentFeed,
        now: () => Date.now(),
        onZoomTo: (node) => {
          if (client.scene.get(node)) session.select([node]);
        },
        nameOf: (node) => client.scene.get(node)?.node.name ?? null,
      })
    : undefined;
  void agent?.load();
  const createRenderer: RendererFactory = (canvas) =>
    createRendererWith(canvas, module, {
      width: Math.max(canvas.clientWidth, 1),
      height: Math.max(canvas.clientHeight, 1),
      dpr: globalThis.devicePixelRatio || 1,
    });
  return {
    device,
    client,
    session,
    workspace,
    controller,
    design,
    paintEditor,
    agent,
    content: engine.content,
    createRenderer,
    nextNodeId: () => engine.nextId(),
    /** Another page: the engine's scene and the session follow it together. */
    setPage(page: PageId) {
      engine.setPage(page);
      session.setPage(page);
    },
    dispose() {
      client.dispose();
    },
  };
}
export type LiveEditor = ReturnType<typeof createLiveEditor>;
