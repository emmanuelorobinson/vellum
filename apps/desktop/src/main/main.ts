import { type FSWatcher, watch } from "node:fs";
import { readFile, realpath, stat } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { brand } from "@vellum/config";
import { externalUrl } from "@vellum/host-api";
import { DaemonProcess } from "@vellum/host-api/native-relay";
import {
  type DesktopEvent,
  type DesktopReply,
  type DesktopRequest,
  type DesktopState,
  type DesktopTab,
  parseRequest,
} from "@vellum/host-electron/bridge";
import {
  app,
  BrowserWindow,
  clipboard,
  dialog,
  type IpcMainInvokeEvent,
  ipcMain,
  Menu,
  type MenuItemConstructorOptions,
  nativeTheme,
  protocol,
  shell,
} from "electron";
import { EVENT_CHANNEL, REQUEST_CHANNEL } from "../shared/bridge";
import { type ConfirmClose, FolderWindows } from "./folders";
import { WEB_PREFERENCES } from "./index";
import { type MenuAction, menuModel } from "./menus";
import { APP_URL, assetPath, CSP, trustedSender } from "./security";

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
    window: BrowserWindow;
    folder: string | null;
    relay: DaemonProcess | null;
    watchers: Map<string, FSWatcher>;
    closing: boolean;
  }
>();
const binary = join(__dirname, "../bin", process.platform === "win32" ? "vellum.exe" : "vellum");
const model = new FolderWindows({
  canonical: realpath,
  folderOf: dirname,
  nameOf: basename,
  nextId: () => String(++nextId),
  runtime: {
    async open(path) {
      const info = await stat(path);
      if (!info.isFile() || info.size > 16 * 1024 * 1024) throw Error("invalid");
      // No document ABI exists yet. Do not parse JSON into a second system of record.
      return { dirty: false, running: false };
    },
    async save() {
      throw Error("read-only");
    },
    async close() {},
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
    problem: "document-runtime-unavailable",
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
  const folder = requiredFolder(entry);
  const result = await dialog.showOpenDialog(entry.window, {
    properties: ["openFile", "multiSelections"],
    defaultPath: model.folderPath(folder),
    filters: [{ name: brand.name, extensions: ["vellum"] }],
  });
  if (result.canceled) return;
  for (const path of result.filePaths) {
    const tab = await model.openDocument(folder, path);
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
    return;
  }
  const entry = createWindow(folder);
  if (from && from.folder === null) from.window.close();
  void connect(entry);
}
async function newFile(entry: WindowEntry) {
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
  push(entry);
}
async function action(entry: WindowEntry, item: MenuAction) {
  if (!item.enabled) return;
  try {
    if (item.command) {
      push(entry, { type: "command", command: item.command });
      return;
    }
    switch (item.id) {
      case "folder.open":
        await openFolder(entry);
        break;
      case "file.open":
        await openFiles(entry);
        break;
      case "file.new":
        await newFile(entry);
        break;
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
  return menuModel({
    hasFolder: entry.folder !== null,
    hasDocument: s.active !== null,
    daemon: s.daemon,
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
    folder,
    relay: folder
      ? new DaemonProcess(binary, model.folderPath(folder), runDir, "vellum-desktop")
      : null,
    watchers: new Map(),
    closing: false,
  };
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
    windows.delete(contentsId);
    for (const watcher of entry.watchers.values()) watcher.close();
    entry.relay?.dispose();
  });
  void window.loadURL(APP_URL);
  return entry;
}
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
  app.on("second-instance", () => {
    const entry = [...windows.values()][0];
    entry?.window.show();
    entry?.window.focus();
  });
  app.on("activate", () => {
    if (!windows.size) createWindow(null);
  });
  app.on("window-all-closed", () => {
    if (process.platform !== "darwin") app.quit();
  });
  app.on("will-quit", () => {
    for (const entry of windows.values()) entry.relay?.dispose();
  });
  void app.whenReady().then(() => {
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
    // The renderer has no web permissions, downloads, navigation or network access.
    const first = createWindow(null);
    const session = first.window.webContents.session;
    session.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
    session.setPermissionCheckHandler(() => false);
    session.on("will-download", (event) => event.preventDefault());
    session.webRequest.onBeforeRequest((details, callback) =>
      callback({ cancel: assetPath(details.url) === null }),
    );
    nativeTheme.on("updated", () => {
      for (const entry of windows.values()) push(entry);
    });
  });
}
