import type { DeviceId, Node, NodePatch } from "@vellum/types-ts";
import type { SceneItem, ToolOp, ToolTransaction } from "../tool-types";

const PROTECTED = new Set(["id", "type", "children"]);

/** A copy of `node` with `patch` applied the way the core applies it: `null` puts a property back to its default. */
export function applyPatch(node: Node, patch: NodePatch): Node {
  const out = { ...node } as Record<string, unknown>;
  for (const [key, value] of Object.entries(patch)) {
    if (PROTECTED.has(key)) continue;
    if (value === null || value === undefined) delete out[key];
    else out[key] = value;
  }
  return out as unknown as Node;
}

/**
 * One op per layer the patch applies to. A part of an instance is changed through the instance (an override),
 * never by writing the component's own tree.
 */
export function patchOps(
  items: readonly SceneItem[],
  patchFor: (item: SceneItem) => NodePatch | null,
): ToolOp[] {
  const ops: ToolOp[] = [];
  for (const item of items) {
    const patch = patchFor(item);
    if (!patch || Object.keys(patch).length === 0) continue;
    ops.push(
      item.instance
        ? { type: "replaceInInstance", ...item.instance, node: applyPatch(item.node, patch) }
        : { type: "update", id: item.node.id, patch },
    );
  }
  return ops;
}

export function personTransaction(device: DeviceId, ops: ToolOp[]): ToolTransaction | null {
  return ops.length ? { origin: { actor: { kind: "person", device } }, ops } : null;
}

/** The whole selection in one transaction, so one edit is one undo step. */
export function editTransaction(
  device: DeviceId,
  items: readonly SceneItem[],
  patchFor: (item: SceneItem) => NodePatch | null,
): ToolTransaction | null {
  return personTransaction(device, patchOps(items, patchFor));
}
