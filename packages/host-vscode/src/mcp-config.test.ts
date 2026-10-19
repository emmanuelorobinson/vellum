// TEST DATA: client configs contain unrelated servers, settings and comments.
import { readFileSync } from "node:fs";
import { parse } from "jsonc-parser";
import { describe, expect, it } from "vitest";
import { clients, editConfig, readEntry, snippet } from "./mcp-config";

const binary = "/installed extension/bin/vellum";
describe("client MCP entries", () => {
  for (const client of clients) {
    it(`${client.id}: Connect and Disconnect are idempotent and preserve other entries`, () => {
      const source = readFileSync(
        new URL(`../fixtures/mcp/${client.id}.${client.format}`, import.meta.url),
        "utf8",
      );
      const connected = editConfig(client, source, binary, "connect");
      expect(connected).not.toBe(source);
      expect(readEntry(client, connected)).toEqual(readEntry(client, snippet(client, binary)));
      expect(editConfig(client, connected, binary, "connect")).toBe(connected);
      const removed = editConfig(client, connected, binary, "disconnect");
      expect(readEntry(client, removed)).toBeUndefined();
      expect(removed).toContain("other-server");
      expect(removed).toContain("TEST DATA");
      expect(editConfig(client, removed, binary, "disconnect")).toBe(removed);
      if (client.format === "toml") expect(removed).toBe(source);
      else expect(parse(removed)).toEqual(parse(source));
    });
    it(`${client.id}: refuses invalid, foreign and person-modified entries`, () => {
      expect(() => editConfig(client, "{broken", binary, "connect")).toThrow();
      const foreign = snippet(client, "/someone-else/server");
      expect(() => editConfig(client, foreign, binary, "connect")).toThrow();
      expect(() => editConfig(client, foreign, binary, "disconnect")).toThrow();
      const modified = snippet(client, binary).replace(/"mcp"(?=\s*[\],])/, '"something-else"');
      expect(() => editConfig(client, modified, binary, "disconnect")).toThrow();
    });
  }
  it("rejects array/null server maps and duplicate JSON keys", () => {
    const client = clients[0]!;
    for (const source of [
      '{"mcpServers":null}',
      '{"mcpServers":[]}',
      '{"mcpServers":{},"mcpServers":{}}',
    ])
      expect(() => editConfig(client, source, binary, "connect")).toThrow();
  });
  it("preserves TOML multiline text that looks like table headers", () => {
    const client = clients.find((c) => c.id === "codex")!;
    const source = '# TEST DATA\nprompt = """\n[mcp_servers.vellum]\nnot a table\n"""\n';
    const connected = editConfig(client, source, binary, "connect");
    expect(editConfig(client, connected, binary, "disconnect")).toBe(source);
  });
});
