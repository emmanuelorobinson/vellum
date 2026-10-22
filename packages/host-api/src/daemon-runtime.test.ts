// TEST DATA: the daemon-backed runtime over a scripted `vellum/editor` call.

import { describe, expect, it } from "vitest";
import { DaemonRuntime, panelRequest } from "./daemon-runtime";
import { fromBase64, toBase64 } from "./editor-replica";

function daemon() {
  const calls: Record<string, string>[] = [];
  let revision = 1;
  const call = async (params: Record<string, string>) => {
    calls.push(params);
    if (params.action === "push" || params.action === "revert" || params.action === "replace")
      revision++;
    if (params.action === "snapshot")
      return { revision, bytes: toBase64(new TextEncoder().encode("{}")) };
    return { revision, updates: "" };
  };
  return { calls, call };
}

describe("DaemonRuntime", () => {
  it("opens one host editor, then follows the daemon's revision", async () => {
    const d = daemon();
    const runtime = await DaemonRuntime.open(d.call, "/w/a.vellum", "01JA0000000000000000000A11");
    expect(d.calls[0]).toMatchObject({
      action: "open",
      path: "/w/a.vellum",
      device: "01JA0000000000000000000A11",
    });
    const seen: number[] = [];
    runtime.subscribe(({ revision }) => seen.push(revision));
    const panel = runtime.newPanel();
    await runtime.relay(panel, "open", {});
    expect(d.calls[1]).toMatchObject({
      action: "open",
      editor: panel,
      device: "01JA0000000000000000000A11",
    });
    await runtime.relay(panel, "push", { updates: "AA==" });
    expect(seen).toEqual([2]);
    const snap = await runtime.snapshot();
    expect(new TextDecoder().decode(snap.bytes)).toBe("{}");
    expect(snap.revision).toBe(2);
  });

  it("revert asks the daemon to read the file; recovery sends the backup; save is acknowledged", async () => {
    const d = daemon();
    const runtime = await DaemonRuntime.open(d.call, "/w/a.vellum", "D");
    await runtime.replace(new Uint8Array([1]), "revert");
    await runtime.replace(new Uint8Array([1, 2]), "recovery");
    await runtime.replace(new Uint8Array(), "external");
    await runtime.written();
    expect(d.calls.slice(1).map((c) => c.action)).toEqual([
      "revert",
      "replace",
      "replace",
      "saved",
    ]);
    expect(fromBase64(d.calls[2]?.bytes ?? "")).toEqual(new Uint8Array([1, 2]));
    expect(d.calls[3]).toMatchObject({ reason: "external", bytes: "" });
  });

  it("a closed panel cannot be relayed for, and dispose closes everything", async () => {
    const d = daemon();
    const runtime = await DaemonRuntime.open(d.call, "/w/a.vellum", "D");
    await expect(runtime.relay("panel-x", "pull", { cursor: "" })).rejects.toThrow();
    const panel = runtime.newPanel();
    runtime.dispose();
    await Promise.resolve();
    expect(d.calls.filter((c) => c.action === "close").map((c) => c.editor)).toEqual([
      panel,
      d.calls[0]?.editor,
    ]);
  });

  it("panel requests carry only their own fields", () => {
    expect(panelRequest("push", { updates: "AA==" })).toEqual({
      action: "push",
      params: { updates: "AA==" },
    });
    expect(panelRequest("push", { updates: "AA==", path: "/etc" })).toBeNull();
    expect(panelRequest("open", { device: "someone" })).toBeNull();
    expect(panelRequest("save", {})).toBeNull();
    expect(panelRequest("pull", { cursor: 1 })).toBeNull();
  });
});
