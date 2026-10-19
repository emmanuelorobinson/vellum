// TEST DATA: all native filesystem and client detection boundaries are injected.
import { expect, it, vi } from "vitest";
import { AgentConnections } from "./connect";

function setup() {
  const files = new Map<string, string>();
  const write = vi.fn(async (path: string, value: string, expected: string | null) => {
    expect(files.get(path) ?? null).toBe(expected);
    files.set(path, value);
  });
  const service = new AgentConnections({
    home: "/test-home",
    platform: "darwin",
    configHome: "/test-home/.config",
    appData: "/test-home/Library/Application Support",
    binary: "/bundle/bin/vellum",
    read: async (path) => files.get(path) ?? null,
    exists: async (path) => files.has(path),
    executable: async (name) => name === "claude",
    write,
  });
  return { service, files, write };
}
it("detects all ten clients without executing any installed client", async () => {
  const { service } = setup();
  const state = await service.list([]);
  expect(state).toHaveLength(10);
  expect(state[0]).toMatchObject({ id: "claude-code", state: "ready" });
  expect(state.find((c) => c.id === "kiro")).toMatchObject({ state: "missing" });
});
it("reports configured until a real matching client initializes; never invents a tool count", async () => {
  const { service, write } = setup();
  const result = await service.change("claude-code", "connect");
  expect(result.path).toBe("/test-home/.claude.json");
  expect(result.entry).toContain("/bundle/bin/vellum");
  expect((await service.list([]))[0]?.state).toBe("configured");
  expect((await service.list(["claude-code"]))[0]?.state).toBe("connected");
  await service.change("claude-code", "connect");
  expect(write).toHaveBeenCalledTimes(1);
  await service.change("claude-code", "disconnect");
  await service.change("claude-code", "disconnect");
  expect(write).toHaveBeenCalledTimes(2);
});
it("keeps malformed/foreign configurations intact and returns no file content in errors", async () => {
  const { service, files, write } = setup();
  files.set("/test-home/.claude.json", "TEST DATA private invalid content");
  expect((await service.list([]))[0]?.state).toBe("error");
  await expect(service.change("claude-code", "connect")).rejects.toThrow(
    "Configuration could not be changed",
  );
  expect(write).not.toHaveBeenCalled();
});
it("shows the exact existing OpenCode schema that was changed", async () => {
  const { service, files } = setup();
  files.set(
    "/test-home/.config/opencode/opencode.json",
    '{"mcp":{"other-server":{"command":["other"]}}}',
  );
  const result = await service.change("opencode", "connect");
  const actual = JSON.parse(files.get(result.path) ?? "{}");
  expect(JSON.parse(result.entry).mcp.vellum).toEqual(actual.mcp.vellum);
  expect(JSON.parse(result.entry).mcp.servers).toBeUndefined();
});
