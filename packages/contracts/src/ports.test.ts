import { FakeEngineClient, WasmEngineClient } from "@vellum/engine-client";
import {
  FakeDaemonClient,
  NoopSyncClient,
  NoopTelemetry,
  RelayedDaemonClient,
} from "@vellum/host-api";
import { createBrowserHost } from "@vellum/host-browser";
import { createElectronHost } from "@vellum/host-electron";
import { FakeHost } from "@vellum/host-fake";
import { createVscodeHost } from "@vellum/host-vscode";
import { describe, expect, it } from "vitest";
import {
  daemonContract,
  engineContract,
  hostContract,
  syncContract,
  telemetryContract,
} from "./ports";

describe("editor port contracts", () => {
  for (const kind of ["fake", "browser", "electron", "vscode"] as const)
    it(`Host: ${kind}`, async () => {
      const backing = new FakeHost();
      const host =
        kind === "fake"
          ? backing
          : kind === "browser"
            ? createBrowserHost(backing)
            : kind === "electron"
              ? createElectronHost(backing)
              : createVscodeHost(backing);
      await hostContract({
        host,
        failNextWrite: () => backing.failNextWrite(),
        emitTheme: () => backing.setTheme({ appearance: "dark" }),
      });
    });
  for (const wasm of [false, true])
    it(`EngineClient: ${wasm ? "wasm boundary" : "fake"}`, async () => {
      const backing = new FakeEngineClient();
      engineContract(wasm ? new WasmEngineClient(backing) : backing, (snapshot, operation) =>
        backing.script(snapshot, operation),
      );
    });
  for (const relayed of [false, true])
    it(`DaemonClient: ${relayed ? "relayed" : "fake"}`, async () => {
      const backing = new FakeDaemonClient();
      await daemonContract(relayed ? new RelayedDaemonClient(backing) : backing, backing);
    });
  it("SyncClient stays offline and sends nothing", async () => {
    await syncContract(new NoopSyncClient());
  });
  it("Telemetry is off and discards events", () => {
    telemetryContract(new NoopTelemetry());
  });
  it("browser without file access reports read-only instead of fabricating data", async () => {
    const host = createBrowserHost();
    await expect(host.readFile("missing")).rejects.toThrow();
    await expect(host.writeFile("missing", new Uint8Array())).rejects.toThrow();
    expect(await host.discoverDaemon()).toBeNull();
  });
  it("the suites reject broken implementations", async () => {
    class MissingHost extends FakeHost {
      override async readFile() {
        return new Uint8Array();
      }
    }
    const host = new MissingHost();
    await expect(
      hostContract({
        host,
        failNextWrite: () => host.failNextWrite(),
        emitTheme: () => host.setTheme({ appearance: "dark" }),
      }),
    ).rejects.toThrow("missing file");
    const engine = new FakeEngineClient();
    engine.apply = () => ({ status: "rejected", reason: "invalid" });
    expect(() =>
      engineContract(engine, (snapshot, operation) => engine.script(snapshot, operation)),
    ).toThrow("one commit");
    const daemon = new FakeDaemonClient();
    daemon.subscribe = () => ({ dispose() {} });
    await expect(daemonContract(daemon, daemon)).rejects.toThrow("events delivered");
    const sync = new NoopSyncClient();
    sync.send = async () => {};
    await expect(syncContract(sync)).rejects.toThrow("cannot claim to send");
    expect(() => telemetryContract({ enabled: true, event() {}, dispose() {} })).toThrow("opt-in");
    host.dispose();
    engine.dispose();
    daemon.dispose();
    sync.dispose();
  });
});
