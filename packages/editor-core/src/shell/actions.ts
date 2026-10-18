/**
 * What the palette and the context menu do once something is chosen: the entries the palette searches, the commands
 * it lists, and the editor actions behind "Ask the agent", "Ask about this…" and "Redesign this…". Nothing here
 * writes the document: it selects, navigates and puts words in the composer.
 */
import type { Document, Node } from "@vellum/types-ts";
import type { ComposerController } from "../agent/composer";
import type { SessionInfo } from "../agent/sessions";
import { sessionTitle } from "../agent/sessions";
import type { Command, Registry } from "../registries";
import type { ResourceController } from "../resources/controller";
import type { EditorSession } from "../session";
import { fitViewport } from "../viewport";
import type { Workspace } from "../workspace";
import type { PaletteCommand, PaletteEntry } from "./palette";

/** Layers, pages, components, variables and sessions of the open document, for the palette to search. */
export function paletteEntries(
  doc: Document | null,
  sessions: readonly SessionInfo[] = [],
): PaletteEntry[] {
  const out: PaletteEntry[] = [];
  if (doc) {
    for (const page of doc.pages) {
      out.push({ kind: "page", id: page.id, label: page.name });
      const walk = (nodes: readonly Node[]) => {
        for (const n of nodes) {
          out.push({ kind: "layer", id: n.id, label: n.name ?? n.id });
          walk((n as { children?: Node[] }).children ?? []);
        }
      };
      walk(page.children ?? []);
    }
    for (const [id, c] of Object.entries(doc.components ?? {}))
      out.push({ kind: "component", id, label: c.name });
    for (const [id, v] of Object.entries(doc.variables ?? {}))
      out.push({ kind: "variable", id, label: v.name });
  }
  for (const s of sessions) out.push({ kind: "session", id: s.id, label: sessionTitle(s) });
  return out;
}

/** The registry as the palette lists it: each command with whether it can run right now. */
export function paletteCommands(commands: Registry<Command>): PaletteCommand[] {
  return commands.list().map((c) => ({
    id: c.id,
    title: c.title,
    category: c.category,
    enabled: c.enabled ? c.enabled() : true,
  }));
}

export interface ShellActionDeps {
  workspace: Workspace;
  session: EditorSession;
  document(): Document | null;
  composer?: ComposerController | undefined;
  /** Selects a session in the Agent tab. */
  selectSession?(id: string): void;
  resources?: ResourceController | undefined;
}

export function createShellActions(deps: ShellActionDeps) {
  const { workspace, session, composer } = deps;
  const toAgent = () => workspace.switchTab("agent", { reveal: true });
  return {
    /** Ask the agent: the words go to the composer for the person to read and send. */
    ask(text: string) {
      toAgent();
      composer?.prefill(text);
    },
    /** Ask about this…: the selection is already the composer's context, so the field just takes focus. */
    askAbout() {
      toAgent();
      composer?.requestFocus();
    },
    /** Redesign this…: starts the request; the selection travels as the context chip. */
    redesign() {
      toAgent();
      composer?.prefill("Redesign ");
    },
    open(entry: PaletteEntry) {
      if (entry.kind === "layer") {
        session.select([entry.id]);
        const box = session.sceneBounds(entry.id);
        if (box)
          session.setViewport(
            fitViewport(box, workspace.state, workspace.layout().canvasInsets, { maxZoom: 1 }),
          );
      } else if (entry.kind === "page") session.setPage(entry.id);
      else if (entry.kind === "component") workspace.switchTab("assets", { reveal: true });
      else if (entry.kind === "variable") deps.resources?.open("variables");
      else if (entry.kind === "session") {
        deps.selectSession?.(entry.id);
        toAgent();
      }
    },
  };
}
export type ShellActions = ReturnType<typeof createShellActions>;
