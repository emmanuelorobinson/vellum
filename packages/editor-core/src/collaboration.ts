/** Live-room UI policy. All content is read from the shared engine; only drafts and camera state live here. */
import type {
  Actor,
  CommentRequest,
  Document,
  Grant,
  Node,
  NodeId,
  Participant,
  Point,
  PresenceState,
  PresenceViewport,
  Role,
  ServerFrame,
  SyncRequest,
} from "@vellum/types-ts";
import { documentNodes } from "./resources/controller";
export interface CollaborationLink {
  readonly state:
    | "offline"
    | "connecting"
    | "ready"
    | "needs-review"
    | "incompatible"
    | "forbidden"
    | "disposed";
  readonly pending: number;
  readonly persistent?: boolean;
  connect(): Promise<void>;
  disconnect(): void;
  request(request: SyncRequest): Promise<void>;
  ephemeral(request: SyncRequest): void;
  subscribeState(listener: () => void): { dispose(): void };
  subscribeFrames(listener: (frame: ServerFrame) => void): { dispose(): void };
  recovery?(): unknown;
}
export interface RoomMember {
  owner?: boolean;
  id: string;
  name: string;
  email: string;
  role: Role;
  status: "pending" | "accepted";
}
export interface LiveSharing {
  canInvite?: boolean;
  enabled: boolean;
  owner: boolean;
  entitled: boolean;
  selfHosted: boolean;
  server: string;
  members: RoomMember[];
}
export interface LiveSharePort {
  read(): Promise<LiveSharing>;
  setEnabled(enabled: boolean): Promise<LiveSharing>;
  invite(email: string, role: Role): Promise<LiveSharing>;
  setRole(member: string, role: Role): Promise<LiveSharing>;
  remove(member: string): Promise<LiveSharing>;
}
export interface CollaborationDeps {
  link: CollaborationLink;
  document(): Document | null;
  subscribeDocument?(listener: () => void): { dispose(): void };
  now(): number;
  nextId(): NodeId;
  local: PresenceState;
  viewport(): PresenceViewport;
  follow(viewport: PresenceViewport): void;
  share?: LiveSharePort;
  copy?(text: string): Promise<void>;
  openLocalCopy?(): Promise<void>;
  reviewRebase?(): Promise<void>;
  update?(): void;
}
export function createCollaboration(deps: CollaborationDeps) {
  let state = {
    active: true,
    panel: false,
    placing: false,
    menu: false,
    filter: "open" as "open" | "resolved",
    selected: null as NodeId | null,
    draft: "",
    draftKey: "new",
    posting: false,
    error: null as string | null,
    grant: null as Grant | null,
    participants: [] as Participant[],
    following: null as string | null,
    returned: false,
    sharing: null as LiveSharing | null,
    sharingBusy: false,
    sharingError: null as string | null,
    newAnchor: null as NodeId | null,
    newPoint: { x: 0, y: 0 } as Point,
  };
  const drafts = new Map<string, string>();
  const pendingMessages = new Map<string, { text: string; id: NodeId }>();
  const listeners = new Set<() => void>();
  let ownViewport: PresenceViewport | null = null;
  let local = { ...deps.local };
  let lastPresenceAt = Number.NEGATIVE_INFINITY;
  let disposed = false;
  const notify = () => {
    if (!disposed) for (const l of [...listeners]) l();
  };
  const live = (p: Participant) =>
    state.active && deps.link.state === "ready" && p.expires_ms > deps.now();
  const follow = (id: string | null) => {
    const target = state.participants.find((p) => p.connection === id && live(p));
    if (!id || !target?.state?.viewport) {
      if (ownViewport) deps.follow(ownViewport);
      state = { ...state, following: null, returned: !!ownViewport };
      ownViewport = null;
    } else {
      if (!ownViewport) ownViewport = { ...deps.viewport() };
      state = { ...state, following: id, returned: false };
      deps.follow(target.state.viewport);
    }
    notify();
  };
  const frames = deps.link.subscribeFrames((f) => {
    if (f.body.type !== "collaboration" || disposed) return;
    const present = new Set(f.body.participants.map((p) => p.connection));
    const lastKnown = state.participants
      .filter((p) => !present.has(p.connection))
      .slice(-64)
      .map((p) => ({ ...p, expires_ms: Math.min(p.expires_ms, deps.now() - 1) }));
    state = {
      ...state,
      grant: JSON.parse(JSON.stringify(f.body.grant)),
      participants: JSON.parse(JSON.stringify([...f.body.participants, ...lastKnown])),
    };
    if (state.following) {
      const p = state.participants.find((p) => p.connection === state.following && live(p));
      if (!p) follow(null);
      else if (p.state?.viewport) deps.follow(p.state.viewport);
    }
    notify();
  });
  const states = deps.link.subscribeState(() => {
    if (deps.link.state !== "ready" && state.following) follow(null);
    notify();
  });
  const documents = deps.subscribeDocument?.(notify);
  const canComment = () =>
    state.active &&
    deps.link.state === "ready" &&
    !!state.grant?.actor &&
    state.grant.actor.kind === "person" &&
    state.grant.role !== "viewer";
  const threads = () =>
    Object.entries(deps.document()?.comments ?? {}).sort(
      ([a, x], [b, y]) => x.createdMs - y.createdMs || a.localeCompare(b),
    );
  const switchDraft = (key: string) => {
    drafts.set(state.draftKey, state.draft);
    state = { ...state, draftKey: key, draft: drafts.get(key) ?? "", error: null };
  };
  const sharing = async (action: () => Promise<LiveSharing>) => {
    if (state.sharingBusy) return;
    state = { ...state, sharingBusy: true, sharingError: null };
    notify();
    try {
      state = { ...state, sharing: await action() };
    } catch {
      state = { ...state, sharingError: "Live collaboration unavailable" };
    } finally {
      state = { ...state, sharingBusy: false };
      notify();
    }
  };
  return {
    get state() {
      return state;
    },
    get connection() {
      return deps.link.state;
    },
    get documentName() {
      return deps.document()?.meta.name ?? "this document";
    },
    get pending() {
      return deps.link.pending;
    },
    get persistent() {
      return deps.link.persistent === true;
    },
    canEdit: () =>
      !state.active ||
      (state.grant?.role === "editor" &&
        !["needs-review", "incompatible", "forbidden"].includes(deps.link.state)),
    canComment,
    threads,
    since(at: number) {
      const minutes = Math.max(0, Math.floor((deps.now() - at) / 60000));
      return minutes ? `${minutes} min ago` : "Just now";
    },
    actorRole(actor: Actor) {
      const id = JSON.stringify(actor);
      const role = state.sharing?.members.find((m) => m.id === id)?.role;
      return role ? role.charAt(0).toUpperCase() + role.slice(1) : "";
    },
    anchor(id: NodeId) {
      const anchor = deps.document()?.comments?.[id]?.anchor;
      if (!anchor) return { available: true, label: "Canvas", path: "Canvas" };
      const doc = deps.document();
      const slug = (s: string) => s.toLowerCase().replace(/\s+/g, "-");
      const walk = (
        nodes: Node[],
        parents: string[],
      ): { available: boolean; label: string; path: string } | null => {
        for (const n of nodes) {
          if (n.id === anchor)
            return {
              available: true,
              label: [parents.at(-1), n.name ?? n.id].filter(Boolean).join(" · "),
              path: `${parents.map(slug).join("/")}#${n.id}`,
            };
          const found = walk((n as { children?: Node[] }).children ?? [], [
            ...parents,
            n.name ?? n.id,
          ]);
          if (found) return found;
        }
        return null;
      };
      for (const p of doc?.pages ?? []) {
        const found = walk(p.children ?? [], [p.name]);
        if (found) return found;
      }
      const node = doc ? documentNodes(doc).find((n) => n.id === anchor) : null;
      return { available: !!node, label: node?.name ?? "Anchor unavailable", path: anchor };
    },
    participants: () => state.participants.filter(live),
    lastKnown: () => state.participants.filter((p) => !live(p)),
    isLive: live,
    actorName: (actor: Actor) =>
      state.participants.find((p) => JSON.stringify(p.actor) === JSON.stringify(actor))?.state
        ?.name ?? (actor.kind === "person" ? "Person" : "Agent"),
    open() {
      state = { ...state, panel: true };
      notify();
    },
    close() {
      state = { ...state, panel: false, placing: false };
      notify();
    },
    menu(open: boolean) {
      state = { ...state, menu: open };
      notify();
    },
    filter(filter: "open" | "resolved") {
      const next = threads().find(([, t]) => t.resolved === (filter === "resolved"));
      switchDraft(next?.[0] ?? "new");
      state = { ...state, filter, selected: next?.[0] ?? null };
      notify();
    },
    select(id: NodeId) {
      switchDraft(id);
      state = { ...state, selected: id, panel: true };
      notify();
    },
    newComment(anchor: NodeId | null, point: Point) {
      switchDraft("new");
      state = {
        ...state,
        placing: false,
        selected: null,
        panel: true,
        newAnchor: anchor,
        newPoint: { ...point },
      };
      notify();
    },
    placeComment(placing: boolean) {
      state = { ...state, placing: placing && canComment(), panel: false };
      notify();
    },
    draft(text: string) {
      state = { ...state, draft: text.slice(0, 8192), error: null };
      drafts.set(state.draftKey, state.draft);
      notify();
    },
    async post() {
      if (!canComment() || state.posting || !state.draft.trim()) return;
      const draft = state.draft,
        draftKey = state.draftKey;
      let pending = pendingMessages.get(draftKey);
      if (!pending || pending.text !== draft) {
        pending = { text: draft, id: deps.nextId() };
        pendingMessages.set(draftKey, pending);
      }
      const request: CommentRequest = state.selected
        ? { type: "reply", thread: state.selected, message: pending.id, text: draft }
        : {
            type: "create",
            thread: pending.id,
            message: pending.id,
            anchor: state.newAnchor,
            page: local.page,
            point: state.newPoint,
            text: draft,
          };
      state = { ...state, posting: true, error: null };
      notify();
      try {
        await deps.link.request({ type: "comment", op: request });
        drafts.delete(draftKey);
        pendingMessages.delete(draftKey);
        if (state.draftKey === draftKey && state.draft === draft)
          state = { ...state, draft: "", selected: request.thread, draftKey: request.thread };
      } catch {
        state = { ...state, error: "Couldn’t post your reply" };
      } finally {
        state = { ...state, posting: false };
        notify();
      }
    },
    canResolve(id: NodeId) {
      const t = deps.document()?.comments?.[id];
      return (
        !!t &&
        canComment() &&
        (state.grant?.role === "editor" ||
          JSON.stringify(t.author) === JSON.stringify(state.grant?.actor))
      );
    },
    async resolve(id: NodeId, resolved: boolean) {
      if (!this.canResolve(id) || state.posting) return;
      state = { ...state, posting: true, error: null };
      notify();
      try {
        await deps.link.request({ type: "comment", op: { type: "resolve", thread: id, resolved } });
      } catch {
        state = { ...state, error: "Couldn’t update this thread" };
      } finally {
        state = { ...state, posting: false };
        notify();
      }
    },
    follow,
    async connect() {
      try {
        await deps.link.connect();
      } catch {
        notify();
      }
    },
    publish(value?: PresenceState) {
      if (value) local = { ...value };
      if (deps.link.state === "ready" && state.active)
        deps.link.ephemeral({ type: "presence", state: local });
    },
    publishView(value: Pick<PresenceState, "page" | "cursor" | "viewport" | "selection">) {
      local = { ...local, ...value };
      if (deps.now() - lastPresenceAt < 50) return;
      lastPresenceAt = deps.now();
      this.publish();
    },
    tick() {
      if (
        state.following &&
        !state.participants.some((p) => p.connection === state.following && live(p))
      )
        follow(null);
      notify();
    },
    async workOffline() {
      deps.link.disconnect();
      await deps.openLocalCopy?.();
      state = { ...state, active: false, following: null };
      notify();
    },
    async copyDraft() {
      await deps.copy?.(state.draft);
    },
    async copyUnsent() {
      await deps.copy?.(JSON.stringify(deps.link.recovery?.() ?? { unavailable: true }));
    },
    async reviewRebase() {
      await deps.reviewRebase?.();
    },
    get canRebase() {
      return !!deps.reviewRebase;
    },
    update() {
      deps.update?.();
    },
    loadSharing: () => (deps.share ? sharing(() => deps.share!.read()) : Promise.resolve()),
    setLive: (enabled: boolean) =>
      state.sharing?.owner &&
      (!enabled || state.sharing.entitled || state.sharing.selfHosted) &&
      deps.share
        ? sharing(() => deps.share!.setEnabled(enabled))
        : Promise.resolve(),
    invite: (email: string, role: Role) =>
      state.sharing?.owner && state.sharing.enabled && deps.share
        ? sharing(() => deps.share!.invite(email, role))
        : Promise.resolve(),
    setRole: (id: string, role: Role) =>
      state.sharing?.owner && deps.share
        ? sharing(() => deps.share!.setRole(id, role))
        : Promise.resolve(),
    remove: (id: string) =>
      state.sharing?.owner && deps.share
        ? sharing(() => deps.share!.remove(id))
        : Promise.resolve(),
    subscribe(l: () => void) {
      listeners.add(l);
      return { dispose: () => void listeners.delete(l) };
    },
    dispose() {
      disposed = true;
      frames.dispose();
      states.dispose();
      documents?.dispose();
      listeners.clear();
    },
  };
}
export type CollaborationController = ReturnType<typeof createCollaboration>;
