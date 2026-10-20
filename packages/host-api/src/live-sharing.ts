/** Adapter over the room protocol. Public state contains no capability, email invitation or keychain entry. */
import type { Actor, Role, SharingCommand, SharingState } from "@vellum/types-ts";
import type { RoomSyncClient } from "./sync";
export class RoomLiveSharing {
  private received: SharingState | null = null;
  private sub: { dispose(): void };
  private actors = new Map<string, Actor>();
  constructor(
    private client: RoomSyncClient,
    private options: { selfHosted: boolean; server: string; name?(actor: Actor): string },
  ) {
    this.sub = client.subscribeFrames((f) => {
      if (f.body.type === "sharing") this.received = f.body.state;
    });
  }
  private async command(command: SharingCommand) {
    this.received = null;
    await this.client.connect();
    await this.client.request({ type: "sharing", command });
    await this.client.idle();
    const s = this.received as SharingState | null;
    if (!s) throw Error("Live collaboration unavailable");
    this.actors.clear();
    return {
      enabled: s.enabled,
      owner: s.owner,
      entitled: s.can_live,
      selfHosted: this.options.selfHosted,
      server: this.options.server,
      canInvite: false,
      members: s.members
        .filter((g) => g.actor.kind === "person")
        .map((g) => {
          const id = JSON.stringify(g.actor);
          this.actors.set(id, g.actor);
          return {
            id,
            owner: JSON.stringify(g.actor) === JSON.stringify(s.owner_actor),
            name: this.options.name?.(g.actor) ?? "Person",
            email: "",
            role: g.role,
            status: "accepted" as const,
          };
        }),
    };
  }
  read() {
    return this.command({ type: "read" });
  }
  setEnabled(enabled: boolean) {
    return this.command({ type: "set_live", enabled });
  }
  setRole(id: string, role: Role) {
    const actor = this.actors.get(id);
    if (!actor) return Promise.reject(Error("Member unavailable"));
    return this.command({ type: "set_role", actor, role });
  }
  remove(id: string) {
    const actor = this.actors.get(id);
    if (!actor) return Promise.reject(Error("Member unavailable"));
    return this.command({ type: "remove", actor });
  }
  async invite(_email: string, _role: Role): Promise<never> {
    throw Error("Invitations require an account issuer");
  }
  dispose() {
    this.sub.dispose();
  }
}
