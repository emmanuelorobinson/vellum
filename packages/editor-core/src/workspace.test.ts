import { describe, expect, it } from "vitest";
import { computeLayout, createWorkspace, MIN_EDITOR_WIDTH, type WorkspaceState } from "./workspace";

const base = (over: Partial<WorkspaceState> = {}): WorkspaceState => ({
  ...createWorkspace().state,
  ...over,
});
const at = (width: number, over: Partial<WorkspaceState> = {}, selected = false, height = 800) =>
  computeLayout({ width, height }, base({ hasSelection: selected, ...over }));

describe("responsive layout (DESIGN.md Layout and navigation)", () => {
  it("floats the agent panel at 360px with an 8px inset at 1280", () => {
    const l = at(1280);
    expect(l.agent).toEqual({ mode: "open", rect: { x: 8, y: 56, width: 360, height: 736 } });
    expect(l.documentBar).toEqual({ x: 8, y: 8, width: 360, height: 40 });
    expect(l.toolRail).toEqual({ x: 376, y: 56, width: 44, height: 310 });
    expect(l.zoomPill).toEqual({ x: 1212, y: 760, width: 60, height: 32 });
  });
  it("keeps the top-right bar against the right inset and the bottom strip centred", () => {
    const l = at(1280, {}, false);
    expect(l.topRight).toEqual({ right: 8, y: 8, height: 40 });
    expect(l.bottomStrip).toEqual({ centerX: 640, y: 760, height: 32 });
  });
  it("narrows the agent panel to 320 below 1040", () => {
    expect(at(1040).agent.rect.width).toBe(360);
    expect(at(1039).agent.rect.width).toBe(320);
    const l = at(960);
    expect(l.agent).toEqual({ mode: "open", rect: { x: 8, y: 56, width: 320, height: 736 } });
    expect(l.documentBar.width).toBe(320);
    expect(l.toolRail.x).toBe(336);
  });
  it("collapses the agent panel to a 44px strip below 960 and keeps the rail clear of it", () => {
    expect(MIN_EDITOR_WIDTH).toBe(960);
    const l = at(760);
    expect(l.agent.mode).toBe("strip");
    expect(l.agent.rect).toEqual({ x: 8, y: 56, width: 44, height: 736 });
    expect(l.toolRail.x).toBe(60);
  });
  it("opens the collapsed panel as an overlay from the strip and closes it again", () => {
    const w = createWorkspace();
    w.resize({ width: 760, height: 640 });
    expect(w.layout().agent.mode).toBe("strip");
    w.openAgentOverlay();
    expect(w.layout().agent.mode).toBe("overlay");
    expect(w.layout().agent.rect.width).toBe(320);
    w.closeAgentOverlay();
    expect(w.layout().agent.mode).toBe("strip");
  });
  it("closes the overlay when the window grows back to a docked width", () => {
    const w = createWorkspace();
    w.resize({ width: 760, height: 640 });
    w.openAgentOverlay();
    w.resize({ width: 1280, height: 800 });
    expect(w.layout().agent.mode).toBe("open");
    w.resize({ width: 760, height: 640 });
    expect(w.layout().agent.mode).toBe("strip");
  });
  it("honours a resized agent panel between 320 and 520 only at full width", () => {
    expect(at(1400, { agentWidth: 480 }).agent.rect.width).toBe(480);
    expect(at(1400, { agentWidth: 900 }).agent.rect.width).toBe(520);
    expect(at(1400, { agentWidth: 100 }).agent.rect.width).toBe(320);
    expect(at(1000, { agentWidth: 480 }).agent.rect.width).toBe(320);
    expect(at(1400, { agentWidth: 480 }).documentBar.width).toBe(480);
    expect(at(1400, { agentWidth: 480 }).toolRail.x).toBe(496);
  });
});

describe("right panel", () => {
  it("appears only when something is selected or document properties are open", () => {
    expect(at(1280).right.mode).toBe("hidden");
    expect(at(1280, {}, true).right.mode).toBe("docked");
    expect(at(1280, { documentPropertiesOpen: true }).right.mode).toBe("docked");
  });
  it("is 280px wide, floating on the right inset, below the top-right bar", () => {
    const l = at(1280, {}, true);
    expect(l.right.rect).toEqual({ x: 992, y: 56, width: 280, height: 736 });
  });
  it("overlays the canvas below 1200 and docks from 1200", () => {
    expect(at(1200, {}, true).right.mode).toBe("docked");
    expect(at(1199, {}, true).right.mode).toBe("overlay");
  });
  it("clamps a resized right panel to 260-400", () => {
    expect(at(1400, { rightWidth: 500 }, true).right.rect.width).toBe(400);
    expect(at(1400, { rightWidth: 100 }, true).right.rect.width).toBe(260);
  });
  it("reports the canvas insets that docked panels take and that overlays do not", () => {
    expect(at(1280, {}, false).canvasInsets).toEqual({ left: 428, right: 0 });
    expect(at(1280, {}, true).canvasInsets.right).toBe(280 + 8 + 8);
    expect(at(1100, {}, true).canvasInsets.right).toBe(0);
  });
});

describe("hiding panels", () => {
  it("Tab hides every piece of chrome and Tab again brings it back", () => {
    const w = createWorkspace();
    w.resize({ width: 1280, height: 800 });
    w.setSelection(true);
    w.hideAllPanels();
    const l = w.layout();
    expect(l.chromeHidden).toBe(true);
    expect(l.agent.mode).toBe("hidden");
    expect(l.right.mode).toBe("hidden");
    expect(l.visible).toEqual({
      documentBar: false,
      toolRail: false,
      topRight: false,
      bottomStrip: false,
      zoomPill: false,
    });
    w.hideAllPanels();
    expect(w.layout().agent.mode).toBe("open");
    expect(w.layout().visible.documentBar).toBe(true);
  });
  it("⌘\\ toggles only the agent and right panels and leaves the bars", () => {
    const w = createWorkspace();
    w.resize({ width: 1280, height: 800 });
    w.setSelection(true);
    w.togglePanels();
    let l = w.layout();
    expect(l.agent.mode).toBe("hidden");
    expect(l.right.mode).toBe("hidden");
    expect(l.visible.documentBar).toBe(true);
    expect(l.visible.toolRail).toBe(true);
    // A rail docked to a panel that is gone moves back to the inset.
    expect(l.toolRail.x).toBe(8);
    w.togglePanels();
    l = w.layout();
    expect(l.agent.mode).toBe("open");
    expect(l.right.mode).toBe("docked");
  });
  it("⌘\\ with everything hidden by Tab shows the panels again", () => {
    const w = createWorkspace();
    w.resize({ width: 1280, height: 800 });
    w.hideAllPanels();
    w.togglePanels();
    expect(w.layout().chromeHidden).toBe(false);
    expect(w.layout().agent.mode).toBe("open");
  });
  it("Tab wins over ⌘\\ and neither clears the other", () => {
    const w = createWorkspace();
    w.resize({ width: 1280, height: 800 });
    w.togglePanels();
    w.hideAllPanels();
    w.hideAllPanels();
    expect(w.layout().agent.mode).toBe("hidden");
    w.togglePanels();
    expect(w.layout().agent.mode).toBe("open");
  });
});

describe("panel tabs", () => {
  it("switches Agent, Layers, Assets and History and remembers the last tab per document", () => {
    const w = createWorkspace();
    expect(w.state.tab).toBe("agent");
    w.switchTab("layers");
    expect(w.state.tab).toBe("layers");
    w.openDocument("a.vellum");
    expect(w.state.tab).toBe("agent");
    w.switchTab("assets");
    w.openDocument("b.vellum");
    expect(w.state.tab).toBe("agent");
    w.openDocument("a.vellum");
    expect(w.state.tab).toBe("assets");
    w.openDocument("b.vellum");
    w.switchTab("history");
    w.openDocument("a.vellum");
    expect(w.state.tab).toBe("assets");
  });
  it("shows a hidden panel when a tab is asked for by shortcut", () => {
    const w = createWorkspace();
    w.resize({ width: 1280, height: 800 });
    w.togglePanels();
    w.switchTab("layers", { reveal: true });
    expect(w.layout().agent.mode).toBe("open");
    expect(w.state.tab).toBe("layers");
  });
  it("opens the overlay for a tab chosen from the strip", () => {
    const w = createWorkspace();
    w.resize({ width: 760, height: 640 });
    w.switchTab("assets", { reveal: true });
    expect(w.layout().agent.mode).toBe("overlay");
    expect(w.state.tab).toBe("assets");
  });
  it("notifies subscribers once per change and not after dispose", () => {
    const w = createWorkspace();
    let calls = 0;
    const s = w.subscribe(() => {
      calls++;
    });
    w.switchTab("layers");
    w.switchTab("layers");
    expect(calls).toBe(1);
    s.dispose();
    w.switchTab("assets");
    expect(calls).toBe(1);
  });
});

describe("zoom pill", () => {
  it("shows a whole-number percentage and the menu steps", () => {
    const w = createWorkspace();
    expect(w.zoomLabel(0.5)).toBe("50%");
    expect(w.zoomLabel(0.7249)).toBe("72%");
    expect(w.zoomLabel(0.0125)).toBe("1%");
    expect(w.zoomLabel(64)).toBe("6400%");
  });
});
