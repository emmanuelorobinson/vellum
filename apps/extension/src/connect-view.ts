import { randomBytes } from "node:crypto";
import { copy } from "@vellum/config";
import { type ClientId, clients } from "@vellum/host-vscode/mcp-config";
import * as vscode from "vscode";
import type { AgentConnections } from "./connect";
import type { DaemonProcess } from "./relay";
import { appearance, webviewHtml } from "./security";

export class ConnectView {
  ready = false;
  private panel?: vscode.WebviewPanel;
  private pending = false;
  private refreshId = 0;
  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly connections: AgentConnections,
    private readonly daemon: DaemonProcess,
  ) {}
  open() {
    if (this.panel) {
      this.panel.reveal();
      return;
    }
    const media = vscode.Uri.joinPath(this.context.extensionUri, "dist/webview");
    const panel = vscode.window.createWebviewPanel(
      "vellum.connect",
      "Connect agents",
      vscode.ViewColumn.Active,
      { enableScripts: true, localResourceRoots: [media], enableCommandUris: false },
    );
    this.panel = panel;
    const message = panel.webview.onDidReceiveMessage(async (value) => {
      if (!value || typeof value !== "object" || Array.isArray(value)) return;
      const keys = Object.keys(value).sort().join(",");
      if (keys === "type" && value.type === "ready") {
        await this.refresh();
        return;
      }
      if (keys === "type" && value.type === "rendered") {
        this.ready = true;
        return;
      }
      if (keys === "type" && value.type === "close") {
        panel.dispose();
        return;
      }
      if (keys === "type" && value.type === "refresh") {
        await this.refresh();
        return;
      }
      const client = clients.find((c) => c.id === value.id);
      if (!client || this.pending) return;
      if (keys === "id,type" && value.type === "copy") {
        const row = (await this.connections.list([])).find((c) => c.id === client.id);
        if (row) await vscode.env.clipboard.writeText(row.entry);
        await this.refresh(undefined, "Config copied");
      } else if (
        keys === "action,id,type" &&
        value.type === "change" &&
        ["connect", "disconnect"].includes(value.action)
      ) {
        this.pending = true;
        await this.refresh();
        try {
          const result = await this.connections.change(client.id as ClientId, value.action);
          this.pending = false;
          await this.refresh(
            result,
            result.action === "connect"
              ? `${client.name} configured · restart the client to connect`
              : `${client.name} entry removed · active sessions can continue until the client disconnects`,
          );
        } catch {
          this.pending = false;
          await this.refresh(
            undefined,
            undefined,
            "Configuration could not be changed. Check its syntax, permissions and existing Vellum entry.",
          );
        }
      }
    });
    const theme = vscode.window.onDidChangeActiveColorTheme(() => void this.refresh());
    const timer = setInterval(() => void this.refresh(), 10_000);
    panel.onDidDispose(() => {
      message.dispose();
      theme.dispose();
      clearInterval(timer);
      this.panel = undefined;
      this.ready = false;
    });
    panel.webview.html = webviewHtml({
      source: panel.webview.cspSource,
      script: panel.webview.asWebviewUri(vscode.Uri.joinPath(media, "connect.js")).toString(),
      style: panel.webview.asWebviewUri(vscode.Uri.joinPath(media, "connect.css")).toString(),
      nonce: randomBytes(24).toString("base64"),
      title: "Connect agents",
    });
  }
  private result?: { path: string; entry: string; action: "connect" | "disconnect" };
  private status?: string;
  private error?: string;
  private async refresh(result?: typeof this.result, status?: string, error?: string) {
    const panel = this.panel;
    if (!panel) return;
    const refreshId = ++this.refreshId;
    if (result) this.result = result;
    if (status || error) {
      this.status = status;
      this.error = error;
    }
    let connected: string[] = [];
    let toolCount: number | null = null;
    try {
      const state = await this.daemon.hostState([]);
      connected = state.clients;
      toolCount = state.toolCount;
    } catch {
      /* Offline is shown explicitly. */
    }
    const rows = await this.connections.list(connected);
    if (this.panel !== panel || refreshId !== this.refreshId) return;
    const live = rows.filter((c) => c.state === "connected").map((c) => c.name);
    await panel.webview.postMessage({
      type: "connections",
      clients: rows,
      pending: this.pending,
      result: this.result,
      status: this.status,
      error: this.error,
      appearance: appearance(vscode.window.activeColorTheme.kind),
      connection:
        toolCount === null
          ? `⊘ ${copy.daemonOffline}`
          : live.length
            ? `✓ ${live.join(", ")} connected · ${toolCount} tools available`
            : `○ ${toolCount} tools available · waiting for a client`,
    });
  }
  dispose() {
    this.panel?.dispose();
  }
}
