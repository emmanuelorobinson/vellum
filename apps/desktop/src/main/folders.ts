import type { DesktopTab } from "@vellum/host-electron/bridge";

export interface DocumentRuntime {
  open(path: string): Promise<{ dirty: boolean; running: boolean }>;
  save(path: string): Promise<void>;
  close(path: string): Promise<void>;
}
export type CloseChoice = "save" | "discard" | "cancel";
export type ConfirmClose = (tab: DesktopTab) => Promise<CloseChoice>;
interface Tab extends DesktopTab {
  path: string;
  revision: number;
}
interface Folder {
  path: string;
  tabs: Tab[];
  active: string | null;
}
/** Main-process ownership. Renderer ids are capabilities scoped to one registered window. */
export class FolderWindows {
  private folders = new Map<string, Folder>();
  private queue: Promise<unknown> = Promise.resolve();
  constructor(
    private readonly ports: {
      canonical(path: string): Promise<string>;
      folderOf(path: string): string;
      nameOf(path: string): string;
      nextId(): string;
      runtime: DocumentRuntime;
    },
  ) {}
  private serial<T>(run: () => Promise<T>): Promise<T> {
    const result = this.queue.then(run);
    this.queue = result.catch(() => {});
    return result;
  }
  has(id: string) {
    return this.folders.has(id);
  }
  private folder(id: string) {
    const folder = this.folders.get(id);
    if (!folder) throw Error("invalid");
    return folder;
  }
  private tab(folder: string, id: string) {
    const tab = this.folder(folder).tabs.find((t) => t.id === id);
    if (!tab) throw Error("invalid");
    return tab;
  }
  openFolder(path: string): Promise<string> {
    return this.serial(async () => {
      const canonical = await this.ports.canonical(path);
      for (const [id, folder] of this.folders) if (folder.path === canonical) return id;
      const id = this.ports.nextId();
      this.folders.set(id, { path: canonical, tabs: [], active: null });
      return id;
    });
  }
  openDocument(folderId: string, path: string): Promise<string> {
    return this.serial(async () => {
      const folder = this.folder(folderId);
      const canonical = await this.ports.canonical(path);
      const normalized = canonical.replaceAll("\\", "/");
      const root = folder.path.replaceAll("\\", "/");
      const prefix = root.endsWith("/") ? root : `${root}/`;
      if (
        !normalized.startsWith(prefix) ||
        canonical.split(/[\\/]/).includes("..") ||
        !canonical.endsWith(".vellum")
      )
        throw Error("invalid");
      let tab = folder.tabs.find((t) => t.path === canonical);
      if (!tab) {
        const state = await this.ports.runtime.open(canonical);
        tab = {
          id: this.ports.nextId(),
          path: canonical,
          name: this.ports.nameOf(canonical),
          ...state,
          revision: 0,
        };
        folder.tabs.push(tab);
      }
      folder.active = tab.id;
      return tab.id;
    });
  }
  folderPath(id: string) {
    return this.folder(id).path;
  }
  path(folder: string, tab: string) {
    return this.tab(folder, tab).path;
  }
  state(id: string) {
    const folder = this.folder(id);
    return {
      folderName: this.ports.nameOf(folder.path),
      active: folder.active,
      tabs: folder.tabs.map(({ id, name, dirty, running }) => ({ id, name, dirty, running })),
    };
  }
  activate(folder: string, tab: string) {
    this.tab(folder, tab);
    this.folder(folder).active = tab;
  }
  update(folder: string, id: string, state: Pick<DesktopTab, "dirty" | "running">) {
    const tab = this.tab(folder, id);
    Object.assign(tab, state);
    tab.revision++;
  }
  private async decide(tab: Tab, confirm: ConfirmClose): Promise<boolean> {
    if (!tab.dirty && !tab.running) return true;
    const revision = tab.revision;
    const choice = await confirm({
      id: tab.id,
      name: tab.name,
      dirty: tab.dirty,
      running: tab.running,
    });
    if (choice === "cancel" || tab.revision !== revision) return false;
    if (choice === "save") {
      await this.ports.runtime.save(tab.path);
      if (tab.revision !== revision) return false;
      tab.dirty = false;
    }
    return true;
  }
  closeTab(folderId: string, id: string, confirm: ConfirmClose): Promise<boolean> {
    return this.serial(async () => {
      const tab = this.tab(folderId, id);
      if (!(await this.decide(tab, confirm))) return false;
      await this.ports.runtime.close(tab.path);
      const folder = this.folder(folderId);
      const index = folder.tabs.indexOf(tab);
      folder.tabs.splice(index, 1);
      if (folder.active === id)
        folder.active = folder.tabs[Math.min(index, folder.tabs.length - 1)]?.id ?? null;
      return true;
    });
  }
  closeWindow(folderId: string, confirm: ConfirmClose): Promise<boolean> {
    return this.serial(async () => {
      const tabs = [...this.folder(folderId).tabs];
      for (const tab of tabs) if (!(await this.decide(tab, confirm))) return false;
      for (const tab of tabs) await this.ports.runtime.close(tab.path);
      this.folders.delete(folderId);
      return true;
    });
  }
}
