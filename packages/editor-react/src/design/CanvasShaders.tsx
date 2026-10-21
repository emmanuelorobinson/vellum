import type { EditorSession, ShaderController } from "@vellum/editor-core";
import { useEffect } from "react";
import { useStore } from "../hooks";
import { ShaderError } from "./ShaderControls";
export function CanvasShaders({
  session,
  controller,
}: {
  session: EditorSession;
  controller: ShaderController;
}) {
  useStore(session.subscribe);
  useStore(controller.subscribe);
  const seen = new Set<string>();
  const nodes = session.roots().flatMap(function walk(id): string[] {
    const item = session.sceneItem(id);
    if (!item || item.node.visible === false || seen.has(id)) return [];
    seen.add(id);
    return [id, ...item.children.flatMap(walk)];
  });
  const sources = nodes.flatMap((id) => {
    const n = session.sceneItem(id)?.node as unknown as {
      fills?: { type: string; src?: string; visible?: boolean }[];
      stroke?: { paints?: { type: string; src?: string; visible?: boolean }[] };
      effects?: { type: string; src?: string; visible?: boolean }[];
    };
    return [...(n.fills ?? []), ...(n.stroke?.paints ?? []), ...(n.effects ?? [])]
      .filter((p) => p.type === "shader" && p.visible !== false && p.src)
      .map((p) => ({ id, src: p.src! }));
  });
  const unique = [...new Map(sources.map((s) => [`${s.id}:${s.src}`, s])).values()];
  const key = [...new Set(unique.map((s) => s.src))].sort().join("\n");
  useEffect(() => {
    for (const src of key.split("\n").filter(Boolean)) void controller.refresh(src);
  }, [controller, key]);
  const v = session.state.viewport;
  return (
    <div className="vc-canvas-shaders">
      {unique.map(({ id, src }) => {
        const item = session.sceneItem(id);
        const view = controller.view(src);
        if (!item || view.status !== "error") return null;
        return (
          <section
            key={`${id}:${src}`}
            className="vc-shader-node-error"
            style={{
              left: item.bounds.x * v.zoom + v.x,
              top: item.bounds.y * v.zoom + v.y,
              width: Math.max(180, item.bounds.width * v.zoom),
            }}
            aria-label={`${item.node.name} shader error`}
          >
            <ShaderError view={view} />
            <span className="vc-mono">{src}</span>
          </section>
        );
      })}
    </div>
  );
}
