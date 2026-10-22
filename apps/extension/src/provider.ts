import { randomBytes } from "node:crypto";
import { brand } from "@vellum/config";
import { editorReporter } from "@vellum/telemetry";
import * as vscode from "vscode";
import { DesignDocument, type DocumentRuntime, type FilePort } from "./document";
import type { DaemonProcess } from "./relay";
import { appearance, parseMessage, webviewHtml } from "./security";

export const VIEW_TYPE = "vellum.design";
export interface RuntimeFactory {
  open(uri: vscode.Uri, bytes: Uint8Array): Promise<DocumentRuntime>;
}
/** Until the compiled core exports the document ABI and editor ownership can be registered over IPC,
 * the host can preserve and inspect original bytes only. It must never grant itself write authority. */
export class ReadonlyRuntime implements DocumentRuntime {
  readonly editable = false;
  revision = 0;
  constructor(private readonly bytes: Uint8Array) {}
  async snapshot() {
    return { bytes: this.bytes.slice(), revision: this.revision };
  }
  async replace(): Promise<{ revision: number }> {
    throw Error("document runtime unavailable");
  }
  subscribe() {
    return { dispose() {} };
  }
  dispose() {}
}
// A wrapper avoids conflating the native URI with the lifecycle's transport-independent resource key.
export class CustomDocument implements vscode.CustomDocument {
  constructor(
    readonly uri: vscode.Uri,
    readonly lifecycle: DesignDocument,
  ) {}
  dispose() {
    this.lifecycle.dispose();
  }
}
export class DesignEditorProvider
  implements vscode.CustomEditorProvider<CustomDocument>, vscode.Disposable
{
  private changes = new vscode.EventEmitter<
    vscode.CustomDocumentContentChangeEvent<CustomDocument>
  >();
  readonly onDidChangeCustomDocument = this.changes.event;
  private documents = new Set<CustomDocument>();
  private panels = new Map<vscode.WebviewPanel, CustomDocument>();
  private resources = new Map<CustomDocument, vscode.Disposable[]>();
  private active: CustomDocument | undefined;
  private readyPanels = new Set<vscode.WebviewPanel>();
  private opened = new vscode.EventEmitter<vscode.Uri>();
  readonly onDidOpenDocument = this.opened.event;
  get status() {
    return { editors: this.panels.size, ready: this.readyPanels.size };
  }
  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly daemon: DaemonProcess,
    private readonly files: FilePort,
    private readonly factory: RuntimeFactory = {
      open: async (_uri, bytes) => new ReadonlyRuntime(bytes),
    },
  ) {}

  async openCustomDocument(
    uri: vscode.Uri,
    open: vscode.CustomDocumentOpenContext,
    token: vscode.CancellationToken,
  ): Promise<CustomDocument> {
    if (token.isCancellationRequested) throw new vscode.CancellationError();
    const bytes = open.untitledDocumentData ?? (await this.files.read(uri.toString()));
    const runtime = await this.factory.open(uri, bytes);
    const lifecycle = new DesignDocument(uri.toString(), runtime, this.files, bytes);
    try {
      if (open.backupId)
        await lifecycle.restore(open.backupId, () => token.isCancellationRequested);
      if (token.isCancellationRequested) throw new vscode.CancellationError();
    } catch (error) {
      lifecycle.dispose();
      throw error;
    }
    const document = new CustomDocument(uri, lifecycle);
    this.documents.add(document);
    this.opened.fire(uri);
    const subscription = lifecycle.subscribe(() => {
      if (lifecycle.dirty) this.changes.fire({ document });
      void this.broadcast(document);
    });
    const disposables: vscode.Disposable[] = [subscription];
    const watcher = vscode.workspace.createFileSystemWatcher(
      new vscode.RelativePattern(
        vscode.Uri.joinPath(uri, ".."),
        uri.path.split("/").at(-1) ?? "*.vellum",
      ),
    );
    const update = () => {
      if (!runtime.editable) {
        // No TypeScript diff or silent reload: external changes need core's merge and Actor::File transaction.
        void this.broadcast(document, "external-runtime-unavailable");
        return;
      }
      void lifecycle.external().catch(() => this.broadcast(document, "external-unreadable"));
    };
    disposables.push(
      watcher,
      watcher.onDidChange(update),
      watcher.onDidCreate(update),
      watcher.onDidDelete(() => {
        void this.broadcast(document, "file-missing");
      }),
    );
    const originalDispose = document.dispose.bind(document);
    document.dispose = () => {
      if (!this.documents.delete(document)) return;
      for (const d of this.resources.get(document) ?? []) d.dispose();
      this.resources.delete(document);
      originalDispose();
      if (this.active === document) this.active = undefined;
    };
    this.resources.set(document, disposables);
    return document;
  }
  async resolveCustomEditor(document: CustomDocument, panel: vscode.WebviewPanel): Promise<void> {
    const media = vscode.Uri.joinPath(this.context.extensionUri, "dist", "webview");
    panel.webview.options = {
      enableScripts: true,
      enableCommandUris: false,
      localResourceRoots: [media],
    };
    this.panels.set(panel, document);
    this.active = document;
    const telemetry = editorReporter({
      endpoint: process.env.VELLUM_TELEMETRY_ORIGIN,
      version: this.context.extension.packageJSON.version,
      os:
        process.platform === "darwin"
          ? "macos"
          : process.platform === "win32"
            ? "windows"
            : "linux",
    });
    let ready = false;
    let shareUrl: string | undefined;
    const message = panel.webview.onDidReceiveMessage(async (raw: unknown) => {
      const value = parseMessage(raw);
      if (!value) return;
      try {
        if (value.type === "telemetry") {
          telemetry.consent(value.enabled && vscode.env.isTelemetryEnabled);
          return;
        }
        if (value.type === "ready") {
          ready = true;
          this.readyPanels.add(panel);
          await this.send(panel, document);
          return;
        }
        if (value.type === "shareOpen") {
          if (shareUrl) await vscode.env.openExternal(vscode.Uri.parse(shareUrl, true));
          return;
        }
        if (value.type === "shareCopy") {
          if (shareUrl) await vscode.env.clipboard.writeText(shareUrl);
          return;
        }
        if (value.type === "share") {
          try {
            if (document.uri.scheme !== "file") throw Error();
            const state = await this.daemon.share(document.uri.fsPath, value.request);
            shareUrl = state.url;
            await panel.webview.postMessage({ type: "share", requestId: value.requestId, state });
          } catch {
            await panel.webview.postMessage({
              type: "share",
              requestId: value.requestId,
              state: { state: "failed" },
            });
          }
          return;
        }
        switch (value.command) {
          case "save":
            await vscode.commands.executeCommand("workbench.action.files.save");
            break;
          case "openAsText":
            await vscode.commands.executeCommand("vscode.openWith", document.uri, "default");
            break;
          case "pauseAll":
            await this.daemon.pauseAll();
            break;
          case "restartDaemon":
            await this.daemon.connect();
            await this.send(panel, document);
            break;
          case "connectAgents":
            await vscode.commands.executeCommand("vellum.connectAgents");
            break;
        }
      } catch {
        await this.send(panel, document, "host-request-failed");
      }
    });
    const telemetryChange = vscode.env.onDidChangeTelemetryEnabled((on) => {
      if (!on) telemetry.consent(false);
    });
    const theme = vscode.window.onDidChangeActiveColorTheme(() => {
      if (ready) void this.send(panel, document);
    });
    const config = vscode.workspace.onDidChangeConfiguration((e) => {
      if (ready && e.affectsConfiguration("vellum.matchEditorColours"))
        void this.send(panel, document);
    });
    const focus = panel.onDidChangeViewState(() => {
      if (panel.active) this.active = document;
    });
    panel.onDidDispose(() => {
      this.panels.delete(panel);
      this.readyPanels.delete(panel);
      telemetryChange.dispose();
      telemetry.dispose();
      message.dispose();
      theme.dispose();
      config.dispose();
      focus.dispose();
    });
    panel.webview.html = webviewHtml({
      source: panel.webview.cspSource,
      script: panel.webview.asWebviewUri(vscode.Uri.joinPath(media, "editor.js")).toString(),
      style: panel.webview.asWebviewUri(vscode.Uri.joinPath(media, "editor.css")).toString(),
      nonce: randomBytes(24).toString("base64"),
      title: brand.name,
    });
    void this.daemon.connect().then(
      () => {
        if (ready) void this.send(panel, document);
      },
      () => {
        if (ready) void this.send(panel, document);
      },
    );
  }
  get activeUri() {
    return this.active?.uri;
  }
  private async send(panel: vscode.WebviewPanel, document: CustomDocument, problem?: string) {
    if (!this.panels.has(panel)) return;
    await panel.webview.postMessage({
      type: "state",
      fileName: document.uri.path.split("/").at(-1) ?? brand.name,
      appearance: appearance(vscode.window.activeColorTheme.kind),
      matchEditorColours: vscode.workspace
        .getConfiguration("vellum")
        .get("matchEditorColours", false),
      dirty: document.lifecycle.dirty,
      daemon: this.daemon.connected,
      editable: false,
      problem: problem ?? "document-runtime-unavailable",
    });
  }
  private async broadcast(document: CustomDocument, problem?: string) {
    await Promise.all(
      [...this.panels]
        .filter(([, doc]) => doc === document)
        .map(([panel]) => this.send(panel, document, problem)),
    );
  }
  saveCustomDocument(doc: CustomDocument, token: vscode.CancellationToken) {
    return doc.lifecycle.save(() => token.isCancellationRequested);
  }
  saveCustomDocumentAs(doc: CustomDocument, uri: vscode.Uri, token: vscode.CancellationToken) {
    return doc.lifecycle.saveAs(uri.toString(), () => token.isCancellationRequested);
  }
  revertCustomDocument(doc: CustomDocument, token: vscode.CancellationToken) {
    return doc.lifecycle.revert(() => token.isCancellationRequested);
  }
  backupCustomDocument(
    doc: CustomDocument,
    context: vscode.CustomDocumentBackupContext,
    token: vscode.CancellationToken,
  ) {
    return doc.lifecycle.backup(
      context.destination.toString(),
      () => token.isCancellationRequested,
    );
  }
  dispose() {
    for (const doc of [...this.documents]) doc.dispose();
    for (const panel of this.panels.keys()) panel.dispose();
    this.panels.clear();
    this.changes.dispose();
    this.opened.dispose();
  }
}
