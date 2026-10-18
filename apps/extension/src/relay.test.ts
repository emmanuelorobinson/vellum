// TEST DATA: child-process boundary fake. Authentication stays inside the Rust stdio bridge.
import { EventEmitter } from "node:events";
import { PassThrough, Writable } from "node:stream";
import { afterEach, describe, expect, it, vi } from "vitest";

const mock = vi.hoisted(() => ({ spawn: vi.fn() }));
vi.mock("node:child_process", () => ({ spawn: mock.spawn }));

import { DaemonProcess } from "./relay";

function setup() {
  const requests: { id: number; method: string }[] = [];
  const child = Object.assign(new EventEmitter(), {
    stdin: new Writable({
      write(bytes, _encoding, callback) {
        requests.push(JSON.parse(bytes.toString()));
        callback();
      },
    }),
    stdout: new PassThrough(),
    stderr: new PassThrough(),
    kill: vi.fn(),
  });
  mock.spawn.mockReturnValueOnce(child);
  const relay = new DaemonProcess("/bundle/vellum", "/repo");
  const reply = (id: number, result: unknown) =>
    child.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result })}\n`);
  return { child, relay, reply, requests };
}
afterEach(() => {
  mock.spawn.mockReset();
  vi.useRealTimers();
});
describe("native daemon process", () => {
  it("starts the bundled bridge once, initializes and relays only the typed action", async () => {
    const s = setup();
    const a = s.relay.connect();
    const b = s.relay.connect();
    expect(a).toBe(b);
    expect(s.requests[0]?.method).toBe("initialize");
    s.reply(1, { protocolVersion: "2025-11-25" });
    await a;
    expect(mock.spawn).toHaveBeenCalledWith(
      "/bundle/vellum",
      ["mcp"],
      expect.objectContaining({ cwd: "/repo", shell: false }),
    );
    const pause = s.relay.pauseAll();
    await Promise.resolve();
    expect(s.requests[1]?.method).toBe("vellum/pauseAll");
    s.reply(2, { paused: true });
    await pause;
    s.relay.dispose();
    expect(s.child.kill).toHaveBeenCalledOnce();
    expect(s.relay.connected).toBe(false);
  });
  it("rejects pending work after a crash and does not replay it", async () => {
    const s = setup();
    const connect = s.relay.connect();
    s.reply(1, { protocolVersion: "2025-11-25" });
    await connect;
    const pause = s.relay.pauseAll();
    await Promise.resolve();
    s.child.emit("exit", 1);
    await expect(pause).rejects.toThrow("offline");
    expect(s.requests).toHaveLength(2);
    expect(s.relay.connected).toBe(false);
    s.relay.dispose();
  });
  it("refuses malformed replies without leaking their contents", async () => {
    const s = setup();
    const connect = s.relay.connect();
    s.child.stdout.write("not-json\n");
    await expect(connect).rejects.toThrow("offline");
    s.relay.dispose();
  });
  it("handles truncated output as an offline result without an uncaught exception", async () => {
    const s = setup();
    const connect = s.relay.connect();
    s.child.stdout.write('{"id":');
    s.child.stdout.emit("end");
    await expect(connect).rejects.toThrow("offline");
    s.relay.dispose();
  });
  it("bounds unanswered handshakes and refuses future work after disposal", async () => {
    vi.useFakeTimers();
    const s = setup();
    const connect = s.relay.connect();
    const rejected = expect(connect).rejects.toThrow("offline");
    await vi.advanceTimersByTimeAsync(15_000);
    await rejected;
    s.relay.dispose();
    await expect(s.relay.connect()).rejects.toThrow("disposed");
  });
});
