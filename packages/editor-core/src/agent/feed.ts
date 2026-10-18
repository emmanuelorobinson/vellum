/**
 * The Agent panel's feature port (host-api's DaemonClient: "session data is fetched through later typed feature
 * ports, never raw RPC"). A host composes it from the daemon's session store (ADR-006) and its live turn stream;
 * the panel never talks to an engine.
 */
import type { SessionId, TurnId } from "@vellum/types-ts";
import type { SessionEvent } from "./log";
import type { SessionInfo } from "./sessions";

/** What an engine streams while a turn runs and the log does not keep (`TurnEvent` in vellum-core). */
export type LiveEvent =
  | { type: "text"; text: string }
  | { type: "toolStarted"; call: string; name: string; arguments?: unknown }
  | { type: "toolFinished"; call: string; name: string; failed: boolean };

export type FeedUpdate =
  | { type: "sessions"; sessions: SessionInfo[] }
  | { type: "event"; session: SessionId; event: SessionEvent }
  | { type: "live"; session: SessionId; turn: TurnId; item: LiveEvent };

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
}

export type FeedRequest =
  | { type: "remove"; session: SessionId }
  | { type: "undoTurn"; session: SessionId; turn: TurnId | null }
  | { type: "retry"; session: SessionId; turn: TurnId }
  | { type: "retryStep"; session: SessionId; turn: TurnId; seq: number }
  | { type: "stop"; session: SessionId };
