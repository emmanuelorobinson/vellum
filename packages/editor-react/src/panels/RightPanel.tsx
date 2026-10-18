import { type ReactNode, useState } from "react";

type RightTab = "design" | "inspect" | "activity";
const TABS: readonly { id: RightTab; label: string }[] = [
  { id: "design", label: "Design" },
  { id: "inspect", label: "Inspect" },
  { id: "activity", label: "Activity" },
];

/**
 * The properties panel's frame: Design, Inspect and Activity. Design is the properties of the selection;
 * Inspect and Activity belong to later parts of the editor and are mounted by the host.
 */
export function RightPanel(props: {
  design: ReactNode;
  inspect?: ReactNode;
  activity?: ReactNode;
}) {
  const [tab, setTab] = useState<RightTab>("design");
  const body: Record<RightTab, ReactNode> = {
    design: props.design,
    inspect: props.inspect ?? <p className="vc-design__empty">Inspect is not available yet.</p>,
    activity: props.activity ?? <p className="vc-design__empty">Activity is not available yet.</p>,
  };
  return (
    <>
      <div className="vc-panel__tabs vc-right__tabs" role="tablist" aria-label="Properties">
        {TABS.map((t) => (
          <button
            key={t.id}
            id={`vc-rtab-${t.id}`}
            type="button"
            role="tab"
            className="vc-panel__tab"
            aria-selected={tab === t.id}
            aria-controls={`vc-rpanel-${t.id}`}
            tabIndex={tab === t.id ? 0 : -1}
            onClick={() => setTab(t.id)}
            onKeyDown={(e) => {
              const at = TABS.findIndex((x) => x.id === tab);
              const to =
                e.key === "ArrowRight"
                  ? (at + 1) % TABS.length
                  : e.key === "ArrowLeft"
                    ? (at + TABS.length - 1) % TABS.length
                    : -1;
              if (to < 0) return;
              e.preventDefault();
              const next = TABS[to] as (typeof TABS)[number];
              setTab(next.id);
              queueMicrotask(() => document.getElementById(`vc-rtab-${next.id}`)?.focus());
            }}
          >
            {t.label}
          </button>
        ))}
      </div>
      <div
        className="vc-panel__body vc-right__body"
        role="tabpanel"
        id={`vc-rpanel-${tab}`}
        aria-labelledby={`vc-rtab-${tab}`}
      >
        {body[tab]}
      </div>
    </>
  );
}
