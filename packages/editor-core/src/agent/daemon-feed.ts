/**
 * The Agent panel's feed over the daemon's session store (`vellum/agentFeed`, ADR-006): the sessions of the open
 * document, their events as they are recorded, step pictures, and the person's controls (remove, undo a turn,
 * stop). The host relays `call` and adds the document's path. Starting turns from the composer and provider
 * setup are not served by the daemon yet: they fail with "unavailable", and the panel says so.
 */
import type { DeviceId, SessionId, TurnId } from "@vellum/types-ts";
import type { AgentFeed, FeedUpdate } from "./feed";
import type { SessionEvent } from "./log";
import type { SessionInfo } from "./sessions";

export type FeedCall = (action: string, params: Record<string, unknown>) => Promise<unknown>;

const platform = globalThis as unknown as {
  setInterval(run: () => void, ms: number): unknown;
  clearInterval(timer: unknown): void;
};

export class DaemonAgentFeed implements AgentFeed {
  private listeners = new Set<(update: FeedUpdate) => void>();
  private timer: unknown;
  private seen = new Map<SessionId, number>();
  private listed = "";
  private pictures = new Map<string, string | "loading">();
  private polling = false;

  constructor(
    private readonly call: FeedCall,
    private readonly options: { device: DeviceId; interval?: number },
  ) {}

  private emit(update: FeedUpdate) {
    for (const listener of [...this.listeners]) listener(update);
  }

  async sessions(): Promise<SessionInfo[]> {
    return (await this.call("sessions", {})) as SessionInfo[];
  }

  async events(session: SessionId): Promise<SessionEvent[]> {
    const events = (await this.call("events", { session, since: 0 })) as SessionEvent[];
    this.seen.set(session, events.at(-1)?.seq ?? 0);
    return events;
  }

  /** One round: new and changed sessions, then what each loaded session recorded since it was last read. */
  async poll() {
    if (this.polling) return;
    this.polling = true;
    try {
      const sessions = await this.sessions();
      const listed = JSON.stringify(sessions);
      if (listed !== this.listed) {
        this.listed = listed;
        this.emit({ type: "sessions", sessions });
      }
      for (const [session, since] of this.seen) {
        if (!sessions.some((s) => s.id === session)) {
          this.seen.delete(session);
          continue;
        }
        const fresh = (await this.call("events", { session, since })) as SessionEvent[];
        for (const event of fresh) {
          this.seen.set(session, event.seq);
          this.emit({ type: "event", session, event });
        }
      }
    } catch {
      // The daemon is restarting or offline: the next round tries again.
    } finally {
      this.polling = false;
    }
  }

  subscribe(listener: (update: FeedUpdate) => void) {
    this.listeners.add(listener);
    this.timer ??= platform.setInterval(() => void this.poll(), this.options.interval ?? 1000);
    return {
      dispose: () => {
        this.listeners.delete(listener);
        if (!this.listeners.size && this.timer !== undefined) {
          platform.clearInterval(this.timer);
          this.timer = undefined;
        }
      },
    };
  }

  async remove(session: SessionId) {
    await this.call("remove", { session });
    this.seen.delete(session);
    await this.poll();
  }

  async undoTurn(session: SessionId, turn: TurnId | null) {
    await this.call("undoTurn", {
      session,
      device: this.options.device,
      ...(turn ? { turn } : {}),
    });
  }

  async stop(session: SessionId) {
    await this.call("stop", { session });
  }

  async stopAgent(session: SessionId) {
    await this.stop(session);
  }

  /** A step's picture as a data URL; null until it has been fetched (the panel draws again when it has). */
  image(hash: string): string | null {
    const picture = this.pictures.get(hash);
    if (picture && picture !== "loading") return picture;
    if (!picture) {
      this.pictures.set(hash, "loading");
      void this.call("image", { hash }).then(
        (reply) => {
          const { mime, base64 } = reply as { mime: string; base64: string };
          this.pictures.set(hash, `data:${mime};base64,${base64}`);
          if (this.listed) this.emit({ type: "sessions", sessions: JSON.parse(this.listed) });
        },
        () => this.pictures.delete(hash),
      );
    }
    return null;
  }

  // Not served by the daemon yet: turns start from a CLI or an MCP client (reports.md).
  private unavailable(): Promise<never> {
    return Promise.reject(Error("unavailable"));
  }
  create() {
    return this.unavailable();
  }
  retry() {
    return this.unavailable();
  }
  retryStep() {
    return this.unavailable();
  }
  send() {
    return this.unavailable();
  }
  buildPlan() {
    return this.unavailable();
  }
  editRequest() {
    return this.unavailable();
  }
  keepOptions() {
    return this.unavailable();
  }
  answer() {
    return this.unavailable();
  }
  setSkipAll() {
    return this.unavailable();
  }
  saveProvider() {
    return this.unavailable();
  }
  recheckProvider() {
    return this.unavailable();
  }
  clearKey() {
    return this.unavailable();
  }
  async addSkill() {
    return null;
  }
  async questions() {
    return [];
  }
  async permissionSettings() {
    return { skipAll: false };
  }
  async usage() {
    return null;
  }
  async skills() {
    return [];
  }
  async files() {
    return [];
  }
  async engines() {
    return { builtin: [], installed: [] };
  }
}
