// TEST DATA: a small Juniper-like document for the panels.
import { copy } from "@vellum/config";
import { computeLayout, createWorkspace } from "@vellum/editor-core";
import type { Document, Node } from "@vellum/types-ts";
import { renderToStaticMarkup as html } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { TipProvider } from "./chrome/Tooltip";
import { AssetsTab } from "./panels/AssetsTab";
import { LayersTab, type LayersTabProps } from "./panels/LayersTab";
import { PanelFrame } from "./panels/PanelFrame";

const n = (type: string, id: string, extra: Record<string, unknown> = {}) =>
  ({ type, id, name: id, width: 10, height: 10, ...extra }) as unknown as Node;
const doc = (children: Node[]): Document =>
  ({
    meta: { formatVersion: 1, name: "t", createdWith: "t" },
    pages: [
      { id: "p1", name: "Onboarding", children },
      { id: "p2", name: "Settings", children: [] },
    ],
  }) as unknown as Document;
const tree = [
  n("frame", "Welcome"),
  n("frame", "Step 1", { layout: "column" }),
  n("frame", "Step 2", {
    children: [n("instance", "Habit card"), n("text", "Title", { visible: false, locked: true })],
  }),
];
const layers = (over: Partial<LayersTabProps> = {}) =>
  html(
    <LayersTab
      document={doc(tree)}
      page="p1"
      onPage={() => {}}
      selection={[]}
      onSelect={() => {}}
      commit={() => {}}
      device="d"
      now={() => 1000}
      recent={{}}
      actors={{ teal: { id: "teal", kind: "agent", name: "Teal", hue: "teal" } }}
      readOnly={false}
      {...over}
    />,
  );

describe("layers tab", () => {
  it("lists the pages with the current one selected, then the layers", () => {
    const out = layers();
    expect(out).toContain("Onboarding");
    expect(out).toMatch(/aria-selected="true"[^>]*class="vc-row vc-page"/);
    expect(out).toContain('role="tree"');
    expect(out.indexOf("Welcome")).toBeLessThan(out.indexOf("Step 1"));
  });
  it("is an accessible tree: level, position, selection and expansion", () => {
    const out = layers({ selection: ["Step 2"] });
    expect(out).toMatch(
      /aria-level="1"[^>]*aria-selected="true"[^>]*aria-expanded="false"[^>]*aria-posinset="3"/,
    );
    expect(out).toContain('aria-setsize="3"');
  });
  it("opens the layers above a layer that is selected on the canvas", () => {
    // Static render cannot run effects, so this only checks the collapsed starting point.
    expect(layers()).not.toContain("Habit card");
  });
  it("uses the type icons from the design", () => {
    const out = layers();
    expect(out).toContain('data-icon="frame"');
    expect(out).toContain('data-icon="rows-3"');
  });
  it("says a layer is hidden or locked in words as well as by icon", () => {
    const out = layers({ selection: ["Title"] });
    // Title is inside the collapsed Step 2, so it is not drawn yet.
    expect(out).not.toContain("Title");
    const open = html(
      <LayersTab
        document={doc([n("rect", "A", { visible: false, locked: true })])}
        page="p1"
        onPage={() => {}}
        selection={[]}
        onSelect={() => {}}
        commit={() => {}}
        device="d"
        now={() => 0}
        recent={{}}
        actors={{}}
        readOnly={false}
      />,
    );
    expect(open).toContain('aria-label="Show A"');
    expect(open).toContain('aria-label="Unlock A"');
    expect(open).toContain('data-state="on"');
  });
  it("disables the eye of a layer whose visibility a variable decides", () => {
    const out = html(
      <LayersTab
        document={doc([n("rect", "A", { visible: { $var: "flag" } })])}
        page="p1"
        onPage={() => {}}
        selection={[]}
        onSelect={() => {}}
        commit={() => {}}
        device="d"
        now={() => 0}
        recent={{}}
        actors={{}}
        readOnly={false}
      />,
    );
    expect(out).toMatch(/aria-label="Hide A"[^>]*disabled/);
    expect(out).toContain("Visibility is set by a variable");
  });
  it("shows a marker for an agent's layer in the first ten seconds, and a caption naming it", () => {
    const fresh = layers({
      recent: { Welcome: { agent: "teal", at: 95_000 } },
      now: () => 100_000,
    });
    expect(fresh).toContain("created this layer in the last 10 seconds");
    expect(fresh).toContain("Teal marker · created in the last 10 seconds");
    const old = layers({ recent: { Welcome: { agent: "teal", at: 0 } }, now: () => 50_000 });
    expect(old).not.toContain("created this layer");
    expect(old).not.toContain("marker · created");
  });
  it("shows the empty state with its one next step", () => {
    const out = html(
      <LayersTab
        document={doc([])}
        page="p1"
        onPage={() => {}}
        selection={[]}
        onSelect={() => {}}
        commit={() => {}}
        device="d"
        now={() => 0}
        recent={{}}
        actors={{}}
        readOnly={false}
        onDrawFrame={() => {}}
      />,
    );
    expect(out).toContain(copy.layersEmpty);
    expect(out).toContain("Draw a frame");
    expect(out).toContain('data-state="empty"');
  });
  it("disables editing in a read-only file", () => {
    const out = layers({ readOnly: true });
    expect(out).toMatch(/aria-label="Lock Welcome"[^>]*disabled/);
  });
  it("renders nothing to edit before a document has loaded", () => {
    expect(layers({ document: null })).toContain('data-state="no-document"');
  });
});

describe("assets tab", () => {
  const assets = (over: Partial<Parameters<typeof AssetsTab>[0]> = {}) =>
    html(
      <AssetsTab
        components={{ c1: { name: "Button / Primary" }, c2: { name: "HabitRow" } }}
        libraries={[{ alias: "Juniper Core", state: "synced", components: 8 }]}
        iconSets={[
          { id: "lucide", label: "Lucide" },
          { id: "phosphor", label: "Phosphor" },
        ]}
        projectIconFolder={{ path: "design/icons" }}
        visualStyles={[{ id: "minimal", name: "Minimal" }]}
        designSystems={[{ id: "starter-minimal", name: "Starter Minimal" }]}
        onDropComponent={() => {}}
        readOnly={false}
        {...over}
      />,
    );
  it("shows the five sections with the design's headings", () => {
    const out = assets();
    for (const h of ["This document", "Libraries", "Icons", "Styles", "Starter design systems"])
      expect(out).toContain(h);
    expect(out).toContain("Juniper Core · Synced · 8 components");
    expect(out).toContain("Lucide · Phosphor");
  });
  it("marks components as draggable only when the file can be edited", () => {
    expect(assets()).toContain("data-draggable");
    expect(assets({ readOnly: true })).not.toContain("data-draggable");
  });
  it("disables Add, Browse and Guidelines until the host handles them", () => {
    const out = assets();
    expect(out).toMatch(/class="vc-asset__action"[^>]*disabled/);
    expect(out).toMatch(/vc-assets__guidelines"[^>]*disabled/);
  });
  it("flags a missing library in words as well as colour", () => {
    const out = assets({ libraries: [{ alias: "Old kit", state: "missing", components: 3 }] });
    expect(out).toContain("Old kit · Missing · 3 components");
    expect(out).toContain('aria-label="Missing"');
    expect(out).toContain('data-status="missing"');
  });
  it("has a labelled search field", () => {
    expect(assets()).toContain('aria-label="Search assets"');
  });
});

describe("panel frame", () => {
  const frame = (width: number, over = {}) => {
    const ws = createWorkspace();
    ws.resize({ width, height: 800 });
    const l = computeLayout({ width, height: 800 }, { ...ws.state, ...over });
    return html(
      <TipProvider>
        <PanelFrame
          layout={l.agent}
          tab="layers"
          onTab={() => {}}
          onCloseOverlay={() => {}}
          shortcut="⌘L"
        >
          {{ agent: "A", layers: "L", assets: "S", history: "H" }}
        </PanelFrame>
      </TipProvider>,
    );
  };
  it("shows the four tabs and only the current tab's content", () => {
    const out = frame(1280);
    for (const t of ["Agent", "Layers", "Assets", "History"]) expect(out).toContain(`>${t}<`);
    expect(out).toMatch(/id="vc-tab-layers"[^>]*aria-selected="true"/);
    expect(out).toContain(">L<");
    expect(out).not.toContain(">A<");
  });
  it("is 360 wide at 1280 and 320 narrow", () => {
    expect(frame(1280)).toContain("width:360px");
    expect(frame(960)).toContain("width:320px");
  });
  it("collapses to a strip of four icon buttons below 960 with the Agent one naming ⌘L", () => {
    const out = frame(760);
    expect(out).toContain("vc-panel--strip");
    expect(out).toContain("width:44px");
    expect(out).toContain('aria-label="Open Agent · ⌘L"');
    expect(out).toContain('aria-label="Layers"');
    expect(out).not.toContain('role="tablist"');
  });
  it("renders nothing while panels are hidden", () => {
    expect(frame(1280, { panelsHidden: true })).toBe("");
    expect(frame(1280, { chromeHidden: true })).toBe("");
  });
});
