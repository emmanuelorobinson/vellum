/**
 * The Engine and model picker's data (DESIGN.md C11): built-in providers with their models, installed engines, and
 * provider setup. The daemon owns keys and connections; the editor sees only names, capabilities and status words.
 */

export type ProviderStatusName =
  | "connected"
  | "api_key_set"
  | "signed_in"
  | "configured_externally"
  | "failed";

/** As vellum-core `ProviderStatus`, in words (DESIGN.md "provider status row"). */
const STATUS: Record<ProviderStatusName, string> = {
  connected: "Connected",
  api_key_set: "API key set · not tested",
  signed_in: "Signed in",
  configured_externally: "Configured outside Vellum (not tested)",
  failed: "Failed",
};
export const providerStatusLine = (s: ProviderStatusName) => STATUS[s];

export type Effort = "low" | "medium" | "high";
export const EFFORTS: readonly { id: Effort; label: string }[] = [
  { id: "low", label: "Low" },
  { id: "medium", label: "Medium" },
  { id: "high", label: "High" },
];

export interface ModelCaps {
  vision: boolean;
  fast: boolean;
  /** Supports reasoning effort. */
  reasoning: boolean;
  /** `null`: unknown, never unlimited. */
  contextTokens: number | null;
  /** Runs on this computer (a local endpoint). */
  local?: boolean;
}

/** A context this large reads as "Large context". */
const LARGE_CONTEXT = 200_000;

/** "Vision · Fast · Large context"; a local model says where it runs instead. */
export function capabilityWords(caps: ModelCaps): string {
  if (caps.local) return "On your computer";
  const words: string[] = [];
  if (caps.vision) words.push("Vision");
  if (caps.fast) words.push("Fast");
  if ((caps.contextTokens ?? 0) >= LARGE_CONTEXT) words.push("Large context");
  if (caps.reasoning) words.push("Reasoning");
  return words.join(" · ");
}

export interface ModelOption {
  /** What the daemon calls it ("deepseek/deepseek-v4.1-flash"). */
  id: string;
  /** What the picker shows, in mono. */
  label: string;
  caps: ModelCaps;
}

export interface BuiltinProvider {
  id: string;
  name: string;
  status: ProviderStatusName;
  models: ModelOption[];
}

export type InstalledState = "signed_in" | "signed_out" | "missing" | "usage_limit";
export interface InstalledEngine {
  /** "claude-code", "codex". */
  id: string;
  name: string;
  state: InstalledState;
  /** The command that signs in ("claude"), for the signed-out line. */
  command?: string;
  /** Usage limit: when the plan's allowance comes back, in UTC milliseconds. */
  resetsAt?: number;
  /** Models the program offers, as it lists them. Without any, it chooses its own. */
  models?: ModelOption[];
}

const INSTALLED: Record<InstalledState, string> = {
  signed_in: "Installed · signed in",
  signed_out: "Installed · not signed in",
  missing: "Not installed",
  usage_limit: "Installed · usage limit reached",
};
export const installedLine = (e: InstalledEngine) =>
  e.state === "signed_out" && e.command
    ? `${e.name} isn't signed in · Run \`${e.command}\` in a terminal to sign in`
    : INSTALLED[e.state];

export interface EngineCatalog {
  builtin: BuiltinProvider[];
  installed: InstalledEngine[];
}

/** What the next turn runs on. Installed engines choose their own model unless one is named. */
export interface EngineChoice {
  /** "builtin", or an installed engine's id. */
  engine: string;
  provider?: string;
  model: string;
  effort?: Effort;
}

export const sameProvider = (a: EngineChoice | null, b: EngineChoice | null) =>
  !!a && !!b && a.engine === b.engine && (a.provider ?? null) === (b.provider ?? null);

export type ApiStyle = "chat_completions" | "responses" | "anthropic_messages";
export const API_STYLES: readonly { id: ApiStyle; label: string }[] = [
  { id: "chat_completions", label: "Chat Completions" },
  { id: "responses", label: "Responses" },
  { id: "anthropic_messages", label: "Anthropic Messages" },
];

/** Add provider's list (C11), in the design's order. `endpoint`: it asks for one (optional for Local). */
export const PROVIDERS: readonly {
  id: string;
  name: string;
  endpoint?: "optional" | "required";
}[] = [
  { id: "openrouter", name: "OpenRouter" },
  { id: "anthropic", name: "Anthropic" },
  { id: "openai", name: "OpenAI" },
  { id: "google", name: "Google Gemini" },
  { id: "deepseek", name: "DeepSeek" },
  { id: "xai", name: "xAI" },
  { id: "moonshot", name: "Moonshot / Kimi" },
  { id: "together", name: "Together" },
  { id: "fireworks", name: "Fireworks" },
  { id: "zai", name: "Z.AI" },
  { id: "opencode", name: "OpenCode" },
  { id: "bedrock", name: "AWS Bedrock" },
  { id: "vertex", name: "Google Vertex" },
  { id: "foundry", name: "Microsoft Foundry" },
  { id: "local", name: "Local", endpoint: "optional" },
  { id: "custom", name: "Custom endpoint", endpoint: "required" },
];

/** One provider's setup. The key passes through to the daemon, which keeps it in the system keychain only. */
export interface ProviderSetup {
  provider: string;
  key?: string;
  endpoint?: string;
  apiStyle?: ApiStyle;
}
