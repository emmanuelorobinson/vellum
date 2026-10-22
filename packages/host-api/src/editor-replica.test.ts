// TEST DATA: the replica loop over a scripted daemon wire and a counting engine.
import { describe, expect, it, vi } from "vitest";
import { EditorReplica, fromBase64, openReplica, toBase64 } from "./editor-replica";

/** An engine whose history is a list of numbered changes; a cursor is how many it has. */
function engine(changes: number[] = []) {
  return {
    changes,
    cursor: () => Uint8Array.of(changes.length),
    updatesSince: (c: Uint8Array) => Uint8Array.from(changes.slice(c[0] ?? 0)),
    import: (u: Uint8Array) => {
      const fresh = [...u].filter((n) => !changes.includes(n));
      changes.push(...fresh);
      return fresh.length > 0;
    },
  };
}

describe("EditorReplica", () => {
  it("base64 round-trips bytes of any length", () => {
    const bytes = Uint8Array.from({ length: 70_000 }, (_, i) => i % 256);
    expect(fromBase64(toBase64(bytes))).toEqual(bytes);
  });

  it("opens from the daemon's genesis and updates", async () => {
    const wire = vi.fn(async () => ({
      role: "writer",
      genesis: "{}",
      updates: toBase64(Uint8Array.of(7)),
      revision: 3,
      recovery: null,
    }));
    const start = await openReplica(wire, "01JA0000000000000000000A11");
    expect(wire).toHaveBeenCalledWith("open", { device: "01JA0000000000000000000A11" });
    expect(start).toMatchObject({ role: "writer", revision: 3, recovery: null });
    expect([...start.updates]).toEqual([7]);
    await expect(openReplica(async () => ({}), "x")).rejects.toThrow("role");
  });

  it("pushes only the person's new changes, once, and pulls others'", async () => {
    const e = engine([1]);
    const sent: number[][] = [];
    const revisions: number[] = [];
    const wire = vi.fn(async (action: string, params: Record<string, string>) => {
      if (action === "push") sent.push([...fromBase64(params.updates ?? "")]);
      return action === "pull"
        ? { updates: toBase64(Uint8Array.of(9)), revision: 5 }
        : { changed: true, revision: 4 };
    });
    const replica = new EditorReplica(e, wire, { onRevision: (r) => revisions.push(r) });
    e.changes.push(2, 3);
    await replica.push();
    await replica.push();
    expect(sent).toEqual([[2, 3]]);
    expect(await replica.pull()).toBe(true);
    expect(e.changes).toEqual([1, 2, 3, 9]);
    expect(revisions).toEqual([4, 5]);
  });

  it("stops and reports when the daemon refuses", async () => {
    const e = engine();
    const failures: string[] = [];
    const replica = new EditorReplica(
      e,
      async () => {
        throw Error("This editor is not open");
      },
      { onFailure: (m) => failures.push(m) },
    );
    e.changes.push(1);
    await replica.push();
    expect(replica.failure).toBe("This editor is not open");
    expect(failures).toEqual(["This editor is not open"]);
    expect(await replica.pull()).toBe(false);
  });

  it("closing pushes what is left, then closes at the daemon", async () => {
    const e = engine();
    const actions: string[] = [];
    const replica = new EditorReplica(e, async (action) => {
      actions.push(action);
      return { revision: 1 };
    });
    e.changes.push(1);
    await replica.close();
    await replica.close();
    expect(actions).toEqual(["push", "close"]);
  });
});
