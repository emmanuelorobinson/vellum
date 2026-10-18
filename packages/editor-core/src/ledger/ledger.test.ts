// TEST DATA: Juniper ledger entries (Teal, Saffron, Sam) and scripted host ports.
import { copy } from "@vellum/config";
import { FakeEngineClient, type ToolOp, type ToolScene } from "@vellum/engine-client";
import type { Document, Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import {
  conflictNodes,
  createConflictController,
  createExportController,
  createHistoryController,
  createInspectController,
  createTokenSyncController,
  exportFileName,
  FakeFileChanges,
  FakeHandoffHost,
  FakeHistoryHost,
  type LedgerEntry,
  resolveConflictOps,
} from "./index";

const DEVICE = "device-test";
const frame = (id: string, extra: Record<string, unknown> = {}): Node =>
  ({ type: "frame", id, name: id, width: 100, height: 100, ...extra }) as unknown as Node;
const doc = (children: Node[]): Document =>
  ({
    format: 1,
    meta: {},
    pages: [{ id: "p1", name: "Onboarding", children }],
  }) as unknown as Document;
const scene = (ids: string[]): ToolScene => ({
  get: (id) =>
    ids.includes(id)
      ? {
          node: frame(id),
          bounds: { x: 0, y: 0, width: 100, height: 100 },
          children: [],
          parent: null,
        }
      : undefined,
  roots: () => ids,
  hitPoint: () => null,
  hitRect: () => [],
  path: () => null,
  snap: () => ({ offset: { x: 0, y: 0 }, guides: [] }),
  toParent: (_a, p) => p,
  toLocal: (_a, p) => p,
});
const inverse: ToolOp[] = [{ type: "update", id: "cta", patch: { name: "Continue" } as never }];
const entry = (id: string, extra: Partial<LedgerEntry> = {}): LedgerEntry => ({
  id,
  at: 100,
  actor: { id: "teal", kind: "agent", name: "Teal", hue: "teal" },
  summary: "Inserted 24 layers in Step 2",
  count: 24,
  page: "p1",
  session: "s1",
  sessionTitle: "Onboarding screens",
  nodes: ["step2"],
  ops: [
    {
      id: `${id}-1`,
      label: "Update Continue · #cta-02",
      detail: "fill: #F3F3F1 → {color/surface}",
      json: { type: "update", id: "cta-02" },
    },
  ],
  inverse,
  ...extra,
});
const entries = () => [
  entry("e1"),
  entry("e2", {
    actor: { id: "sam", kind: "person", name: "Sam" },
    summary: "Resized 3 layers in Welcome",
    count: 3,
    page: "p2",
    inverse: undefined,
    nodes: [],
  }),
  entry("e3", {
    actor: { id: "file", kind: "file", name: "File" },
    summary: "File changed on disk · git checkout",
    count: 14,
    page: null,
    session: null,
  }),
];
const setup = (readOnly = false) => {
  const engine = new FakeEngineClient(
    { revision: 1, document: doc([frame("step2")]), readOnly },
    scene(["step2"]),
  );
  const host = new FakeHistoryHost(entries());
  host.attach(engine, { id: "sam", kind: "person", name: "Sam" }, () => 200);
  const selected: string[][] = [];
  const zoomed: string[][] = [];
  const history = createHistoryController({
    engine,
    device: DEVICE,
    host,
    select: (ids) => selected.push([...ids]),
    zoom: (ids) => zoomed.push([...ids]),
  });
  return { engine, host, history, selected, zoomed };
};

describe("history ledger", () => {
  it("lists newest first, expands an entry to its ops and filters by actor and page", () => {
    const { history } = setup();
    expect(history.view().map((e) => e.id)).toEqual(["e3", "e2", "e1"]);
    history.toggle("e1");
    expect(history.state.expanded).toBe("e1");
    history.toggle("e1");
    expect(history.state.expanded).toBeNull();
    history.filter({ actor: "agents" });
    expect(history.view().map((e) => e.id)).toEqual(["e1"]);
    history.filter({ actor: "you" });
    expect(history.view().map((e) => e.id)).toEqual(["e2"]);
    history.filter({ actor: "file" });
    expect(history.view().map((e) => e.id)).toEqual(["e3"]);
    history.filter({ actor: "all", page: "p1" });
    expect(history.view().map((e) => e.id)).toEqual(["e1"]);
    history.filter({ page: "all", session: "s1" });
    expect(history.view().map((e) => e.id)).toEqual(["e2", "e1"]);
    expect(history.sessions()).toEqual([{ id: "s1", title: "Onboarding screens" }]);
    expect(history.empty()).toBe(false);
  });
  it("says what the ledger is empty of in the guide's words", () => {
    const engine = new FakeEngineClient();
    const history = createHistoryController({
      engine,
      device: DEVICE,
      host: new FakeHistoryHost([]),
    });
    expect(history.empty()).toBe(true);
    expect(history.footer).toBe(copy.historyFooter);
    expect(history.emptyText).toBe(copy.historyEmpty);
  });
  it("selects and zooms to an entry's layers that still exist", () => {
    const { history, selected, zoomed } = setup();
    history.selectLayers("e1");
    history.zoomTo("e1");
    expect(selected).toEqual([["step2"]]);
    expect(zoomed).toEqual([["step2"]]);
  });
  it("copies an entry's ops as JSON", () => {
    const { history, host } = setup();
    history.copyJson("e1");
    expect(JSON.parse(host.copied[0] as string)).toEqual([{ type: "update", id: "cta-02" }]);
  });
  it("revert asks first, then applies the inverse as a new person entry and keeps the old one", async () => {
    const { history, engine, host } = setup();
    engine.script({
      revision: 2,
      document: doc([frame("step2")]),
      readOnly: false,
    });
    history.askRevert("e1");
    expect(history.state.confirm).toBe("e1");
    expect(engine.transactions).toHaveLength(0);
    history.confirmRevert();
    expect(engine.transactions).toHaveLength(1);
    expect(engine.transactions[0]?.ops).toEqual(inverse);
    expect(engine.transactions[0]?.origin.actor).toEqual({ kind: "person", device: DEVICE });
    const fresh = host.entries().at(-1) as LedgerEntry;
    expect(fresh).toMatchObject({
      actor: { kind: "person", name: "Sam" },
      revertOf: "e1",
      count: 1,
    });
    expect(fresh.id).not.toBe("e1");
    expect(host.entries().some((e) => e.id === "e1")).toBe(true);
    expect(history.state.confirm).toBeNull();
    expect(history.state.notice).toEqual({
      title: "Reverted by you",
      body: "Applied inverse changes as a new batch. Later work remains in the ledger.",
    });
  });
  it("shows an error and records nothing when the engine refuses the revert", () => {
    const { history, engine, host } = setup();
    history.askRevert("e1");
    history.confirmRevert();
    expect(engine.transactions).toHaveLength(0);
    expect(host.entries()).toHaveLength(3);
    expect(history.state.notice).toBeNull();
    expect(history.state.error).toMatch(/refused/i);
  });
  it("refuses to revert in a read-only file or an entry with no inverse", () => {
    const ro = setup(true);
    ro.history.askRevert("e1");
    expect(ro.history.state.confirm).toBeNull();
    expect(ro.history.state.error).toMatch(/read-only/i);
    const { history } = setup();
    history.askRevert("e2");
    expect(history.state.confirm).toBeNull();
    expect(history.state.error).toMatch(/cannot be reverted/i);
  });
  it("undoes an agent batch through the host and says whose it was", async () => {
    const { history, host } = setup();
    await history.undo("e1");
    expect(host.undone).toEqual(["e1"]);
    expect(history.state.notice).toEqual({
      title: "Undone by you",
      body: "Teal’s batch was undone. Your edits are unchanged.",
    });
  });
  it("closes the notice", () => {
    const { history } = setup();
    history.askRevert("e2");
    history.dismiss();
    expect(history.state.error).toBeNull();
  });
});

const handoff = () => {
  const engine = new FakeEngineClient(
    { revision: 1, document: doc([frame("step2")]), readOnly: false },
    scene(["step2"]),
  );
  const host = new FakeHandoffHost();
  return { engine, host };
};

describe("token sync", () => {
  it("previews the exact diff of an import before anything is written", async () => {
    const { engine, host } = handoff();
    const sync = createTokenSyncController({ engine, device: DEVICE, host });
    sync.openImport();
    await sync.preview();
    expect(sync.state.plan?.changes).toEqual([
      { kind: "added", name: "space/4", from: null, to: "16 px" },
      { kind: "changed", name: "color/primary", from: "#28513F", to: "#234D3C" },
      { kind: "removed", name: "color/legacy", from: "#999999", to: null },
    ]);
    expect(sync.summary()).toBe("3 changes to review");
    expect(engine.transactions).toHaveLength(0);
    expect(host.calls).toContain("preview");
  });
  it("applies the plan as one person transaction", async () => {
    const { engine, host } = handoff();
    const sync = createTokenSyncController({ engine, device: DEVICE, host });
    sync.openImport();
    await sync.preview();
    engine.script({ revision: 2, document: doc([]), readOnly: false });
    await sync.apply();
    expect(engine.transactions).toHaveLength(1);
    expect(engine.transactions[0]?.ops).toEqual(host.planOps);
    expect(sync.state.dialog).toBeNull();
    expect(sync.state.notice).toEqual({
      title: "Applied 3 token changes",
      body: "Imported from src/tokens.css. Review the batch in Activity.",
    });
  });
  it("will not apply an empty plan, a plan with errors or a stale one", async () => {
    const { engine, host } = handoff();
    const sync = createTokenSyncController({ engine, device: DEVICE, host });
    host.plan = { changes: [], ops: [], errors: [] };
    sync.openImport();
    await sync.preview();
    expect(sync.canApply()).toBe(false);
    host.plan = {
      changes: [{ kind: "added", name: "x", from: null, to: "1" }],
      ops: host.planOps,
      errors: ["Unknown token kind at line 3"],
    };
    await sync.preview();
    expect(sync.canApply()).toBe(false);
    host.plan = { ...host.plan, errors: [] };
    await sync.preview();
    expect(sync.canApply()).toBe(true);
    engine.script({ revision: 2, document: doc([]), readOnly: false });
    engine.apply({
      origin: { actor: { kind: "person", device: DEVICE } },
      ops: inverse,
    });
    await sync.apply();
    expect(engine.transactions).toHaveLength(1);
    expect(sync.state.error).toMatch(/changed since/i);
  });
  it("exports the variables to the chosen target and previews the file", async () => {
    const { engine, host } = handoff();
    const sync = createTokenSyncController({ engine, device: DEVICE, host });
    sync.openExport();
    await sync.preview();
    expect(sync.state.exportText).toContain("--color-primary: #234D3C");
    sync.set({ format: "tailwind" });
    await sync.preview();
    expect(host.calls.filter((c) => c === "exportPreview")).toHaveLength(2);
    await sync.write();
    expect(host.written).toHaveLength(1);
    expect(host.written[0]).toMatchObject({ format: "tailwind", light: ":root", dark: ".dark" });
    expect(sync.state.dialog).toBeNull();
  });
});

describe("export dialog", () => {
  it("names files from the frame name, suffix and format", () => {
    expect(exportFileName("Step 1", "png", "@2x", [])).toBe("step-1@2x.png");
    expect(exportFileName("Step 2", "svg", "", [])).toBe("step-2.svg");
    expect(exportFileName("../Step", "pdf", "", [])).toBe("step.pdf");
    expect(exportFileName("Step 1", "png", "@2x", ["step-1@2x.png"])).toBe("step-1-2@2x.png");
  });
  it("sets format-specific defaults, lists files and writes the request", async () => {
    const { engine, host } = handoff();
    const exp = createExportController({ engine, host });
    await exp.open(["a", "b", "c"]);
    expect(exp.state.rows.map((r) => r.name)).toEqual(["Step 1", "Step 2", "Step 3"]);
    exp.setRow("a", { format: "png", scale: 2, suffix: "@2x" });
    exp.setRow("b", { format: "svg" });
    exp.setRow("c", { format: "pdf" });
    expect(exp.files()).toEqual(["step-1@2x.png", "step-2.svg", "step-3.pdf"]);
    expect(exp.state.rows[1]).toMatchObject({ scale: 1, suffix: "" });
    expect(exp.title()).toBe("3 frames will be written to your chosen folder.");
    expect(exp.action()).toBe("Export 3 frames");
    await exp.write();
    expect(host.exported[0]?.destination).toBe("~/code/juniper/design/exports");
    expect(host.exported[0]?.rows.map((r) => r.file)).toEqual([
      "step-1@2x.png",
      "step-2.svg",
      "step-3.pdf",
    ]);
    expect(exp.state.open).toBe(false);
  });
  it("copies as PNG and HTML through the host", async () => {
    const { engine, host } = handoff();
    const exp = createExportController({ engine, host });
    await exp.copyPng(["a"]);
    await exp.copyHtml(["a"]);
    expect(host.copiedAs).toEqual(["png:a", "html:a"]);
  });
});

describe("inspect", () => {
  it("shows the spec, copies the implementation prompt and exports HTML with settings", async () => {
    const { engine, host } = handoff();
    const inspect = createInspectController({ engine, host });
    await inspect.load("step2");
    expect(inspect.state.spec?.summary[0]).toBe("Column · Hug height · Fixed 240 px");
    expect(inspect.code()).toContain("display: flex;");
    inspect.set({ stack: "tailwind" });
    expect(inspect.code()).toContain("flex flex-col");
    await inspect.copyPrompt();
    expect(host.clipboard.at(-1)).toBe(
      "Implement design/juniper.vellum · Onboarding / Step 2 using get_spec.",
    );
    expect(inspect.state.notice).toEqual({
      title: "Copied prompt for your agent",
      body: "Implement design/juniper.vellum · Onboarding / Step 2 using get_spec.",
    });
    inspect.set({ flavour: "css", scaffold: false });
    await inspect.copyHtml();
    expect(host.htmlRequests.at(-1)).toMatchObject({ flavour: "css", scaffold: false });
    expect(inspect.settingsLine()).toBe("Scaffold: Off · Assets: Write as files");
    await inspect.implementWith("claude-code");
    expect(host.implemented).toEqual([["claude-code", "step2"]]);
  });
  it("drops a spec that arrives after the selection moved", async () => {
    const { engine, host } = handoff();
    const inspect = createInspectController({ engine, host });
    const slow = inspect.load("step2");
    await inspect.load(null);
    await slow;
    expect(inspect.state.spec).toBeNull();
  });
});

describe("file changes and conflicts", () => {
  const conflictDoc = doc([
    {
      type: "conflict",
      id: "step2",
      name: "Step 2",
      width: 100,
      height: 100,
      ours: frame("step2", { name: "Step 2 ours" }),
      theirs: frame("step2", { name: "Step 2 theirs" }),
      properties: ["name"],
    } as unknown as Node,
  ]);
  it("finds conflict nodes in the document", () => {
    expect(conflictNodes(conflictDoc).map((c) => c.id)).toEqual(["step2"]);
  });
  it("builds replace ops for ours, theirs and both", () => {
    const [node] = conflictNodes(conflictDoc);
    const place = { parent: { type: "page", id: "p1" }, index: 0 } as const;
    const ours = resolveConflictOps(node as never, "ours", () => "n1", place);
    expect(ours).toEqual([
      { type: "replace", id: "step2", node: expect.objectContaining({ name: "Step 2 ours" }) },
    ]);
    const theirs = resolveConflictOps(node as never, "theirs", () => "n1", place);
    expect((theirs[0] as { node: Node }).node.name).toBe("Step 2 theirs");
    const both = resolveConflictOps(node as never, "both", () => "n1", place);
    expect(both).toHaveLength(2);
    expect(both[1]).toMatchObject({
      type: "insert",
      to: { parent: { type: "page", id: "p1" }, index: 1 },
      node: { id: "n1", name: "Step 2 theirs" },
    });
  });
  it("keeps the conflict's merged children whichever version wins", () => {
    const place = { parent: { type: "page", id: "p1" }, index: 0 } as const;
    const withKids = {
      ...(conflictNodes(conflictDoc)[0] as object),
      children: [frame("kid")],
    };
    const [op] = resolveConflictOps(withKids as never, "ours", () => "n1", place);
    expect((op as { node: { children: Node[] } }).node.children.map((n) => n.id)).toEqual(["kid"]);
  });
  it("resolves through one person transaction and can ask the agent to merge", async () => {
    const engine = new FakeEngineClient(
      { revision: 1, document: conflictDoc, readOnly: false },
      scene(["step2"]),
    );
    const feed = new FakeFileChanges({ kind: "conflict", versions: 2, nodes: ["step2"] });
    const asked: string[] = [];
    const conflicts = createConflictController({
      engine,
      device: DEVICE,
      feed,
      fileName: "juniper.vellum",
      nextNodeId: () => "n1",
      askAgent: (node) => asked.push(node),
    });
    expect(conflicts.banner()).toEqual({
      text: "juniper.vellum has a merge conflict · 2 versions",
      action: "Resolve",
    });
    conflicts.open("step2");
    expect(conflicts.state.node?.id).toBe("step2");
    expect(conflicts.versions().map((v) => v.label)).toEqual(["Ours", "Theirs"]);
    conflicts.askAgent();
    expect(asked).toEqual(["step2"]);
    engine.script({ revision: 2, document: doc([frame("step2")]), readOnly: false });
    conflicts.pick("theirs");
    expect(engine.transactions).toHaveLength(1);
    expect(conflicts.state.node).toBeNull();
  });
  it("announces a checkout as a banner that opens the ledger entry", () => {
    const feed = new FakeFileChanges({
      kind: "applied",
      source: "git checkout",
      count: 14,
      entry: "e3",
    });
    const opened: string[] = [];
    const engine = new FakeEngineClient();
    const conflicts = createConflictController({
      engine,
      device: DEVICE,
      feed,
      fileName: "juniper.vellum",
      nextNodeId: () => "n1",
      openLedger: (id) => opened.push(id),
    });
    expect(conflicts.banner()).toEqual({
      text: "juniper.vellum changed on disk (git checkout) · 14 changes applied",
      action: "View in ledger",
    });
    conflicts.runBanner();
    expect(opened).toEqual(["e3"]);
    conflicts.dismiss();
    expect(conflicts.banner()).toBeNull();
  });
});
