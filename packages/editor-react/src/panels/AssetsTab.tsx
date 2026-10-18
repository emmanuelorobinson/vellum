import {
  type AssetItem,
  type AssetPayload,
  type AssetsInput,
  assetSections,
} from "@vellum/editor-core";
import { useEffect, useMemo, useState } from "react";
import { Icon } from "../icons/Icon";

export interface AssetsTabProps extends Omit<AssetsInput, "query"> {
  /** A component was dragged out of the panel and released at this screen point. */
  onDropComponent(
    payload: Extract<AssetPayload, { kind: "component" }>,
    point: { x: number; y: number },
  ): void;
  /** Add (starter system), Browse (SVG folder), Guidelines: the host does these; absent disables them. */
  onAction?(action: "add" | "browse" | "guidelines" | "icons" | "library", item: AssetItem): void;
  readOnly: boolean;
  onVariables?(): void;
  onLibraries?(): void;
  onThemes?(): void;
}

export function AssetsTab(props: AssetsTabProps) {
  const [query, setQuery] = useState("");
  const sections = useMemo(
    () => assetSections({ ...props, query }),
    [
      props.components,
      props.libraries,
      props.iconSets,
      props.projectIconFolder,
      props.visualStyles,
      props.designSystems,
      query,
    ],
  );
  const [drag, setDrag] = useState<{ label: string; x: number; y: number } | null>(null);
  useEffect(() => {
    if (!drag) return;
    const cancel = (e: KeyboardEvent) => e.key === "Escape" && setDrag(null);
    window.addEventListener("keydown", cancel, true);
    return () => window.removeEventListener("keydown", cancel, true);
  }, [drag]);

  const startDrag = (item: AssetItem, e: React.PointerEvent) => {
    if (e.button !== 0 || props.readOnly || item.payload.kind !== "component") return;
    const payload = item.payload;
    const start = { x: e.clientX, y: e.clientY };
    let active = false;
    const move = (m: PointerEvent) => {
      if (!active && Math.hypot(m.clientX - start.x, m.clientY - start.y) < 4) return;
      active = true;
      setDrag({ label: item.label, x: m.clientX, y: m.clientY });
    };
    const end = (u: PointerEvent) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      setDrag((d) => {
        if (d && active) props.onDropComponent(payload, { x: u.clientX, y: u.clientY });
        return null;
      });
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
  };

  return (
    <div className="vc-assets">
      <label className="vc-search">
        <input
          type="search"
          aria-label="Search assets"
          placeholder="Search components, icons, styles…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </label>
      {props.onVariables ? (
        <div className="vc-resource-actions">
          <button type="button" className="vc-secondary" onClick={props.onVariables}>
            Variables
          </button>
          <button type="button" className="vc-secondary" onClick={props.onThemes}>
            Themes
          </button>
          <button type="button" className="vc-secondary" onClick={props.onLibraries}>
            Libraries
          </button>
        </div>
      ) : null}
      {sections.length === 0 ? (
        <p className="vc-caption" role="status">{`Nothing matches “${query.trim()}”`}</p>
      ) : null}
      {sections.map((s) => (
        <section key={s.id} className="vc-assets__section" aria-labelledby={`vc-assets-${s.id}`}>
          <h3 className="vc-heading" id={`vc-assets-${s.id}`}>
            {s.heading}
          </h3>
          <ul className="vc-assets__list">
            {s.items.map((item) => {
              const draggable = item.payload.kind === "component" && !props.readOnly;
              const open: Record<string, "library" | "icons" | undefined> = {
                library: "library",
                iconSets: "icons",
              };
              const openAction = open[item.payload.kind];
              return (
                <li
                  key={item.id}
                  className="vc-asset"
                  data-kind={item.payload.kind}
                  data-status={item.status ?? undefined}
                  data-draggable={draggable ? "" : undefined}
                  onPointerDown={(e) => startDrag(item, e)}
                >
                  <Icon name={item.icon} className="vc-asset__icon" />
                  {openAction ? (
                    <button
                      type="button"
                      className="vc-asset__label vc-asset__open"
                      disabled={!props.onAction}
                      onClick={() =>
                        openAction === "library" && props.onLibraries
                          ? props.onLibraries()
                          : props.onAction?.(openAction, item)
                      }
                    >
                      {item.label}
                      {item.detail ? ` · ${item.detail}` : ""}
                    </button>
                  ) : (
                    <span className="vc-asset__label">
                      {item.label}
                      {item.detail ? ` · ${item.detail}` : ""}
                    </span>
                  )}
                  {item.status === "missing" ? (
                    <span
                      className="vc-asset__status"
                      role="img"
                      aria-label="Missing"
                      title="Missing"
                    >
                      <Icon name="triangle-alert" size={12} />
                    </span>
                  ) : null}
                  {item.action === "browse" ? <span aria-hidden>·</span> : null}
                  {item.action ? (
                    <button
                      type="button"
                      className="vc-asset__action"
                      data-inline={item.action === "browse" ? "" : undefined}
                      disabled={!props.onAction || props.readOnly}
                      onClick={() => props.onAction?.(item.action as "add" | "browse", item)}
                    >
                      {item.action === "add" ? "Add" : "Browse"}
                    </button>
                  ) : null}
                </li>
              );
            })}
          </ul>
        </section>
      ))}
      <button
        type="button"
        className="vc-secondary vc-assets__guidelines"
        disabled={!props.onAction}
        onClick={() => props.onAction?.("guidelines", { id: "guidelines" } as AssetItem)}
      >
        Guidelines
      </button>
      {drag ? (
        <div className="vc-dragpreview" style={{ left: drag.x + 12, top: drag.y + 12 }} aria-hidden>
          <Icon name="component" />
          <span>{drag.label}</span>
        </div>
      ) : null}
    </div>
  );
}
