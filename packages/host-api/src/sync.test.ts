// TEST DATA: deterministic transport, no sockets, timers or credentials in editor state.

import type { ClientFrame } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import { RoomSyncClient, type SyncWire } from "./sync";

describe("RoomSyncClient", () => {
  it("restores the outbox before a shared engine can replay accepted local updates", () => {
    const client = new RoomSyncClient(
      { async open() {}, send() {}, close() {} },
      {
        apply: async () => {},
        subscribeLocal(listener) {
          listener(new Uint8Array([2]));
          return { dispose() {} };
        },
      },
      undefined,
      { epoch: 1, revision: 1, requests: [{ type: "push", bytes: [1] }] },
    );
    expect(client.recovery().requests).toEqual([
      { type: "push", bytes: [1] },
      { type: "push", bytes: [2] },
    ]);
    client.dispose();
  });
  it("keeps an acknowledged update when the atomic outbox write fails", async () => {
    let receive: (value: unknown) => void = () => {};
    let saves = 0;
    const client = new RoomSyncClient(
      {
        async open(f) {
          receive = f;
        },
        send() {},
        close() {},
      },
      { apply: async () => {} },
      {
        async save() {
          if (++saves === 3) throw Error("TEST DATA: disk failure");
        },
      },
    );
    await client.connect();
    receive({
      v: 1,
      adapter: "loro-1",
      epoch: 1,
      revision: 1,
      body: { type: "state", snapshot: [1], updates: [] },
    });
    await client.idle();
    const pending = client.send(new Uint8Array([2]));
    const rejected = expect(pending).rejects.toThrow("Sync recovery unavailable");
    await client.idle();
    receive({ v: 1, adapter: "loro-1", epoch: 1, revision: 2, body: { type: "ack" } });
    await rejected;
    await client.idle();
    expect(client.pending).toBe(1);
    expect(client.cursor).toEqual({ epoch: 1, revision: 1 });
    expect(client.state).toBe("needs-review");
    client.dispose();
  });
  it("retains a denied design update while still allowing comment and sharing requests", async () => {
    let receive: (value: unknown) => void = () => {};
    const sent: ClientFrame[] = [];
    const client = new RoomSyncClient(
      {
        async open(f) {
          receive = f;
        },
        send(f) {
          sent.push(f);
        },
        close() {},
      },
      { apply: async () => {} },
    );
    await client.connect();
    receive({
      v: 1,
      adapter: "loro-1",
      epoch: 1,
      revision: 1,
      body: { type: "state", snapshot: [1], updates: [] },
    });
    await client.idle();
    const denied = client.send(new Uint8Array([1])).catch(() => {});
    await client.idle();
    receive({
      v: 1,
      adapter: "loro-1",
      epoch: 1,
      revision: 0,
      body: { type: "error", code: "forbidden" },
    });
    await denied;
    await client.idle();
    receive({
      v: 1,
      adapter: "loro-1",
      epoch: 1,
      revision: 2,
      body: { type: "state", snapshot: null, updates: [] },
    });
    await client.idle();
    expect(sent.filter((x) => x.request.type === "push")).toHaveLength(1);
    const management = client.request({ type: "sharing", command: { type: "read" } });
    await client.idle();
    receive({
      v: 1,
      adapter: "loro-1",
      epoch: 1,
      revision: 0,
      body: {
        type: "sharing",
        state: {
          owner_actor: { kind: "person", device: "00000000000000000000000001" },
          owner: false,
          enabled: true,
          can_live: true,
          members: [],
        },
      },
    });
    await management;
    await client.idle();
    expect(client.pending).toBe(1);
    expect(client.state).toBe("ready");
    client.dispose();
  });
  it("bootstraps before writes, retains unacked edits and blocks epoch resets", async () => {
    const sent: unknown[] = [];
    let receive: (value: unknown) => void = () => {};
    const wire: SyncWire = {
      async open(onFrame) {
        receive = onFrame;
      },
      send(frame) {
        sent.push(frame);
      },
      close() {},
    };
    const applied: unknown[] = [];
    const client = new RoomSyncClient(wire, {
      apply: async (s) => {
        applied.push(s);
      },
    });
    await client.connect();
    expect(sent).toEqual([
      { v: 1, adapter: "loro-1", epoch: 0, request: { type: "pull", after: 0 } },
    ]);
    receive({
      v: 1,
      adapter: "loro-1",
      epoch: 1,
      revision: 1,
      body: { type: "state", snapshot: [123, 125], updates: [] },
    });
    await client.idle();
    expect(client.state).toBe("ready");
    const pushing = client.send(new Uint8Array([1, 2]));
    expect(client.pending).toBe(1);
    receive({ v: 1, adapter: "loro-1", epoch: 1, revision: 2, body: { type: "ack" } });
    await pushing;
    expect(client.pending).toBe(0);
    void client.send(new Uint8Array([3])).catch(() => {});
    receive({
      v: 1,
      adapter: "loro-1",
      epoch: 2,
      revision: 3,
      body: { type: "state", snapshot: [123, 125], updates: [] },
    });
    await client.idle();
    expect(client.state).toBe("needs-review");
    expect(client.pending).toBe(1);
    expect(applied).toHaveLength(1);
    client.dispose();
  });
  it("never advances the applied cursor when engine import fails", async () => {
    let receive: (value: unknown) => void = () => {};
    const client = new RoomSyncClient(
      {
        async open(f) {
          receive = f;
        },
        send() {},
        close() {},
      },
      {
        apply: async () => {
          throw Error("TEST DATA failure");
        },
      },
    );
    await client.connect();
    receive({
      v: 1,
      adapter: "loro-1",
      epoch: 1,
      revision: 1,
      body: { type: "state", snapshot: [1], updates: [] },
    });
    await client.idle();
    expect(client.cursor).toEqual({ epoch: 0, revision: 0 });
    expect(client.state).toBe("needs-review");
    client.dispose();
  });
});
