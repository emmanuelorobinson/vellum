// TEST DATA. The Juniper edge states of design screens S01–S16 and the global state matrix, as props for the test app.
import type { MissingFont, Unreadable } from "@vellum/editor-core";
import type { Rejection } from "../src/shell/EdgeLayer";

export type EdgeScenario =
  | "s01"
  | "s02"
  | "s03"
  | "s03-closed"
  | "s04"
  | "s05"
  | "s06"
  | "s07"
  | "s08"
  | "s09"
  | "s10"
  | "s11"
  | "s16"
  | "m-loading"
  | "m-empty"
  | "m-offline"
  | "m-readonly"
  | "m-basic"
  | "m-fonts"
  | "m-library";

export const EDGE_FONTS: readonly MissingFont[] = [
  { family: "Lora", layers: 4, substitute: "Inter", nodes: ["step2"] },
  { family: "IBM Plex Sans Arabic", layers: 2, substitute: "System Arabic", nodes: ["step3"] },
];
export const EDGE_UNREADABLE: Unreadable = {
  line: 128,
  column: 17,
  reason: "Unexpected token",
  detail: 'Expected a closing brace after "children".',
};
export const EDGE_REJECTION: Rejection = {
  agent: "Saffron",
  leasedBy: "Teal",
  following: "Cursor",
};
/** When the usage limit lifts: 18:00 on October 11, as S09 says. */
export const EDGE_RESET = Date.UTC(2026, 9, 11, 18, 0);

/** What the test app turns each scenario into. Nothing here is the editor's own logic. */
export interface EdgeSetup {
  connection?: "offline" | "headless";
  basicRenderer?: boolean;
  readOnly?: boolean;
  newerFormat?: number;
  unreadable?: Unreadable;
  fonts?: readonly MissingFont[];
  largeFile?: { layers: number };
  rejection?: Rejection;
  closing?: { fileName: string; running: number };
  openFonts?: boolean;
  loading?: boolean;
  empty?: boolean;
  library?: boolean;
  /** An engine problem the composer's catalog shows. */
  engine?: "key-failed" | "signed-out" | "usage-limit";
  agent: "edge" | "c03-stuck";
}
export const edgeSetup = (name: EdgeScenario): EdgeSetup => {
  const base: EdgeSetup = { agent: "edge" };
  switch (name) {
    case "s01":
    case "m-offline":
      return { ...base, connection: "offline" };
    case "s02":
    case "m-basic":
      return { ...base, basicRenderer: true };
    case "s03":
      return { ...base, fonts: EDGE_FONTS, openFonts: true };
    case "s03-closed":
    case "m-fonts":
      return { ...base, fonts: EDGE_FONTS };
    case "s04":
    case "m-readonly":
      return { ...base, readOnly: true, newerFormat: 3 };
    case "s05":
      return { ...base, unreadable: EDGE_UNREADABLE };
    case "s06":
      return { ...base, largeFile: { layers: 18_420 } };
    case "s07":
      return { ...base, rejection: EDGE_REJECTION };
    case "s08":
      return { ...base, engine: "key-failed" };
    case "s09":
      return { ...base, engine: "usage-limit" };
    case "s10":
      return { agent: "c03-stuck" };
    case "s11":
      return { ...base, closing: { fileName: "juniper.vellum", running: 3 } };
    case "s16":
      return { ...base, engine: "signed-out" };
    case "m-loading":
      return { ...base, loading: true };
    case "m-empty":
      return { ...base, empty: true };
    case "m-library":
      return { ...base, library: true };
  }
};

/** A backdrop the person picks in Settings → Canvas. */
export const EDGE_BACKDROP = "#102030";
