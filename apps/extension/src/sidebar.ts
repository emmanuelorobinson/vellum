import { basename, relative } from "node:path";
import { copy } from "@vellum/config";
import * as vscode from "vscode";
import type { DesignEditorProvider } from "./provider";
import type { DaemonProcess } from "./relay";
import { RecentDesigns, type SessionRow, sessionRows } from "./sidebar-model";

type Item = { kind: "file"; uri: string } | { kind: "session"; row: SessionRow };
export class DesignSidebar implements vscode.TreeDataProvider<Item> {
  private emitter = new vscode.EventEmitter<Item | undefined>();
  readonly onDidChangeTreeData = this.emitter.event;
  private recent: RecentDesigns;
  private sessions: SessionRow[] = [];
  private message = "Loading sessions";
  private pending = false;
  private disposed = false;
  private subscriptions: vscode.Disposable[] = [];
  constructor(
    context: vscode.ExtensionContext,
    private readonly daemon: DaemonProcess,
    provider: DesignEditorProvider,
    private readonly cwd: string,
  ) {
    this.recent = new RecentDesigns(context.globalState.get<unknown[]>("recentDesigns", []));
    const files: vscode.TreeDataProvider<Item> = {
      getChildren: () => this.recent.items.map((uri) => ({ kind: "file", uri })),
      getTreeItem: (item) => this.getTreeItem(item),
      onDidChangeTreeData: this.emitter.event,
    };
    const recentView = vscode.window.createTreeView("vellum.recent", { treeDataProvider: files });
    const sessionView = vscode.window.createTreeView("vellum.sessions", { treeDataProvider: this });
    this.subscriptions.push(
      recentView,
      sessionView,
      provider.onDidOpenDocument((uri) => {
        this.recent.open(uri.toString());
        void context.globalState.update("recentDesigns", this.recent.items);
        this.emitter.fire(undefined);
        void this.refresh();
      }),
    );
    const timer = setInterval(() => void this.refresh(), 15_000);
    this.subscriptions.push({ dispose: () => clearInterval(timer) });
    void this.refresh();
    this.subscriptions.push(
      this.onDidChangeTreeData(() => {
        sessionView.message = this.message;
      }),
    );
    sessionView.message = this.message;
  }
  getChildren(): Item[] {
    return this.sessions.map((row) => ({ kind: "session", row }));
  }
  getTreeItem(item: Item): vscode.TreeItem {
    if (item.kind === "file") {
      const uri = vscode.Uri.parse(item.uri);
      const row = new vscode.TreeItem(uri);
      row.description = vscode.workspace.asRelativePath(uri);
      row.command = {
        command: "vscode.openWith",
        title: "Open design",
        arguments: [uri, "vellum.design"],
      };
      return row;
    }
    const row = new vscode.TreeItem(item.row.title);
    row.id = item.row.id;
    row.description = `${item.row.client} · ${basename(item.row.path)}`;
    row.iconPath = new vscode.ThemeIcon("symbol-method");
    row.tooltip = `${item.row.client}\n${item.row.path}\nLast activity: ${new Date(item.row.updatedAt).toISOString()}`;
    row.command = {
      command: "vscode.openWith",
      title: "Open session document",
      arguments: [vscode.Uri.file(item.row.path), "vellum.design"],
    };
    return row;
  }
  async refresh() {
    if (this.pending || this.disposed) return;
    this.pending = true;
    try {
      const paths = this.recent.items
        .map((uri) => vscode.Uri.parse(uri).fsPath)
        .filter((path) => {
          const inside = relative(this.cwd, path);
          return !!inside && !inside.startsWith("..") && !inside.startsWith("/");
        });
      const state = await this.daemon.hostState(paths);
      this.sessions = sessionRows(state.sessions);
      this.message = state.sessionsAvailable
        ? this.sessions.length
          ? ""
          : "No sessions for recent designs in this workspace"
        : "Sessions unavailable";
    } catch {
      this.sessions = [];
      this.message = `${copy.daemonOffline} · refresh to retry`;
    } finally {
      this.pending = false;
      if (!this.disposed) this.emitter.fire(undefined);
    }
  }
  dispose() {
    this.disposed = true;
    for (const s of this.subscriptions) s.dispose();
    this.emitter.dispose();
  }
}
