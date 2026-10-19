import type { HomeData, HomeFile } from "@vellum/host-electron/bridge";

interface FileRecord extends HomeFile {
  path: string;
}
export interface LibraryData {
  version: 1;
  files: FileRecord[];
  folders: { id: string; path: string }[];
}
interface Ports {
  load(): Promise<LibraryData | null>;
  persist(data: LibraryData): Promise<void>;
  canonical(path: string): Promise<string>;
  inspect(path: string): Promise<{ created: number; modified: number }>;
  folderOf(path: string): string;
  nameOf(path: string): string;
  draftPath(id: string): string;
  now(): number;
  nextId(): string;
  create(path: string): Promise<void>;
  move(from: string, to: string): Promise<void>;
  remove(path: string): Promise<void>;
}
/** Only native file references and metadata. Document bytes remain owned by core. */
export class DesktopLibrary {
  private data: LibraryData = { version: 1, files: [], folders: [] };
  private queue: Promise<unknown>;
  constructor(private readonly ports: Ports) {
    this.queue = ports.load().then((data) => {
      if (data) this.data = data;
    });
  }
  async ready() {
    await this.queue;
  }
  private serial<T>(run: () => Promise<T>): Promise<T> {
    const result = this.queue.then(run);
    this.queue = result.catch(() => {});
    return result;
  }
  private async commit(data: LibraryData) {
    await this.ports.persist(data);
    this.data = data;
  }
  private file(id: string) {
    const file = this.data.files.find((f) => f.id === id);
    if (!file) throw Error("invalid");
    return file;
  }
  path(id: string) {
    return this.file(id).path;
  }
  folderPath(id: string) {
    const folder = this.data.folders.find((f) => f.id === id);
    if (!folder) throw Error("invalid");
    return folder.path;
  }
  view(): HomeData {
    return {
      files: this.data.files.map(
        ({ id, name, folder, kind, created, opened, branch, running }) => ({
          id,
          name,
          folder,
          kind,
          created,
          opened,
          branch,
          running,
        }),
      ),
      folders: this.data.folders.map((f) => ({ id: f.id, label: f.path })),
    };
  }
  opened(path: string) {
    return this.serial(async () => {
      const canonical = await this.ports.canonical(path);
      if (!canonical.endsWith(".vellum")) throw Error("invalid");
      const existing = this.data.files.find((f) => f.path === canonical);
      const info = await this.ports.inspect(canonical);
      const id = existing?.id ?? this.ports.nextId();
      const file: FileRecord = {
        id,
        path: canonical,
        name: existing?.kind === "draft" ? existing.name : this.ports.nameOf(canonical),
        folder: this.ports.folderOf(canonical),
        kind: existing?.kind ?? "file",
        created: existing?.created ?? info.created,
        opened: this.ports.now(),
        branch: null,
        running: null,
      };
      const folder = this.ports.folderOf(canonical);
      let recentCount = 0;
      await this.commit({
        version: 1,
        files: [file, ...this.data.files.filter((f) => f.id !== id)].filter(
          (f) => f.kind === "draft" || recentCount++ < 200,
        ),
        folders:
          file.kind === "draft"
            ? this.data.folders
            : [
                this.data.folders.find((f) => f.path === folder) ?? {
                  id: this.ports.nextId(),
                  path: folder,
                },
                ...this.data.folders.filter((f) => f.path !== folder),
              ].slice(0, 20),
      });
      return id;
    });
  }
  newDraft() {
    return this.serial(async () => {
      if (this.data.files.filter((f) => f.kind === "draft").length >= 200)
        throw Error("draft limit reached");
      const id = this.ports.nextId();
      const path = this.ports.draftPath(id);
      await this.ports.create(path);
      const now = this.ports.now();
      await this.commit({
        ...this.data,
        files: [
          {
            id,
            path,
            name: "Untitled",
            folder: this.ports.folderOf(path),
            kind: "draft",
            created: now,
            opened: now,
            branch: null,
            running: null,
          },
          ...this.data.files,
        ],
      });
      return id;
    });
  }
  forget(ids: string[]) {
    return this.serial(async () => {
      for (const id of ids) if (this.file(id).kind === "draft") throw Error("invalid");
      await this.commit({
        ...this.data,
        files: this.data.files.filter((f) => !ids.includes(f.id)),
      });
    });
  }
  deleteDraft(id: string, confirmed: boolean) {
    return this.serial(async () => {
      const file = this.file(id);
      if (file.kind !== "draft") throw Error("invalid");
      if (!confirmed) return false;
      await this.ports.remove(file.path);
      await this.commit({ ...this.data, files: this.data.files.filter((f) => f.id !== id) });
      return true;
    });
  }
  saveAs(id: string, destination: string) {
    return this.serial(async () => {
      const file = this.file(id);
      if (file.kind !== "draft" || !destination.endsWith(".vellum")) throw Error("invalid");
      await this.ports.move(file.path, destination);
      const folder = this.ports.folderOf(destination);
      await this.commit({
        ...this.data,
        files: this.data.files.map((f) =>
          f.id !== id
            ? f
            : {
                ...f,
                path: destination,
                folder,
                name: this.ports.nameOf(destination),
                kind: "file",
              },
        ),
      });
      await this.ports.remove(file.path);
    });
  }
}
