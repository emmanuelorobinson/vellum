/** Native host adapter for client config syntax. Never return a person's full configuration to UI. */
import {
  applyEdits,
  findNodeAtLocation,
  modify,
  type Node,
  type ParseError,
  parseTree,
} from "jsonc-parser";
import { parse as parseToml } from "smol-toml";

export const clients = [
  { id: "claude-code", name: "Claude Code", format: "jsonc", keys: ["mcpServers"] },
  { id: "codex", name: "Codex", format: "toml", keys: ["mcp_servers"] },
  { id: "cursor", name: "Cursor", format: "jsonc", keys: ["mcpServers"] },
  { id: "windsurf", name: "Windsurf", format: "jsonc", keys: ["mcpServers"] },
  { id: "copilot", name: "VS Code Copilot", format: "jsonc", keys: ["servers"] },
  { id: "gemini", name: "Gemini CLI", format: "jsonc", keys: ["mcpServers"] },
  { id: "antigravity", name: "Antigravity", format: "jsonc", keys: ["mcpServers"] },
  { id: "opencode", name: "OpenCode", format: "jsonc", keys: ["mcp", "servers"] },
  { id: "kiro", name: "Kiro", format: "jsonc", keys: ["mcpServers"] },
  { id: "claude-desktop", name: "Claude Desktop", format: "jsonc", keys: ["mcpServers"] },
] as const;
export type McpClient = (typeof clients)[number];
export type ClientId = McpClient["id"];
export type ConfigAction = "connect" | "disconnect";
export function entry(client: McpClient, binary: string) {
  return client.id === "opencode"
    ? { type: "local", command: [binary, "mcp"], enabled: true }
    : {
        ...(client.id === "copilot" || client.id === "claude-code" ? { type: "stdio" } : {}),
        command: binary,
        args: ["mcp"],
      };
}
const plain = (v: unknown): v is Record<string, unknown> =>
  !!v && typeof v === "object" && !Array.isArray(v);
function jsonTree(text: string): Node {
  const errors: ParseError[] = [];
  const tree = parseTree(text, errors, { allowTrailingComma: true });
  if (!tree || errors.length || tree.type !== "object") throw Error("invalid configuration");
  const check = (node: Node) => {
    if (node.type === "object") {
      const keys = node.children?.map((n) => n.children?.[0]?.value);
      if (keys && new Set(keys).size !== keys.length) throw Error("duplicate key");
    }
    for (const child of node.children ?? []) check(child);
  };
  check(tree);
  return tree;
}
function value(node?: Node): unknown {
  if (!node) return undefined;
  if (node.type === "object")
    return Object.fromEntries(
      (node.children ?? []).map((n) => [n.children?.[0]?.value, value(n.children?.[1])]),
    );
  if (node.type === "array") return node.children?.map(value) ?? [];
  return node.value;
}
function keysFor(client: McpClient, tree: Node): readonly string[] {
  if (client.id === "opencode") {
    const mcp = findNodeAtLocation(tree, ["mcp"]);
    if (mcp && mcp.type !== "object") throw Error("invalid server map");
    // Preserve v1 files; new files follow the current mcp.servers schema.
    if (mcp?.children?.length && !findNodeAtLocation(tree, ["mcp", "servers"])) return ["mcp"];
  }
  return client.keys;
}
export function readEntry(client: McpClient, source: string): unknown {
  if (client.format === "toml") {
    const doc = parseToml(source);
    const map = doc.mcp_servers;
    if (map !== undefined && !plain(map)) throw Error("invalid server map");
    return map?.vellum;
  }
  const tree = jsonTree(source);
  const keys = keysFor(client, tree);
  const map = findNodeAtLocation(tree, [...keys]);
  if (map && map.type !== "object") throw Error("invalid server map");
  return value(findNodeAtLocation(tree, [...keys, "vellum"]));
}
const equal = (a: unknown, b: unknown): boolean => {
  if (plain(a) && plain(b))
    return (
      Object.keys(a).length === Object.keys(b).length &&
      Object.keys(a).every((k) => equal(a[k], b[k]))
    );
  if (Array.isArray(a) && Array.isArray(b))
    return a.length === b.length && a.every((v, i) => equal(v, b[i]));
  return a === b;
};
function tomlBlock(binary: string): string {
  return `\n# Vellum MCP begin\n[mcp_servers.vellum]\ncommand = ${JSON.stringify(binary)}\nargs = ["mcp"]\n# Vellum MCP end\n`;
}
export function snippet(client: McpClient, binary: string, source?: string): string {
  if (client.format === "toml") return tomlBlock(binary);
  let result: unknown = { vellum: entry(client, binary) };
  for (const key of [
    ...(source === undefined ? client.keys : keysFor(client, jsonTree(source))),
  ].reverse())
    result = { [key]: result };
  return `${JSON.stringify(result, null, 2)}\n`;
}
/** Only an exact entry pointing at this installed binary is owned. Modified entries fail closed. */
export function editConfig(
  client: McpClient,
  source: string,
  binary: string,
  action: ConfigAction,
): string {
  const old = readEntry(client, source);
  const expected = entry(client, binary);
  if (old !== undefined && !equal(old, expected)) throw Error("entry conflict");
  if ((action === "connect") === (old !== undefined)) return source;
  if (client.format === "toml") {
    const block = tomlBlock(binary);
    if (action === "connect") {
      const result = source + block;
      readEntry(client, result);
      return result;
    }
    // Never split arbitrary TOML by table-looking lines: they can be inside multiline strings.
    if (source.split(block).length !== 2) throw Error("entry not managed");
    const result = source.replace(block, "");
    if (readEntry(client, result) !== undefined) throw Error("entry conflict");
    return result;
  }
  return applyEdits(
    source,
    modify(
      source,
      [...keysFor(client, jsonTree(source)), "vellum"],
      action === "connect" ? expected : undefined,
      {
        formattingOptions: {
          insertSpaces: true,
          tabSize: 2,
          eol: source.includes("\r\n") ? "\r\n" : "\n",
        },
      },
    ),
  );
}
