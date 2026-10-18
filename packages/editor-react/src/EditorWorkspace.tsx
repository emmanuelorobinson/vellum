import {
  type BindingContext,
  type createCommandController,
  type EditorHost,
  type EditorSession,
  instanceTransaction,
  type Platform,
  type ToolTransaction,
  type Workspace,
  world,
} from "@vellum/editor-core";
import type { EngineClient } from "@vellum/engine-client";
import type { AgentId, ComponentId, DeviceId, Node, NodeId, PageId } from "@vellum/types-ts";
import type { Mode } from "@vellum/ui-tokens";
import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { CanvasHost, type ContentSource, type RendererFactory } from "./canvas/CanvasHost";
import type { ActorView } from "./chrome/ActorMarker";
import { BottomStrip, type Connection } from "./chrome/BottomStrip";
import { DocumentBar, type SaveState } from "./chrome/DocumentBar";
import { shortcutLabel } from "./chrome/shortcuts";
import { ToolRail } from "./chrome/ToolRail";
import { TipProvider } from "./chrome/Tooltip";
import { TopRightBar } from "./chrome/TopRightBar";
import { ZoomPill } from "./chrome/ZoomPill";
import { useEngineSnapshot, useStore, useWorkspace } from "./hooks";
import { AssetsTab, type AssetsTabProps } from "./panels/AssetsTab";
import { LayersTab } from "./panels/LayersTab";
import { PanelFrame } from "./panels/PanelFrame";
import "./editor.css";

export type CommandController = ReturnType<typeof createCommandController>;

export interface EditorWorkspaceProps {
  workspace: Workspace;
  session: EditorSession;
  engine: EngineClient;
  controller: CommandController;
  device: DeviceId;
  host: EditorHost;
  platform: Platform;
  /** The chrome's appearance; the canvas follows `canvasAppearance` (defaults to the same). */
  appearance: Mode;
  canvasAppearance?: Mode;
  fileName: string;
  saveState: SaveState;
  branch: string | null;
  dirty?: boolean;
  connection: Connection;
  basicRenderer?: boolean;
  /** Agents active in this document, in presence order. */
  agents: readonly ActorView[];
  /** Everyone who can be named on a layer, by agent id. */
  actors: Readonly<Record<AgentId, ActorView>>;
  /** Layers created by agents lately; the panel shows a marker for ten seconds. */
  recent: Readonly<Record<NodeId, { agent: AgentId; at: number }>>;
  componentRoots?: ReadonlySet<NodeId>;
  now(): number;
  nextNodeId(): NodeId;
  assets: Omit<AssetsTabProps, "onDropComponent" | "readOnly" | "components">;
  /** The Agent and History tabs and the right panel belong to later prompts; the host mounts them here. */
  slots?: { agent?: ReactNode; history?: ReactNode; right?: ReactNode };
  content?: ContentSource;
  createRenderer?: RendererFactory;
  onShare?: () => void;
  onPreview?: () => void;
  onSettings?: () => void;
  onPauseAll?: () => void;
  /** The host's own actions for chrome that needs it (insert image, command palette, inline prompt). */
  onAction?: (command: string) => void;
}

const focusOf = (target: EventTarget | null): BindingContext["focus"] => {
  const el = target as HTMLElement | null;
  if (el?.closest("input,textarea,select,[contenteditable='true']")) return "text";
  if (el?.closest(".vc-canvas")) return "canvas";
  if (el?.closest(".vc-panel")) return "panel";
  return "outside";
};

export function EditorWorkspace(props: EditorWorkspaceProps) {
  const { workspace, session, engine, controller } = props;
  const { state, layout } = useWorkspace(workspace);
  const snapshot = useEngineSnapshot(engine);
  useStore(session.subscribe);
  useStore(controller.commands.subscribe.bind(controller.commands));
  const root = useRef<HTMLDivElement>(null);
  const doc = snapshot.document;
  const [pageId, setPageId] = useState<PageId | null>(session.page || null);

  // The window is the canvas; the workspace works out what floats over it.
  useEffect(() => {
    const el = root.current;
    if (!el) return;
    const measure = () => {
      const b = el.getBoundingClientRect();
      if (b.width > 0 && b.height > 0) workspace.resize({ width: b.width, height: b.height });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [workspace]);

  const selectionCount = session.state.selection.length;
  useEffect(() => workspace.setSelection(selectionCount > 0), [workspace, selectionCount]);

  const context = (target: EventTarget | null): BindingContext => ({
    host: props.host,
    platform: props.platform,
    focus: focusOf(target),
  });
  const shortcut = (command: string) =>
    shortcutLabel(command, { host: props.host, platform: props.platform });
  const run = (command: string) => {
    if (controller.commands.get(command)) void controller.execute(command);
    else props.onAction?.(command);
  };
  const enabled = (command: string) => {
    const c = controller.commands.get(command);
    return c ? (c.enabled ? c.enabled() : true) : !!props.onAction;
  };

  const commit = (tx: ToolTransaction) => {
    const result = engine.apply(tx);
    if (result.status === "rejected") session.reportRejection(result.reason);
  };

  const components = useMemo(
    () =>
      Object.fromEntries(
        Object.entries(doc?.components ?? {}).map(([id, c]) => [id, { name: c.name }]),
      ),
    [doc?.components],
  );

  const dropComponent: AssetsTabProps["onDropComponent"] = (payload, point) => {
    const canvas = root.current?.querySelector<HTMLElement>(".vc-canvas");
    const hit = document.elementFromPoint(point.x, point.y);
    if (!canvas || !hit || !canvas.contains(hit)) return;
    const root_ = doc?.components?.[payload.componentId as ComponentId];
    const target = doc?.pages.find((p) => p.id === (pageId ?? session.page));
    if (!root_ || !target) return;
    const b = canvas.getBoundingClientRect();
    const at = world({ x: point.x - b.left, y: point.y - b.top }, session.state.viewport);
    commit(
      instanceTransaction({
        device: props.device,
        componentId: payload.componentId as ComponentId,
        library: payload.library,
        name: root_.name,
        root: root_.root as Node,
        id: props.nextNodeId(),
        at,
        to: { parent: { type: "page", id: target.id }, index: target.children?.length ?? 0 },
      }),
    );
  };

  const readOnly = snapshot.readOnly;
  const canvasMode = props.canvasAppearance ?? props.appearance;
  const zoomLabel = workspace.zoomLabel(session.state.viewport.zoom);
  const zoomItems = [
    ["Zoom in", "view.zoomIn"],
    ["Zoom out", "view.zoomOut"],
    ["Zoom to 100%", "view.actualSize"],
    ["Zoom to fit", "view.fitAll"],
    ["Zoom to selection", "view.fitSelection"],
  ].map(([label, command]) => ({
    label: label as string,
    run: () => run(command as string),
    enabled: enabled(command as string),
    key: shortcut(command as string),
  }));
  const visible = layout.visible;
  const tool = session.state.tool;

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: one delegating key listener; focus is in the children
    <div
      ref={root}
      className="vc-workspace"
      data-appearance={props.appearance}
      data-canvas-appearance={canvasMode}
      data-chrome-hidden={layout.chromeHidden ? "" : undefined}
      onKeyDown={(e) => {
        if (e.defaultPrevented) return;
        if (e.key === "Escape" && layout.agent.mode === "overlay") {
          workspace.closeAgentOverlay();
          return;
        }
        const ctx = context(e.target);
        // Tab hides the chrome from the canvas; inside a panel it must still walk the controls (keyboard access).
        if (e.key === "Tab" && ctx.focus !== "canvas" && ctx.focus !== "outside") return;
        void controller.keyDown(e.nativeEvent, ctx);
      }}
      onKeyUp={(e) => void controller.keyUp(e.nativeEvent)}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as globalThis.Node | null))
          void controller.blur();
      }}
    >
      <TipProvider>
        <CanvasHost
          session={session}
          size={{ width: state.width, height: state.height }}
          {...(props.componentRoots ? { componentRoots: props.componentRoots } : {})}
          {...(props.content ? { content: props.content } : {})}
          {...(props.createRenderer ? { createRenderer: props.createRenderer } : {})}
        />
        {visible.documentBar ? (
          <DocumentBar
            fileName={props.fileName}
            saveState={props.saveState}
            width={layout.documentBar.width}
            commands={controller.commands.list()}
          />
        ) : null}
        <PanelFrame
          layout={layout.agent}
          tab={state.tab}
          onTab={(tab) => workspace.switchTab(tab, { reveal: true })}
          onCloseOverlay={() => workspace.closeAgentOverlay()}
          shortcut={shortcut("agent.focus")}
        >
          {{
            agent: props.slots?.agent ?? null,
            history: props.slots?.history ?? null,
            layers: (
              <LayersTab
                document={doc}
                page={pageId ?? session.page}
                onPage={(id) => {
                  setPageId(id);
                  session.setPage(id);
                }}
                selection={session.state.selection}
                onSelect={(ids) => session.select(ids)}
                commit={commit}
                device={props.device}
                now={props.now}
                recent={props.recent}
                actors={props.actors}
                {...(props.componentRoots ? { componentRoots: props.componentRoots } : {})}
                readOnly={readOnly}
                onDrawFrame={() => session.dispatch({ type: "tool", tool: "frame" })}
              />
            ),
            assets: (
              <AssetsTab
                {...props.assets}
                components={components}
                readOnly={readOnly}
                onDropComponent={dropComponent}
              />
            ),
          }}
        </PanelFrame>
        {visible.toolRail ? (
          <ToolRail
            x={layout.toolRail.x}
            y={layout.toolRail.y}
            tool={tool}
            selectTool={(t) => session.dispatch({ type: "tool", tool: t })}
            run={run}
            enabled={enabled}
            shortcut={shortcut}
          />
        ) : null}
        {visible.topRight ? (
          <TopRightBar
            right={layout.topRight.right}
            agents={props.agents}
            {...(props.onShare ? { onShare: props.onShare } : {})}
            {...(props.onPreview ? { onPreview: props.onPreview } : {})}
            {...(props.onSettings ? { onSettings: props.onSettings } : {})}
            {...(props.onPauseAll ? { onPauseAll: props.onPauseAll } : {})}
          />
        ) : null}
        {layout.right.mode !== "hidden" && props.slots?.right ? (
          <aside
            className="vc-bar vc-right"
            data-mode={layout.right.mode}
            style={{
              left: layout.right.rect.x,
              top: layout.right.rect.y,
              width: layout.right.rect.width,
              height: layout.right.rect.height,
            }}
            aria-label="Properties"
          >
            {props.slots.right}
          </aside>
        ) : null}
        {visible.bottomStrip ? (
          <BottomStrip
            centerX={layout.bottomStrip.centerX}
            y={layout.bottomStrip.y}
            branch={props.branch}
            dirty={props.dirty ?? false}
            connection={props.connection}
            basicRenderer={props.basicRenderer ?? false}
          />
        ) : null}
        {visible.zoomPill ? (
          <ZoomPill
            x={layout.zoomPill.x}
            y={layout.zoomPill.y}
            label={zoomLabel}
            items={zoomItems}
          />
        ) : null}
        {session.notice ? (
          <div className="vc-notice" role="status" data-kind={session.notice.kind}>
            <span>{noticeText(session.notice)}</span>
            <button
              type="button"
              className="vc-iconbutton"
              aria-label="Dismiss"
              onClick={() => session.clearNotice()}
            >
              ×
            </button>
          </div>
        ) : null}
      </TipProvider>
    </div>
  );
}

function noticeText(n: NonNullable<EditorSession["notice"]>): string {
  if (n.kind === "read-only") return "This file is read-only";
  if (n.kind === "rejected")
    return n.reason === "read-only"
      ? "This file is read-only"
      : n.reason === "invalid"
        ? "That change was not valid"
        : "The editor could not apply that change";
  return "That action is not available here";
}
