/** Custom-editor lifecycle. Serialization, merge, diff and every change remain in the Rust runtime. */
export interface FilePort {
  read(uri: string): Promise<Uint8Array>;
  writeAtomic(uri: string, bytes: Uint8Array): Promise<void>;
  delete(uri: string): Promise<void>;
}
export interface DocumentRuntime {
  readonly editable: boolean;
  revision: number;
  snapshot(): Promise<{ bytes: Uint8Array; revision: number }>;
  /** Apply a diff/three-way merge through core Ops, never replace a DocStore directly. */
  replace(
    bytes: Uint8Array,
    reason: "revert" | "external" | "recovery",
  ): Promise<{ revision: number }>;
  subscribe(listener: (change: { revision: number }) => void): { dispose(): void };
  dispose(): void;
}
type Cancelled = () => boolean;
const never = () => false;
const equal = (a: Uint8Array | null, b: Uint8Array) =>
  a !== null && a.length === b.length && a.every((value, i) => value === b[i]);
export class DesignDocument {
  private savedRevision: number;
  private lastDisk: Uint8Array | null = null;
  private disposed = false;
  private pending: Promise<unknown> = Promise.resolve();
  private listeners = new Set<() => void>();
  private subscription: { dispose(): void };
  get dirty() {
    return this.runtime.revision !== this.savedRevision;
  }
  constructor(
    readonly uri: string,
    readonly runtime: DocumentRuntime,
    private readonly files: FilePort,
    initialBytes?: Uint8Array,
  ) {
    this.savedRevision = runtime.revision;
    this.lastDisk = initialBytes?.slice() ?? null;
    this.subscription = runtime.subscribe(() => this.notify());
  }
  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return {
      dispose: () => {
        this.listeners.delete(listener);
      },
    };
  }
  private notify() {
    for (const listener of this.listeners) listener();
  }
  private check(cancelled: Cancelled) {
    if (this.disposed) throw Error("disposed");
    if (cancelled()) throw Error("cancelled");
  }
  private run<T>(cancelled: Cancelled, action: () => Promise<T>): Promise<T> {
    const next = this.pending.then(() => {
      this.check(cancelled);
      return action();
    });
    this.pending = next.catch(() => {});
    return next;
  }
  save(cancelled = never): Promise<void> {
    return this.write(this.uri, true, cancelled);
  }
  saveAs(uri: string, cancelled = never): Promise<void> {
    return this.write(uri, false, cancelled);
  }
  private write(uri: string, save: boolean, cancelled: Cancelled): Promise<void> {
    return this.run(cancelled, async () => {
      if ((save || uri === this.uri) && !this.runtime.editable) throw Error("read-only");
      const snapshot = await this.runtime.snapshot();
      this.check(cancelled);
      await this.files.writeAtomic(uri, snapshot.bytes);
      if (save) {
        this.savedRevision = snapshot.revision;
        this.lastDisk = snapshot.bytes.slice();
        this.notify();
      }
    });
  }
  revert(cancelled = never): Promise<void> {
    return this.run(cancelled, async () => {
      const bytes = await this.files.read(this.uri);
      this.check(cancelled);
      const applied = await this.runtime.replace(bytes, "revert");
      this.savedRevision = applied.revision;
      this.lastDisk = bytes.slice();
      this.notify();
    });
  }
  external(): Promise<void> {
    return this.run(never, async () => {
      const bytes = await this.files.read(this.uri);
      if (equal(this.lastDisk, bytes)) return;
      await this.runtime.replace(bytes, "external");
      const current = await this.runtime.snapshot();
      this.lastDisk = bytes.slice();
      if (equal(bytes, current.bytes)) this.savedRevision = current.revision;
      this.notify();
    });
  }
  restore(id: string, cancelled = never): Promise<void> {
    return this.run(cancelled, async () => {
      const bytes = await this.files.read(id);
      this.check(cancelled);
      await this.runtime.replace(bytes, "recovery");
      // Recovery is unsaved even when the engine starts its revision counter at zero.
      this.savedRevision = -1;
      this.notify();
    });
  }
  backup(uri: string, cancelled = never) {
    return this.run(cancelled, async () => {
      const snapshot = await this.runtime.snapshot();
      this.check(cancelled);
      await this.files.writeAtomic(uri, snapshot.bytes);
      return { id: uri, delete: () => this.files.delete(uri) };
    });
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.subscription.dispose();
    this.runtime.dispose();
    this.listeners.clear();
  }
}
