/**
 * The usage meter and its popover (DESIGN.md "Usage meter", C27). Money is always an estimate at API rates
 * (guide §9 "Usage and money"); installed engines on a subscription show their plan's limit instead.
 */
import { copy } from "@vellum/config";

export interface SessionUsage {
  /** Tokens in the model's context now. */
  contextTokens: number;
  /** The model's context window; `null` when unknown. */
  window: number | null;
  conversation: number;
  files: number;
  screenshots: number;
  /** Micro-USD at API rates; `null` when the engine bills a plan. */
  estimate: number | null;
  /** An installed engine's subscription: the share of its limit used, and when it resets. */
  plan?: { used: number; resetsAt: number | null };
}

export interface UsageView {
  /** How full the context is, 0–1, for the ring. */
  fraction: number;
  title: string;
  rows: [string, string][];
  cost: string | null;
  note: string | null;
  plan: string | null;
}

/** "4.2k tokens", "880 tokens", "1.2M tokens". */
export function tokenCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M tokens`;
  if (n >= 10_000) return `${Math.round(n / 1000)}k tokens`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k tokens`;
  return `${n} tokens`;
}

export function money(micro: number): string {
  if (micro < 10_000) return "less than $0.01";
  return `$${(micro / 1_000_000).toFixed(2)}`;
}

export function usageView(u: SessionUsage, options: { timeZone?: string } = {}): UsageView {
  const fraction = u.window ? Math.min(1, Math.round((u.contextTokens / u.window) * 100) / 100) : 0;
  const plan = u.plan
    ? `${Math.round(u.plan.used * 100)}% of your plan's limit${
        u.plan.resetsAt === null
          ? ""
          : ` · resets at ${new Intl.DateTimeFormat("en-GB", {
              timeZone: options.timeZone,
              hour: "2-digit",
              minute: "2-digit",
              hourCycle: "h23",
            }).format(u.plan.resetsAt)}`
      }`
    : null;
  return {
    fraction,
    title: u.window ? `Context usage · ${Math.round(fraction * 100)}%` : "Context usage",
    rows: [
      ["Conversation", tokenCount(u.conversation)],
      ["Files", tokenCount(u.files)],
      ["Screenshots", tokenCount(u.screenshots)],
    ],
    cost: u.estimate === null ? null : `${copy.costLabel} · ${money(u.estimate)}`,
    note:
      u.estimate === null ? null : "Estimate for this session. Your provider bills actual usage.",
    plan,
  };
}
