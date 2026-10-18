// TEST DATA: the Juniper change ledger (Teal, Saffron, Rose, Green, Sam, the file), tokens and export rows.
import {
  type ExportFrame,
  FakeHandoffHost,
  type FileChange,
  type LedgerEntry,
} from "@vellum/editor-core";
import type { Node } from "@vellum/types-ts";

export type LedgerScenario =
  | "c10"
  | "c10-revert"
  | "c10-undone"
  | "c10-undoable"
  | "c10-reverted"
  | "c19"
  | "c19-copied"
  | "c20"
  | "c20-export"
  | "c20-applied"
  | "c21"
  | "c22"
  | "c22-conflict";

const MINUTE = 60_000;
/** 10:00 UTC on the day the entries were made; the app shows times in UTC so goldens do not depend on a zone. */
export const LEDGER_BASE = Date.UTC(2026, 9, 18, 10, 0);
const at = (m: number) => LEDGER_BASE + m * MINUTE;
const actor = (id: string, name: string, hue?: string) =>
  ({ id, kind: "agent", name, ...(hue ? { hue } : {}) }) as const;
const inverse = [{ type: "update", id: "cta-02", patch: { name: "Continue" } }] as never;

export const ledgerEntries = (name: LedgerScenario = "c10"): LedgerEntry[] => [
  {
    id: "file",
    at: at(38),
    actor: { id: "file", kind: "file", name: "File" },
    summary: "File changed on disk · git checkout",
    count: 14,
    page: null,
    session: null,
    nodes: [],
    ops: [],
    source: "git checkout",
  },
  {
    id: "sam",
    at: at(39),
    actor: { id: "sam", kind: "person", name: "Sam" },
    summary: "Resized 3 layers in Welcome",
    count: 3,
    page: "p1",
    session: "s1",
    sessionTitle: "Onboarding screens",
    nodes: ["welcome"],
    ops: [],
  },
  {
    id: "green",
    at: at(40),
    actor: actor("green", "Green", "green"),
    summary: "Built Components / Habit card",
    count: 12,
    page: "p1",
    session: "s1",
    sessionTitle: "Onboarding screens",
    nodes: [],
    ops: [],
  },
  {
    id: "rose",
    at: at(41),
    actor: actor("rose", "Rose", "rose"),
    summary: "Built Paywall / Plans",
    count: 12,
    page: "p1",
    session: "s1",
    sessionTitle: "Onboarding screens",
    nodes: [],
    ops: [],
  },
  {
    id: "saffron",
    at: at(42),
    actor: actor("saffron", "Saffron", "saffron"),
    summary: "Changed color/primary in both modes",
    count: 2,
    page: "p1",
    session: "s1",
    sessionTitle: "Onboarding screens",
    nodes: [],
    ops: [],
  },
  {
    id: "teal",
    at: at(43),
    // The design's undone toast names the client ("Claude Code"); its ledger names the hue ("Teal").
    actor: actor("teal", name === "c10-undone" ? "Claude Code" : "Teal", "teal"),
    summary: "Inserted 24 layers in Step 2",
    count: 24,
    page: "p1",
    session: "s1",
    sessionTitle: "Onboarding screens",
    nodes: ["step2"],
    undoable: name === "c10-undoable",
    ops: [
      {
        id: "teal-1",
        label: "Update Continue · #cta-02",
        detail: "fill: #F3F3F1 → {color/surface}",
        json: { type: "update", id: "cta-02", patch: { fill: { $var: "surface" } } },
      },
    ],
    inverse,
  },
];

const step = (id: string, name: string, x: number, extra: Record<string, unknown> = {}) =>
  ({
    type: "frame",
    id,
    name,
    width: 208,
    height: 480,
    position: { type: "absolute", x, y: 0 },
    ...extra,
  }) as unknown as Node;
/** Three onboarding frames 224 px apart, so `pan=484,197` puts them where the design draws them. */
export function ledgerNodes(name: LedgerScenario): Node[] {
  const step2 =
    name === "c22-conflict"
      ? ({
          type: "conflict",
          id: "step2",
          name: "Step 2",
          width: 208,
          height: 480,
          position: { type: "absolute", x: 224, y: 0 },
          ours: step("step2", "Step 2", 224),
          theirs: step("step2", "Step 2", 224, { width: 224 }),
          properties: ["name", "width"],
        } as unknown as Node)
      : step("step2", "Step 2", 224);
  return [step("step1", "Step 1", 0), step2, step("step3", "Step 3", 448)];
}
export const ledgerChange = (name: LedgerScenario): FileChange | null =>
  name === "c22"
    ? { kind: "applied", source: "git checkout", count: 14, entry: "file" }
    : name === "c22-conflict"
      ? { kind: "conflict", versions: 2, nodes: ["step2"] }
      : null;

/** The Export dialog's three rows: PNG at 2×, then SVG and PDF at 1× with no suffix. */
export class JuniperHandoff extends FakeHandoffHost {
  override frames(ids: readonly string[]): ExportFrame[] {
    const formats = [
      { format: "png" as const },
      { format: "svg" as const },
      { format: "pdf" as const },
    ];
    return ids.map((id, i) => ({ id, name: `Step ${i + 1}`, ...(formats[i] ?? {}) }));
  }
}

/** What the token dialogs show for the Juniper change set; kept here because colour literals live in fixtures. */
export const TOKEN_TEXT = {
  changed: "#28513F → #234D3C",
  removed: "#999999 → —",
  exported: "--color-primary: #234D3C",
  opDetail: "fill: #F3F3F1 → {color/surface}",
};
