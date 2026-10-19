import { join } from "node:path";
import {
  type ClientId,
  type ConfigAction,
  clients,
  editConfig,
  type McpClient,
  readEntry,
  snippet,
} from "./mcp-config";

export interface ConfigPort {
  home: string;
  platform: string;
  configHome: string;
  appData: string;
  codexHome?: string;
  claudeConfig?: string;
  workspace?: string;
  binary: string;
  read(path: string): Promise<string | null>;
  exists(path: string): Promise<boolean>;
  executable(name: string): Promise<boolean>;
  /** Compare with expected contents before atomic replacement; never clobber a concurrent edit. */
  write(path: string, text: string, expected: string | null): Promise<void>;
}
export interface ClientView {
  id: ClientId;
  name: string;
  state: "connected" | "configured" | "ready" | "missing" | "error";
  path: string;
  entry: string;
}
const executable: Record<ClientId, string> = {
  "claude-code": "claude",
  codex: "codex",
  cursor: "cursor",
  windsurf: "windsurf",
  copilot: "code",
  gemini: "gemini",
  antigravity: "agy",
  opencode: "opencode",
  kiro: "kiro",
  "claude-desktop": "claude-desktop",
};
const apps: Partial<Record<ClientId, string>> = {
  cursor: "Cursor",
  windsurf: "Windsurf",
  antigravity: "Antigravity",
  kiro: "Kiro",
  "claude-desktop": "Claude",
  codex: "Codex",
};
const aliases: Partial<Record<ClientId, readonly string[]>> = {
  codex: ["codex-mcp-client", "codex_cli_rs", "OpenAI Codex"],
  "claude-code": ["claude-code-ide", "claude-cli"],
  "claude-desktop": ["Claude AI"],
  cursor: ["cursor-vscode"],
  copilot: ["Visual Studio Code", "vscode", "github.copilot-chat", "GitHub Copilot"],
  windsurf: ["Windsurf Cascade", "Devin"],
};
export class AgentConnections {
  private queue: Promise<unknown> = Promise.resolve();
  constructor(private readonly port: ConfigPort) {}
  private async path(client: McpClient): Promise<string> {
    const p = this.port;
    switch (client.id) {
      case "claude-code":
        return p.claudeConfig ?? join(p.home, ".claude.json");
      case "codex":
        return join(p.codexHome ?? join(p.home, ".codex"), "config.toml");
      case "cursor":
        return join(p.home, ".cursor/mcp.json");
      case "windsurf": {
        const legacy = join(p.home, ".codeium/windsurf/mcp_config.json");
        const current = join(
          p.platform === "win32" ? p.appData : p.configHome,
          "devin/mcp_config.json",
        );
        return (await p.exists(current)) ? current : (await p.exists(legacy)) ? legacy : current;
      }
      case "copilot":
        return join(
          p.workspace ?? p.appData,
          p.workspace ? ".vscode/mcp.json" : "Code/User/mcp.json",
        );
      case "gemini":
        return join(p.home, ".gemini/settings.json");
      case "antigravity": {
        const current = join(p.home, ".gemini/config/mcp_config.json");
        const legacy = join(p.home, ".gemini/antigravity/mcp_config.json");
        return (await p.exists(current)) ? current : (await p.exists(legacy)) ? legacy : current;
      }
      case "opencode": {
        const jsonc = join(p.configHome, "opencode/opencode.jsonc");
        return (await p.exists(jsonc)) ? jsonc : join(p.configHome, "opencode/opencode.json");
      }
      case "kiro":
        return join(p.home, ".kiro/settings/mcp.json");
      case "claude-desktop":
        return join(p.appData, "Claude/claude_desktop_config.json");
    }
  }
  async list(connectedNames: readonly string[]): Promise<ClientView[]> {
    return Promise.all(
      clients.map(async (client) => {
        const path = await this.path(client);
        let entry = snippet(client, this.port.binary);
        try {
          const source = await this.port.read(path);
          if (source !== null)
            entry = snippet(
              client,
              this.port.binary,
              client.format === "toml" ? undefined : source,
            );
          const installed =
            source !== null ||
            (await this.port.executable(executable[client.id])) ||
            (this.port.platform === "darwin" &&
              !!apps[client.id] &&
              ((await this.port.exists(`/Applications/${apps[client.id]}.app`)) ||
                (await this.port.exists(
                  join(this.port.home, "Applications", `${apps[client.id]}.app`),
                ))));
          const configured = source !== null && readEntry(client, source) !== undefined;
          if (configured && source !== null)
            editConfig(client, source, this.port.binary, "connect");
          const normalize = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");
          const known = [client.name, client.id, ...(aliases[client.id] ?? [])].map(normalize);
          const live = connectedNames.some((n) => known.includes(normalize(n)));
          return {
            id: client.id,
            name: client.name,
            path,
            entry,
            state: live ? "connected" : configured ? "configured" : installed ? "ready" : "missing",
          };
        } catch {
          return { id: client.id, name: client.name, path, entry, state: "error" };
        }
      }),
    );
  }
  change(
    id: ClientId,
    action: ConfigAction,
  ): Promise<{ path: string; entry: string; action: ConfigAction }> {
    const run = this.queue.then(async () => {
      const client = clients.find((c) => c.id === id);
      if (!client) throw Error("Unknown client");
      try {
        const path = await this.path(client);
        const original = await this.port.read(path);
        const source = original ?? (client.format === "toml" ? "" : "{}\n");
        const updated = editConfig(client, source, this.port.binary, action);
        if (updated !== source) await this.port.write(path, updated, original);
        return {
          path,
          entry: snippet(client, this.port.binary, client.format === "toml" ? undefined : source),
          action,
        };
      } catch {
        throw Error(
          "Configuration could not be changed. Check its syntax, permissions and existing Vellum entry.",
        );
      }
    });
    this.queue = run.catch(() => {});
    return run;
  }
}
