import type { EngineClient, EngineCommit } from "@vellum/engine-client";
import type { Document, Value } from "@vellum/types-ts";
import { frameTheme } from "../resources/variables";
import type { EditorSession } from "../session";
import type { ToolOp } from "../tool-types";
import { isVarRef, type Mode, resolveVariable } from "./bindings";
import { type DesignModel, type DesignTarget, readDesign } from "./model";
import { editTransaction, personTransaction } from "./patch";
import type { Edit } from "./patches";
import type { TextContext } from "./text";

/** The colours new paints start with. The editor invents none: the host supplies them from its tokens. */
export interface DesignDefaults {
  fill: string;
  stroke: string;
  shadow: string;
}
export interface DesignDeps {
  session: EditorSession;
  engine: EngineClient;
  defaults: DesignDefaults;
  text: TextContext;
}

const IN_FLOW = new Set(["row", "column", "grid"]);

/**
 * The properties panel's side of the editor: the selected layers as design targets, the model the panel shows,
 * and the one way it changes the document, a Person transaction through the session.
 */
export function createDesignController(deps: DesignDeps) {
  const { session, engine } = deps;
  let mode: Mode | null = null;
  let request: "replace" | null = null;
  const listeners = new Set<() => void>();
  const document = (): Document | null => engine.snapshot().document;

  const targetsFor = (): DesignTarget[] =>
    session.state.selection.flatMap((address) => {
      const item = session.sceneItem(address);
      if (!item) return [];
      const parent = item.parent ? session.sceneItem(item.parent) : null;
      const flow =
        parent?.node.type === "frame" &&
        IN_FLOW.has(String((parent.node as { layout?: string }).layout));
      const position = (item.node as { position?: { type?: string } }).position;
      // A text layer open for typing is read as the draft, so the panel shows what is on the canvas.
      const mode = session.state.mode;
      const shown =
        mode.type === "editing" && mode.address === address ? { ...item, node: mode.node } : item;
      return [{ item: shown, inLayout: !!flow && position?.type !== "absolute" }];
    });

  const controller = {
    defaults: deps.defaults,
    targets: targetsFor,
    /** The one selected layer, for what only makes sense for one: its paints on the canvas, its component. */
    single(): DesignTarget | null {
      const all = targetsFor();
      return all.length === 1 ? (all[0] as DesignTarget) : null;
    },
    editing: () => session.state.mode.type === "editing",
    /** The theme the colours are shown under. */
    mode: (doc: Document | null = document()): Mode =>
      mode ?? frameTheme(doc, session.state.selection[0] ?? null, (id) => session.sceneItem(id)),
    setMode(next: Mode | null) {
      mode = next;
    },
    colour(value: Value<string>, doc: Document | null = document()): string {
      if (!isVarRef(value)) return value;
      return (
        (resolveVariable(doc, value.$var, controller.mode(doc)) as string | undefined) ??
        "#00000000"
      );
    },
    model(doc: Document | null = document()): DesignModel {
      return readDesign({
        targets: targetsFor(),
        document: doc,
        mode: controller.mode(doc),
        readOnly: engine.snapshot().readOnly,
        text: deps.text,
        editing: controller.editing(),
      });
    },
    /** Applies an edit to every selected layer it fits, as one transaction. Null when it changed nothing. */
    edit(edit: Edit): EngineCommit | null {
      const targets = targetsFor();
      const byItem = new Map(targets.map((t) => [t.item, t]));
      const tx = editTransaction(
        session.device,
        targets.map((t) => t.item),
        (item) => edit(byItem.get(item) as DesignTarget),
      );
      return tx ? session.commit(tx) : null;
    },
    /** Requests that are not property patches: detach, component updates. */
    ops(ops: ToolOp[]): EngineCommit | null {
      const tx = personTransaction(session.device, ops);
      return tx ? session.commit(tx) : null;
    },
    subscribe(listener: () => void): { dispose(): void } {
      listeners.add(listener);
      const inner = session.subscribe(listener);
      return {
        dispose() {
          listeners.delete(listener);
          inner.dispose();
        },
      };
    },
    /** A command (Replace inside instance) asks the panel to open its picker; the panel takes the request. */
    requestReplace() {
      request = "replace";
      for (const l of [...listeners]) l();
    },
    takeRequest(): "replace" | null {
      const r = request;
      request = null;
      return r;
    },
  };
  return controller;
}
export type DesignController = ReturnType<typeof createDesignController>;
