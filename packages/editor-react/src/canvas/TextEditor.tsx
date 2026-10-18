import { type EditorSession, plainText } from "@vellum/editor-core";
import type { TextRun, Value } from "@vellum/types-ts";
import { useEffect, useRef } from "react";

type Loose = Record<string, unknown>;

/**
 * The caret and the characters of a text layer open for typing (DESIGN.md C14), laid over the canvas where the
 * layer is, in its first run's font. Typing sends the new content to the tool controller as a preview; Escape puts
 * the old text back, and leaving the field (or ⌘ Enter) writes one change.
 */
export function TextEditor(props: {
  session: EditorSession;
  colour?: (value: Value<string>) => string;
}) {
  const { session } = props;
  const mode = session.state.mode;
  const field = useRef<HTMLTextAreaElement>(null);
  const editing = mode.type === "editing" ? mode : null;
  const kind = editing?.node.type;
  const address = editing?.address ?? null;
  // Focus once when the editor opens, with everything selected as a double-click on text should.
  useEffect(() => {
    if (!editing) return;
    field.current?.focus({ preventScroll: true });
    field.current?.select();
  }, [kind, address]);
  if (!editing || (kind !== "text" && kind !== "note")) return null;
  const node = editing.node as unknown as Loose;
  const v = session.state.viewport;
  const found = address ? session.sceneBounds(address) : null;
  const pos = node.position as { type?: string; x?: number; y?: number } | undefined;
  const box = found ?? {
    x: pos?.type === "absolute" ? (pos.x ?? 0) : 0,
    y: pos?.type === "absolute" ? (pos.y ?? 0) : 0,
    width: typeof node.width === "number" ? node.width : 0,
    height: typeof node.height === "number" ? node.height : 0,
  };
  const runs = kind === "text" ? ((node.content as TextRun[] | undefined) ?? []) : [];
  const first = runs[0] as (TextRun & Loose) | undefined;
  const size = typeof first?.size === "number" ? first.size : 16;
  const lineHeight = typeof first?.lineHeight === "number" ? first.lineHeight : size * 1.25;
  const fills = node.fills as { type?: string; color?: Value<string> }[] | undefined;
  const solid = fills?.find((f) => f.type === "solid" && f.color !== undefined);
  const colour =
    solid?.color !== undefined ? (props.colour?.(solid.color) ?? undefined) : undefined;
  const text = kind === "text" ? plainText(editing.node) : String(node.content ?? "");
  const send = (value: string) => {
    if (kind === "text") {
      if (first) session.dispatch({ type: "content", content: [{ ...first, text: value }] });
    } else session.dispatch({ type: "content", content: value });
  };
  return (
    <textarea
      ref={field}
      className="vc-textedit"
      aria-label={kind === "note" ? "Note text" : "Text layer"}
      value={text}
      spellCheck={false}
      style={{
        left: box.x * v.zoom + v.x,
        top: box.y * v.zoom + v.y,
        width: Math.max(box.width * v.zoom, 24),
        height: Math.max(box.height * v.zoom, lineHeight * v.zoom),
        fontFamily: first?.font ? `"${first.font}", var(--font-ui), sans-serif` : undefined,
        fontSize: size * v.zoom,
        fontWeight: first?.weight ?? 400,
        fontStyle: first?.style === "italic" ? "italic" : "normal",
        lineHeight: `${lineHeight * v.zoom}px`,
        letterSpacing:
          typeof first?.letterSpacing === "number" ? first.letterSpacing * v.zoom : undefined,
        color: colour,
      }}
      onPointerDown={(e) => e.stopPropagation()}
      onPointerMove={(e) => e.stopPropagation()}
      onPointerUp={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
      onChange={(e) => send(e.currentTarget.value)}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          session.dispatch({ type: "cancel" });
        } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
          e.preventDefault();
          session.dispatch({ type: "finish" });
        }
      }}
      onBlur={(e) => {
        // Moving into the properties panel (Size, Weight, Content) keeps the text open: those edit the draft.
        if ((e.relatedTarget as Element | null)?.closest?.(".vc-right")) return;
        if (session.state.mode.type === "editing") session.dispatch({ type: "finish" });
      }}
    />
  );
}
