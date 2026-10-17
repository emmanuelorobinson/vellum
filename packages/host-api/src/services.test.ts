import { describe, expect, it } from "vitest";
import { FakeDaemonClient, RelayedDaemonClient } from "./index";

describe("host relay lifecycle", () => {
  it("coalesces concurrent connection requests and ignores completion after disposal", async () => {
    let complete!: () => void;
    let connections = 0;
    const relay = new FakeDaemonClient();
    const client = new RelayedDaemonClient({
      connect: () => {
        connections++;
        return new Promise<void>((resolve) => {
          complete = resolve;
        });
      },
      request: (action) => relay.request(action),
      subscribe: (listener) => relay.subscribe(listener),
      disconnect: () => relay.disconnect(),
    });
    const first = client.connect(),
      second = client.connect();
    expect(connections).toBe(1);
    expect(first).toBe(second);
    client.dispose();
    complete();
    await expect(first).rejects.toThrow("offline");
    expect(client.state).toBe("disposed");
  });
  it("clears stale data when a relay request fails and supports reconnect", async () => {
    const relay = new FakeDaemonClient();
    const client = new RelayedDaemonClient(relay);
    await client.connect();
    relay.disconnect();
    await expect(client.request({ type: "pauseAll" })).rejects.toThrow("offline");
    expect(client.state).toBe("offline");
    await client.connect();
    expect(client.state).toBe("ready");
    client.dispose();
  });
});
