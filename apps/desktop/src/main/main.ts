import { randomBytes, randomUUID } from "node:crypto";
import { type FSWatcher, watch } from "node:fs";
import { readFile, realpath, stat, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { basename, dirname, join, parse, resolve } from "node:path";
import { brand } from "@vellum/config";
import { DaemonRuntime, externalUrl } from "@vellum/host-api";
import { accountRunner, NativeAccount } from "@vellum/host-api/native-account";
import { DaemonProcess } from "@vellum/host-api/native-relay";
import type { AgentSettings, RecoveryView } from "@vellum/host-electron/bridge";
import {
  type DesktopEvent,
  type DesktopReply,
  type DesktopRequest,
  type DesktopState,
  type DesktopTab,
  parseRequest,
} from "@vellum/host-electron/bridge";
import type { ClientId } from "@vellum/host-vscode/mcp-config";
import { editorReporter } from "@vellum/telemetry";
import {
  app,
  BrowserWindow,
  clipboard,
  dialog,
  type IpcMainInvokeEvent,
  ipcMain,
  Menu,
  type MenuItemConstructorOptions,
  Notification,
  nativeTheme,
  protocol,
  shell,
} from "electron";
import { EVENT_CHANNEL, REQUEST_CHANNEL } from "../shared/bridge";
import { createElectronBrowser } from "./browser";
import { connections } from "./connections";
import { type ConfirmClose, FolderWindows } from "./folders";
import { WEB_PREFERENCES, windowChrome } from "./index";
import { DesktopLibrary } from "./library";
import { type MenuAction, menuModel } from "./menus";
import { TurnNotifications } from "./notifications";
import { APP_URL, assetPath, CSP, trustedSender } from "./security";
import { downloadSnapshot, installSnapshot, parseShareLink } from "./share-link";
import {
  atomicMetadata,
  metadataPath,
  moveDraft,
  privateDirectory,
  readMetadata,
  recoverDraftIndex,
  removeDraft,
} from "./storage";
import { updateDriver } from "./update-driver";
import { Updates } from "./updates";

const arg = (name: string) => {
  const index = process.argv.indexOf(name);
  return index < 0 ? undefined : process.argv[index + 1];
};
const userData = arg("--user-data");
if (userData) app.setPath("userData", resolve(userData));
const runDir = arg("--run-dir");
app.setName(brand.name);
protocol.registerSchemesAsPrivileged([
  { scheme: "vellum-app", privileges: { standard: true, secure: true, supportFetchAPI: true } },
]);
let nextId = 0;
const windows = new Map<
  number,
  {
    telemetry?: ReturnType<typeof editorReporter>;
    window: BrowserWindow;
    folder: string | null;
    relay: DaemonProcess | null;
    watchers: Map<string, FSWatcher>;
    closing: boolean;
    page: "home" | "agents" | "signin" | "editor";
    agentSettings?: AgentSettings;
    recovery: (RecoveryView & { path: string }) | null;
    noticeTimer?: ReturnType<typeof setInterval>;
  }
>();
const binary = app.isPackaged
  ? join(process.resourcesPath, "bin", process.platform === "win32" ? "vellum.exe" : "vellum")
  : join(__dirname, "../bin", process.platform === "win32" ? "vellum.exe" : "vellum");
const account = new NativeAccount(accountRunner(binary), () => pushAll());
let library: DesktopLibrary;
let draftDirectory: string;
let notifications = false;
let updates = new Updates(null, app.getVersion());
/**
 * Open files held by the daemon's engine (ADR-033): one host editor per file on its window's daemon connection,
 * and one more for the tab's own engine in the renderer. A file without one (no daemon) is read-only.
 */
const runtimes = new Map<
  string,
  { runtime: DaemonRuntime; saved: number; panels: Map<string, string> }
>();
/** Daemon attaches still in flight, by path. */
const attaching = new Map<string, Promise<void>>();
/** This installation's device (a ULID in user data): people are known by the device they edit from. */
let device = "";
async function desktopDevice(): Promise<string> {
  const file = join(app.getPath("userData"), "device");
  const saved = await readFile(file, "utf8").catch(() => "");
  if (/^[0-7][0-9A-HJKMNP-TV-Z]{25}$/.test(saved.trim())) return saved.trim();
  const alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
  let time = Date.now();
  let head = "";
  for (let i = 0; i < 10; i++) {
    head = alphabet[time % 32] + head;
    time = Math.floor(time / 32);
  }
  const id = head + [...randomBytes(16)].map((b) => alphabet[b % 32]).join("");
  await writeFile(file, id, { mode: 0o600 });
  return id;
}
function windowFor(path: string) {
  for (const entry of windows.values()) {
    if (!entry.folder || !entry.relay) continue;
    const root = model.folderPath(entry.folder);
    if (path.startsWith(root.endsWith("/") ? root : `${root}/`)) return entry;
  }
  return undefined;
}
function tabOf(path: string) {
  for (const entry of windows.values()) {
    if (!entry.folder) continue;
    for (const tab of model.state(entry.folder).tabs)
      if (model.path(entry.folder, tab.id) === path) return { entry, folder: entry.folder, tab };
  }
  return undefined;
}
const model = new FolderWindows({
  canonical: realpath,
  folderOf: dirname,
  nameOf: basename,
  nextId: () => String(++nextId),
  runtime: {
    async open(path) {
      const info = await stat(path);
      if (!info.isFile() || info.size > 16 * 1024 * 1024) throw Error("invalid");
      const relay = windowFor(path)?.relay;
      // The tab opens at once; the daemon attaches behind it (a cold daemon takes seconds to start), and the
      // tab's engine waits for that before it decides between live and read-only.
      if (relay && device)
        attaching.set(
          path,
          DaemonRuntime.open((p) => relay.editor(p), path, device).then(
            (runtime) => {
              if (!tabOf(path)) {
                runtime.dispose();
                return;
              }
              const held = { runtime, saved: runtime.revision, panels: new Map<string, string>() };
              runtimes.set(path, held);
              runtime.subscribe(({ revision }) => {
                const at = tabOf(path);
                if (!at) return;
                model.update(at.folder, at.tab.id, {
                  dirty: revision !== held.saved,
                  running: false,
                });
                push(at.entry);
              });
              const at = tabOf(path);
              if (at) push(at.entry);
            },
            () => {
              /* No daemon, or a file it cannot read: read-only. */
            },
          ),
        );
      return { dirty: false, running: false };
    },
    async save(path) {
      const held = runtimes.get(path);
      if (!held) throw Error("read-only");
      held.saved = (await held.runtime.save()).revision;
    },
    async close(path) {
      attaching.delete(path);
      const held = runtimes.get(path);
      runtimes.delete(path);
      await held?.runtime.close();
    },
  },
});
type WindowEntry = NonNullable<ReturnType<typeof windows.get>>;
function appearance(): DesktopState["appearance"] {
  return nativeTheme.shouldUseHighContrastColors
    ? nativeTheme.shouldUseDarkColors
      ? "hc-dark"
      : "hc-light"
    : nativeTheme.shouldUseDarkColors
      ? "dark"
      : "light";
}
function state(entry: WindowEntry): DesktopState {
  return {
    ...(entry.folder ? model.state(entry.folder) : { folderName: "", tabs: [], active: null }),
    appearance: appearance(),
    platform:
      process.platform === "darwin" ? "mac" : process.platform === "win32" ? "windows" : "linux",
    daemon: entry.relay?.connected ?? false,
    problem: (() => {
      const active = entry.folder ? model.state(entry.folder).active : null;
      const path = active && entry.folder ? model.path(entry.folder, active) : null;
      return path && runtimes.has(path) ? "" : "document-runtime-unavailable";
    })(),
    home: library?.view(),
    welcome: !entry.folder && !library?.view().files.length,
    page: entry.page,
    updates: updates.state,
    agentSettings: entry.agentSettings
      ? {
          ...entry.agentSettings,
          clients: entry.agentSettings.clients.map((client) =>
            !entry.relay?.connected && client.state === "connected"
              ? { ...client, state: "error" }
              : client,
          ),
        }
      : undefined,
    recovery: entry.recovery
      ? { id: entry.recovery.id, name: entry.recovery.name, savedAt: entry.recovery.savedAt }
      : null,
    notifications,
    account: account.state,
  };
}
function push(entry: WindowEntry, event?: DesktopEvent) {
  if (entry.window.isDestroyed()) return;
  entry.window.webContents.send(EVENT_CHANNEL, event ?? { type: "state", state: state(entry) });
  const s = state(entry);
  entry.window.setTitle(
    `${s.tabs.find((t) => t.id === s.active)?.name ?? s.folderName ?? brand.name} · ${brand.name}`,
  );
  installMenu(entry);
}
function confirm(entry: WindowEntry): ConfirmClose {
  return async (tab: DesktopTab) => {
    const response = await dialog.showMessageBox(entry.window, {
      type: "warning",
      message: `Close ${tab.name}?`,
      detail: tab.running ? "A session is running in this file." : "This file has unsaved changes.",
      buttons: ["Cancel", "Discard", "Save"],
      defaultId: 0,
      cancelId: 0,
      noLink: true,
    });
    return response.response === 2 ? "save" : response.response === 1 ? "discard" : "cancel";
  };
}
async function connect(entry: WindowEntry) {
  try {
    await entry.relay?.connect();
  } catch {
    /* UI reports offline; stderr and native failures stay native. */
  }
  push(entry);
}
function findEntry(event: IpcMainInvokeEvent): WindowEntry {
  const entry = windows.get(event.sender.id);
  if (!entry || !trustedSender(event) || entry.closing) throw Error("invalid");
  return entry;
}
function requiredFolder(entry: WindowEntry) {
  if (!entry.folder) throw Error("invalid");
  return entry.folder;
}
async function openFiles(entry: WindowEntry) {
  if (!entry.folder) {
    const result = await dialog.showOpenDialog(entry.window, {
      properties: ["openFile", "multiSelections"],
      filters: [{ name: brand.name, extensions: ["vellum"] }],
    });
    if (!result.canceled) {
      let current = entry;
      for (const path of result.filePaths) current = await openKnown(current, path);
    }
    return;
  }
  const folder = requiredFolder(entry);
  const result = await dialog.showOpenDialog(entry.window, {
    properties: ["openFile", "multiSelections"],
    defaultPath: model.folderPath(folder),
    filters: [{ name: brand.name, extensions: ["vellum"] }],
  });
  if (result.canceled) return;
  for (const path of result.filePaths) {
    const tab = await model.openDocument(folder, path);
    await library.opened(path);
    entry.page = "editor";
    await findRecovery(entry, path);
    if (!entry.watchers.has(tab)) {
      const canonical = model.path(folder, tab);
      const watcher = watch(dirname(canonical), (_event, filename) => {
        if (filename === null || filename.toString() === basename(canonical))
          push(entry, { type: "changed", tab });
      });
      watcher.on("error", () => {
        watcher.close();
        entry.watchers.delete(tab);
        push(entry);
      });
      entry.watchers.set(tab, watcher);
    }
    push(entry);
  }
}
function pushAll() {
  for (const entry of windows.values()) push(entry);
}
function connectionManager(entry: WindowEntry) {
  return connections(
    binary,
    app.getPath("appData"),
    entry.folder ? model.folderPath(entry.folder) : undefined,
  );
}
async function showAgents(entry: WindowEntry) {
  entry.page = "agents";
  let clients: string[] = [];
  let sessionsAvailable = false;
  try {
    const state = await entry.relay?.hostState([]);
    clients = state?.clients ?? [];
    sessionsAvailable = state?.sessionsAvailable ?? false;
  } catch {
    /* offline */
  }
  entry.agentSettings = {
    clients: await connectionManager(entry).list(clients),
    sessionsAvailable,
  };
  push(entry);
}
function commonFolder(a: string, b: string) {
  while (
    b !== a &&
    !b.startsWith(
      a.endsWith("/") || a.endsWith("\\") ? a : `${a}${process.platform === "win32" ? "\\" : "/"}`,
    )
  ) {
    const parent = dirname(a);
    if (a === parent) break;
    a = parent;
  }
  if (parse(a).root !== parse(b).root) throw Error("different drives");
  return a;
}
async function findRecovery(entry: WindowEntry, path: string) {
  try {
    const result = await entry.relay?.recovery(path, "find");
    if (result?.savedAt !== null && result?.savedAt !== undefined)
      entry.recovery = { id: randomUUID(), path, name: basename(path), savedAt: result.savedAt };
  } catch {
    /* opening still works offline */
  }
}
async function openKnown(from: WindowEntry, path: string) {
  const canonical = await realpath(path);
  if (!canonical.endsWith(".vellum")) throw Error("invalid");
  const entry =
    from.folder && model.folderPath(from.folder) === dirname(canonical)
      ? from
      : await openFolder(from, dirname(canonical));
  if (!entry?.folder) throw Error("invalid");
  await model.openDocument(entry.folder, canonical);
  await library.opened(canonical);
  entry.page = "editor";
  await findRecovery(entry, canonical);
  push(entry);
  return entry;
}
async function newDraft(from: WindowEntry) {
  const id = await library.newDraft();
  await openKnown(from, library.path(id));
  pushAll();
}
async function closeKnown(path: string) {
  // Tabs first: their editors hold the file at the daemon until they close (ADR-033).
  for (const entry of windows.values()) {
    if (!entry.folder) continue;
    for (const tab of model.state(entry.folder).tabs)
      if (model.path(entry.folder, tab.id) === path) {
        await closeTab(entry, tab.id);
        if (model.state(entry.folder).tabs.some((t) => t.id === tab.id))
          throw Error("close cancelled");
      }
  }
  const relay = new DaemonProcess(binary, dirname(path), runDir, "vellum-desktop");
  try {
    await relay.recovery(path, "closeHeadless");
  } finally {
    relay.dispose();
  }
}
async function closeTab(entry: WindowEntry, tab: string) {
  if (await model.closeTab(requiredFolder(entry), tab, confirm(entry))) {
    entry.watchers.get(tab)?.close();
    entry.watchers.delete(tab);
    push(entry);
  }
}
async function openFolder(from?: WindowEntry, path?: string) {
  if (!path) {
    const options = { properties: ["openDirectory"] as "openDirectory"[] };
    const result = from
      ? await dialog.showOpenDialog(from.window, options)
      : await dialog.showOpenDialog(options);
    if (result.canceled) return;
    path = result.filePaths[0];
  }
  if (!path || !(await stat(path)).isDirectory()) throw Error("invalid");
  const folder = await model.openFolder(path);
  const existing = [...windows.values()].find((e) => e.folder === folder);
  if (existing) {
    existing.window.show();
    existing.window.focus();
    return existing;
  }
  const entry = createWindow(folder);
  if (from && from.folder === null) from.window.close();
  await connect(entry);
  return entry;
}
async function newFile(entry: WindowEntry) {
  if (!entry.folder) {
    await newDraft(entry);
    return;
  }
  const folder = requiredFolder(entry);
  const result = await dialog.showSaveDialog(entry.window, {
    title: "New Design File",
    defaultPath: join(model.folderPath(folder), "untitled.vellum"),
    filters: [{ name: brand.name, extensions: ["vellum"] }],
  });
  if (result.canceled || !result.filePath) return;
  const parent = await realpath(dirname(result.filePath));
  const normalized = parent.replaceAll("\\", "/");
  const root = model.folderPath(folder).replaceAll("\\", "/");
  if (normalized !== root && !normalized.startsWith(root.endsWith("/") ? root : `${root}/`))
    throw Error("invalid");
  const path = join(parent, basename(result.filePath));
  if (!path.endsWith(".vellum")) throw Error("invalid");
  // Rust creates a missing document or opens an existing one. It is the only document write path.
  await entry.relay?.createDocument(path);
  await model.openDocument(folder, path);
  await library.opened(path);
  entry.page = "editor";
  push(entry);
}
const browserWatches = new WeakMap<WindowEntry, Map<string, { dispose(): void }>>();
async function action(entry: WindowEntry, item: MenuAction) {
  if (!item.enabled) return;
  try {
    if (item.command) {
      push(entry, { type: "command", command: item.command });
      return;
    }
    switch (item.id) {
      case "home":
        entry.page = "home";
        push(entry);
        break;
      case "agents.manage":
        await showAgents(entry);
        break;
      case "account.signin":
        entry.page = "signin";
        push(entry);
        break;
      case "updates.check":
        await updates.check();
        push(entry);
        break;
      case "folder.open":
        await openFolder(entry);
        break;
      case "file.open":
        await openFiles(entry);
        break;
      case "file.new":
        await newFile(entry);
        break;
      case "file.save": {
        const active = state(entry).active;
        if (!active || !entry.folder) break;
        const path = model.path(entry.folder, active);
        const held = runtimes.get(path);
        if (!held) break;
        held.saved = (await held.runtime.save()).revision;
        model.update(entry.folder, active, { dirty: false, running: false });
        push(entry);
        break;
      }
      case "file.saveAs": {
        const active = state(entry).active;
        const item =
          active &&
          library
            .view()
            .files.find((f) => library.path(f.id) === model.path(requiredFolder(entry), active));
        if (item) await request(entry, { type: "draftSaveAs", id: item.id });
        break;
      }
      case "file.close": {
        const tab = state(entry).active;
        if (tab) await closeTab(entry, tab);
        break;
      }
      case "window.close":
        entry.window.close();
        break;
      case "agent.pauseAll":
        await entry.relay?.pauseAll();
        break;
      case "daemon.restart":
        entry.relay?.disconnect();
        await connect(entry);
        break;
      case "help.about":
        await dialog.showMessageBox(entry.window, { message: brand.name, detail: brand.tagline });
        break;
    }
  } catch {
    await dialog.showMessageBox(entry.window, {
      type: "error",
      message: "The action could not be completed.",
      detail: "Check the file and daemon connection, then try again.",
    });
  }
}
function nativeMenu(entry: WindowEntry): MenuItemConstructorOptions[] {
  const s = state(entry);
  const activePath = s.active && entry.folder ? model.path(entry.folder, s.active) : null;
  return menuModel({
    hasFolder: entry.folder !== null,
    hasDocument: s.active !== null,
    daemon: s.daemon,
    editable: activePath !== null && runtimes.has(activePath),
    draft:
      !!s.active &&
      library
        .view()
        .files.some(
          (f) =>
            f.kind === "draft" &&
            library.path(f.id) === model.path(requiredFolder(entry), s.active as string),
        ),
  }).map((group) => ({
    label: group.label,
    submenu: group.items.map((item) => ({
      id: item.id,
      label: item.label,
      enabled: item.enabled,
      accelerator: item.accelerator,
      click: () => {
        void action(entry, item);
      },
    })),
  }));
}
function installMenu(entry: WindowEntry) {
  if (BrowserWindow.getFocusedWindow() !== entry.window) return;
  const template = nativeMenu(entry);
  if (process.platform === "darwin")
    template.unshift({
      label: brand.name,
      submenu: [
        { role: "about" },
        { type: "separator" },
        { role: "hide" },
        { role: "hideOthers" },
        { role: "unhide" },
        { type: "separator" },
        { role: "quit" },
      ],
    });
  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
}
async function request(entry: WindowEntry, value: DesktopRequest): Promise<unknown> {
  switch (value.type) {
    case "telemetry":
      entry.telemetry?.consent(value.enabled);
      return;
    case "home":
    case "signin":
    case "editor":
      entry.page = value.type;
      push(entry);
      return;
    case "accountLogin":
      void account.login();
      return;
    case "accountCancel":
      await account.cancel();
      return;
    case "accountRefresh":
      void account.refresh();
      return;
    case "accountLogout":
      void account.logout();
      return;
    case "accountOpen": {
      const current = account.state;
      if (current.kind !== "waiting") throw Error("invalid");
      await shell.openExternal(current.verificationUrl);
      return;
    }
    case "agents":
      await showAgents(entry);
      return;
    case "newDraft":
      await newDraft(entry);
      return;
    case "homeOpen":
      await openKnown(entry, library.path(value.id));
      return;
    case "folderRecent":
      await openFolder(entry, library.folderPath(value.id));
      return;
    case "openSelected": {
      let current = entry;
      const paths = value.ids.map((id) => library.path(id));
      for (const path of paths) current = await openKnown(current, path);
      return;
    }
    case "forget":
      await library.forget(value.ids);
      pushAll();
      return;
    case "draftDelete": {
      const path = library.path(value.id);
      if (path !== join(draftDirectory, `${value.id}.vellum`)) throw Error("invalid");
      const answer = await dialog.showMessageBox(entry.window, {
        type: "warning",
        message: "Delete this draft?",
        detail: "This permanently deletes the draft on your computer.",
        buttons: ["Cancel", "Delete draft"],
        defaultId: 0,
        cancelId: 0,
      });
      if (answer.response !== 1) return;
      await closeKnown(path);
      await library.deleteDraft(value.id, true);
      pushAll();
      return;
    }
    case "draftSaveAs": {
      const path = library.path(value.id);
      if (path !== join(draftDirectory, `${value.id}.vellum`)) throw Error("invalid");
      const answer = await dialog.showSaveDialog(entry.window, {
        title: "Save As…",
        defaultPath: "untitled.vellum",
        filters: [{ name: brand.name, extensions: ["vellum"] }],
      });
      if (answer.canceled || !answer.filePath) return;
      await closeKnown(path);
      await library.saveAs(
        value.id,
        join(await realpath(dirname(answer.filePath)), basename(answer.filePath)),
      );
      await openKnown(entry, library.path(value.id));
      pushAll();
      return;
    }
    case "notifications":
      notifications = value.enabled;
      await atomicMetadata(join(app.getPath("userData"), "preferences.json"), { notifications });
      pushAll();
      return;
    case "checkUpdates":
      await updates.check();
      pushAll();
      return;
    case "downloadUpdate":
      await updates.download();
      pushAll();
      return;
    case "installUpdate": {
      const answer = await dialog.showMessageBox(entry.window, {
        message: "Restart and install the verified update?",
        buttons: ["Cancel", "Restart and install"],
        defaultId: 0,
        cancelId: 0,
      });
      if (answer.response === 1) updates.install();
      return;
    }
    case "agentConfig": {
      const manager = connectionManager(entry);
      const rows = await manager.list([]);
      const row = rows.find((r) => r.id === value.id);
      if (!row) throw Error("invalid");
      const answer = await dialog.showMessageBox(entry.window, {
        message:
          value.action === "connect"
            ? `Configure ${row.name}?`
            : `Remove the managed entry for ${row.name}?`,
        detail: `${row.path}\n\n${row.entry}`,
        buttons: ["Cancel", value.action === "connect" ? "Connect" : "Remove"],
        defaultId: 0,
        cancelId: 0,
      });
      if (answer.response === 1) await manager.change(value.id as ClientId, value.action);
      await showAgents(entry);
      return;
    }
    case "recovery": {
      const recovery = entry.recovery;
      if (!recovery || recovery.id !== value.id || !entry.relay) throw Error("invalid");
      if (value.action === "cancel") {
        entry.recovery = null;
        push(entry);
        return;
      }
      if ((await realpath(recovery.path)) !== recovery.path) throw Error("invalid");
      if (value.action === "discard") {
        const answer = await dialog.showMessageBox(entry.window, {
          type: "warning",
          message: "Discard the recovery backup?",
          detail: "Unsaved changes will be permanently removed. The original file is kept.",
          buttons: ["Cancel", "Discard backup"],
          defaultId: 0,
          cancelId: 0,
        });
        if (answer.response !== 1) return;
      }
      if (value.action === "saveAs") {
        const answer = await dialog.showSaveDialog(entry.window, {
          title: "Save As…",
          defaultPath: recovery.path,
          filters: [{ name: brand.name, extensions: ["vellum"] }],
        });
        if (answer.canceled || !answer.filePath) return;
        const destination = join(
          await realpath(dirname(answer.filePath)),
          basename(answer.filePath),
        );
        if (!destination.endsWith(".vellum")) throw Error("invalid");
        const root = commonFolder(dirname(recovery.path), dirname(destination));
        const relay = new DaemonProcess(binary, root, runDir, "vellum-desktop");
        try {
          await relay.recovery(recovery.path, "saveAs", destination);
        } finally {
          relay.dispose();
        }
        entry.recovery = null;
        await openKnown(entry, destination);
      } else await entry.relay.recovery(recovery.path, value.action);
      entry.recovery = null;
      push(entry);
      return;
    }
    case "state":
      return state(entry);
    case "daemon":
      return entry.relay?.connected ?? false;
    case "openFolder":
      await openFolder(entry);
      return;
    case "openFiles":
      await openFiles(entry);
      return;
    case "newFile":
      await newFile(entry);
      return;
    case "pauseAll":
      if (!entry.relay) throw Error("offline");
      await entry.relay.pauseAll();
      return;
    case "restartDaemon":
      entry.relay?.disconnect();
      await connect(entry);
      return;
    case "activate":
      model.activate(requiredFolder(entry), value.tab);
      push(entry);
      return;
    case "close":
      await closeTab(entry, value.tab);
      return;
    case "browserStop": {
      browserWatches.get(entry)?.get(value.id)?.dispose();
      browserWatches.get(entry)?.delete(value.id);
      return;
    }
    case "browserWatch":
    case "browserCapture": {
      const browser = createElectronBrowser({
        create: (options) => new BrowserWindow(options),
        authorize: async (origin) => {
          const answer = await dialog.showMessageBox(entry.window, {
            message: "Allow the import browser to load this origin?",
            detail: origin,
            buttons: ["Cancel", "Allow once"],
            cancelId: 0,
            defaultId: 0,
          });
          if (answer.response !== 1) throw Error("denied");
        },
      });
      if (value.type === "browserCapture") return browser.capture(value.request);
      let watches = browserWatches.get(entry);
      if (!watches) {
        watches = new Map();
        browserWatches.set(entry, watches);
      }
      if (watches.size >= 8 || watches.has(value.id)) throw Error("invalid");
      const watch = await browser.watch?.(
        value.request,
        (capture) => push(entry, { type: "browserFrame", id: value.id, capture }),
        () => push(entry, { type: "browserFrame", id: value.id, capture: null }),
      );
      if (!watch) throw Error("failed");
      if (entry.closing || entry.window.isDestroyed()) {
        watch.dispose();
        throw Error("failed");
      }
      watches.set(value.id, watch);
      return;
    }
    case "share": {
      const path = model.path(requiredFolder(entry), value.tab);
      if (!entry.relay) throw Error("offline");
      return entry.relay.share(path, value.request);
    }
    case "engineModule": {
      const engine = (name: string) => readFile(join(__dirname, "../engine", name));
      const [wasm, inter] = await Promise.all([
        engine("vellum_editor.wasm"),
        engine("InterVariable.ttf"),
      ]);
      return { wasm: new Uint8Array(wasm), fonts: [new Uint8Array(inter)] };
    }
    case "engineLoad": {
      const path = model.path(requiredFolder(entry), value.tab);
      const pending = attaching.get(path);
      if (pending)
        await Promise.race([pending, new Promise((resolve) => setTimeout(resolve, 20_000))]);
      const held = runtimes.get(path);
      if (held && !held.panels.has(value.tab)) held.panels.set(value.tab, held.runtime.newPanel());
      return {
        live: held !== undefined,
        device,
        path,
        // A read-only file is shown from its bytes; a live one comes from the daemon's engine.
        file: held ? null : new Uint8Array(await readFile(path)),
      };
    }
    case "agentFeed": {
      const path = model.path(requiredFolder(entry), value.tab);
      if (!entry.relay) throw Error("offline");
      return entry.relay.agentFeed({
        action: value.action,
        path,
        ...value.params,
        ...(value.action === "undoTurn" ? { device } : {}),
      });
    }
    case "engine": {
      const path = model.path(requiredFolder(entry), value.tab);
      const held = runtimes.get(path);
      const panel = held?.panels.get(value.tab);
      if (!held || !panel) throw Error("read-only");
      const reply = await held.runtime.relay(panel, value.action, value.params);
      if (value.action === "close") held.panels.delete(value.tab);
      return reply;
    }
    case "readDocument": {
      const path = model.path(requiredFolder(entry), value.tab);
      if ((await realpath(path)) !== path) throw Error("invalid");
      const info = await stat(path);
      if (!info.isFile() || info.size > 16 * 1024 * 1024) throw Error("invalid");
      return Array.from(await readFile(path));
    }
    case "clipboardRead": {
      const text = await clipboard.readText();
      if (text.length > 1024 * 1024) throw Error("invalid");
      return text;
    }
    case "clipboardWrite":
      await clipboard.writeText(value.text);
      return;
    case "external": {
      const url = externalUrl(value.url);
      // Even a compromised renderer cannot silently launch an external browser.
      const answer = await dialog.showMessageBox(entry.window, {
        message: "Open this link in your browser?",
        detail: url,
        buttons: ["Cancel", "Open"],
        cancelId: 0,
        defaultId: 0,
      });
      if (answer.response === 1) await shell.openExternal(url);
      return;
    }
    case "menu": {
      const group = nativeMenu(entry).find((g) => g.label === value.category);
      if (group?.submenu && Array.isArray(group.submenu))
        Menu.buildFromTemplate(group.submenu).popup({ window: entry.window });
      return;
    }
  }
}
function createWindow(folder: string | null): WindowEntry {
  const window = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 960,
    minHeight: 600,
    useContentSize: true,
    show: false,
    title: brand.name,
    autoHideMenuBar: process.platform !== "darwin",
    ...windowChrome(process.platform),
    webPreferences: {
      ...WEB_PREFERENCES,
      preload: join(__dirname, "../preload/index.cjs"),
      nodeIntegrationInWorker: false,
      nodeIntegrationInSubFrames: false,
      webSecurity: true,
      allowRunningInsecureContent: false,
      webviewTag: false,
      spellcheck: false,
      navigateOnDragDrop: false,
    },
  });
  const entry: WindowEntry = {
    window,
    telemetry: editorReporter({
      endpoint: process.env.VELLUM_TELEMETRY_ORIGIN,
      version: app.getVersion(),
      os:
        process.platform === "darwin"
          ? "macos"
          : process.platform === "win32"
            ? "windows"
            : "linux",
    }),
    folder,
    relay: new DaemonProcess(
      binary,
      folder ? model.folderPath(folder) : draftDirectory,
      runDir,
      "vellum-desktop",
    ),
    watchers: new Map(),
    closing: false,
    page: "home",
    recovery: null,
  };
  const notices = new TurnNotifications((kind, path) => {
    if (!Notification.isSupported()) return;
    const notification = new Notification({
      title: brand.name,
      body:
        kind === "done"
          ? "Agent turn finished"
          : kind === "failed"
            ? "Agent turn failed"
            : "An agent needs your permission",
    });
    notification.on("click", () => {
      if (window.isDestroyed()) return;
      if (entry.folder) {
        const tab = model
          .state(entry.folder)
          .tabs.find((tab) => model.path(entry.folder as string, tab.id) === path);
        if (tab) model.activate(entry.folder, tab.id);
      }
      entry.page = "editor";
      window.show();
      window.focus();
      push(entry);
    });
    notification.show();
  });
  let polling = false;
  const pollNotices = async () => {
    if (polling || !entry.relay?.connected || entry.closing) return;
    polling = true;
    try {
      const paths = entry.folder
        ? model
            .state(entry.folder)
            .tabs.slice(0, 20)
            .map((t) => model.path(entry.folder as string, t.id))
        : [];
      const state = await entry.relay.hostState(paths);
      if (entry.page === "agents" && !window.isDestroyed()) {
        entry.agentSettings = {
          clients: await connectionManager(entry).list(state.clients),
          sessionsAvailable: state.sessionsAvailable,
        };
        push(entry);
      }
      if (!window.isDestroyed())
        notices.receive(
          state.sessions.flatMap((r) => (r.turn ? [{ ...r.turn, id: r.id, path: r.path }] : [])),
          notifications,
          window.isFocused(),
        );
    } catch {
      /* No stale notification is synthesized when disconnected. */
    } finally {
      polling = false;
    }
  };
  entry.noticeTimer = setInterval(() => void pollNotices(), 2000);
  const contentsId = window.webContents.id;
  windows.set(contentsId, entry);
  entry.relay?.onConnection(() => push(entry));
  window.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
  window.webContents.on("will-navigate", (event) => event.preventDefault());
  window.webContents.on("will-frame-navigate", (event) => event.preventDefault());
  window.webContents.on("will-attach-webview", (event) => event.preventDefault());
  window.on("focus", () => installMenu(entry));
  window.once("ready-to-show", () => {
    window.show();
    push(entry);
  });
  window.webContents.on("did-finish-load", () => push(entry));
  window.on("close", (event) => {
    if (!entry.folder || entry.closing) return;
    event.preventDefault();
    entry.closing = true;
    void model
      .closeWindow(entry.folder, confirm(entry))
      .then((closed) => {
        if (closed) window.destroy();
        else {
          entry.closing = false;
          push(entry);
        }
      })
      .catch(() => {
        entry.closing = false;
        push(entry);
      });
  });
  window.on("closed", () => {
    entry.telemetry?.dispose();
    for (const watch of browserWatches.get(entry)?.values() ?? []) watch.dispose();
    browserWatches.delete(entry);
    windows.delete(contentsId);
    if (!windows.size) void account.cancel();
    clearInterval(entry.noticeTimer);
    for (const watcher of entry.watchers.values()) watcher.close();
    entry.relay?.dispose();
  });
  void window.loadURL(APP_URL);
  return entry;
}
const pendingShareLinks: string[] = [];
let importingShare = false;
let shareReady = false;
async function receiveShareLink(value: string) {
  if (importingShare) return;
  let link: ReturnType<typeof parseShareLink>;
  try {
    link = parseShareLink(value);
  } catch {
    return;
  }
  if (!shareReady) {
    if (pendingShareLinks.length < 10) pendingShareLinks.push(value);
    return;
  }
  importingShare = true;
  try {
    const picked = await dialog.showOpenDialog({
      title: `Save shared snapshot from ${new URL(link.origin).hostname}`,
      properties: ["openDirectory", "createDirectory"],
    });
    const parent = picked.filePaths[0];
    if (picked.canceled || !parent) return;
    const bundle = await downloadSnapshot(link, fetch);
    const path = await installSnapshot(parent, bundle);
    const entry = [...windows.values()][0] ?? createWindow(null);
    await openKnown(entry, path);
  } catch {
    await dialog.showMessageBox({
      type: "error",
      message: "Share unavailable",
      detail: "The snapshot could not be downloaded or opened. Check the link and try again.",
    });
  } finally {
    importingShare = false;
  }
}
app.on("open-url", (event, value) => {
  event.preventDefault();
  void receiveShareLink(value);
});
const gotLock = app.requestSingleInstanceLock();
if (!gotLock) app.quit();
else {
  let quitting = false;
  let quitConfirmed = false;
  app.on("before-quit", (event) => {
    if (quitConfirmed) return;
    event.preventDefault();
    if (quitting) return;
    quitting = true;
    void (async () => {
      try {
        for (const entry of [...windows.values()]) {
          if (entry.folder && model.has(entry.folder)) {
            if (!(await model.closeWindow(entry.folder, confirm(entry)))) return;
          }
          entry.closing = true;
          entry.window.destroy();
        }
        quitConfirmed = true;
        app.quit();
      } finally {
        quitting = false;
      }
    })().catch(() => {
      /* A failed save keeps the app open. */
    });
  });
  app.on("second-instance", (_event, args) => {
    for (const value of args)
      if (value.startsWith(`${brand.desktopProtocol}://`)) void receiveShareLink(value);
    const entry = [...windows.values()][0];
    entry?.window.show();
    entry?.window.focus();
  });
  app.on("activate", () => {
    if (library && !windows.size) void connect(createWindow(null));
  });
  app.on("window-all-closed", () => {
    if (process.platform !== "darwin") app.quit();
  });
  app.on("will-quit", () => {
    account.dispose();
    for (const entry of windows.values()) entry.relay?.dispose();
  });
  void app
    .whenReady()
    .then(async () => {
      const base = await privateDirectory(
        userData ? join(app.getPath("userData"), "local") : join(homedir(), ".vellum"),
      );
      draftDirectory = await privateDirectory(join(base, "drafts"));
      device = await desktopDevice();
      library = new DesktopLibrary({
        load: async () =>
          recoverDraftIndex(
            await readMetadata(metadataPath(app.getPath("userData"))),
            draftDirectory,
          ),
        persist: (data) => atomicMetadata(metadataPath(app.getPath("userData")), data),
        canonical: realpath,
        inspect: async (path) => {
          const info = await stat(path);
          if (!info.isFile() || info.size > 16 * 1024 * 1024) throw Error("invalid");
          return { created: Math.trunc(info.birthtimeMs), modified: Math.trunc(info.mtimeMs) };
        },
        folderOf: dirname,
        nameOf: basename,
        draftPath: (id) => join(draftDirectory, `${id}.vellum`),
        now: Date.now,
        nextId: randomUUID,
        create: async (path) => {
          const relay = new DaemonProcess(binary, draftDirectory, runDir, "vellum-desktop");
          try {
            await relay.createDocument(path);
          } finally {
            relay.dispose();
          }
        },
        move: moveDraft,
        remove: (path) => removeDraft(path, draftDirectory),
      });
      try {
        await library.ready();
      } catch {
        await dialog.showErrorBox(
          "Home metadata unavailable",
          "Your files are kept. Repair or move home.json in the application profile before reopening.",
        );
        app.exit(1);
        return;
      }
      try {
        notifications =
          JSON.parse(await readFile(join(app.getPath("userData"), "preferences.json"), "utf8"))
            .notifications === true;
      } catch {
        /* default off */
      }
      updates = new Updates(await updateDriver(), app.getVersion());
      protocol.handle("vellum-app", async (req) => {
        const path = assetPath(req.url);
        if (!path || req.method !== "GET") return new Response(null, { status: 404 });
        try {
          const bytes = await readFile(join(__dirname, "../renderer", path));
          const type = path.endsWith(".html")
            ? "text/html"
            : path.endsWith(".js")
              ? "text/javascript"
              : path.endsWith(".css")
                ? "text/css"
                : path.endsWith(".woff2")
                  ? "font/woff2"
                  : "application/octet-stream";
          return new Response(bytes, {
            headers: {
              "Content-Type": type,
              "Content-Security-Policy": CSP,
              "X-Content-Type-Options": "nosniff",
            },
          });
        } catch {
          return new Response(null, { status: 404 });
        }
      });
      ipcMain.handle(REQUEST_CHANNEL, async (event, payload): Promise<DesktopReply> => {
        try {
          const entry = findEntry(event);
          const value = parseRequest(payload);
          if (!value) return { ok: false, code: "invalid" };
          const result = await request(entry, value);
          return result === undefined ? { ok: true } : { ok: true, value: result };
        } catch {
          return { ok: false, code: "failed" };
        }
      });
      ipcMain.handle(
        "vellum:desktop:drop",
        async (event, paths: unknown): Promise<DesktopReply> => {
          try {
            let entry = findEntry(event);
            if (
              !Array.isArray(paths) ||
              paths.length > 20 ||
              !paths.every(
                (p) => typeof p === "string" && p.length <= 4096 && p.endsWith(".vellum"),
              )
            )
              throw Error("invalid");
            for (const path of paths) {
              const next = await openKnown(entry, path);
              if (next) entry = next;
            }
            return { ok: true };
          } catch {
            return { ok: false, code: "failed" };
          }
        },
      );
      // The renderer has no web permissions, downloads, navigation or network access.
      if (app.isPackaged) app.setAsDefaultProtocolClient(brand.desktopProtocol);
      for (const value of process.argv)
        if (value.startsWith(`${brand.desktopProtocol}://`)) pendingShareLinks.push(value);
      const first = createWindow(null);
      void connect(first);
      const session = first.window.webContents.session;
      session.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
      session.setPermissionCheckHandler(() => false);
      session.on("will-download", (event) => event.preventDefault());
      session.webRequest.onBeforeRequest((details, callback) =>
        callback({ cancel: assetPath(details.url) === null }),
      );
      shareReady = true;
      for (const value of pendingShareLinks.splice(0)) void receiveShareLink(value);
      nativeTheme.on("updated", () => {
        for (const entry of windows.values()) push(entry);
      });
    })
    .catch(() => {
      dialog.showErrorBox(
        "Desktop storage unavailable",
        "The application could not open its private storage. Your design files are kept.",
      );
      app.exit(1);
    });
}
