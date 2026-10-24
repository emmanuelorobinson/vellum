import type { DesktopBridge, DesktopState } from "@vellum/host-electron/bridge";
import { MENU_CATEGORIES } from "@vellum/host-electron/bridge";
import { useRef } from "react";

export function DocumentTabs({ state, bridge }: { state: DesktopState; bridge: DesktopBridge }) {
  const refs = useRef(new Map<string, HTMLButtonElement>());
  const request = (type: "activate" | "close", tab: string) => {
    void bridge.request({ type, tab });
  };
  return (
    <section className="vc-bar desktop-docbar" aria-label="Documents">
      {state.platform !== "mac" && (
        <button
          type="button"
          className="desktop-menu"
          aria-label="Main menu"
          popoverTarget="desktop-menu"
        >
          ☰
        </button>
      )}
      {state.platform !== "mac" && (
        <div id="desktop-menu" popover="auto" className="desktop-menu-popover">
          {MENU_CATEGORIES.map((category) => (
            <button
              key={category}
              type="button"
              onClick={(event) => {
                event.currentTarget.parentElement?.hidePopover();
                void bridge.request({ type: "menu", category });
              }}
            >
              {category}
            </button>
          ))}
        </div>
      )}
      <div
        className="desktop-tabs"
        role="tablist"
        aria-label="Document tabs"
        data-single={state.tabs.length < 2}
      >
        {state.tabs.map((tab, index) => (
          <div className="desktop-tab" data-selected={tab.id === state.active} key={tab.id}>
            <button
              ref={(element) => {
                if (element) refs.current.set(tab.id, element);
                else refs.current.delete(tab.id);
              }}
              type="button"
              role="tab"
              aria-selected={tab.id === state.active}
              tabIndex={tab.id === state.active ? 0 : -1}
              aria-controls={`document-${tab.id}`}
              id={`tab-${tab.id}`}
              className="desktop-tab__select"
              onClick={() => request("activate", tab.id)}
              onKeyDown={(event) => {
                let target: number | undefined;
                if (event.key === "ArrowRight") target = (index + 1) % state.tabs.length;
                if (event.key === "ArrowLeft")
                  target = (index + state.tabs.length - 1) % state.tabs.length;
                if (event.key === "Home") target = 0;
                if (event.key === "End") target = state.tabs.length - 1;
                if (target !== undefined) {
                  event.preventDefault();
                  const next = state.tabs[target];
                  if (next) {
                    request("activate", next.id);
                    refs.current.get(next.id)?.focus();
                  }
                }
              }}
            >
              {tab.running && <span className="desktop-tab__running" aria-hidden="true" />}
              <span className="desktop-tab__name">{tab.name}</span>
              <span className="desktop-tab__state" data-dirty={tab.dirty}>
                {tab.dirty ? "Unsaved" : tab.running ? "Working" : "Saved"}
              </span>
            </button>
            <button
              type="button"
              className="desktop-tab__close"
              aria-label={`Close ${tab.name}`}
              onClick={() => request("close", tab.id)}
            >
              ×
            </button>
          </div>
        ))}
      </div>
    </section>
  );
}
