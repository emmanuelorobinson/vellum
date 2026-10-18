/**
 * System and edge states (DESIGN.md "System and edge states", S01–S16, guide Appendix B). One environment says what is
 * true of the editor right now; pure functions turn it into the notice the Agent panel shows, the banner under the
 * document bar, the composer's disabled line and what every panel shows. The words are here once: views render them.
 */
import { copy } from "@vellum/config";
import type { Document } from "@vellum/types-ts";
import type { EngineCatalog, EngineChoice } from "../agent/engines";
import { fontUsage } from "../resources/controller";

export type PanelId = "agent" | "layers" | "assets" | "history" | "design";
export const PANELS: readonly PanelId[] = ["agent", "layers", "assets", "history", "design"];

/** What an edge notice's buttons ask the host (or the view) to do. */
export type EdgeAction =
  | "restartDaemon"
  | "troubleshoot"
  | "rendererSettings"
  | "reviewFonts"
  | "updateVellum"
  | "openAsText"
  | "formatSpec"
  | "learnMultiFile"
  | "updateKey"
  | "switchEngine"
  | "openTerminal"
  | "recheck"
  | "reviewChanges"
  | "dismiss";
export interface Act {
  id: EdgeAction;
  label: string;
}

export interface MissingFont {
  family: string;
  /** Layers that use it. */
  layers: number;
  substitute: string;
  nodes: readonly string[];
}
export interface Unreadable {
  line: number;
  column: number;
  /** Short: "Unexpected token". */
  reason: string;
  /** The parser's full words, shown in the dialog. */
  detail?: string;
  /** What kind of failure it was; JSON unless the host says otherwise. */
  summary?: string;
}
export type SessionProblem =
  | { kind: "key-failed"; provider: string }
  | { kind: "not-signed-in"; engine: string; command?: string }
  | { kind: "usage-limit"; engine: string; resets: string | null };

export interface EdgeEnvironment {
  daemon: "live" | "headless" | "offline";
  renderer: "full" | "basic";
  /** A file from a newer Vellum opens read-only; `locked` is any other reason the host says so. */
  readOnly: { newerFormat: number } | "locked" | null;
  unreadable: Unreadable | null;
  fonts: readonly MissingFont[];
  /** Imported libraries whose path no longer resolves. */
  libraries: number;
  layers: number;
  largeFile?: boolean;
  /** The open session's engine problem, when it has one. */
  problem: SessionProblem | null;
  /** The window is closing with unsaved changes. */
  closing: { fileName: string; running: number } | null;
}

/** ADR-004: the multi-file format earns a suggestion past this many layers. */
export const LARGE_FILE_LAYERS = 10_000;

export type Tone = "neutral" | "attention" | "error";
export interface Notice {
  id: string;
  heading: string;
  body: string;
  tone: Tone;
  /** The word beside the agent's name above the notice ("Unavailable", "Idle", "Failed"). */
  status: string;
  action?: Act;
  secondary?: Act[];
  link?: Act;
  /** A command to run in a terminal, shown in mono. */
  command?: string;
  footnote?: string;
}

const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);
/** Appendix B strings are "<what> · <next step>"; the panels show the parts apart. */
const parts = (line: string, values: Record<string, string>) =>
  line.split(" · ").map((p) => p.replace(/\{(\w+)\}/g, (_, k: string) => values[k] ?? `{${k}}`));
const sentence = (s: string) => `${s.charAt(0).toUpperCase()}${s.slice(1)}`;
const formatter = (n: number) => n.toLocaleString("en-US");

/** "18:00 · October 11", in the viewer's zone. */
export function formatReset(at: number, timeZone?: string): string {
  const zone = timeZone ? { timeZone } : {};
  const time = new Intl.DateTimeFormat("en-GB", {
    ...zone,
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).format(at);
  const day = new Intl.DateTimeFormat("en-US", { ...zone, month: "long", day: "numeric" }).format(
    at,
  );
  return `${time} · ${day}`;
}

/** The first of the session's engine problems the catalog shows for the engine the session uses. */
export function sessionProblem(input: {
  catalog: EngineCatalog | null;
  choice: EngineChoice | null;
  timeZone?: string;
}): SessionProblem | null {
  const { catalog, choice } = input;
  if (!catalog || !choice) return null;
  if (choice.engine === "builtin") {
    const provider = catalog.builtin.find((p) => p.id === choice.provider);
    return provider?.status === "failed" ? { kind: "key-failed", provider: provider.name } : null;
  }
  const engine = catalog.installed.find((e) => e.id === choice.engine);
  if (!engine) return null;
  if (engine.state === "signed_out")
    return {
      kind: "not-signed-in",
      engine: engine.name,
      ...(engine.command ? { command: engine.command } : {}),
    };
  if (engine.state === "usage_limit")
    return {
      kind: "usage-limit",
      engine: engine.name,
      resets: engine.resetsAt === undefined ? null : formatReset(engine.resetsAt, input.timeZone),
    };
  return null;
}

/** Families the computer does not have, with how many layers use them. A font stored in the file is never missing. */
export function missingFonts(
  doc: Document,
  available: readonly string[] | undefined,
  fallback: string,
): MissingFont[] {
  if (!available) return [];
  return fontUsage(doc)
    .filter((u) => !available.includes(u.family) && u.assets.length === 0)
    .map((u) => ({
      family: u.family,
      layers: u.nodes.length,
      substitute: fallback,
      nodes: u.nodes,
    }));
}

const act = (id: EdgeAction, label: string): Act => ({ id, label });

/** The notice that takes the Agent panel's place, most important first; null when nothing is wrong. */
export function agentNotice(env: EdgeEnvironment): Notice | null {
  if (env.unreadable)
    return {
      id: "unreadable",
      status: "Failed",
      heading: "File can't be read",
      body: `${env.unreadable.reason} at line ${env.unreadable.line}, column ${env.unreadable.column}. No partial design was loaded.`,
      tone: "error",
      action: act("openAsText", "Open as text"),
      link: act("formatSpec", "View the file format spec"),
    };
  if (env.closing) {
    const n = env.closing.running;
    return {
      id: "closing",
      status: "Idle",
      heading: "Unsaved changes",
      body: n
        ? `${n} ${plural(n, "agent is", "agents are")} still working. Stop ${plural(n, "it", "them")} and close?`
        : "Your changes have not been written to disk.",
      tone: "attention",
      action: act("reviewChanges", "Review changes"),
    };
  }
  if (env.readOnly) {
    const format = typeof env.readOnly === "object" ? env.readOnly.newerFormat : null;
    return format === null
      ? {
          id: "read-only",
          status: "Unavailable",
          heading: "Read-only",
          body: "This file is read-only.",
          tone: "attention",
        }
      : {
          id: "read-only",
          status: "Unavailable",
          heading: "Read-only",
          body: `${copy.newerFormat.replace("{n}", String(format)).replace(" · ", ". ")}.`,
          tone: "attention",
          action: act("updateVellum", "Update Vellum"),
        };
  }
  if (env.daemon === "offline")
    return {
      id: "daemon-offline",
      status: "Unavailable",
      heading: copy.daemonOffline,
      body: "Agents need the Vellum daemon. Hand editing and saving still work.",
      tone: "neutral",
      action: act("restartDaemon", "Restart"),
    };
  const p = env.problem;
  if (p?.kind === "key-failed") {
    const [what, next] = parts(copy.keyFailed, { provider: p.provider }) as [string, string];
    return {
      id: "key-failed",
      status: "Failed",
      heading: "Engine failed",
      body: `${what}. Other sessions are unaffected.`,
      tone: "error",
      action: act("updateKey", next),
    };
  }
  if (p?.kind === "not-signed-in") {
    const [what, next] = parts(copy.engineNotSignedIn, { engine: p.engine }) as [string, string];
    return {
      id: "not-signed-in",
      status: "Paused",
      heading: what,
      body: "This session is paused. Your other sessions are unaffected.",
      tone: "attention",
      ...(p.command ? { command: `Run ${p.command} in a terminal to sign in.` } : {}),
      action: act("openTerminal", next.replace(" to sign in", "")),
      secondary: [act("recheck", "Re-check"), act("switchEngine", "Switch engine")],
    };
  }
  if (p?.kind === "usage-limit") {
    const [what, resets, next] = parts(copy.usageLimit, {
      engine: p.engine,
      time: p.resets ?? "",
    }) as [string, string, string];
    return {
      id: "usage-limit",
      status: "Stopped",
      heading: what,
      body: p.resets ? sentence(resets) : "Try again later, or use another engine.",
      tone: "attention",
      action: act("switchEngine", next),
      footnote: "Other sessions are unaffected.",
    };
  }
  if (env.fonts.length) {
    const n = env.fonts.length;
    return {
      id: "missing-fonts",
      status: "Idle",
      heading: "Missing fonts",
      body: `${n} ${plural(n, "font is", "fonts are")} unavailable. Choose a replacement or install ${plural(n, "it", "them")}.`,
      tone: "attention",
      action: act("reviewFonts", "Review fonts"),
    };
  }
  if (env.renderer === "basic")
    return {
      id: "basic-renderer",
      status: "Idle",
      heading: copy.basicRenderer,
      body: "Effects are simplified on screen. Exports are unaffected.",
      tone: "neutral",
      action: act("rendererSettings", "Renderer settings"),
    };
  if (env.largeFile)
    return {
      id: "large-file",
      status: "Idle",
      heading: "Large file",
      body: `This file has ${formatter(env.layers)} layers. The multi-file format can make changes easier to review.`,
      tone: "neutral",
      action: act("learnMultiFile", "Learn about multi-file"),
    };
  return null;
}

export interface Banner {
  id: "read-only" | "renderer" | "large-file";
  tone: "neutral" | "attention";
  message: string;
  action: Act;
  dismissible: boolean;
}

/** The strip under the document bar (S02, S04, S06): one at a time, never blocking. */
export function edgeBanner(
  env: EdgeEnvironment,
  options: { dismissed?: readonly string[] } = {},
): Banner | null {
  const dismissed = options.dismissed ?? [];
  if (typeof env.readOnly === "object" && env.readOnly)
    return {
      id: "read-only",
      tone: "attention",
      message: copy.newerFormat.replace("{n}", String(env.readOnly.newerFormat)),
      action: act("updateVellum", "Update"),
      dismissible: false,
    };
  if (env.renderer === "basic" && !dismissed.includes("renderer"))
    return {
      id: "renderer",
      tone: "neutral",
      message: "Effects are simplified on screen. Exports are unaffected.",
      action: act("dismiss", "Dismiss"),
      dismissible: true,
    };
  if (env.largeFile && !dismissed.includes("large-file"))
    return {
      id: "large-file",
      tone: "neutral",
      message: `This file has ${formatter(env.layers)} layers. The multi-file format can make changes easier to review.`,
      action: act("learnMultiFile", "Learn more"),
      dismissible: false,
    };
  return null;
}

/** Why the composer cannot send, in the words under it; null when it can. */
export function composerLine(env: EdgeEnvironment): { text: string; action: EdgeAction } | null {
  if (env.unreadable)
    return { text: "Open a readable file to start a session", action: "openAsText" };
  if (typeof env.readOnly === "object" && env.readOnly)
    return {
      text: `Update Vellum to edit format ${env.readOnly.newerFormat}`,
      action: "updateVellum",
    };
  if (env.daemon === "offline") return { text: copy.composerWhenOffline, action: "restartDaemon" };
  return null;
}

export type PanelView =
  | { kind: "loading"; text: string }
  | { kind: "empty"; text: string }
  | { kind: "notice"; notice: Notice; heading: string }
  | { kind: "content"; disabled: boolean; flag?: "missing-fonts" | "missing-library" };

/** What a panel says while its content is on the way. */
export const PANEL_LOADING: Readonly<Record<PanelId, string>> = {
  agent: "Loading sessions",
  layers: "Loading layers",
  assets: "Loading assets",
  history: "Loading history",
  design: "Loading properties",
};
/** What a panel says when it has nothing to show: one sentence, one next step beside it. */
export const PANEL_EMPTY: Readonly<Record<Exclude<PanelId, "agent">, string>> = {
  layers: copy.layersEmpty,
  assets: "No components or libraries yet",
  history: copy.historyEmpty,
  design: "Select a layer to see its properties.",
};

/**
 * What a panel shows in the given state. Only the Agent panel is replaced by a notice; the others keep working and
 * are disabled when the file is read-only. A panel that depends on nothing the state broke says so by being content.
 */
export function panelView(
  panel: PanelId,
  env: EdgeEnvironment,
  content: { loading: boolean; empty: boolean },
): PanelView {
  if (panel === "agent") {
    const notice = agentNotice(env);
    if (notice) return { kind: "notice", notice, heading: notice.heading };
    return content.loading
      ? { kind: "loading", text: PANEL_LOADING.agent }
      : { kind: "content", disabled: false };
  }
  if (content.loading) return { kind: "loading", text: PANEL_LOADING[panel] };
  if (content.empty) return { kind: "empty", text: PANEL_EMPTY[panel] };
  const flag =
    panel === "assets" || panel === "design"
      ? panel === "design" && env.fonts.length
        ? "missing-fonts"
        : env.libraries
          ? "missing-library"
          : undefined
      : undefined;
  return { kind: "content", disabled: env.readOnly !== null, ...(flag ? { flag } : {}) };
}

/** What the buttons of an edge notice, banner or dialog ask of the host. The editor handles what it can itself. */
export interface EdgeHost {
  action(id: EdgeAction, detail?: Record<string, unknown>): void;
  /** The window is closing with unsaved changes: the person's choice from the Save dialog. */
  closeChoice?(choice: "cancel" | "discard" | "save"): void;
}

export interface EdgeUiState {
  /** Banners, toasts and dialogs the person closed; they come back only when the state clears and returns. */
  dismissed: readonly string[];
  /** The Missing fonts dialog (S03). */
  fonts: boolean;
}

/** What the person has closed or opened among the edge surfaces; the environment itself belongs to the host. */
export function createEdgeUi() {
  let state: EdgeUiState = { dismissed: [], fonts: false };
  const listeners = new Set<() => void>();
  const change = (patch: Partial<EdgeUiState>) => {
    state = { ...state, ...patch };
    for (const l of [...listeners]) l();
  };
  return {
    get state() {
      return state;
    },
    dismiss(id: string) {
      if (!state.dismissed.includes(id)) change({ dismissed: [...state.dismissed, id] });
    },
    /** The cause went away: if it returns, it is news again. */
    restore(id: string) {
      if (state.dismissed.includes(id))
        change({ dismissed: state.dismissed.filter((d) => d !== id) });
    },
    isDismissed: (id: string) => state.dismissed.includes(id),
    openFonts() {
      change({ fonts: true });
    },
    closeFonts() {
      change({ fonts: false });
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return { dispose: () => void listeners.delete(listener) };
    },
  };
}
export type EdgeUi = ReturnType<typeof createEdgeUi>;
