import { Dialog } from "@base-ui/react/dialog";
import type { PaletteController, PaletteResult } from "@vellum/editor-core";
import { useEffect, useRef } from "react";
import { usePortalContainer } from "../chrome/portal";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import "./shell.css";

const KIND_WORDS: Record<string, string> = {
  layer: "Layer",
  page: "Page",
  component: "Component",
  variable: "Variable",
  session: "Session",
};

function iconFor(r: PaletteResult): string {
  if (r.kind === "ask") return "message-square";
  if (r.kind === "layer") return "frame";
  if (r.kind === "page") return "file";
  if (r.kind === "component") return "component";
  if (r.kind === "variable") return "braces";
  if (r.kind === "session") return "message-square";
  const id = r.command ?? "";
  if (id.startsWith("component.")) return "component";
  if (id === "edit.autoLayout" || id === "edit.removeAutoLayout") return "rows-3";
  if (id.startsWith("agent.pause")) return "pause";
  if (id.includes("export") || id.includes("copyPng")) return "download";
  if (id.startsWith("view.")) return "eye";
  if (id.startsWith("tool.")) return "mouse-pointer-2";
  return "square";
}

/** ⌘/ (C23): search actions, layers, pages, components, variables and sessions; the first row is Ask the agent. */
export function Palette({
  palette,
  shortcut,
}: {
  palette: PaletteController;
  /** The chord shown at a command's right edge, in the person's own keys. */
  shortcut(command: string): string | null;
}) {
  useStore(palette.subscribe);
  const container = usePortalContainer();
  const field = useRef<HTMLInputElement>(null);
  const { open, query, active } = palette.state;
  useEffect(() => {
    if (open) field.current?.focus();
  }, [open]);
  if (!open) return null;
  const results = palette.results();
  const note = `${shortcut("palette.open") ?? "⌘/"} opens this palette · Enter to run · Esc to close`;
  return (
    <Dialog.Root open onOpenChange={(o) => !o && palette.close()}>
      <Dialog.Portal container={container}>
        <Dialog.Backdrop className="vc-resource-backdrop" />
        <Dialog.Popup className="vc-resource-dialog vp-palette" initialFocus={field}>
          <header className="vc-resource-header">
            <Dialog.Title>Command palette</Dialog.Title>
            <Dialog.Close className="vc-iconbutton" aria-label="Close dialog">
              <Icon name="x" />
            </Dialog.Close>
          </header>
          <Dialog.Description className="vc-resource-description">
            Search actions, layers, pages, components, variables and sessions.
          </Dialog.Description>
          <label className="vp-search">
            <span>Search</span>
            <input
              ref={field}
              className="vc-mono"
              value={query}
              role="combobox"
              aria-expanded
              aria-controls="vp-results"
              aria-activedescendant={results[active] ? `vp-option-${active}` : undefined}
              autoComplete="off"
              spellCheck={false}
              onChange={(e) => palette.setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.nativeEvent.isComposing) return;
                if (e.key === "Escape") return;
                if (palette.key(e.key)) e.preventDefault();
              }}
            />
          </label>
          <div className="vp-results" id="vp-results" role="listbox" aria-label="Results">
            {results.map((r, i) => (
              <button
                type="button"
                key={r.key}
                id={`vp-option-${i}`}
                role="option"
                tabIndex={-1}
                aria-selected={i === active}
                className="vp-result"
                data-kind={r.kind}
                data-active={i === active ? "" : undefined}
                onMouseMove={() => i !== active && palette.setActive(i)}
                onClick={() => palette.choose(i)}
              >
                <Icon name={iconFor(r)} className="vp-result__icon" />
                <span className="vp-result__label">{r.label}</span>
                <span className="vp-result__hint vc-mono">
                  {r.command ? (shortcut(r.command) ?? "") : (KIND_WORDS[r.kind] ?? "")}
                </span>
              </button>
            ))}
          </div>
          <p className="vp-note">{note}</p>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
