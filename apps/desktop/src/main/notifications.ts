export interface TurnNotice {
  id: string;
  seq: number;
  path: string;
  kind: "working" | "done" | "failed" | "permission" | "stopped";
}
/** Consume authoritative recorded transitions, never infer success from inactivity. */
export class TurnNotifications {
  private seen = new Map<string, number>();
  private initialized = false;
  constructor(
    private readonly show: (kind: "done" | "failed" | "permission", path: string) => void,
  ) {}
  receive(events: TurnNotice[], enabled: boolean, focused: boolean) {
    for (const event of events) {
      const previous = this.seen.get(event.id) ?? 0;
      this.seen.set(event.id, Math.max(previous, event.seq));
      if (
        !this.initialized ||
        event.seq <= previous ||
        !enabled ||
        focused ||
        event.kind === "working" ||
        event.kind === "stopped"
      )
        continue;
      this.show(event.kind, event.path);
    }
    this.initialized = true;
    if (this.seen.size > 2000) this.seen = new Map([...this.seen].slice(-1000));
  }
}
