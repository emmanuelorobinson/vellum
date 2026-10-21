import type { EditorSession } from "@vellum/editor-core";
import type { BrowserPort } from "@vellum/host-api";
import { useStore } from "../hooks";
import { EmbedPreview } from "./BrowserImport";
/** Screenshot fallback for live web content; vector canvas input stays with the existing session. */
export function CanvasEmbeds({
  session,
  browser,
  open,
}: {
  session: EditorSession;
  browser: BrowserPort;
  open?: { address: string; revision: number };
}) {
  useStore(session.subscribe);
  const viewport = session.state.viewport;
  const seen = new Set<string>();
  const walk = (address: string): string[] => {
    if (seen.has(address)) return [];
    seen.add(address);
    const item = session.sceneItem(address);
    return item ? [address, ...item.children.flatMap(walk)] : [];
  };
  return (
    <div className="vb-canvas-embeds">
      {session
        .roots()
        .flatMap(walk)
        .map((address) => {
          const item = session.sceneItem(address);
          if (item?.node.type !== "embed") return null;
          return (
            <div
              key={address}
              style={{
                position: "absolute",
                left: item.bounds.x * viewport.zoom + viewport.x,
                top: item.bounds.y * viewport.zoom + viewport.y,
                transform: `scale(${viewport.zoom})`,
                transformOrigin: "top left",
                pointerEvents: "auto",
              }}
            >
              <EmbedPreview
                node={item.node}
                width={item.bounds.width}
                height={item.bounds.height}
                browser={browser}
                openRequest={open?.address === address ? open.revision : 0}
              />
            </div>
          );
        })}
    </div>
  );
}
