import { externalUrl, type HostServices, PortError, type ThemeInfo } from "@vellum/host-api";
import type { AccountState } from "@vellum/host-api/account-state";

export const MENU_CATEGORIES = [
  "File",
  "Edit",
  "View",
  "Object",
  "Arrange",
  "Agents",
  "Help",
] as const;
export type MenuCategory = (typeof MENU_CATEGORIES)[number];
export interface DesktopTab {
  id: string;
  name: string;
  dirty: boolean;
  running: boolean;
}
export interface HomeFile {
  id: string;
  name: string;
  folder: string;
  kind: "file" | "draft";
  created: number;
  opened: number;
  branch: string | null;
  running: boolean | null;
}
export interface HomeData {
  files: HomeFile[];
  folders: { id: string; label: string }[];
}
export type UpdateState =
  | { kind: "unavailable" | "idle" | "checking" | "current" | "downloading" | "failed" }
  | { kind: "available" | "ready"; version: string };
export interface AgentSettings {
  clients: { id: string; name: string; state: string; path: string; entry: string }[];
  sessionsAvailable: boolean;
}
export interface RecoveryView {
  id: string;
  name: string;
  savedAt: number;
}
export interface DesktopState {
  folderName: string;
  tabs: DesktopTab[];
  active: string | null;
  appearance: ThemeInfo["appearance"];
  platform: "mac" | "windows" | "linux";
  daemon: boolean;
  problem: "document-runtime-unavailable" | "offline" | "failed";
  home?: HomeData;
  page?: "home" | "agents" | "signin" | "editor";
  welcome?: boolean;
  updates?: UpdateState;
  agentSettings?: AgentSettings;
  recovery?: RecoveryView | null;
  notifications?: boolean;
  account?: AccountState;
}
export type DesktopRequest =
  | {
      type:
        | "state"
        | "openFolder"
        | "openFiles"
        | "newFile"
        | "pauseAll"
        | "restartDaemon"
        | "daemon"
        | "clipboardRead"
        | "home"
        | "agents"
        | "accountLogin"
        | "accountCancel"
        | "accountRefresh"
        | "accountLogout"
        | "accountOpen"
        | "signin"
        | "editor"
        | "newDraft"
        | "checkUpdates"
        | "downloadUpdate"
        | "installUpdate";
    }
  | { type: "homeOpen" | "folderRecent" | "draftSaveAs" | "draftDelete"; id: string }
  | { type: "forget" | "openSelected"; ids: string[] }
  | { type: "agentConfig"; id: string; action: "connect" | "disconnect" }
  | { type: "recovery"; id: string; action: "save" | "saveAs" | "discard" | "cancel" }
  | { type: "notifications"; enabled: boolean }
  | { type: "activate" | "close" | "readDocument"; tab: string }
  | { type: "menu"; category: MenuCategory }
  | { type: "external"; url: string }
  | { type: "clipboardWrite"; text: string };
export type DesktopEvent =
  | { type: "state"; state: DesktopState }
  | { type: "changed"; tab: string }
  | { type: "command"; command: string };
export type DesktopReply =
  | { ok: true; value?: unknown }
  | { ok: false; code: "invalid" | "failed" | "read-only" | "offline" };
/** No generic channel, RPC, native path, document-write or credential API is exposed. */
export interface DesktopBridge {
  request(request: DesktopRequest): Promise<DesktopReply>;
  subscribe(listener: (event: DesktopEvent) => void): () => void;
}
export function parseRequest(value: unknown): DesktopRequest | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const v = value as Record<string, unknown>;
  const keys = Object.keys(v).sort().join(",");
  if (
    keys === "type" &&
    [
      "state",
      "openFolder",
      "openFiles",
      "newFile",
      "pauseAll",
      "restartDaemon",
      "daemon",
      "clipboardRead",
      "home",
      "agents",
      "accountLogin",
      "accountCancel",
      "accountRefresh",
      "accountLogout",
      "accountOpen",
      "signin",
      "editor",
      "newDraft",
      "checkUpdates",
      "downloadUpdate",
      "installUpdate",
    ].includes(String(v.type))
  )
    return v as DesktopRequest;
  const validId = (id: unknown) => typeof id === "string" && /^[a-zA-Z0-9_-]{1,80}$/.test(id);
  if (
    keys === "id,type" &&
    ["homeOpen", "folderRecent", "draftSaveAs", "draftDelete"].includes(String(v.type)) &&
    validId(v.id)
  )
    return v as DesktopRequest;
  if (
    keys === "ids,type" &&
    ["forget", "openSelected"].includes(String(v.type)) &&
    Array.isArray(v.ids) &&
    v.ids.length <= 200 &&
    v.ids.every(validId)
  )
    return v as DesktopRequest;
  if (
    keys === "action,id,type" &&
    validId(v.id) &&
    ((v.type === "agentConfig" && ["connect", "disconnect"].includes(String(v.action))) ||
      (v.type === "recovery" && ["save", "saveAs", "discard", "cancel"].includes(String(v.action))))
  )
    return v as DesktopRequest;
  if (keys === "enabled,type" && v.type === "notifications" && typeof v.enabled === "boolean")
    return v as DesktopRequest;
  if (
    keys === "tab,type" &&
    ["activate", "close", "readDocument"].includes(String(v.type)) &&
    typeof v.tab === "string" &&
    /^[a-zA-Z0-9_-]{1,80}$/.test(v.tab)
  )
    return v as DesktopRequest;
  if (
    keys === "category,type" &&
    v.type === "menu" &&
    MENU_CATEGORIES.includes(v.category as MenuCategory)
  )
    return v as DesktopRequest;
  if (
    keys === "type,url" &&
    v.type === "external" &&
    typeof v.url === "string" &&
    v.url.length <= 4096
  ) {
    try {
      externalUrl(v.url);
      return v as DesktopRequest;
    } catch {
      return null;
    }
  }
  if (
    keys === "text,type" &&
    v.type === "clipboardWrite" &&
    typeof v.text === "string" &&
    v.text.length <= 1024 * 1024
  )
    return v as DesktopRequest;
  return null;
}
const tabId = (uri: string) => {
  const id = /^document:([a-zA-Z0-9_-]{1,80})$/.exec(uri)?.[1];
  if (!id) throw new PortError("invalid");
  return id;
};
export function createBridgeServices(
  bridge: DesktopBridge,
  initial: ThemeInfo,
): HostServices & { dispose(): void } {
  let theme = { ...initial };
  let disposed = false;
  const themes = new Set<(theme: ThemeInfo) => void>();
  const unsubscribe = bridge.subscribe((event) => {
    if (!disposed && event.type === "state" && event.state.appearance !== theme.appearance) {
      theme = { appearance: event.state.appearance };
      for (const listener of themes) listener({ ...theme });
    }
  });
  const request = async (value: DesktopRequest) => {
    if (disposed) throw new PortError("disposed");
    let result: DesktopReply;
    try {
      result = await bridge.request(value);
    } catch {
      throw new PortError("failed");
    }
    if (!result.ok) throw new PortError(result.code);
    return result.value;
  };
  return {
    async readFile(uri) {
      const value = await request({ type: "readDocument", tab: tabId(uri) });
      if (!Array.isArray(value) || !value.every((v) => Number.isInteger(v) && v >= 0 && v <= 255))
        throw new PortError("failed");
      return new Uint8Array(value);
    },
    async writeFile() {
      throw new PortError("read-only");
    },
    watch(uri, listener) {
      const tab = tabId(uri);
      return {
        dispose: bridge.subscribe((event) => {
          if (event.type === "changed" && event.tab === tab) listener();
        }),
      };
    },
    async discoverDaemon() {
      return (await request({ type: "daemon" })) === true ? { kind: "relayed", protocol: 1 } : null;
    },
    async openExternal(url) {
      await request({ type: "external", url: externalUrl(url) });
    },
    clipboard: {
      async read() {
        const text = await request({ type: "clipboardRead" });
        if (typeof text !== "string") throw new PortError("failed");
        return { items: { "text/plain": new TextEncoder().encode(text) } };
      },
      async write(data) {
        if (
          Object.keys(data.items).some((key) => key !== "text/plain") ||
          !data.items["text/plain"]
        )
          throw new PortError("unsupported");
        await request({
          type: "clipboardWrite",
          text: new TextDecoder().decode(data.items["text/plain"]),
        });
      },
    },
    theme: () => ({ ...theme }),
    onThemeChange(listener) {
      themes.add(listener);
      return {
        dispose() {
          themes.delete(listener);
        },
      };
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      unsubscribe();
      themes.clear();
    },
  };
}
