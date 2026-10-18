/** Collaboration nodes use the same document Ops and composer request boundary as other editor actions. */

import type { ToolTransaction } from "@vellum/engine-client";
import type { DeviceId, NodeId } from "@vellum/types-ts";
import type { EngineChoice } from "./engines";
import type { ContextChip, PromptRequest } from "./feed";

interface Layer {
  id: string;
  type?: string;
  name?: string;
  content?: string;
  pointsAt?: string;
  children?: readonly unknown[];
}
interface Pages {
  pages: readonly { id: string; children?: readonly unknown[] }[];
}
/** Explicit pointsAt and enclosing-frame scope mirror the context model. No guessed proximity radius. */
export function contextForSelection(
  doc: Pages,
  page: string,
  selection: readonly string[],
): ContextChip[] {
  if (!selection.length) return [];
  const nodes = new Map<string, { node: Layer; ancestors: string[] }>();
  const visit = (items: readonly unknown[], ancestors: string[]) => {
    for (const item of items) {
      const n = item as Layer;
      nodes.set(n.id, { node: n, ancestors });
      visit(n.children ?? [], [...ancestors, n.id]);
    }
  };
  visit(doc.pages.find((p) => p.id === page)?.children ?? [], []);
  const scope = new Set(selection.flatMap((id) => [id, ...(nodes.get(id)?.ancestors ?? [])]));
  return [...nodes.values()]
    .filter(
      ({ node, ancestors }) =>
        node.type === "context" &&
        (node.pointsAt ? scope.has(node.pointsAt) : ancestors.some((id) => scope.has(id))),
    )
    .map(({ node }) => ({
      kind: "context",
      label: `${node.name ?? "Context"} · automatic`,
      value: node.id,
    }));
}
export function collaborationTransaction(
  device: DeviceId,
  id: NodeId,
  content: string,
): ToolTransaction {
  return {
    origin: { actor: { kind: "person", device } },
    ops: [{ type: "update", id, patch: { content } }],
  };
}
export function promptNodeRequest(
  node: { content: string; engine?: string | null; model?: string | null },
  fallback: EngineChoice | null,
  context: ContextChip[],
): PromptRequest {
  const engine: EngineChoice | null = node.engine
    ? {
        engine: node.engine,
        ...(node.engine === "builtin" && fallback?.provider ? { provider: fallback.provider } : {}),
        model: node.model ?? fallback?.model ?? "",
      }
    : node.model
      ? { ...fallback, engine: fallback?.engine ?? "builtin", model: node.model }
      : fallback;
  return {
    text: node.content,
    context,
    skills: [],
    parallel: { count: 1, mode: "split" },
    explore: null,
    engine,
    queue: null,
  };
}
