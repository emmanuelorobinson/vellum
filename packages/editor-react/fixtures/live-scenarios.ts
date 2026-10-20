// TEST DATA. Scripted transport and engine snapshots, never a TypeScript document engine.

import type { EditorSession } from "@vellum/editor-core";
import { type CollaborationLink, createCollaboration, type LiveSharing } from "@vellum/editor-core";
import type { EngineClient } from "@vellum/engine-client";
import type {
  Document,
  Grant,
  Node,
  Participant,
  Role,
  ServerFrame,
  SyncRequest,
} from "@vellum/types-ts";
export const LIVE_NOW = 1792496520000;
export const LIVE_PEOPLE = [
  { kind: "person", device: "01JA0000000000000000000A11" },
  { kind: "person", device: "01JA0000000000000000000B0B" },
] as const;
export const LIVE_PARTICIPANTS: Participant[] = [
  {
    connection: "sam",
    actor: LIVE_PEOPLE[0],
    expires_ms: LIVE_NOW + 30000,
    state: {
      name: "Sam Okafor",
      page: null,
      cursor: { x: 826, y: 520 },
      viewport: { x: 0, y: 0, zoom: 1 },
      selection: [],
      activity: "Editing button label",
      hue: null,
    },
  },
  {
    connection: "teal",
    actor: { kind: "agent", agent: "01JA00000000000000000TEA10" },
    expires_ms: LIVE_NOW + 30000,
    state: {
      name: "Teal",
      page: null,
      cursor: { x: 880, y: 340 },
      viewport: { x: 0, y: 0, zoom: 1 },
      selection: [],
      activity: "Designing Step 2",
      hue: "teal",
    },
  },
  {
    connection: "alex",
    actor: LIVE_PEOPLE[1],
    expires_ms: LIVE_NOW + 30000,
    state: {
      name: "Alex Rivera",
      page: null,
      cursor: { x: 520, y: 480 },
      viewport: { x: 100, y: 50, zoom: 1.2 },
      selection: [],
      activity: "Viewing Welcome",
      hue: null,
    },
  },
  {
    connection: "claude",
    actor: { kind: "agent", agent: "01JA00000000000000000R0SE0" },
    expires_ms: LIVE_NOW + 30000,
    state: {
      name: "Claude Code",
      page: null,
      cursor: { x: 1080, y: 360 },
      viewport: { x: 0, y: 0, zoom: 1 },
      selection: [],
      activity: "Writing Step 3",
      hue: "saffron",
    },
  },
];
export function liveDocument(doc: Document): Document {
  doc = JSON.parse(JSON.stringify(doc)) as Document;
  const rename = (nodes: Node[]) => {
    for (const n of nodes) {
      if (n.id === "continue") n.name = "Continue button";
      if ((n as { children?: Node[] }).children?.some((child) => child.id === "continue"))
        n.name = "Step 1";
      rename((n as { children?: Node[] }).children ?? []);
    }
  };
  for (const p of doc.pages) rename(p.children ?? []);
  return {
    ...doc,
    comments: {
      "comment-1": {
        author: LIVE_PEOPLE[1],
        anchor: "welcome",
        point: { x: 610, y: 330 },
        createdMs: LIVE_NOW - 480000,
        resolved: false,
        resolvedBy: null,
        messages: {
          "message-1": {
            author: LIVE_PEOPLE[1],
            atMs: LIVE_NOW - 480000,
            text: "The sprout could be a little larger.",
          },
        },
      },
      "comment-2": {
        author: LIVE_PEOPLE[0],
        anchor: "continue",
        point: { x: 630, y: 560 },
        createdMs: LIVE_NOW - 120000,
        resolved: false,
        resolvedBy: null,
        messages: {
          "message-2": {
            author: LIVE_PEOPLE[0],
            atMs: LIVE_NOW - 120000,
            text: "Should this button say “Find your rhythm” or “Start your habit”?",
          },
          "message-3": {
            author: LIVE_PEOPLE[1],
            atMs: LIVE_NOW - 60000,
            text: "“Find your rhythm” matches the welcome screen.",
          },
        },
      },
      "comment-3": {
        author: LIVE_PEOPLE[1],
        anchor: "step2",
        point: { x: 1000, y: 270 },
        createdMs: LIVE_NOW - 60000,
        resolved: false,
        resolvedBy: null,
        messages: {
          "message-4": {
            author: LIVE_PEOPLE[1],
            atMs: LIVE_NOW - 60000,
            text: "Is Step 2 still planned?",
          },
        },
      },
      "comment-4": {
        author: LIVE_PEOPLE[0],
        anchor: null,
        point: { x: 0, y: 0 },
        createdMs: LIVE_NOW - 30000,
        resolved: true,
        resolvedBy: LIVE_PEOPLE[0],
        messages: {
          "resolved-1": {
            author: LIVE_PEOPLE[0],
            atMs: LIVE_NOW - 30000,
            text: "TEST DATA: resolved thread",
          },
        },
      },
      "comment-5": {
        author: LIVE_PEOPLE[1],
        anchor: null,
        point: { x: 0, y: 0 },
        createdMs: LIVE_NOW - 20000,
        resolved: true,
        resolvedBy: LIVE_PEOPLE[1],
        messages: {
          "resolved-2": {
            author: LIVE_PEOPLE[1],
            atMs: LIVE_NOW - 20000,
            text: "TEST DATA: resolved thread",
          },
        },
      },
    },
  };
}
export function liveScenario(engine: EngineClient, session: EditorSession, name: string) {
  const frameListeners = new Set<(f: ServerFrame) => void>();
  const stateListeners = new Set<() => void>();
  const requests: SyncRequest[] = [];
  const role: Role = name === "viewer" ? "viewer" : name === "commenter" ? "commenter" : "editor";
  let grant: Grant = { actor: LIVE_PEOPLE[0], role };
  const link: CollaborationLink = {
    state: ["offline", "connecting", "incompatible", "forbidden", "needs-review"].includes(name)
      ? (name as CollaborationLink["state"])
      : "ready",
    pending: 0,
    async connect() {
      Object.assign(link, { state: "ready" });
      for (const l of stateListeners) l();
    },
    disconnect() {
      Object.assign(link, { state: "offline" });
      for (const l of stateListeners) l();
    },
    async request(r) {
      requests.push(r);
      if (name === "failed") throw Error("TEST DATA: transport refused");
    },
    ephemeral(r) {
      requests.push(r);
    },
    subscribeState(l) {
      stateListeners.add(l);
      return { dispose: () => void stateListeners.delete(l) };
    },
    subscribeFrames(l) {
      frameListeners.add(l);
      return { dispose: () => void frameListeners.delete(l) };
    },
  };
  let counter = 0;
  let sharing: LiveSharing = {
    canInvite: true,
    enabled: true,
    owner: true,
    entitled: true,
    selfHosted: name === "selfhost",
    server: name === "selfhost" ? "studio.vellum.internal" : "Hosted Team · vellum.design",
    members: [
      {
        id: "sam",
        name: "Sam Okafor",
        email: "sam@studio.example",
        role: "editor",
        status: "accepted",
        owner: true,
      },
      {
        id: JSON.stringify(LIVE_PEOPLE[1]),
        name: "Alex Rivera",
        email: "alex@studio.example",
        role: ["comments", "viewer", "commenter", "failed", "lost", "missing"].includes(name)
          ? "commenter"
          : "editor",
        status: "accepted",
      },
    ],
  };
  const c = createCollaboration({
    link,
    document: () => engine.snapshot().document,
    subscribeDocument: (l) => engine.subscribe(() => l()),
    now: () => LIVE_NOW,
    nextId: () => `new-comment-${++counter}`,
    local: LIVE_PARTICIPANTS[0]!.state!,
    viewport: () => session.state.viewport,
    follow: (v) => session.setViewport(v),
    copy: async () => {},
    openLocalCopy: async () => {},
    share: {
      read: async () => sharing,
      setEnabled: async (enabled) => (sharing = { ...sharing, enabled }),
      invite: async (email, role) =>
        (sharing = {
          ...sharing,
          members: [
            ...sharing.members,
            { id: "invited", email, name: "", role, status: "pending" },
          ],
        }),
      setRole: async (id, role) =>
        (sharing = {
          ...sharing,
          members: sharing.members.map((m) => (m.id === id ? { ...m, role } : m)),
        }),
      remove: async (id) =>
        (sharing = { ...sharing, members: sharing.members.filter((m) => m.id !== id) }),
    },
  });
  const emit = () => {
    for (const l of frameListeners)
      l({
        v: 1,
        adapter: "loro-1",
        epoch: 1,
        revision: 1,
        body: { type: "collaboration", grant, participants: LIVE_PARTICIPANTS },
      });
  };
  emit();
  if (["comments", "viewer", "commenter", "failed", "lost", "missing"].includes(name)) {
    c.select("comment-2");
  }
  if (name === "lost") {
    c.draft("TEST DATA: draft retained after permission loss");
    grant = { ...grant, role: "viewer" };
    emit();
  }
  if (name === "following") c.follow("alex");
  return {
    controller: c,
    requests,
    role(role: Role) {
      grant = { ...grant, role };
      emit();
    },
    connection(state: CollaborationLink["state"]) {
      Object.assign(link, { state });
      for (const l of stateListeners) l();
    },
  };
}
