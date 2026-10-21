import {
  type Act,
  type AgentController,
  type AuthoringController,
  agentNotice,
  type BindingContext,
  type CollaborationController,
  type ComposerController,
  type ConflictController,
  composerLine,
  type createCommandController,
  createEdgeUi,
  createShellActions,
  createSlidesController,
  type DesignController,
  documentNodes,
  type EdgeEnvironment,
  type EdgeHost,
  type EdgeUi,
  type EditorHost,
  type EditorSession,
  type ExportController,
  edgeBanner,
  type HistoryController,
  type InspectController,
  instanceTransaction,
  LARGE_FILE_LAYERS,
  type MissingFont,
  missingFonts,
  PANEL_LOADING,
  type PaintEditor,
  type PaletteController,
  type Platform,
  type PresenceController,
  type ResourceController,
  type SessionProblem,
  type SettingsController,
  type SlidesHost,
  selectionLabel,
  sessionProblem,
  type TokenSyncController,
  type ToolTransaction,
  type Unreadable,
  type Workspace,
  world,
} from "@vellum/editor-core";
import type { EngineClient } from "@vellum/engine-client";
import type { BrowserPort } from "@vellum/host-api";
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
import { ImageActions } from "./authoring/ImageActions";
import { EmbedProperties } from "./browser/BrowserImport";
import { CanvasEmbeds } from "./browser/CanvasEmbeds";
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
import { CollaborationLayer, CollaborationToolbar } from "./collaboration/Collaboration";
import { CanvasShaders } from "./design/CanvasShaders";
import type { DesignHost } from "./design/context";
import { DesignTab } from "./design/DesignTab";
import { useEngineSnapshot, useStore, useWorkspace } from "./hooks";
import { Icon } from "./icons/Icon";
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
import { ContextMenu, type ContextMenuState } from "./shell/ContextMenu";
import {
  AgentNotice,
  EdgeBannerView,
  EdgeDialogs,
  type FontsHost,
  type Rejection,
} from "./shell/EdgeLayer";
import { Palette } from "./shell/Palette";
import { SettingsDialog, type SettingsHost } from "./shell/SettingsDialog";
import { Presentation, SlideActions, SlideStrip } from "./slides/Slides";
import "./editor.css";
import "./authoring/authoring.css";
import "./ledger/ledger.css";
import "./shell/shell.css";

export type CommandController = ReturnType<typeof createCommandController>;

/**
 * What only the host knows about system and edge states (S01–S16); everything else is read from the engine, the
 * resources and the composer. Without it the workspace shows none of them.
 */
export interface EdgeProps {
  host: EdgeHost;
  /** Which surfaces the person closed; the workspace makes one when absent. */
  ui?: EdgeUi;
  /** The file is from a newer Vellum: it opens read-only (S04). */
  newerFormat?: number;
  /** The file could not be parsed (S05). */
  unreadable?: Unreadable;
  /** The window is closing with unsaved changes (S11). */
  closing?: { fileName: string; running: number };
  /** Past the multi-file threshold when absent: `LARGE_FILE_LAYERS`. */
  largeFile?: boolean;
  /** How many layers the file has, when the host counts them (a file the editor only partly holds). */
  layers?: number;
  fonts?: readonly MissingFont[];
  /** The session's engine problem; derived from the composer's engine catalog when absent. */
  problem?: SessionProblem | null;
  /** A write by an agent was refused while the person follows another (S07). */
  rejection?: Rejection | null;
  /** Panels whose content is on the way. */
  loading?: Partial<Record<"agent" | "layers" | "assets" | "history" | "design", boolean>>;
}

export interface EditorWorkspaceProps {
  /** Host uses the shared export renderer, live revision checks and native save UI. */
  slidesHost?: SlidesHost;
  nextPageId?(): PageId;
  /** Only supplied for a document whose live collaboration was explicitly enabled. */
  collaboration?: CollaborationController;
  browser?: BrowserPort;
  /** The command palette (⌘/), layer context menu and Editor settings. */
  shell?: {
    palette?: PaletteController;
    settings?: SettingsController;
    settingsHost?: SettingsHost;
  };
  edge?: EdgeProps;
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
    /** Host-owned document tabs, sharing the floating document bar's position. */
    documentBar?: ReactNode;
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
  onConnectAgents?: () => void;
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
  useStore(props.collaboration?.subscribe ?? NO_STORE);
  const collaborationReadOnly = !!props.collaboration && !props.collaboration.canEdit();
  useEffect(() => {
    session.setRoleReadOnly(collaborationReadOnly);
    return () => session.setRoleReadOnly(false);
  }, [session, collaborationReadOnly]);
  useStore(controller.commands.subscribe.bind(controller.commands));
  useStore(props.composer?.subscribe ?? NO_STORE);
  const shell = props.shell;
  const edge = props.edge;
  useStore(shell?.settings?.subscribe ?? NO_STORE);
  useStore(shell?.settings?.preferences.subscribe ?? NO_STORE);
  const edgeUi = useMemo(() => edge?.ui ?? createEdgeUi(), [edge?.ui]);
  useStore(edgeUi.subscribe);
  const root = useRef<HTMLDivElement | null>(null);
  // Menus and popovers mount inside the workspace so they follow its appearance.
  const [openEmbed, setOpenEmbed] = useState<{ address: string; revision: number }>({
    address: "",
    revision: 0,
  });
  const [container, setContainer] = useState<HTMLElement | null>(null);
  const setRoot = useCallback((el: HTMLDivElement | null) => {
    root.current = el;
    setContainer(el);
  }, []);
  const doc = snapshot.document;
  const [pageId, setPageId] = useState<PageId | null>(session.page || null);

  const slides = useMemo(
    () =>
      createSlidesController({
        engine,
        device: props.device,
        page: () => session.page,
        nextPageId: props.nextPageId ?? props.nextNodeId,
        onPage: (id) => {
          setPageId(id);
          session.setPage(id);
        },
        commit: (tx) => session.commit(tx),
        ...(props.slidesHost ? { host: props.slidesHost } : {}),
      }),
    [engine, session, props.device, props.nextPageId, props.nextNodeId, props.slidesHost],
  );
  useStore(slides.subscribe);
  useEffect(() => () => slides.dispose(), [slides]);
  const slidePage = doc?.pages.find((p) => p.id === session.page)?.kind === "slides";
  useEffect(() => {
    if (slides.state.presenting && slides.state.page !== session.page) slides.stop();
  }, [slides, session.page]);
  const selectSlide = (id: string, additive: boolean) => {
    const ids = additive
      ? session.state.selection.includes(id)
        ? session.state.selection.filter((n) => n !== id)
        : [...session.state.selection, id]
      : [id];
    session.select(ids);
    const b = session.sceneBounds(id);
    if (b && b.width > 0 && b.height > 0) {
      const left = layout.canvasInsets.left + 32,
        right = layout.canvasInsets.right + 32;
      const width = Math.max(1, state.width - left - right),
        height = Math.max(1, state.height - 240);
      const zoom = Math.min(width / b.width, height / b.height);
      session.setViewport({
        x: b.x - (left + (width - b.width * zoom) / 2) / zoom,
        y: b.y - (160 + (height - b.height * zoom) / 2) / zoom,
        zoom,
      });
    }
  };

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

  const readOnly = snapshot.readOnly || !!edge?.newerFormat || collaborationReadOnly;
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
  const prefs = shell?.settings?.preferences.state;
  const shellActions = useMemo(
    () =>
      createShellActions({
        workspace,
        session,
        document: () => engine.snapshot().document,
        composer,
        selectSession: (id) => void props.agent?.select(id),
        resources: props.resources,
      }),
    [workspace, session, engine, composer, props.agent, props.resources],
  );
  // The palette runs commands the way a shortcut does, and falls back to the host's own actions.
  const runRef = useRef(run);
  runRef.current = run;
  const palette = shell?.palette;
  useEffect(
    () =>
      palette?.bind({
        run: (id) => runRef.current(id),
        open: shellActions.open,
        ask: shellActions.ask,
      }),
    [palette, shellActions],
  );
  const blockedRef = useRef(false);
  const settings = shell?.settings;
  useEffect(() => {
    const added: { dispose(): void }[] = [];
    const add = (c: Parameters<typeof controller.registerCommand>[0]) => {
      if (!controller.commands.get(c.id)) added.push(controller.registerCommand(c));
    };
    if (settings)
      add({
        id: "settings.open",
        title: "Editor settings",
        category: "View",
        run: () => settings.open(),
      });
    if (composer) {
      const usable = () => session.state.selection.length > 0 && !blockedRef.current;
      add({
        id: "agent.askAbout",
        title: "Ask about this…",
        category: "Agents",
        enabled: usable,
        run: () => shellActions.askAbout(),
      });
      add({
        id: "agent.redesign",
        title: "Redesign this…",
        category: "Agents",
        enabled: () => usable() && !engine.snapshot().readOnly,
        run: () => shellActions.redesign(),
      });
    }
    return () => {
      for (const registration of added) registration.dispose();
    };
  }, [controller, settings, composer, session, engine, shellActions]);
  // Settings → Canvas: snapping follows the preferences; the rest is read where it is used.
  const snapObjects = prefs?.["canvas.snapObjects"];
  const snapGrid = prefs?.["canvas.snapPixelGrid"];
  useEffect(() => {
    if (snapObjects === undefined || snapGrid === undefined) return;
    session.setSnap({ objects: snapObjects, grid: snapGrid ? 1 : null });
  }, [session, snapObjects, snapGrid]);

  // The layer context menu (C24): right-click on a layer, in the panel or on the canvas.
  const [menu, setMenu] = useState<ContextMenuState | null>(null);
  const onContextMenu = (e: React.MouseEvent) => {
    if (!shell) return;
    const target = e.target as HTMLElement;
    const frameBox = root.current?.getBoundingClientRect();
    if (!frameBox || target.closest(".vp-menu")) return;
    let id: string | null = target.closest<HTMLElement>("[data-layer]")?.dataset.layer ?? null;
    const canvas = target.closest(".vc-canvas");
    if (!id && canvas) {
      const b = canvas.getBoundingClientRect();
      const hit = engine.scene.hitPoint(
        world({ x: e.clientX - b.left, y: e.clientY - b.top }, session.state.viewport),
        "deepest",
      );
      id = hit ?? null;
    }
    if (!id) return;
    e.preventDefault();
    const current = session.state.selection;
    if (!current.includes(id)) session.select([id]);
    setMenu({
      x: e.clientX - frameBox.left,
      y: e.clientY - frameBox.top,
      ids: current.includes(id) ? current : [id],
    });
  };

  // System and edge states (S01–S16): only when the host says it handles them.
  const layerCount = useMemo(() => (doc ? documentNodes(doc).length : 0), [doc]);
  const resources = props.resources;
  const fontList = useMemo(
    () =>
      edge?.fonts ??
      (doc && resources
        ? missingFonts(doc, resources.host.availableFonts, resources.host.fallbackFont ?? "Inter")
        : []),
    [edge?.fonts, doc, resources],
  );
  const catalog = composer?.state.catalog ?? null;
  const choice = composer?.state.choice ?? null;
  const env: EdgeEnvironment | null = edge
    ? {
        daemon: props.connection.mode,
        renderer: props.basicRenderer ? "basic" : "full",
        readOnly: edge.newerFormat
          ? { newerFormat: edge.newerFormat }
          : snapshot.readOnly
            ? "locked"
            : null,
        unreadable: edge.unreadable ?? null,
        fonts: fontList,
        libraries:
          resources?.host.libraries?.filter(
            (i) => resources.libraryInfo(i.alias)?.state === "missing",
          ).length ?? 0,
        layers: edge.layers ?? layerCount,
        largeFile: edge.largeFile ?? (edge.layers ?? layerCount) >= LARGE_FILE_LAYERS,
        problem:
          edge.problem !== undefined
            ? edge.problem
            : sessionProblem({
                catalog,
                choice,
                ...(props.composerHost?.timeZone ? { timeZone: props.composerHost.timeZone } : {}),
              }),
        closing: edge.closing ?? null,
      }
    : null;
  const notice = env ? agentNotice(env) : null;
  const banner = env ? edgeBanner(env, { dismissed: edgeUi.state.dismissed }) : null;
  const line = env ? composerLine(env) : null;
  blockedRef.current = !!line;
  const problemKey = env?.problem?.kind ?? null;
  const rendererFull = env?.renderer === "full";
  const daemonUp = env ? env.daemon !== "offline" : true;
  useEffect(() => {
    // A cause that went away is news again if it comes back.
    if (rendererFull) edgeUi.restore("renderer");
    if (daemonUp) edgeUi.restore("daemon-toast");
    if (problemKey !== "not-signed-in") edgeUi.restore("sign-in");
    if (!edge?.rejection) edgeUi.restore("rejection");
  }, [edgeUi, rendererFull, daemonUp, problemKey, edge?.rejection]);
  const runEdge = (a: Act) => {
    const engineId = choice?.provider ?? choice?.engine ?? "";
    switch (a.id) {
      case "dismiss":
        if (banner) edgeUi.dismiss(banner.id);
        return;
      case "rendererSettings":
        settings?.open();
        return;
      case "reviewFonts":
        edgeUi.openFonts();
        return;
      case "reviewChanges":
        workspace.switchTab("history", { reveal: true });
        edge?.host.action(a.id);
        return;
      case "switchEngine":
        composer?.open({ type: "picker" });
        return;
      case "updateKey":
        if (choice?.provider) composer?.open({ type: "setup", provider: choice.provider });
        else edge?.host.action(a.id);
        return;
      case "recheck":
        void composer?.recheck(engineId);
        edge?.host.action(a.id);
        return;
      default:
        edge?.host.action(a.id);
    }
  };
  const fontsHost: FontsHost = {
    jump: (nodes) => {
      session.select([...nodes]);
      edgeUi.closeFonts();
    },
    available: resources?.host.availableFonts ?? [],
    ...(resources?.host.action
      ? { install: (family: string) => resources.host.action?.("font.install", { family }) }
      : {}),
    ...(resources && !readOnly
      ? {
          replace: (family: string, to: string) =>
            resources.attempt(() => resources.replaceFont(family, to)),
        }
      : {}),
    ...(edge ? { addFolder: () => edge.host.action("openAsText", { fontFolder: true }) } : {}),
  };
  const sessionActor = props.agent?.session()?.agent;
  const noticeActor: ActorView = (sessionActor && props.actors[sessionActor]) ||
    props.agents[0] || { id: "agent", kind: "agent", name: "Agent" };
  const panelLoading = (
    panel: "agent" | "layers" | "assets" | "history" | "design",
    content: ReactNode,
  ) =>
    edge?.loading?.[panel] ? (
      <p className="vc-caption" role="status">
        {PANEL_LOADING[panel]}
      </p>
    ) : (
      content
    );
  const composerHostWithEdge = line
    ? {
        ...props.composerHost,
        blocked: {
          text: line.text,
          ...(line.action === "restartDaemon"
            ? {
                action: {
                  label: "Restart",
                  run: () => runEdge({ id: "restartDaemon", label: "Restart" }),
                },
              }
            : {}),
        },
      }
    : props.composerHost;
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
      data-animations={prefs && !prefs["canvas.animations"] ? "off" : undefined}
      style={
        prefs?.["canvas.backdrop"]
          ? ({ "--canvas-backdrop": prefs["canvas.backdrop"] } as React.CSSProperties)
          : undefined
      }
      onContextMenu={onContextMenu}
      onPointerDownCapture={(e) => {
        const c = props.collaboration;
        if (!c?.state.placing || focusOf(e.target) !== "canvas" || e.button !== 0) return;
        e.preventDefault();
        e.stopPropagation();
        const rect = e.currentTarget.getBoundingClientRect();
        c.newComment(
          null,
          world({ x: e.clientX - rect.left, y: e.clientY - rect.top }, session.state.viewport),
        );
      }}
      onPointerMove={(e) => {
        if (!props.collaboration || focusOf(e.target) !== "canvas") return;
        const rect = e.currentTarget.getBoundingClientRect();
        props.collaboration.publishView({
          page: session.page,
          cursor: world(
            { x: e.clientX - rect.left, y: e.clientY - rect.top },
            session.state.viewport,
          ),
          viewport: session.state.viewport,
          selection: [...session.state.selection],
        });
      }}
      onKeyDown={(e) => {
        const binding = controller.keys.fromEvent(e.nativeEvent, {
          host: props.host,
          platform: props.platform,
          focus: slides.state.presenting ? "presentation" : context(e.target).focus,
        });
        if (slides.state.presenting) {
          if (binding?.command === "present.next") {
            e.preventDefault();
            slides.next();
          } else if (binding?.command === "present.previous") {
            e.preventDefault();
            slides.previous();
          } else if (binding?.command === "present.stop") {
            e.preventDefault();
            slides.stop();
          }
          return;
        }
        if (binding?.command === "present.start" && slidePage && slides.canPresent()) {
          e.preventDefault();
          slides.start(session.state.selection[0]);
          return;
        }
        if (e.defaultPrevented) return;
        if (props.collaboration && e.key === "Escape") {
          if (props.collaboration.state.placing) {
            props.collaboration.placeComment(false);
            e.preventDefault();
            return;
          }
          if (props.collaboration.state.panel) {
            props.collaboration.close();
            e.preventDefault();
            return;
          }
          if (props.collaboration.state.following) {
            props.collaboration.follow(null);
            e.preventDefault();
            return;
          }
        }
        if (
          props.collaboration &&
          e.key.toLowerCase() === "c" &&
          !e.ctrlKey &&
          !e.metaKey &&
          !e.altKey &&
          focusOf(e.target) === "canvas" &&
          props.collaboration.canComment()
        ) {
          const id = session.state.selection[0] ?? null;
          const bounds = id ? session.sceneBounds(id) : null;
          if (id && bounds)
            props.collaboration.newComment(id, { x: bounds.x + bounds.width, y: bounds.y });
          else props.collaboration.placeComment(true);
          e.preventDefault();
          return;
        }
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
            {...(prefs
              ? {
                  wheel: {
                    zooms: prefs["canvas.wheelZooms"],
                    invert: prefs["canvas.invertZoom"],
                  },
                }
              : {})}
            {...(props.componentRoots ? { componentRoots: props.componentRoots } : {})}
            {...(props.content ? { content: props.content } : {})}
            {...(props.createRenderer ? { createRenderer: props.createRenderer } : {})}
            {...(props.paintEditor ? { paintEditor: props.paintEditor } : {})}
            {...(props.design
              ? { colour: (c: Value<string>) => props.design?.colour(c, doc) ?? (c as string) }
              : {})}
          />
          {props.browser ? (
            <CanvasEmbeds session={session} browser={props.browser} open={openEmbed} />
          ) : null}
          {designHost.shaders ? (
            <CanvasShaders session={session} controller={designHost.shaders} />
          ) : null}
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
          {props.collaboration ? (
            <CollaborationLayer controller={props.collaboration} session={session} />
          ) : null}
          {visible.documentBar
            ? (props.slots?.documentBar ?? (
                <DocumentBar
                  fileName={props.fileName}
                  saveState={props.saveState}
                  width={layout.documentBar.width}
                  commands={controller.commands.list()}
                />
              ))
            : null}
          <PanelFrame
            layout={layout.agent}
            tab={state.tab}
            onTab={(tab) => workspace.switchTab(tab, { reveal: true })}
            onCloseOverlay={() => workspace.closeAgentOverlay()}
            shortcut={shortcut("agent.focus")}
          >
            {{
              agent: props.agent
                ? panelLoading(
                    "agent",
                    <AgentTab
                      agent={props.agent}
                      actors={props.actors}
                      {...(composer ? { composer } : {})}
                      {...(composerHostWithEdge ? { composerHost: composerHostWithEdge } : {})}
                      composerSlot={props.slots?.agent}
                      {...(notice && !edge?.loading?.agent
                        ? {
                            notice: (
                              <AgentNotice
                                notice={notice}
                                actor={noticeActor}
                                model={composer?.modelLine() ?? null}
                                run={runEdge}
                              />
                            ),
                          }
                        : {})}
                    />,
                  )
                : (props.slots?.agent ?? null),
              history: props.ledger?.history
                ? panelLoading(
                    "history",
                    <HistoryTab
                      history={props.ledger.history}
                      readOnly={readOnly}
                      {...(props.ledger.timeZone ? { timeZone: props.ledger.timeZone } : {})}
                      {...(composer ? { onAsk: () => shellActions.askAbout() } : {})}
                    />,
                  )
                : (props.slots?.history ?? null),
              layers: slidePage ? (
                <SlideStrip
                  controller={slides}
                  selection={session.state.selection}
                  onSelect={selectSlide}
                  readOnly={readOnly}
                />
              ) : (
                panelLoading(
                  "layers",
                  <LayersTab
                    onNewSlides={() => slides.newPage()}
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
                  />,
                )
              ),
              assets: panelLoading(
                "assets",
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
                />,
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
          {visible.topRight && slidePage ? (
            <SlideActions
              controller={slides}
              selection={session.state.selection}
              shortcut={shortcut("present.start")}
            />
          ) : null}
          {visible.topRight && props.collaboration ? (
            <CollaborationToolbar
              controller={props.collaboration}
              {...(props.presence ? { presence: props.presence } : {})}
              {...(props.onShare ? { onShare: props.onShare } : {})}
              {...(props.onSettings ? { onSettings: props.onSettings } : {})}
            />
          ) : visible.topRight ? (
            <TopRightBar
              right={layout.topRight.right}
              agents={props.agents}
              {...(props.presence ? { presence: props.presence } : {})}
              {...(props.onShare ? { onShare: props.onShare } : {})}
              {...(props.onPreview ? { onPreview: props.onPreview } : {})}
              {...(props.onSettings
                ? { onSettings: props.onSettings }
                : settings
                  ? { onSettings: () => settings.open() }
                  : {})}
              {...(props.onPauseAll ? { onPauseAll: props.onPauseAll } : {})}
              {...(props.onConnectAgents ? { onConnectAgents: props.onConnectAgents } : {})}
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
                    selectedLibrary?.type === "embed" ? (
                      <EmbedProperties
                        node={selectedLibrary}
                        {...(props.browser
                          ? {
                              onOpen: () =>
                                setOpenEmbed((previous) => ({
                                  address: session.state.selection[0] ?? "",
                                  revision: previous.revision + 1,
                                })),
                            }
                          : {})}
                        disabled={readOnly}
                        onChange={(patch) => {
                          props.design?.edit(() => patch);
                        }}
                      />
                    ) : session.state.mode.type === "scaling" ? (
                      <ScalePanel session={session} disabled={readOnly} />
                    ) : session.state.mode.type === "path" ? (
                      <VectorPanel session={session} design={props.design} disabled={readOnly} />
                    ) : (
                      <>
                        {props.authoring &&
                        inspected &&
                        selectedLibrary &&
                        (selectedLibrary.type === "image" ||
                          ("fills" in selectedLibrary &&
                            selectedLibrary.fills?.some((p) => p.type === "image"))) ? (
                          <ImageActions
                            authoring={props.authoring}
                            address={inspected}
                            disabled={readOnly}
                          />
                        ) : null}
                        <DesignTab
                          design={props.design}
                          session={session}
                          paintEditor={props.paintEditor}
                          document={doc}
                          readOnly={readOnly}
                          host={designHost}
                        />
                      </>
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
          {env ? (
            <>
              {banner ? (
                <EdgeBannerView
                  banner={banner}
                  top={props.ledger?.conflicts?.banner() ? 80 : 44}
                  inset={layout.canvasInsets.left}
                  run={runEdge}
                />
              ) : null}
              {typeof env.readOnly === "object" && env.readOnly ? (
                <div className="ve-viewonly" style={{ left: layout.canvasInsets.left + 44 }}>
                  <Icon name="lock" size={16} />
                  View only · file is not downgraded
                </div>
              ) : null}
              <EdgeDialogs
                env={env}
                ui={edgeUi}
                host={edge?.host as EdgeHost}
                file={props.fileName}
                notice={notice}
                fontsHost={fontsHost}
                readOnly={readOnly}
                run={runEdge}
                rejection={edge?.rejection}
                toastLeft={layout.bottomStrip.centerX - 41}
              />
            </>
          ) : null}
          {shell?.palette ? <Palette palette={shell.palette} shortcut={shortcut} /> : null}
          {settings ? (
            <SettingsDialog
              settings={settings}
              composer={composer}
              {...(shell?.settingsHost ? { host: shell.settingsHost } : {})}
              platform={props.platform}
            />
          ) : null}
          {menu ? (
            <ContextMenu
              menu={menu}
              enabled={enabled}
              shortcut={shortcut}
              size={{ width: state.width, height: state.height }}
              mac={props.platform === "mac"}
              onClose={() => setMenu(null)}
              run={(command) => run(command)}
              readOnly={readOnly}
            />
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
          {slides.state.presenting ? (
            <Presentation controller={slides} revision={snapshot.revision} />
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
