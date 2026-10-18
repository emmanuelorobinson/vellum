import {
  type AgentController,
  type AuthoringController,
  type BindingContext,
  type ComposerController,
  type ConflictController,
  type createCommandController,
  type DesignController,
  type EditorHost,
  type EditorSession,
  type ExportController,
  type HistoryController,
  type InspectController,
  instanceTransaction,
  type PaintEditor,
  type Platform,
  type PresenceController,
  type ResourceController,
  selectionLabel,
  type TokenSyncController,
  type ToolTransaction,
  type Workspace,
  world,
} from "@vellum/editor-core";
import type { EngineClient } from "@vellum/engine-client";
import type { AgentId, ComponentId, DeviceId, Node, NodeId, PageId, Value } from "@vellum/types-ts";
import type { Mode } from "@vellum/ui-tokens";
import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AgentTab } from "./agent/AgentTab";
import { CollaborationNodes } from "./agent/CollaborationNodes";
import type { ComposerHost } from "./agent/Composer";
import { InlinePrompt, type ScreenRect, SelectionToolbar } from "./agent/InlinePrompt";
import { PresenceLayer } from "./agent/PresenceLayer";
import { AuthoringDialogs } from "./authoring/AuthoringDialogs";
import { CanvasAuthoring } from "./authoring/CanvasAuthoring";
import { CanvasHost, type ContentSource, type RendererFactory } from "./canvas/CanvasHost";
import type { ActorView } from "./chrome/ActorMarker";
import { BottomStrip, type Connection } from "./chrome/BottomStrip";
import { DocumentBar, type SaveState } from "./chrome/DocumentBar";
import { PortalContainer } from "./chrome/portal";
import { shortcutLabel } from "./chrome/shortcuts";
import { ToolRail } from "./chrome/ToolRail";
import { TipProvider } from "./chrome/Tooltip";
import { TopRightBar } from "./chrome/TopRightBar";
import { ZoomPill } from "./chrome/ZoomPill";
import type { DesignHost } from "./design/context";
import { DesignTab } from "./design/DesignTab";
import { useEngineSnapshot, useStore, useWorkspace } from "./hooks";
import { HandoffDialogs } from "./ledger/HandoffDialogs";
import { HistoryTab } from "./ledger/HistoryTab";
import { InspectPanel } from "./ledger/InspectPanel";
import { LedgerLayer } from "./ledger/LedgerLayer";
import { AssetsTab, type AssetsTabProps } from "./panels/AssetsTab";
import { LayersTab } from "./panels/LayersTab";
import { PanelFrame } from "./panels/PanelFrame";
import { RightPanel } from "./panels/RightPanel";
import { MissingLibraryPanel } from "./resources/MissingLibraryPanel";
import { ResourceDialogs } from "./resources/ResourceDialogs";
import { ScalePanel, VectorPanel } from "./resources/ToolPanels";
import "./editor.css";
import "./authoring/authoring.css";
import "./ledger/ledger.css";

export type CommandController = ReturnType<typeof createCommandController>;

export interface EditorWorkspaceProps {
  presence?: PresenceController;
  authoring?: AuthoringController;
  /**
   * The change ledger (History tab, file-changed banner, conflict resolver) and the hand-off panels (Inspect,
   * token sync, Export). Each is optional: without one, its slot or its section stays empty.
   */
  ledger?: {
    history?: HistoryController;
    conflicts?: ConflictController;
    inspect?: InspectController;
    tokens?: TokenSyncController;
    exporter?: ExportController;
    /** The IANA zone ledger times show in; the viewer's own when absent. */
    timeZone?: string;
  };
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
  /** The Agent tab's sessions and turns; with it the tab is built in and `slots.agent` is its composer. */
  agent?: AgentController;
  /** The built-in composer, inline prompt and selection toolbar's Ask (with `agent`). */
  composer?: ComposerController;
  composerHost?: ComposerHost;
  /** The properties panel's controller and the fill editor's canvas state; with them the right panel is built in. */
  design?: DesignController;
  resources?: ResourceController;
  paintEditor?: PaintEditor;
  designHost?: DesignHost;
  /**
   * The History tab belongs to a later prompt, as do Inspect and Activity in the right panel; the host mounts them
   * here. Without `agent`, `slots.agent` is the whole Agent tab; without `composer`, it is the composer. `right`
   * replaces the whole right panel when no design controller is given.
   */
  slots?: {
    agent?: ReactNode;
    history?: ReactNode;
    right?: ReactNode;
    inspect?: ReactNode;
    activity?: ReactNode;
  };
  content?: ContentSource;
  createRenderer?: RendererFactory;
  onShare?: () => void;
  onPreview?: () => void;
  onSettings?: () => void;
  onPauseAll?: () => void;
  /** The host's own actions for chrome that needs it (insert image, command palette, inline prompt). */
  onAction?: (command: string) => void;
}

const NO_STORE = () => ({ dispose() {} });

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
  useStore(props.composer?.subscribe ?? NO_STORE);
  const root = useRef<HTMLDivElement | null>(null);
  // Menus and popovers mount inside the workspace so they follow its appearance.
  const [container, setContainer] = useState<HTMLElement | null>(null);
  const setRoot = useCallback((el: HTMLDivElement | null) => {
    root.current = el;
    setContainer(el);
  }, []);
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
    focus:
      focusOf(target) === "canvas" && session.state.mode.type === "path" ? "path" : focusOf(target),
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
  const selectedLibrary = props.design?.single()?.item.node;
  const missingLibrary =
    selectedLibrary?.type === "instance" &&
    selectedLibrary.library &&
    props.resources?.libraryInfo(selectedLibrary.library)?.state === "missing"
      ? selectedLibrary.library
      : null;
  useEffect(() => {
    if (!props.resources) return;
    const r = props.resources;
    const registrations = (
      [
        ["variables.open", "Variables", "variables"],
        ["themes.open", "Themes", "themes"],
        ["libraries.open", "Libraries", "libraries"],
        ["fonts.manage", "Manage fonts", "manageFonts"],
      ] as const
    )
      .filter(([id]) => !controller.commands.get(id))
      .map(([id, title, dialog]) =>
        controller.registerCommand({ id, title, category: "View", run: () => r.open(dialog) }),
      );
    return () => {
      for (const registration of registrations) registration.dispose();
    };
  }, [controller, props.resources]);
  // Go to main component: the main is a layer on a page when the scene has it; otherwise the host shows it.
  const designHost: DesignHost = {
    ...props.designHost,
    ...(props.authoring ? { authoring: props.authoring } : {}),
    ...(props.resources ? { resources: props.resources } : {}),
    ...(props.ledger?.exporter
      ? {
          // "Add export" opens the multi-frame Export dialog for the selection (C21).
          onAction: (command: string, detail?: Record<string, unknown>) => {
            if (command === "export.add")
              void props.ledger?.exporter?.open(session.state.selection);
            else props.designHost?.onAction?.(command, detail);
          },
        }
      : {}),
    onGoToMain:
      props.designHost?.onGoToMain ??
      ((target) => {
        const root = (
          doc?.components?.[target.componentId as ComponentId]?.root as Node | undefined
        )?.id;
        if (target.library && props.resources?.host.openLibrary)
          props.resources.host.openLibrary(target.library);
        else if (!target.library && root && session.sceneItem(root)) session.select([root]);
        else props.designHost?.onAction?.("component.goToMain", target);
      }),
  };
  const [activityRequest, setActivityRequest] = useState(0);
  const composer = props.composer;
  const selection = session.state.selection;
  // Inspect follows the first selected layer; a spec that arrives late for an earlier one is dropped.
  const inspected = selection[0] ?? null;
  const inspect = props.ledger?.inspect;
  useEffect(() => {
    void inspect?.load(inspected);
  }, [inspect, inspected, snapshot.revision]);
  const selectionRect = ((): ScreenRect | null => {
    if (!composer || !selection.length || readOnly) return null;
    const v = session.state.viewport;
    let box: ScreenRect | null = null;
    for (const id of selection) {
      const b = session.sceneBounds(id);
      if (!b) continue;
      box = box
        ? {
            x: Math.min(box.x, b.x),
            y: Math.min(box.y, b.y),
            width: Math.max(box.x + box.width, b.x + b.width) - Math.min(box.x, b.x),
            height: Math.max(box.y + box.height, b.y + b.height) - Math.min(box.y, b.y),
          }
        : { x: b.x, y: b.y, width: b.width, height: b.height };
    }
    return box
      ? {
          x: box.x * v.zoom + v.x,
          y: box.y * v.zoom + v.y,
          width: box.width * v.zoom,
          height: box.height * v.zoom,
        }
      : null;
  })();
  const scopeLabel =
    doc && selection.length ? selectionLabel(doc, pageId ?? session.page, selection) : null;
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
      ref={setRoot}
      className="vc-workspace"
      data-appearance={props.appearance}
      data-canvas-appearance={canvasMode}
      data-chrome-hidden={layout.chromeHidden ? "" : undefined}
      onKeyDown={(e) => {
        if (e.defaultPrevented) return;
        // Escape while dragging a mesh point or crop corner puts it back and nothing else.
        if (e.key === "Escape" && props.paintEditor?.cancel()) {
          e.preventDefault();
          return;
        }
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
      <PortalContainer.Provider value={container}>
        <TipProvider>
          <CanvasHost
            session={session}
            missingLibraries={
              new Map(
                props.resources?.host.libraries
                  ?.filter((i) => props.resources?.libraryInfo(i.alias)?.state === "missing")
                  .map(
                    (i) => [i.alias, !!props.resources?.libraryInfo(i.alias)?.cached] as const,
                  ) ?? [],
              )
            }
            size={{ width: state.width, height: state.height }}
            {...(props.componentRoots ? { componentRoots: props.componentRoots } : {})}
            {...(props.content ? { content: props.content } : {})}
            {...(props.createRenderer ? { createRenderer: props.createRenderer } : {})}
            {...(props.paintEditor ? { paintEditor: props.paintEditor } : {})}
            {...(props.design
              ? { colour: (c: Value<string>) => props.design?.colour(c, doc) ?? (c as string) }
              : {})}
          />
          {props.authoring ? (
            <CanvasAuthoring
              authoring={props.authoring}
              session={session}
              readOnly={readOnly}
              contentAvailable={!!(props.content && props.createRenderer)}
            />
          ) : null}
          <CollaborationNodes
            offline={props.composerHost?.offline ?? false}
            document={doc}
            session={session}
            readOnly={readOnly}
            {...(props.agent ? { agent: props.agent } : {})}
            {...(composer ? { composer } : {})}
          />
          <PresenceLayer
            session={session}
            actors={props.actors}
            {...(props.presence ? { presence: props.presence } : {})}
            {...(props.agent ? { agent: props.agent } : {})}
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
              agent: props.agent ? (
                <AgentTab
                  agent={props.agent}
                  actors={props.actors}
                  {...(composer ? { composer } : {})}
                  {...(props.composerHost ? { composerHost: props.composerHost } : {})}
                  composerSlot={props.slots?.agent}
                />
              ) : (
                (props.slots?.agent ?? null)
              ),
              history: props.ledger?.history ? (
                <HistoryTab
                  history={props.ledger.history}
                  readOnly={readOnly}
                  {...(props.ledger.timeZone ? { timeZone: props.ledger.timeZone } : {})}
                />
              ) : (
                (props.slots?.history ?? null)
              ),
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
                  {...(props.authoring?.host.loadGuidelines
                    ? {
                        onAction: (action, item) => {
                          if (action === "guidelines") void props.authoring?.openGuidelines();
                          else props.assets.onAction?.(action, item);
                        },
                      }
                    : {})}
                  components={components}
                  readOnly={readOnly}
                  onDropComponent={dropComponent}
                  {...(props.resources
                    ? {
                        onVariables: () => props.resources?.open("variables"),
                        onLibraries: () => props.resources?.open("libraries"),
                        onThemes: () => props.resources?.open("themes"),
                      }
                    : {})}
                />
              ),
            }}
          </PanelFrame>
          {visible.toolRail ? (
            <ToolRail
              x={layout.toolRail.x}
              y={layout.toolRail.y}
              tool={tool === "path" ? "move" : tool}
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
              {...(props.presence ? { presence: props.presence } : {})}
              {...(props.onShare ? { onShare: props.onShare } : {})}
              {...(props.onPreview ? { onPreview: props.onPreview } : {})}
              {...(props.onSettings ? { onSettings: props.onSettings } : {})}
              {...(props.onPauseAll ? { onPauseAll: props.onPauseAll } : {})}
            />
          ) : null}
          {layout.right.mode !== "hidden" && (props.design || props.slots?.right) ? (
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
              {props.resources && missingLibrary ? (
                <MissingLibraryPanel
                  resources={props.resources}
                  alias={missingLibrary}
                  disabled={readOnly}
                />
              ) : props.design && props.paintEditor ? (
                <RightPanel
                  activityRequest={activityRequest}
                  design={
                    session.state.mode.type === "scaling" ? (
                      <ScalePanel session={session} disabled={readOnly} />
                    ) : session.state.mode.type === "path" ? (
                      <VectorPanel session={session} design={props.design} disabled={readOnly} />
                    ) : (
                      <DesignTab
                        design={props.design}
                        session={session}
                        paintEditor={props.paintEditor}
                        document={doc}
                        readOnly={readOnly}
                        host={designHost}
                      />
                    )
                  }
                  {...(props.ledger?.inspect
                    ? { inspect: <InspectPanel inspect={props.ledger.inspect} /> }
                    : props.slots?.inspect
                      ? { inspect: props.slots.inspect }
                      : {})}
                  {...(props.slots?.activity ? { activity: props.slots.activity } : {})}
                />
              ) : (
                props.slots?.right
              )}
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
          {selectionRect &&
          composer &&
          !composer.state.inline &&
          !layout.chromeHidden &&
          tool !== "path" &&
          session.state.mode.type !== "path" ? (
            <SelectionToolbar
              rect={selectionRect}
              shortcut={shortcut("agent.inline") ?? ""}
              onAsk={() => composer.openInline()}
            />
          ) : null}
          {composer?.state.inline && (selectionRect || composer.state.inline.phase === "sent") ? (
            <InlinePrompt
              composer={composer}
              rect={selectionRect ?? { x: state.width / 2 - 180, y: 120, width: 0, height: 0 }}
              label={scopeLabel ?? "Selection"}
              bounds={{ width: state.width, height: state.height }}
            />
          ) : null}
          {session.state.mode.type === "path" ? (
            <button
              type="button"
              className="vc-resource-button vc-path-done"
              data-primary=""
              onClick={() => session.dispatch({ type: "finish" })}
            >
              Done · Enter
            </button>
          ) : null}
          {props.resources ? (
            <ResourceDialogs
              resources={props.resources}
              session={session}
              design={props.design}
              readOnly={readOnly}
            />
          ) : null}
          {props.ledger ? (
            <>
              <LedgerLayer
                history={props.ledger.history}
                conflicts={props.ledger.conflicts}
                session={session}
                readOnly={readOnly}
                insets={layout.canvasInsets}
                width={state.width}
              />
              <HandoffDialogs
                tokens={props.ledger.tokens}
                exporter={props.ledger.exporter}
                inspect={props.ledger.inspect}
                readOnly={readOnly}
                onOpenActivity={() => setActivityRequest((n) => n + 1)}
                toastLeft={
                  layout.canvasInsets.left +
                  (state.width - layout.canvasInsets.left - layout.canvasInsets.right) / 2 -
                  178
                }
              />
            </>
          ) : null}
          {props.authoring ? (
            <AuthoringDialogs
              authoring={props.authoring}
              readOnly={readOnly}
              design={props.design}
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
      </PortalContainer.Provider>
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
