import { randomBytes } from "node:crypto";
import { chmod, mkdir, readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { brand } from "@vellum/config";
import { PortError } from "@vellum/host-api";
import { createVscodeHost } from "@vellum/host-vscode";
import * as vscode from "vscode";
import { accountCommands } from "./account";
import { executable, exists, readConfig, writeConfig } from "./config-files";
import { AgentConnections } from "./connect";
import { ConnectView } from "./connect-view";
import { files } from "./files";
import { DesignEditorProvider, VIEW_TYPE } from "./provider";
import { DaemonProcess } from "./relay";
import { appearance } from "./security";
import { DesignSidebar } from "./sidebar";

/** All native capabilities stay in the extension host. Webviews receive only typed, scoped messages. */
export function activate(context: vscode.ExtensionContext): {
  readonly status: { editors: number; ready: number; connectionsReady: boolean };
} {
  const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? context.extensionPath;
  const daemon = new DaemonProcess(
    join(context.extensionPath, "bin", process.platform === "win32" ? "vellum.exe" : "vellum"),
    cwd,
    process.env.VELLUM_RUN_DIR,
  );
  const host = createVscodeHost({
    readFile: files.read,
    writeFile: files.writeAtomic,
    watch(uri, listener) {
      const resource = vscode.Uri.parse(uri);
      const watcher = vscode.workspace.createFileSystemWatcher(
        new vscode.RelativePattern(
          vscode.Uri.joinPath(resource, ".."),
          resource.path.split("/").at(-1) ?? "*.vellum",
        ),
      );
      const subscriptions = [
        watcher,
        watcher.onDidChange(listener),
        watcher.onDidCreate(listener),
        watcher.onDidDelete(listener),
      ];
      return {
        dispose: () => {
          for (const subscription of subscriptions) subscription.dispose();
        },
      };
    },
    async discoverDaemon() {
      try {
        await daemon.connect();
        return { kind: "relayed", protocol: 1 };
      } catch {
        return null;
      }
    },
    openExternal: async (url) => {
      if (!(await vscode.env.openExternal(vscode.Uri.parse(url)))) throw new PortError("failed");
    },
    clipboard: {
      read: async () => ({
        items: { "text/plain": new TextEncoder().encode(await vscode.env.clipboard.readText()) },
      }),
      async write(data) {
        const entries = Object.entries(data.items);
        if (entries.length !== 1 || entries[0]?.[0] !== "text/plain")
          throw new PortError("unsupported");
        await vscode.env.clipboard.writeText(new TextDecoder().decode(entries[0][1]));
      },
    },
    theme: () => ({ appearance: appearance(vscode.window.activeColorTheme.kind) }),
    onThemeChange: (listener) =>
      vscode.window.onDidChangeActiveColorTheme((theme) =>
        listener({ appearance: appearance(theme.kind) }),
      ),
  });
  const provider = new DesignEditorProvider(
    context,
    daemon,
    {
      ...files,
      read: (uri) => host.readFile(uri),
      writeAtomic: (uri, bytes) => host.writeFile(uri, bytes),
    },
    undefined,
    deviceId(context),
  );
  const stableBinary = join(
    context.globalStorageUri.fsPath,
    "bin",
    process.platform === "win32" ? "vellum.exe" : "vellum",
  );
  const home = process.env.VELLUM_CLIENT_HOME ?? homedir();
  const connections = new AgentConnections({
    home,
    platform: process.platform,
    configHome: process.env.XDG_CONFIG_HOME ?? join(home, ".config"),
    appData:
      process.env.APPDATA ??
      (process.platform === "darwin"
        ? join(home, "Library/Application Support")
        : join(home, ".config")),
    codexHome: process.env.CODEX_HOME,
    claudeConfig: process.env.CLAUDE_CONFIG_DIR
      ? join(process.env.CLAUDE_CONFIG_DIR, ".claude.json")
      : undefined,
    workspace: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
    binary: stableBinary,
    read: readConfig,
    write: writeConfig,
    exists,
    executable: async (name) =>
      name === "code"
        ? !!vscode.extensions.getExtension("github.copilot-chat") ||
          (await executable("code")) ||
          (await executable("code-insiders"))
        : executable(name),
  });
  const connectView = new ConnectView(context, connections, daemon);
  const sidebar = new DesignSidebar(context, daemon, provider, cwd);
  const command = (id: string, action: () => Promise<unknown>) =>
    vscode.commands.registerCommand(id, async () => {
      try {
        return await action();
      } catch {
        void vscode.window.showErrorMessage(
          `${brand.name}: the host request failed. Open the file as text to inspect it.`,
        );
        return false;
      }
    });
  context.subscriptions.push(
    host,
    daemon,
    provider,
    connectView,
    sidebar,
    vscode.window.registerCustomEditorProvider(VIEW_TYPE, provider, {
      supportsMultipleEditorsPerDocument: true,
    }),
    command("vellum.newDesign", async () => {
      const uri = await vscode.window.showSaveDialog({
        filters: { [brand.name]: ["vellum"] },
        saveLabel: "Create design file",
      });
      if (!uri) return;
      if (uri.scheme !== "file" || !uri.path.endsWith(".vellum"))
        throw new PortError("unsupported");
      // Core creates and encodes new files; an existing file is opened, never replaced.
      await daemon.createDocument(uri.fsPath);
      return vscode.commands.executeCommand("vscode.openWith", uri, VIEW_TYPE);
    }),
    command("vellum.toggleView", async () => {
      const activeText = vscode.window.activeTextEditor;
      const uri = activeText?.document.uri ?? provider.activeUri;
      if (!uri?.path.endsWith(".vellum")) return;
      return vscode.commands.executeCommand(
        "vscode.openWith",
        uri,
        activeText ? VIEW_TYPE : "default",
      );
    }),
    command("vellum.openWelcome", async () => {
      await vscode.workspace.fs.createDirectory(context.globalStorageUri);
      const uri = vscode.Uri.joinPath(context.globalStorageUri, "welcome.vellum");
      try {
        await vscode.workspace.fs.stat(uri);
      } catch (error) {
        if (!(error instanceof vscode.FileSystemError) || error.code !== "FileNotFound")
          throw error;
        await vscode.workspace.fs.copy(
          vscode.Uri.joinPath(context.extensionUri, "dist", "welcome.vellum"),
          uri,
          { overwrite: false },
        );
      }
      return vscode.commands.executeCommand("vscode.openWith", uri, VIEW_TYPE);
    }),
    command("vellum.pauseAllAgents", async () => {
      await daemon.pauseAll();
      return true;
    }),
    command("vellum.connectAgents", async () => {
      // A stable path survives extension version-directory changes. No config file is changed here.
      const binary = join(
        context.extensionPath,
        "bin",
        process.platform === "win32" ? "vellum.exe" : "vellum",
      );
      const bytes = await readFile(binary);
      await mkdir(join(context.globalStorageUri.fsPath, "bin"), { recursive: true, mode: 0o700 });
      await files.writeAtomic(vscode.Uri.file(stableBinary).toString(), bytes);
      if (process.platform !== "win32") await chmod(stableBinary, 0o700);
      connectView.open();
      return true;
    }),
    command("vellum.refreshSidebar", async () => sidebar.refresh()),
    ...accountCommands(
      join(context.extensionPath, "bin", process.platform === "win32" ? "vellum.exe" : "vellum"),
    ),
  );
  return {
    get status() {
      return { ...provider.status, connectionsReady: connectView.ready };
    },
  };
}
export function deactivate(): void {}

/** This installation's device: a ULID kept in global state. A person is known by the device they edit from. */
function deviceId(context: vscode.ExtensionContext): string {
  const saved = context.globalState.get<string>("vellum.device");
  if (saved && /^[0-7][0-9A-HJKMNP-TV-Z]{25}$/.test(saved)) return saved;
  const alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
  let time = Date.now();
  let head = "";
  for (let i = 0; i < 10; i++) {
    head = alphabet[time % 32] + head;
    time = Math.floor(time / 32);
  }
  const tail = [...randomBytes(16)]
    .slice(0, 16)
    .map((b) => alphabet[b % 32])
    .join("");
  const id = head + tail;
  void context.globalState.update("vellum.device", id);
  return id;
}
