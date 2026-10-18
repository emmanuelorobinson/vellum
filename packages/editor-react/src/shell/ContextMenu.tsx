import { type ContextMenuItem, contextMenuItems } from "@vellum/editor-core";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import "./shell.css";

export interface ContextMenuState {
  /** Pointer position inside the workspace. */
  x: number;
  y: number;
  /** The layers the menu acts on. */
  ids: readonly string[];
}

/** Right-click on layers (C24): the rows run commands; shortcuts show in mono at the right edge. */
export function ContextMenu({
  menu,
  enabled,
  shortcut,
  run,
  onClose,
  size,
  mac,
  readOnly = false,
}: {
  menu: ContextMenuState;
  enabled(command: string): boolean;
  shortcut(command: string): string | null;
  run(command: string, ids: readonly string[]): void;
  onClose(): void;
  /** The workspace's size, to keep the menu on screen. */
  size: { width: number; height: number };
  mac: boolean;
  readOnly?: boolean;
}) {
  const root = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState({ x: menu.x, y: menu.y });
  const items = contextMenuItems({ count: menu.ids.length, enabled, readOnly });
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    setAt({
      x: Math.max(8, Math.min(menu.x, size.width - width - 8)),
      y: Math.max(8, Math.min(menu.y, size.height - height - 8)),
    });
  }, [menu.x, menu.y, size.width, size.height]);
  useEffect(() => {
    root.current
      ?.querySelector<HTMLElement>('[role="menuitem"]:not([aria-disabled="true"])')
      ?.focus();
    const away = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) onClose();
    };
    document.addEventListener("pointerdown", away, true);
    return () => document.removeEventListener("pointerdown", away, true);
  }, [onClose]);
  const groups = [...new Set(items.map((i) => i.group))];
  const keyOf = (i: ContextMenuItem) =>
    i.command === "edit.delete" ? (mac ? "⌫" : "Del") : (shortcut(i.command) ?? "");
  const move = (by: 1 | -1) => {
    const list = [
      ...(root.current?.querySelectorAll<HTMLElement>(
        '[role="menuitem"]:not([aria-disabled="true"])',
      ) ?? []),
    ];
    const at = list.indexOf(document.activeElement as HTMLElement);
    list[(at + by + list.length) % list.length]?.focus();
  };
  const choose = (i: ContextMenuItem) => {
    if (!i.enabled) return;
    onClose();
    run(i.command, menu.ids);
  };
  return (
    // biome-ignore lint/a11y/useSemanticElements: a custom menu, so it can open at the pointer
    <div
      ref={root}
      className="vp-menu"
      role="menu"
      aria-label="Layer actions"
      style={{ left: at.x, top: at.y }}
      onKeyDown={(e) => {
        if (e.key === "ArrowDown") move(1);
        else if (e.key === "ArrowUp") move(-1);
        else if (e.key === "Escape" || e.key === "Tab") onClose();
        else return;
        e.preventDefault();
        e.stopPropagation();
      }}
      onContextMenu={(e) => e.preventDefault()}
    >
      {groups.map((g) => (
        <div key={g} className="vp-menu__group" data-group={g}>
          {items
            .filter((i) => i.group === g)
            .map((i) => (
              <div
                key={i.id}
                role="menuitem"
                tabIndex={-1}
                aria-disabled={!i.enabled}
                className="vp-menu__row"
                data-destructive={i.destructive ? "" : undefined}
                onClick={() => choose(i)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    choose(i);
                  }
                }}
              >
                <span>{i.label}</span>
                <span className="vp-menu__key vc-mono">{keyOf(i)}</span>
              </div>
            ))}
        </div>
      ))}
    </div>
  );
}
