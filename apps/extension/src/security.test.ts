import { describe, expect, it } from "vitest";
import { RpcFrames } from "./relay";
import { appearance, parseMessage, webviewHtml } from "./security";

describe("webview boundary", () => {
  it("maps all four VS Code theme kinds without replacing canvas semantic tokens", () => {
    expect([1, 2, 3, 4].map(appearance)).toEqual(["light", "dark", "hc-dark", "hc-light"]);
  });
  it("uses local scripts, a nonce and no network, frames, forms or command URIs", () => {
    const html = webviewHtml({
      source: "vscode-webview://test",
      script: "local.js",
      style: "local.css",
      nonce: "abc",
      title: '<bad>"',
    });
    expect(html).toContain("default-src 'none'");
    expect(html).toContain("connect-src 'none'");
    expect(html).toContain("script-src 'nonce-abc'");
    expect(html).not.toContain("unsafe-eval");
    expect(html).toContain("&lt;bad&gt;&quot;");
    expect(html).not.toContain("<bad>");
  });
  it("accepts only bounded typed messages; webviews cannot send raw RPC or paths", () => {
    expect(parseMessage({ type: "ready" })).toEqual({ type: "ready" });
    for (const value of [
      null,
      [],
      { type: "rpc", method: "tools/call" },
      { type: "readFile", path: "/secret" },
      { type: "ready", token: "secret" },
      { type: "command", command: "workbench.action.terminal.new" },
    ])
      expect(parseMessage(value)).toBeNull();
    expect(parseMessage({ type: "command", command: "pauseAll" })).toEqual({
      type: "command",
      command: "pauseAll",
    });
  });
});
describe("daemon relay framing", () => {
  it("decodes split UTF-8 and multiple newline frames", () => {
    const decoder = new RpcFrames();
    const b = Buffer.from('{"id":1,"result":"é"}\n{"id":2,"result":{}}\n');
    const split = b.indexOf(0xc3) + 1;
    expect(decoder.push(b.subarray(0, split))).toEqual([]);
    expect(decoder.push(b.subarray(split))).toEqual([
      { id: 1, result: "é" },
      { id: 2, result: {} },
    ]);
  });
  it("refuses oversized, invalid and truncated replies", () => {
    expect(() => new RpcFrames(8).push(Buffer.from("123456789"))).toThrow();
    expect(() => new RpcFrames().push(Buffer.from("bad\n"))).toThrow();
    const d = new RpcFrames();
    d.push(Buffer.from('{"id":'));
    expect(() => d.end()).toThrow();
  });
});

it("allows only explicit telemetry consent booleans", () => {
  expect(parseMessage({ type: "telemetry", enabled: true })).toEqual({
    type: "telemetry",
    enabled: true,
  });
  expect(parseMessage({ type: "telemetry", enabled: "true" })).toBeNull();
  expect(parseMessage({ type: "telemetry", enabled: true, prompt: "TEST DATA" })).toBeNull();
});
