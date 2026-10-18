import { copy } from "@vellum/config";
import type { AuthoringController, EditorSession } from "@vellum/editor-core";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import { ScriptDiagnostic } from "./ScriptSection";

export function CanvasAuthoring({
  authoring: a,
  session,
  readOnly,
  contentAvailable,
}: {
  authoring: AuthoringController;
  session: EditorSession;
  readOnly: boolean;
  contentAvailable: boolean;
}) {
  useStore(a.subscribe);
  useStore(session.subscribe);
  const v = session.state.viewport;
  const seen = new Set<string>();
  const walk = (id: string): string[] => {
    if (seen.has(id)) return [];
    seen.add(id);
    const i = session.sceneItem(id);
    return i ? [id, ...i.children.flatMap(walk)] : [];
  };
  return (
    <div className="vx-canvas-authoring">
      {session
        .roots()
        .flatMap(walk)
        .map((address) => {
          const item = session.sceneItem(address);
          if (!item) return null;
          const n = item.node;
          const box = {
            left: item.bounds.x * v.zoom + v.x,
            top: item.bounds.y * v.zoom + v.y,
            width: item.bounds.width * v.zoom,
            height: item.bounds.height * v.zoom,
          };
          if (n.type === "frame" && n.slot)
            return (
              <div key={address} className="vx-slot-marker" style={box}>
                <span>Slot · {n.name}</span>
              </div>
            );
          if (n.type !== "script") return null;
          const view = a.view(address, n);
          if (view.status === "ready" && contentAvailable)
            return (
              <span key={address} className="vx-script-accessible" role="status">
                {n.name} · Script output ready
              </span>
            );
          return (
            <div key={address} className="vx-script-node" data-state={view.status} style={box}>
              {view.status === "error" ? (
                <ScriptDiagnostic view={view} src={n.src} canvas />
              ) : view.status === "empty" ? (
                <>
                  <Icon name="code" size={24} />
                  <h2>{copy.scriptNodeEmpty}.</h2>
                  <button
                    type="button"
                    className="vx-button"
                    data-primary=""
                    disabled={readOnly || !a.host.chooseScript || a.state.busy}
                    onClick={() => void a.chooseScript(address)}
                  >
                    Choose a script file
                  </button>
                  <button
                    type="button"
                    className="vx-button"
                    disabled={readOnly || !a.host.askScript}
                    onClick={() => a.host.askScript?.(address, "write")}
                  >
                    Ask the agent
                  </button>
                </>
              ) : (
                <>
                  <Icon name="code" size={24} />
                  <h2>
                    {view.status === "loading"
                      ? "Running script…"
                      : view.status === "ready"
                        ? "Script preview unavailable"
                        : "Script runtime unavailable"}
                  </h2>
                </>
              )}
            </div>
          );
        })}
    </div>
  );
}
