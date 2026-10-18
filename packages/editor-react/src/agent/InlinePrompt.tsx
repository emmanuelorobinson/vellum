/**
 * The selection toolbar's Ask and the inline prompt (DESIGN.md "Inline prompt", C08, C13). DESIGN-DECISIONS: the
 * toolbar holds Ask only, and the prompt opens beside the selection. Its turn runs in the current session and shows
 * in the Agent panel; here it shows only a progress line and Undo.
 */
import type { ComposerController } from "@vellum/editor-core";
import { useLayoutEffect, useRef } from "react";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";

export interface ScreenRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

const WIDTH = 360;
const GAP = 12;

export function SelectionToolbar({
  rect,
  shortcut,
  onAsk,
}: {
  rect: ScreenRect;
  shortcut: string;
  onAsk(): void;
}) {
  return (
    <div
      className="vk-seltool"
      role="toolbar"
      aria-label="Selection"
      style={{ left: rect.x, top: Math.max(8, rect.y - 40) }}
    >
      <button type="button" className="vk-seltool__ask" onClick={onAsk}>
        <Icon name="message-square" size={16} />
        <span>Ask</span>
        <kbd className="vk-seltool__key">{shortcut}</kbd>
      </button>
    </div>
  );
}

export function InlinePrompt({
  composer,
  rect,
  label,
  bounds,
}: {
  composer: ComposerController;
  rect: ScreenRect;
  label: string;
  /** The window, for keeping the prompt on screen. */
  bounds: { width: number; height: number };
}) {
  useStore(composer.subscribe);
  const field = useRef<HTMLTextAreaElement>(null);
  const undo = useRef<HTMLButtonElement>(null);
  const inline = composer.state.inline;
  const sent = inline?.phase === "sent";
  useLayoutEffect(() => {
    if (sent) undo.current?.focus();
    else field.current?.focus();
  }, [sent]);
  if (!inline) return null;
  // Beside the selection: right of it when it fits, otherwise left of it.
  const right = rect.x + rect.width + GAP;
  const left = right + WIDTH <= bounds.width - 8 ? right : Math.max(8, rect.x - GAP - WIDTH);
  const top = Math.min(Math.max(8, rect.y), bounds.height - 140);
  return (
    <div
      className="vk-inline"
      role="dialog"
      aria-label="Ask about the selection"
      style={{ left, top }}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          composer.closeInline();
        }
      }}
    >
      <p className="vk-inline__scope">{label}</p>
      {inline.phase === "edit" ? (
        <>
          <textarea
            ref={field}
            className="vk-inline__field"
            rows={1}
            aria-label="Request for the selection"
            placeholder={composer.placeholder()}
            value={inline.text}
            onChange={(e) => composer.setInlineText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault();
                void composer.sendInline();
              }
            }}
          />
          <div className="vk-inline__actions">
            <button
              type="button"
              className="vk-send"
              disabled={!inline.text.trim()}
              onClick={() => void composer.sendInline()}
            >
              Send
            </button>
            <span className="vk-inline__hint">Enter to send · Esc to close</span>
          </div>
        </>
      ) : (
        <>
          <p className="vk-inline__progress" role="status">
            {composer.inlineLine()}
          </p>
          <div className="vk-inline__actions">
            <button
              ref={undo}
              type="button"
              className="vk-send"
              onClick={() => void composer.undoInline()}
            >
              Undo
            </button>
            <span className="vk-inline__hint">Changes appear in this session</span>
          </div>
        </>
      )}
    </div>
  );
}
