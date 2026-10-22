import type { Disposable } from "./registries";
import type { Box } from "./tool-types";

/** The layout rules of DESIGN.md "Layout and navigation". Pure: sizes in, rectangles out. */
export const MIN_EDITOR_WIDTH = 960;
export const INSET = 8;
export const NARROW_BELOW = 1040;
export const STRIP_BELOW = 960;
export const RIGHT_OVERLAY_BELOW = 1200;
export const AGENT_WIDTH = { default: 360, narrow: 320, min: 320, max: 520 } as const;
export const RIGHT_WIDTH = { default: 280, min: 260, max: 400 } as const;
const BAR_HEIGHT = 40;
const STRIP_WIDTH = 44;
const RAIL = { width: 44, height: 310 } as const;
const PILL = { width: 60, height: 32 } as const;
const TOP = INSET + BAR_HEIGHT + INSET;

export const PANEL_TABS = ["agent", "layers", "assets", "history"] as const;
export type PanelTab = (typeof PANEL_TABS)[number];
export type AgentMode = "open" | "overlay" | "strip" | "hidden";
export type RightMode = "docked" | "overlay" | "hidden";

export interface WorkspaceState {
  /** Chrome rectangles use logical start; canvas fit insets remain physical. */
  direction?: "ltr" | "rtl";
  width: number;
  height: number;
  tab: PanelTab;
  agentWidth: number;
  rightWidth: number;
  /** ⌘\ . */
  panelsHidden: boolean;
  /** Tab: everything but the canvas. */
  chromeHidden: boolean;
  /** The collapsed strip's icon has opened the panel over the canvas. */
  agentOverlayOpen: boolean;
  hasSelection: boolean;
  documentPropertiesOpen: boolean;
}

export interface WorkspaceLayout {
  chromeHidden: boolean;
  agent: { mode: AgentMode; rect: Box };
  right: { mode: RightMode; rect: Box };
  documentBar: Box;
  toolRail: Box;
  /** Content-sized: only the inset it hangs from is known here. */
  topRight: { right: number; y: number; height: number };
  bottomStrip: { centerX: number; y: number; height: number };
  zoomPill: Box;
  visible: {
    documentBar: boolean;
    toolRail: boolean;
    topRight: boolean;
    bottomStrip: boolean;
    zoomPill: boolean;
  };
  /** How much of the canvas docked chrome covers, for fit and centre commands. */
  canvasInsets: { left: number; right: number };
}

const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));

export function computeLayout(
  size: { width: number; height: number },
  state: WorkspaceState,
): WorkspaceLayout {
  const { width, height } = size;
  const panelHeight = height - TOP - INSET;
  const full = width >= NARROW_BELOW;
  const collapsed = width < STRIP_BELOW;
  const openWidth = full
    ? clamp(state.agentWidth, AGENT_WIDTH.min, AGENT_WIDTH.max)
    : AGENT_WIDTH.narrow;
  const hidden = state.chromeHidden || state.panelsHidden;
  let agentMode: AgentMode;
  if (hidden) agentMode = "hidden";
  else if (collapsed) agentMode = state.agentOverlayOpen ? "overlay" : "strip";
  else agentMode = "open";
  const agentRectWidth = agentMode === "strip" ? STRIP_WIDTH : openWidth;
  const agent = {
    mode: agentMode,
    rect: { x: INSET, y: TOP, width: agentRectWidth, height: panelHeight },
  };
  // The document bar keeps the open panel's width so hiding panels never moves it.
  const documentBar = {
    x: INSET,
    y: INSET,
    width: collapsed ? AGENT_WIDTH.narrow : openWidth,
    height: BAR_HEIGHT,
  };
  // The rail docks to a panel that takes layout space (open or strip), never to an overlay or hidden one.
  const railX =
    agentMode === "open" || agentMode === "strip"
      ? INSET + agentRectWidth + INSET
      : collapsed && agentMode === "overlay"
        ? INSET + STRIP_WIDTH + INSET
        : INSET;
  const toolRail = { x: railX, y: TOP, width: RAIL.width, height: RAIL.height };
  const rightWidth = clamp(state.rightWidth, RIGHT_WIDTH.min, RIGHT_WIDTH.max);
  const wanted = state.hasSelection || state.documentPropertiesOpen;
  const rightMode: RightMode =
    hidden || !wanted ? "hidden" : width < RIGHT_OVERLAY_BELOW ? "overlay" : "docked";
  const right = {
    mode: rightMode,
    rect: { x: width - INSET - rightWidth, y: TOP, width: rightWidth, height: panelHeight },
  };
  const shown = !state.chromeHidden;
  return {
    chromeHidden: state.chromeHidden,
    agent,
    right,
    documentBar,
    toolRail,
    topRight: { right: INSET, y: INSET, height: BAR_HEIGHT },
    bottomStrip: { centerX: width / 2, y: height - INSET - PILL.height, height: PILL.height },
    zoomPill: {
      x: width - INSET - PILL.width,
      y: height - INSET - PILL.height,
      width: PILL.width,
      height: PILL.height,
    },
    visible: {
      documentBar: shown,
      toolRail: shown,
      topRight: shown,
      bottomStrip: shown,
      zoomPill: shown,
    },
    canvasInsets:
      state.direction === "rtl"
        ? {
            left: rightMode === "docked" ? rightWidth + INSET * 2 : 0,
            right: shown ? toolRail.x + toolRail.width + INSET : 0,
          }
        : {
            left: shown ? toolRail.x + toolRail.width + INSET : 0,
            right: rightMode === "docked" ? rightWidth + INSET * 2 : 0,
          },
  };
}

export function initialWorkspaceState(): WorkspaceState {
  return {
    width: 1280,
    height: 800,
    tab: "agent",
    agentWidth: AGENT_WIDTH.default,
    rightWidth: RIGHT_WIDTH.default,
    panelsHidden: false,
    chromeHidden: false,
    agentOverlayOpen: false,
    hasSelection: false,
    documentPropertiesOpen: false,
  };
}

/** Editor-scoped layout and panel state. No DOM and no document access; React reads it through `subscribe`. */
export function createWorkspace() {
  let state = initialWorkspaceState();
  let documentId: string | null = null;
  const tabs = new Map<string | null, PanelTab>();
  const listeners = new Set<() => void>();
  const set = (patch: Partial<WorkspaceState>) => {
    const next = { ...state, ...patch };
    if ((Object.keys(next) as (keyof WorkspaceState)[]).every((k) => next[k] === state[k])) return;
    state = next;
    for (const listener of [...listeners]) listener();
  };
  const strip = () => state.width < STRIP_BELOW;
  return {
    get state(): Readonly<WorkspaceState> {
      return state;
    },
    layout: () => computeLayout(state, state),
    setDirection(direction: "ltr" | "rtl") {
      set({ direction });
    },
    subscribe(listener: () => void): Disposable {
      listeners.add(listener);
      return {
        dispose() {
          listeners.delete(listener);
        },
      };
    },
    resize(size: { width: number; height: number }) {
      const grewOut = size.width >= STRIP_BELOW;
      set({
        width: size.width,
        height: size.height,
        ...(grewOut ? { agentOverlayOpen: false } : {}),
      });
    },
    setSelection(hasSelection: boolean) {
      set({ hasSelection });
    },
    setDocumentProperties(open: boolean) {
      set({ documentPropertiesOpen: open });
    },
    setAgentWidth(width: number) {
      set({ agentWidth: clamp(width, AGENT_WIDTH.min, AGENT_WIDTH.max) });
    },
    setRightWidth(width: number) {
      set({ rightWidth: clamp(width, RIGHT_WIDTH.min, RIGHT_WIDTH.max) });
    },
    /** Tab. */
    hideAllPanels() {
      set({ chromeHidden: !state.chromeHidden });
    },
    /** ⌘\ . With everything hidden by Tab it shows the panels again instead. */
    togglePanels() {
      if (state.chromeHidden) set({ chromeHidden: false, panelsHidden: false });
      else set({ panelsHidden: !state.panelsHidden });
    },
    openAgentOverlay() {
      set({ agentOverlayOpen: true, chromeHidden: false, panelsHidden: false });
    },
    closeAgentOverlay() {
      set({ agentOverlayOpen: false });
    },
    /** `reveal` is for shortcuts and the strip: asking for a tab shows the panel that holds it. */
    switchTab(tab: PanelTab, options: { reveal?: boolean } = {}) {
      tabs.set(documentId, tab);
      const reveal = options.reveal
        ? {
            chromeHidden: false,
            panelsHidden: false,
            ...(strip() ? { agentOverlayOpen: true } : {}),
          }
        : {};
      set({ tab, ...reveal });
    },
    /** The panel remembers the last tab per document. */
    openDocument(id: string) {
      tabs.set(documentId, state.tab);
      documentId = id;
      set({ tab: tabs.get(id) ?? "agent" });
    },
    zoomLabel(zoom: number): string {
      return `${Math.max(1, Math.round(zoom * 100))}%`;
    },
  };
}
export type Workspace = ReturnType<typeof createWorkspace>;
