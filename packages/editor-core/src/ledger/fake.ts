/** Scripted ports for the ledger, Inspect, token sync, Export and file changes: tests emit what the daemon would. */
import type { EngineClient, ToolOp } from "@vellum/engine-client";
import type { NodeId } from "@vellum/types-ts";
import type { FileChange, FileChangeFeed } from "./files";
import type {
  CodeStack,
  ExportFrame,
  ExportHost,
  ExportRequest,
  HtmlSettings,
  ImplementEngine,
  InspectHost,
  InspectSpec,
  TokenPlan,
  TokenSyncHost,
  TokenTarget,
} from "./handoff";
import type { HistoryHost, LedgerActor, LedgerEntry } from "./history";

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

export class FakeHistoryHost implements HistoryHost {
  private list: LedgerEntry[];
  private listeners = new Set<() => void>();
  readonly copied: string[] = [];
  readonly undone: string[] = [];
  private counter = 0;
  constructor(entries: readonly LedgerEntry[] = []) {
    this.list = [...entries];
  }
  entries() {
    return this.list;
  }
  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return { dispose: () => void this.listeners.delete(listener) };
  }
  append(entry: LedgerEntry) {
    this.list = [...this.list, entry];
    for (const l of [...this.listeners]) l();
  }
  /**
   * What the daemon does: every accepted person transaction becomes an entry. One whose ops are exactly an
   * earlier entry's inverse is a revert of it.
   */
  attach(engine: EngineClient, actor: LedgerActor, now: () => number) {
    return engine.subscribeCommits?.((tx) => {
      const reverted = this.list.find((e) => e.inverse && same(e.inverse, tx.ops));
      this.append({
        id: `r${++this.counter}`,
        at: now(),
        actor,
        summary: reverted
          ? `Reverted ${reverted.actor.name}’s batch · ${tx.ops.length} inverse change${tx.ops.length === 1 ? "" : "s"} applied`
          : `${tx.ops.length} change${tx.ops.length === 1 ? "" : "s"}`,
        count: tx.ops.length,
        page: reverted?.page ?? null,
        session: null,
        nodes: reverted?.nodes ?? [],
        ops: tx.ops.map((op, i) => ({ id: `r${this.counter}-${i}`, label: op.type, json: op })),
        ...(reverted ? { revertOf: reverted.id } : {}),
      });
    });
  }
  async undo(entry: LedgerEntry) {
    this.undone.push(entry.id);
  }
  copy(text: string) {
    this.copied.push(text);
  }
}

export class FakeFileChanges implements FileChangeFeed {
  private listeners = new Set<() => void>();
  constructor(private now: FileChange | null = null) {}
  current() {
    return this.now;
  }
  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return { dispose: () => void this.listeners.delete(listener) };
  }
  emit(change: FileChange | null) {
    this.now = change;
    for (const l of [...this.listeners]) l();
  }
}

const SPEC: InspectSpec = {
  summary: ["Column · Hug height · Fixed 240 px", "Gap {space/4} → 16 px · Padding 24 px"],
  tokens: [
    { name: "color/primary", value: "Light #234D3C · Dark #A8C8AD" },
    { name: "space/4 → 16 · radius/md → 8", value: "" },
  ],
  components: ["Button · Primary · Medium"],
  code: {
    css: "display: flex;\nflex-direction: column;\ngap: var(--space-4);\npadding: 24px;",
    tailwind: "flex flex-col gap-4 p-6",
    shadcn: '<Card className="flex flex-col gap-4 p-6" />',
    "react-native": "{ flexDirection: 'column', gap: 16, padding: 24 }",
  } satisfies Record<CodeStack, string>,
};

export class FakeHandoffHost implements InspectHost, TokenSyncHost, ExportHost {
  readonly calls: string[] = [];
  readonly clipboard: string[] = [];
  readonly htmlRequests: HtmlSettings[] = [];
  readonly implemented: [string, NodeId][] = [];
  readonly written: TokenTarget[] = [];
  readonly exported: ExportRequest[] = [];
  readonly copiedAs: string[] = [];
  readonly planOps: ToolOp[] = [{ type: "setVariables", set: {}, remove: [] }];
  plan: TokenPlan = {
    changes: [
      { kind: "added", name: "space/4", from: null, to: "16 px" },
      { kind: "changed", name: "color/primary", from: "#28513F", to: "#234D3C" },
      { kind: "removed", name: "color/legacy", from: "#999999", to: null },
    ],
    ops: this.planOps,
    errors: [],
    source: "src/tokens.css",
  };
  async spec(): Promise<InspectSpec | null> {
    return SPEC;
  }
  async prompt() {
    return "Implement design/juniper.vellum · Onboarding / Step 2 using get_spec.";
  }
  async engines(): Promise<ImplementEngine[]> {
    return [
      { id: "claude-code", name: "Claude Code", status: "Installed · signed in", ready: true },
    ];
  }
  async implement(engine: string, node: NodeId) {
    this.implemented.push([engine, node]);
  }
  async html(_node: NodeId, settings: HtmlSettings) {
    this.htmlRequests.push(settings);
    return "<div></div>";
  }
  async exportHtml(_node: NodeId, _settings: HtmlSettings) {
    return "design/exports/step-2.html";
  }
  copy(text: string) {
    this.clipboard.push(text);
  }
  defaults(): TokenTarget {
    return { path: "~/code/juniper/src/tokens.css", format: "css", light: ":root", dark: ".dark" };
  }
  async preview() {
    this.calls.push("preview");
    return this.plan;
  }
  async exportPreview(target: TokenTarget) {
    this.calls.push("exportPreview");
    return target.format === "css"
      ? ":root { --color-primary: #234D3C; --space-4: 16px; }\n.dark { --color-primary: #A8C8AD; }"
      : target.format === "tailwind"
        ? "@theme { --color-primary: #234D3C; --spacing-4: 16px; }"
        : '{ "color": { "primary": { "$value": "#234D3C" } } }';
  }
  async write(target: TokenTarget) {
    this.written.push(target);
  }
  frames(ids: readonly NodeId[]): ExportFrame[] {
    return ids.map((id, i) => ({ id, name: `Step ${i + 1}` }));
  }
  async destination() {
    return "~/code/juniper/design/exports";
  }
  async writeFiles(request: ExportRequest) {
    this.exported.push(request);
  }
  async copyAs(kind: "png" | "html", ids: readonly NodeId[]) {
    this.copiedAs.push(`${kind}:${ids.join(",")}`);
  }
}
