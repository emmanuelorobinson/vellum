import { copy } from "@vellum/config";
import {
  createLayerIndex,
  type DropTarget,
  dropTarget,
  type LayerOrder,
  type LayerRow,
  layerRows,
  planDrop,
  RECENT_AGENT_MS,
  renameTransaction,
  revealPath,
  setLockedTransaction,
  setVisibleTransaction,
  type ToolTransaction,
} from "@vellum/editor-core";
import type { AgentId, DeviceId, Document, NodeId, PageId } from "@vellum/types-ts";
import {
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { ActorMarker, type ActorView } from "../chrome/ActorMarker";
import { Icon } from "../icons/Icon";

export interface LayersTabProps {
  document: Document | null;
  page: PageId | null;
  onPage(id: PageId): void;
  selection: readonly string[];
  onSelect(ids: string[]): void;
  /** Applies a Person transaction; the host reports a refusal. */
  commit(tx: ToolTransaction): void;
  device: DeviceId;
  now(): number;
  recent: Readonly<Record<NodeId, { agent: AgentId; at: number }>>;
  actors: Readonly<Record<AgentId, ActorView>>;
  componentRoots?: ReadonlySet<NodeId>;
  order?: LayerOrder;
  readOnly: boolean;
  /** The one next step of the empty state. */
  onDrawFrame?(): void;
}

const DRAG_START = 4;
/** Layer names truncate in the middle: the end of a name is the part that tells siblings apart. */
function splitName(name: string): [string, string] {
  if (name.length <= 12) return [name, ""];
  return [name.slice(0, name.length - 8), name.slice(-8)];
}

export function LayersTab(props: LayersTabProps) {
  const { document: doc, now, order = "document" } = props;
  const page = doc?.pages.find((p) => p.id === props.page) ?? doc?.pages[0] ?? null;
  const [expanded, setExpanded] = useState<ReadonlySet<NodeId>>(() => new Set());
  const [focused, setFocused] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [drag, setDrag] = useState<{ ids: string[]; target: DropTarget | null } | null>(null);
  const [clock, tick] = useState(0);
  const roots = page?.children ?? [];
  const parent = useMemo(() => ({ type: "page", id: page?.id ?? "" }) as const, [page?.id]);
  const index = useMemo(() => createLayerIndex(roots, parent), [roots, parent]);
  const rows = useMemo(
    () =>
      layerRows({
        roots,
        parent,
        ...(props.componentRoots ? { componentRoots: props.componentRoots } : {}),
        expanded,
        selection: props.selection,
        recent: props.recent,
        now: now(),
        order,
      }),
    [
      roots,
      parent,
      expanded,
      props.selection,
      props.recent,
      props.componentRoots,
      order,
      now,
      clock,
    ],
  );
  const listRef = useRef<HTMLDivElement>(null);

  // Selecting something on the canvas opens the layers above it, and the selected layer itself when it holds others.
  // Only a change of selection does this, so a layer the person collapses stays collapsed.
  const selectionKey = props.selection.join("\u0000");
  useEffect(() => {
    const open = new Set<NodeId>();
    for (const id of props.selection) {
      for (const a of revealPath(roots, parent, id) ?? []) open.add(a);
      if (index.childrenOf({ type: "node", id }).length) open.add(id);
    }
    setExpanded((s) =>
      open.size && [...open].some((a) => !s.has(a)) ? new Set([...s, ...open]) : s,
    );
  }, [selectionKey, roots, parent, index]);

  // The marker leaves after ten seconds without anyone touching the panel.
  useEffect(() => {
    const live = Object.values(props.recent).map((r) => r.at + RECENT_AGENT_MS - now());
    const next = Math.min(...live.filter((ms) => ms > 0));
    if (!Number.isFinite(next)) return;
    const timer = setTimeout(() => tick((n) => n + 1), next + 5);
    return () => clearTimeout(timer);
  }, [props.recent, now]);

  const rowEls = useRef(new Map<string, HTMLElement>());
  const anyRecent = rows.find((r) => r.recentAgent);
  const actorOf = (agent: AgentId): ActorView =>
    props.actors[agent] ?? { id: agent, kind: "agent", name: agent };

  const toggle = (id: NodeId) =>
    setExpanded((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const commit = (tx: ToolTransaction | null) => {
    if (tx && !props.readOnly) props.commit(tx);
  };
  const tx = (ops: ToolTransaction["ops"]): ToolTransaction | null =>
    ops.length ? { origin: { actor: { kind: "person", device: props.device } }, ops } : null;

  const select = (row: LayerRow, e: { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean }) => {
    if (e.shiftKey || e.metaKey || e.ctrlKey) {
      const has = props.selection.includes(row.id);
      props.onSelect(
        has ? props.selection.filter((s) => s !== row.id) : [...props.selection, row.id],
      );
    } else props.onSelect([row.id]);
  };

  const hit = (clientY: number) => {
    for (const [i, r] of rows.entries()) {
      const el = rowEls.current.get(r.id);
      if (!el) continue;
      const b = el.getBoundingClientRect();
      if (clientY >= b.top && clientY < b.bottom)
        return { i, offset: (clientY - b.top) / b.height };
    }
    return null;
  };
  const startDrag = (row: LayerRow, e: ReactPointerEvent) => {
    if (e.button !== 0 || props.readOnly || renaming) return;
    if ((e.target as HTMLElement).closest("button,input")) return;
    const ids = props.selection.includes(row.id) ? [...props.selection] : [row.id];
    const startY = e.clientY;
    let active = false;
    const move = (m: PointerEvent) => {
      if (!active && Math.abs(m.clientY - startY) < DRAG_START) return;
      active = true;
      const at = hit(m.clientY);
      setDrag({
        ids,
        target: at
          ? dropTarget({ rows, index, dragged: ids, over: at.i, offset: at.offset, order })
          : null,
      });
    };
    const end = (u: PointerEvent) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("keydown", esc, true);
      if (!active) return;
      const at = hit(u.clientY);
      const target = at
        ? dropTarget({ rows, index, dragged: ids, over: at.i, offset: at.offset, order })
        : null;
      setDrag(null);
      if (target) commit(tx(planDrop(index, ids, target.place)));
    };
    const esc = (k: KeyboardEvent | globalThis.KeyboardEvent) => {
      if (k.key !== "Escape") return;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("keydown", esc, true);
      setDrag(null);
      active = false;
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
    window.addEventListener("keydown", esc, true);
  };

  const focusRow = (id: string) => {
    setFocused(id);
    queueMicrotask(() => rowEls.current.get(id)?.focus());
  };
  const onKey = (row: LayerRow, i: number, e: KeyboardEvent) => {
    if (renaming) return;
    const go = (to: number) => {
      const r = rows[Math.max(0, Math.min(rows.length - 1, to))];
      if (r) focusRow(r.id);
    };
    if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
      e.preventDefault();
      const up = (e.key === "ArrowUp") !== (order === "front-first");
      const to = row.siblingIndex + (up ? -1 : 1);
      if (to >= 0 && to < row.siblings) commit(tx([{ type: "reorder", id: row.id, index: to }]));
      return;
    }
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        return go(i + 1);
      case "ArrowUp":
        e.preventDefault();
        return go(i - 1);
      case "Home":
        e.preventDefault();
        return go(0);
      case "End":
        e.preventDefault();
        return go(rows.length - 1);
      case "ArrowRight":
        e.preventDefault();
        if (row.expandable && !row.expanded) toggle(row.id);
        else if (row.expanded) go(i + 1);
        return;
      case "ArrowLeft":
        e.preventDefault();
        if (row.expanded) toggle(row.id);
        else {
          const p = index.parentOf(row.id);
          if (p?.type === "node") focusRow(p.id);
        }
        return;
      case "Enter":
      case " ":
        e.preventDefault();
        return select(row, e);
      case "F2":
        e.preventDefault();
        if (!props.readOnly) setRenaming(row.id);
    }
  };

  if (!doc || !page) return <div className="vc-layers vc-layers--empty" data-state="no-document" />;

  return (
    <div className="vc-layers" data-state={rows.length ? "layers" : "empty"}>
      <h3 className="vc-heading">Pages</h3>
      <div className="vc-pages" role="listbox" aria-label="Pages">
        {doc.pages.map((p) => (
          <button
            key={p.id}
            type="button"
            role="option"
            aria-selected={p.id === page.id}
            className="vc-row vc-page"
            data-selected={p.id === page.id ? "" : undefined}
            onClick={() => props.onPage(p.id)}
          >
            <Icon name="file" />
            <span className="vc-row__name">{p.name}</span>
          </button>
        ))}
      </div>
      <h3 className="vc-heading">Layers</h3>
      {rows.length === 0 ? (
        <div className="vc-empty">
          <p>{copy.layersEmpty}</p>
          <button
            type="button"
            className="vc-primary"
            onClick={() => props.onDrawFrame?.()}
            disabled={!props.onDrawFrame || props.readOnly}
          >
            Draw a frame
          </button>
        </div>
      ) : (
        <div
          className="vc-tree"
          role="tree"
          aria-label="Layers"
          aria-multiselectable
          ref={listRef}
          data-dragging={drag ? "" : undefined}
        >
          {rows.map((r, i) => {
            const name = splitName(r.name);
            const marker = r.recentAgent ? actorOf(r.recentAgent) : null;
            const drop = drag?.target?.rowIndex === i ? drag.target.indicator : undefined;
            const dim = r.hiddenByAncestor || !r.visible;
            return (
              <div
                key={r.id}
                ref={(el) => {
                  if (el) rowEls.current.set(r.id, el);
                  else rowEls.current.delete(r.id);
                }}
                role="treeitem"
                aria-level={r.depth + 1}
                aria-selected={r.selected}
                aria-expanded={r.expandable ? r.expanded : undefined}
                aria-posinset={r.siblingIndex + 1}
                aria-setsize={r.siblings}
                aria-label={r.name}
                tabIndex={
                  (focused ?? rows.find((x) => x.selected)?.id ?? rows[0]?.id) === r.id ? 0 : -1
                }
                className="vc-row vc-layer"
                data-kind={r.kind}
                data-selected={r.selected ? "" : undefined}
                data-component={r.component || r.instance ? "" : undefined}
                data-dimmed={dim ? "" : undefined}
                data-drop={drop}
                data-dragged={drag?.ids.includes(r.id) ? "" : undefined}
                style={{ paddingInlineStart: 8 + r.depth * 16 }}
                onPointerDown={(e) => startDrag(r, e)}
                onClick={(e) => {
                  if ((e.target as HTMLElement).closest("button,input")) return;
                  setFocused(r.id);
                  select(r, e);
                }}
                onDoubleClick={() => !props.readOnly && setRenaming(r.id)}
                onFocus={() => setFocused(r.id)}
                onKeyDown={(e) => e.target === e.currentTarget && onKey(r, i, e)}
              >
                <Icon name={r.icon} className="vc-layer__type" />
                {renaming === r.id ? (
                  <input
                    className="vc-layer__rename"
                    // biome-ignore lint/a11y/noAutofocus: the field only exists because the person asked to rename
                    autoFocus
                    aria-label={`Rename ${r.name}`}
                    defaultValue={r.name}
                    onFocus={(e) => e.currentTarget.select()}
                    onKeyDown={(e) => {
                      e.stopPropagation();
                      if (e.key === "Enter") {
                        commit(renameTransaction(props.device, r.id, e.currentTarget.value));
                        setRenaming(null);
                        focusRow(r.id);
                      } else if (e.key === "Escape") {
                        setRenaming(null);
                        focusRow(r.id);
                      }
                    }}
                    onBlur={(e) => {
                      if (renaming === r.id) {
                        commit(renameTransaction(props.device, r.id, e.currentTarget.value));
                        setRenaming(null);
                      }
                    }}
                  />
                ) : (
                  <span className="vc-row__name vc-layer__name" title={r.name}>
                    <span className="vc-layer__head">{name[0]}</span>
                    {name[1] ? <span className="vc-layer__tail">{name[1]}</span> : null}
                  </span>
                )}
                <span className="vc-layer__controls">
                  {r.expandable ? (
                    <button
                      type="button"
                      className="vc-layer__button"
                      aria-label={r.expanded ? `Collapse ${r.name}` : `Expand ${r.name}`}
                      onClick={() => toggle(r.id)}
                    >
                      <Icon name={r.expanded ? "chevron-down" : "chevron-right"} />
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="vc-layer__button"
                    data-state={r.visible ? "off" : "on"}
                    aria-label={r.visible ? `Hide ${r.name}` : `Show ${r.name}`}
                    aria-pressed={!r.visible}
                    title={r.visibleBound ? "Visibility is set by a variable" : undefined}
                    disabled={props.readOnly || r.visibleBound}
                    onClick={() => commit(setVisibleTransaction(props.device, r.id, !r.visible))}
                  >
                    <Icon name={r.visible ? "eye" : "eye-off"} />
                  </button>
                  <button
                    type="button"
                    className="vc-layer__button"
                    data-state={r.locked ? "on" : "off"}
                    aria-label={r.locked ? `Unlock ${r.name}` : `Lock ${r.name}`}
                    aria-pressed={r.locked}
                    disabled={props.readOnly}
                    onClick={() => commit(setLockedTransaction(props.device, r.id, !r.locked))}
                  >
                    <Icon name={r.locked ? "lock" : "lock-open"} />
                  </button>
                  {marker ? (
                    <span className="vc-layer__recent">
                      <ActorMarker
                        actor={{ ...marker, state: "created this layer in the last 10 seconds" }}
                        size={12}
                        outlined
                      />
                    </span>
                  ) : null}
                </span>
              </div>
            );
          })}
        </div>
      )}
      {anyRecent?.recentAgent ? (
        <p className="vc-caption">{`${actorOf(anyRecent.recentAgent).name} marker · created in the last 10 seconds`}</p>
      ) : null}
    </div>
  );
}
