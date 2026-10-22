import {
  type AgentController,
  type ComposerController,
  collaborationTransaction,
  contextForSelection,
  type EditorSession,
  promptNodeRequest,
} from "@vellum/editor-core";
import type { Document, Node } from "@vellum/types-ts";
import { type PointerEvent, useState } from "react";
import { useLocale } from "../accessibility/locale";
import { ActorMarker } from "../chrome/ActorMarker";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import "./collaboration.css";

type CollaborationNode = Extract<Node, { type: "note" | "context" | "prompt" }>;
export function CollaborationNodes({
  document,
  session,
  agent,
  composer,
  readOnly,
  offline = false,
}: {
  document: Document | null;
  session: EditorSession;
  agent?: AgentController;
  composer?: ComposerController;
  readOnly: boolean;
  offline?: boolean;
}) {
  useStore(session.subscribe);
  useStore(agent?.subscribe ?? (() => ({ dispose() {} })));
  const found: CollaborationNode[] = [];
  const walk = (nodes: Node[]) => {
    for (const node of nodes) {
      if (node.type === "note" || node.type === "context" || node.type === "prompt")
        found.push(node);
      if ("children" in node) walk(node.children ?? []);
    }
  };
  walk(document?.pages.find((p) => p.id === session.page)?.children ?? []);
  return (
    <section className="vn-layer" aria-label="Canvas notes, context and prompts">
      {found.map((node) => (
        <CollaborationCard
          key={node.id}
          node={node}
          session={session}
          document={document!}
          {...(agent ? { agent } : {})}
          {...(composer ? { composer } : {})}
          readOnly={readOnly}
          offline={offline}
        />
      ))}
    </section>
  );
}
function CollaborationCard({
  node,
  session,
  document,
  agent,
  composer,
  readOnly,
  offline,
}: {
  node: CollaborationNode;
  session: EditorSession;
  document: Document;
  agent?: AgentController;
  composer?: ComposerController;
  readOnly: boolean;
  offline: boolean;
}) {
  const { t: translateUi } = useLocale();
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(node.content);
  const [error, setError] = useState<string | null>(null);
  const b = session.sceneBounds(node.id);
  if (!b) return null;
  const v = session.state.viewport;
  const type = node.type;
  const last = agent?.state.current
    ? agent.state.lastRuns[`${agent.state.current}:${node.id}`]
    : undefined;
  const pointer = (e: PointerEvent<HTMLButtonElement>) => {
    const root = e.currentTarget.closest(".vc-workspace")?.getBoundingClientRect();
    return {
      point: { x: e.clientX - (root?.left ?? 0), y: e.clientY - (root?.top ?? 0) },
      command: e.metaKey || e.ctrlKey,
      shift: e.shiftKey,
      alt: e.altKey,
      canvasFocus: true,
    };
  };
  return (
    <section
      className="vn-card"
      data-kind={type}
      aria-label={`${type}: ${node.name ?? type}`}
      style={{
        left: b.x * v.zoom + v.x,
        top: b.y * v.zoom + v.y,
        width: b.width,
        minHeight: b.height,
        transform: `scale(${v.zoom})`,
        transformOrigin: "top left",
      }}
    >
      <header>
        <button
          type="button"
          className="vn-move"
          aria-label={`Select or move ${type}`}
          onClick={() => session.select([node.id])}
          onPointerDown={(e) => {
            if (e.button !== 0) return;
            session.select([node.id]);
            if (readOnly) return;
            e.currentTarget.setPointerCapture(e.pointerId);
            session.dispatch({ type: "pointerDown", ...pointer(e) });
          }}
          onPointerMove={(e) => {
            if (e.currentTarget.hasPointerCapture(e.pointerId))
              session.dispatch({ type: "pointerMove", ...pointer(e) });
          }}
          onPointerUp={(e) => {
            if (e.currentTarget.hasPointerCapture(e.pointerId)) {
              session.dispatch({ type: "pointerUp", ...pointer(e) });
              e.currentTarget.releasePointerCapture(e.pointerId);
            }
          }}
          onPointerCancel={() => session.dispatch({ type: "cancel" })}
        >
          <Icon
            name={
              type === "note"
                ? "sticky-note"
                : type === "context"
                  ? "scroll-text"
                  : "message-square-more"
            }
            size={16}
          />
          <strong>
            {type === "note"
              ? (node.name ?? "For people")
              : type === "context"
                ? "Read by agents"
                : (node.name ?? "Saved request")}
          </strong>
          {type === "context" ? (
            <ActorMarker
              actor={{ id: node.id, kind: "agent", name: "Agent context", hue: "teal" }}
              size={16}
            />
          ) : null}
        </button>
      </header>
      {editing ? (
        <>
          <label className="vn-edit">
            {type[0]!.toUpperCase() + type.slice(1)} content
            <textarea
              aria-label={`${type[0]!.toUpperCase() + type.slice(1)} content`}
              value={text}
              onChange={(e) => setText(e.target.value)}
            />
          </label>
          <div className="va-actions">
            <button
              type="button"
              className="va-button"
              onClick={() => {
                const result = session.commit(
                  collaborationTransaction(session.device, node.id, text),
                );
                if (result.status === "applied") {
                  setEditing(false);
                  setError(null);
                } else setError("The editor could not save this change");
              }}
            >
              Save {type}
            </button>
            <button type="button" className="va-link" onClick={() => setEditing(false)}>
              {translateUi("Cancel")}
            </button>
          </div>
        </>
      ) : (
        <p className="vn-content">{node.content}</p>
      )}
      {!editing ? (
        <button
          type="button"
          className="vn-edit-button"
          aria-label={`Edit ${type}`}
          disabled={readOnly}
          onClick={() => {
            setText(node.content);
            setEditing(true);
          }}
        >
          <Icon name="pencil" size={12} />
        </button>
      ) : null}
      {type === "context" ? (
        <p className="vn-secondary">Attached automatically when a nearby frame is selected.</p>
      ) : null}
      {type === "prompt" ? (
        <>
          <p className="vn-secondary vc-mono">
            {node.engine === "builtin"
              ? (composer?.state.catalog?.builtin.find(
                  (p) => p.id === composer.state.choice?.provider,
                )?.name ?? "Built-in")
              : (node.engine ?? composer?.state.choice?.engine ?? "No engine selected")}
            {node.model ? ` · ${node.model.split("/").at(-1)}` : ""}
          </p>
          <p className="vn-secondary vn-last-run">
            {last
              ? `Last run · ${new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit", ...(agent?.timeOptions().timeZone ? { timeZone: agent.timeOptions().timeZone } : {}) }).format(new Date(last))}`
              : "Not run in this session"}
          </p>
          <button
            type="button"
            className="va-button"
            data-primary=""
            disabled={
              readOnly ||
              offline ||
              !agent ||
              !node.content.trim() ||
              agent.state.runningNodes.has(node.id)
            }
            onClick={() => {
              const context = contextForSelection(document, session.page, session.state.selection);
              void agent?.runPromptNode(
                node.id,
                promptNodeRequest(node, composer?.state.choice ?? null, context),
              );
            }}
          >
            Run
          </button>
        </>
      ) : null}
      {error ? <p role="alert">{error}</p> : null}
    </section>
  );
}
