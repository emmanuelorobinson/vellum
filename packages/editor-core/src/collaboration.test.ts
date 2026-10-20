// TEST DATA. Network state, actor shapes and drafts are independent of the local daemon.

import type { Document, ServerFrame, SyncRequest } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import { type CollaborationLink, createCollaboration } from "./collaboration";

describe("collaboration", () => {
  it("preserves a draft after role loss and never presents stale remote cursors as live", async () => {
    let listener: (frame: ServerFrame) => void = () => {};
    let changed: () => void = () => {};
    const sent: SyncRequest[] = [];
    const link: CollaborationLink = {
      state: "ready",
      pending: 0,
      async connect() {},
      disconnect() {},
      async request(r) {
        sent.push(r);
      },
      ephemeral() {},
      subscribeState(l) {
        changed = l;
        return { dispose() {} };
      },
      subscribeFrames(l) {
        listener = l;
        return { dispose() {} };
      },
    };
    const doc: Document = {
      meta: { formatVersion: 1, createdWith: "TEST DATA", name: "TEST DATA" },
      pages: [],
    };
    let viewport = { x: 0, y: 0, zoom: 1 };
    const c = createCollaboration({
      link,
      document: () => doc,
      now: () => 10,
      nextId: () => "test-id",
      local: {
        name: "TEST DATA",
        page: null,
        cursor: null,
        viewport: null,
        selection: [],
        activity: "",
        hue: null,
      },
      viewport: () => viewport,
      follow: (v) => {
        viewport = v;
      },
    });
    const actor = { kind: "person" as const, device: "00000000000000000000000001" };
    listener({
      v: 1,
      adapter: "loro-1",
      epoch: 1,
      revision: 1,
      body: {
        type: "collaboration",
        grant: { actor, role: "editor" },
        participants: [
          {
            connection: "other",
            actor,
            expires_ms: 100,
            state: {
              name: "TEST DATA",
              cursor: { x: 1, y: 2 },
              page: null,
              viewport: { x: 5, y: 6, zoom: 2 },
              selection: [],
              activity: "Viewing",
              hue: null,
            },
          },
        ],
      },
    });
    c.draft("kept text");
    c.placeComment(true);
    expect(c.state.placing).toBe(true);
    expect(c.state.panel).toBe(false);
    c.newComment(null, { x: 120, y: 80 });
    expect(c.state.placing).toBe(false);
    expect(c.state.newPoint).toEqual({ x: 120, y: 80 });
    expect(c.canEdit()).toBe(true);
    c.follow("other");
    expect(viewport.x).toBe(5);
    c.follow(null);
    expect(viewport.x).toBe(0);
    listener({
      v: 1,
      adapter: "loro-1",
      epoch: 1,
      revision: 1,
      body: { type: "collaboration", grant: { actor, role: "viewer" }, participants: [] },
    });
    expect(c.canEdit()).toBe(false);
    expect(c.state.draft).toBe("kept text");
    await c.post();
    expect(sent).toHaveLength(0);
    Object.assign(link, { state: "offline" });
    changed();
    expect(c.participants()).toEqual([]);
    expect(c.state.draft).toBe("kept text");
    c.dispose();
  });
});
