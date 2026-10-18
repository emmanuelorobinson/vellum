/**
 * The Agent panel's feature port (host-api's DaemonClient: "session data is fetched through later typed feature
 * ports, never raw RPC"). A host composes it from the daemon's session store (ADR-006) and its live turn stream;
 * the panel never talks to an engine.
 */
import type { AgentId, SessionId, TurnId } from "@vellum/types-ts";
import type { EngineCatalog, EngineChoice, ProviderSetup, ProviderStatusName } from "./engines";
import type { SessionEvent } from "./log";
import type { PermissionAnswer, PermissionQuestion } from "./permissions";
import type { SessionInfo } from "./sessions";
import type { SessionUsage } from "./usage";

/** What an engine streams while a turn runs and the log does not keep (`TurnEvent` in vellum-core). */
export type LiveEvent =
  | { type: "text"; text: string }
  | { type: "toolStarted"; call: string; name: string; arguments?: unknown }
  | { type: "toolFinished"; call: string; name: string; failed: boolean };

export type FeedUpdate =
  | { type: "sessions"; sessions: SessionInfo[] }
  | { type: "event"; session: SessionId; event: SessionEvent }
  | { type: "live"; session: SessionId; turn: TurnId; item: LiveEvent }
  /** The broker is waiting for the person (the turn is `awaiting_permission`). */
  | { type: "question"; session: SessionId; question: PermissionQuestion }
  /** The broker recorded a decision for it (or the turn stopped): the card goes. */
  | { type: "answered"; session: SessionId; question: string }
  | { type: "usage"; session: SessionId; usage: SessionUsage }
  | { type: "engines"; catalog: EngineCatalog }
  | { type: "permissionSettings"; skipAll: boolean };

/** A composer chip: what the person attached to a request (DESIGN.md "Context chips"). */
export interface ContextChip {
  kind: "selection" | "page" | "file" | "frame" | "component" | "variable" | "attachment";
  /** What the chip says ("3 layers in Step 2", "DESIGN.md"). */
  label: string;
  /** What the daemon resolves: node ids, a page id, a workspace path, an attachment handle. */
  value: string;
}

export type ParallelMode = "split" | "side_by_side";
export type ExploreVary = "layout" | "style";

/** One request from the composer. The daemon starts (or queues) a turn and answers with its id. */
export interface PromptRequest {
  text: string;
  context: ContextChip[];
  /** Skill paths, as the skills list names them. */
  skills: string[];
  parallel: { count: number; mode: ParallelMode };
  /** Explore: `count` successive variants after the first result; `null` is off. */
  explore: { count: number; vary: ExploreVary } | null;
  engine: EngineChoice | null;
  /** Sent while an agent works: wait for the current turn, or stop it first. */
  queue: "after" | "now" | null;
}

export type SkillGroupName = "builtin" | "design_system" | "user";
/** vellum-core `SkillMeta`. */
export interface SkillInfo {
  path: string;
  name: string;
  group: SkillGroupName;
}

export interface AgentFeed {
  /** This document's sessions. */
  sessions(): Promise<SessionInfo[]>;
  /** A session's log, oldest first. */
  events(session: SessionId): Promise<SessionEvent[]>;
  subscribe(listener: (update: FeedUpdate) => void): { dispose(): void };
  create(): Promise<SessionInfo>;
  /** Deletes the session's log and its pictures; never the changes it made. */
  remove(session: SessionId): Promise<void>;
  /** Undo this turn: the daemon reverts only that turn's changes (`ops::undo_turn`). `null`: an external agent's steps. */
  undoTurn(session: SessionId, turn: TurnId | null): Promise<void>;
  retry(session: SessionId, turn: TurnId): Promise<void>;
  retryStep(session: SessionId, turn: TurnId, seq: number): Promise<void>;
  /** Stop every agent working in the session; for an external agent, its writes. */
  stop(session: SessionId): Promise<void>;
  /** A URL for a cached picture (thumbnail or screenshot), by its hash; `null` when the cache does not have it. */
  image(hash: string): string | null;

  /** Starts a turn in the session (or queues it) and answers with the turn's id. */
  send(session: SessionId, request: PromptRequest): Promise<{ turn: TurnId }>;
  /** Stop one agent of the session's running turn; the others go on. */
  stopAgent(session: SessionId, agent: AgentId): Promise<void>;
  /** The questions the broker is waiting on in this session. */
  questions(session: SessionId): Promise<PermissionQuestion[]>;
  /** Answer a Permission card. The broker records it; the card goes when the feed says `answered`. */
  answer(session: SessionId, question: string, answer: PermissionAnswer): Promise<void>;
  /** "Skip all permission prompts" (Settings): off by default. */
  permissionSettings(): Promise<{ skipAll: boolean }>;
  setSkipAll(on: boolean): Promise<void>;
  /** The session's context and estimated cost; `null` before its first turn. */
  usage(session: SessionId): Promise<SessionUsage | null>;
  skills(): Promise<SkillInfo[]>;
  /** Add skill…: the host asks for a folder with a SKILL.md; `null` when the person cancels. */
  addSkill(): Promise<SkillInfo | null>;
  /** Workspace files for @ mentions, matching the query; `extensions` narrows them (".md"). */
  files(query: string, extensions?: readonly string[]): Promise<string[]>;
  engines(): Promise<EngineCatalog>;
  /** Saves a provider's setup and tests it. The key goes to the system keychain and never comes back. */
  saveProvider(setup: ProviderSetup): Promise<ProviderStatusName>;
  recheckProvider(provider: string): Promise<ProviderStatusName>;
  clearKey(provider: string): Promise<ProviderStatusName>;
}

export type FeedRequest =
  | { type: "remove"; session: SessionId }
  | { type: "undoTurn"; session: SessionId; turn: TurnId | null }
  | { type: "retry"; session: SessionId; turn: TurnId }
  | { type: "retryStep"; session: SessionId; turn: TurnId; seq: number }
  | { type: "stop"; session: SessionId }
  | { type: "stopAgent"; session: SessionId; agent: AgentId }
  | { type: "answer"; session: SessionId; question: string; answer: PermissionAnswer }
  | { type: "setSkipAll"; on: boolean }
  | { type: "saveProvider"; provider: string; endpoint?: string; apiStyle?: string }
  | { type: "recheckProvider"; provider: string }
  | { type: "clearKey"; provider: string };
