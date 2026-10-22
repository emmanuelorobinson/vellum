import type { AgentMode, PanelTab, WorkspaceLayout } from "@vellum/editor-core";
import { type KeyboardEvent, type ReactNode, useEffect, useRef } from "react";
import { useLocale } from "../accessibility/locale";
import { Tip } from "../chrome/Tooltip";
import { Icon } from "../icons/Icon";

const TABS: readonly { id: PanelTab; label: string; icon: string; strip: string }[] = [
  { id: "agent", label: "Agent", icon: "message-square", strip: "Open Agent" },
  { id: "layers", label: "Layers", icon: "layers", strip: "Layers" },
  { id: "assets", label: "Assets", icon: "component", strip: "Assets" },
  { id: "history", label: "History", icon: "history", strip: "History" },
];

export function PanelFrame({
  layout,
  tab,
  onTab,
  onCloseOverlay,
  shortcut,
  children,
}: {
  layout: WorkspaceLayout["agent"];
  tab: PanelTab;
  onTab(tab: PanelTab): void;
  onCloseOverlay(): void;
  /** The key to show on the strip's first icon, e.g. "⌘L". */
  shortcut: string | null;
  children: Record<PanelTab, ReactNode>;
}) {
  const { t: translate, direction } = useLocale();
  const mode: AgentMode = layout.mode;
  const list = useRef<HTMLDivElement>(null);
  // An overlay opened from the strip takes focus, so Escape and the tab keys work straight away.
  useEffect(() => {
    if (mode === "overlay")
      list.current?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')?.focus();
  }, [mode]);
  if (mode === "hidden") return null;
  const { x, y, width, height } = layout.rect;
  if (mode === "strip")
    return (
      <nav
        className="vc-bar vc-panel vc-panel--strip"
        style={{ insetInlineStart: x, top: y, width, height }}
        aria-label={translate("Panels")}
      >
        {TABS.map((t) => (
          <Tip
            key={t.id}
            name={translate(t.id === "agent" ? t.strip : t.label)}
            shortcut={t.id === "agent" ? shortcut : null}
          >
            <button
              type="button"
              className="vc-iconbutton vc-panel__stripbutton"
              aria-label={
                t.id === "agent" && shortcut
                  ? `${translate(t.strip)} · ${shortcut}`
                  : translate(t.label)
              }
              onClick={() => onTab(t.id)}
            >
              <Icon name={t.icon} size={18} />
            </button>
          </Tip>
        ))}
      </nav>
    );
  const move = (e: KeyboardEvent) => {
    const at = TABS.findIndex((t) => t.id === tab);
    const to =
      e.key === (direction === "rtl" ? "ArrowLeft" : "ArrowRight")
        ? (at + 1) % TABS.length
        : e.key === (direction === "rtl" ? "ArrowRight" : "ArrowLeft")
          ? (at + TABS.length - 1) % TABS.length
          : e.key === "Home"
            ? 0
            : e.key === "End"
              ? TABS.length - 1
              : -1;
    if (to < 0) return;
    e.preventDefault();
    const next = TABS[to] as (typeof TABS)[number];
    onTab(next.id);
    queueMicrotask(() => list.current?.querySelector<HTMLElement>(`#vc-tab-${next.id}`)?.focus());
  };
  return (
    <section
      className="vc-bar vc-panel"
      data-mode={mode}
      style={{ insetInlineStart: x, top: y, width, height }}
      aria-label={translate("Agent panel")}
      onKeyDown={(e) => {
        if (mode === "overlay" && e.key === "Escape") onCloseOverlay();
      }}
    >
      <div
        className="vc-panel__tabs"
        role="tablist"
        aria-label={translate("Panel")}
        ref={list}
        onKeyDown={move}
      >
        {TABS.map((t) => (
          <button
            key={t.id}
            id={`vc-tab-${t.id}`}
            type="button"
            role="tab"
            className="vc-panel__tab"
            aria-selected={tab === t.id}
            aria-controls={`vc-tabpanel-${t.id}`}
            tabIndex={tab === t.id ? 0 : -1}
            onClick={() => onTab(t.id)}
          >
            {translate(t.label)}
          </button>
        ))}
      </div>
      <div
        className="vc-panel__body"
        role="tabpanel"
        id={`vc-tabpanel-${tab}`}
        aria-labelledby={`vc-tab-${tab}`}
      >
        {children[tab]}
      </div>
    </section>
  );
}
