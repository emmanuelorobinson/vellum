/** The selection as the composer's chip and the inline prompt's scope name it ("3 layers in Step 2"). */

interface Layer {
  id: string;
  name?: string;
  type?: string;
  children?: readonly unknown[];
}
interface Pages {
  pages: readonly { id: string; children?: readonly unknown[] }[];
}

/** One layer by name; several in one top-level frame "n layers in Frame"; top-level frames "n frames". */
export function selectionLabel(doc: Pages, page: string, ids: readonly string[]): string | null {
  if (!ids.length) return null;
  const roots = (doc.pages.find((p) => p.id === page)?.children ?? []) as Layer[];
  const found = new Map<string, { layer: Layer; root: Layer }>();
  const walk = (layer: Layer, root: Layer) => {
    found.set(layer.id, { layer, root });
    for (const child of (layer.children ?? []) as Layer[]) walk(child, root);
  };
  for (const root of roots) walk(root, root);
  const hits = ids.flatMap((id) => {
    const f = found.get(id);
    return f ? [f] : [];
  });
  if (ids.length === 1) return hits[0]?.layer.name ?? "1 layer";
  const n = ids.length;
  if (hits.length === n && hits.every((h) => h.layer === h.root && h.layer.type === "frame"))
    return `${n} frames`;
  const root = hits[0]?.root;
  if (hits.length === n && root && hits.every((h) => h.root === root && h.layer !== root))
    return `${n} layers in ${root.name ?? "a frame"}`;
  return `${n} layers`;
}
