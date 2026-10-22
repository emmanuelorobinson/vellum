/**
 * The document engine in the shared emscripten module (crates/vellum-wasm `abi.rs`, ADR-033): the Rust core
 * behind `@vellum/engine-client`'s `EngineBindings`, and the content source the canvas draws from. Every
 * change goes through the Rust write path; this file only moves bytes and JSON across the ABI.
 */
import type {
  EngineBindings,
  EngineCommit,
  EngineSnapshot,
  ToolScene,
  ToolTransaction,
} from "@vellum/engine-client";
import type { Frame } from "@vellum/renderer-api";
import type { Actor } from "@vellum/types-ts";
import type { SkiaModule } from "./index";

/** The engine exports of the editor module (`vellum-editor`); the viewer module has none of them. */
export interface EngineModule extends SkiaModule {
  _vellum_engine_open(options: number, file: number, updates: number): number;
  _vellum_engine_close(handle: number): void;
  _vellum_engine_call(handle: number, request: number): number;
  _vellum_engine_font(handle: number, bytes: number): number;
  _vellum_engine_encode(handle: number): number;
  _vellum_engine_cursor(handle: number): number;
  _vellum_engine_updates(handle: number, cursor: number): number;
  _vellum_engine_import(handle: number, updates: number): number;
  _vellum_engine_frame(handle: number, request: number): number;
}

export function isEngineModule(module: SkiaModule): module is EngineModule {
  return typeof (module as Partial<EngineModule>)._vellum_engine_open === "function";
}

export interface OpenOptions {
  /** The `.vellum` bytes, or the daemon's genesis document when `updates` is given. */
  file: Uint8Array;
  /** Everything the daemon's engine has since its genesis document (a replica, ADR-033). */
  updates?: Uint8Array;
  /** The document's path: the scope its imports and scripts resolve against. */
  path: string;
  readOnly: boolean;
  /** Font faces to shape and draw with (Inter first). The host loads them; the engine never reads files. */
  fonts: readonly Uint8Array[];
  /** Random bits for this replica's sync peer and new ids. Defaults to `crypto.getRandomValues`. */
  random?: () => bigint;
}

/** The canvas's view of the engine: what `CanvasHost` draws (editor-react's `ContentSource`). */
export interface ContentSource {
  frame(view: {
    width: number;
    height: number;
    dpr: number;
    viewport: { x: number; y: number; zoom: number };
  }): Frame | null;
  subscribe(listener: () => void): { dispose(): void };
}

const random64 = () => {
  const words = new Uint32Array(2);
  crypto.getRandomValues(words);
  return (BigInt(words[0] ?? 0) << 32n) | BigInt(words[1] ?? 0);
};

export class WasmDocumentEngine implements EngineBindings {
  private disposed = false;
  private listeners = new Set<(snapshot: EngineSnapshot) => void>();
  private cached: EngineSnapshot | null = null;
  /** Font bytes by the handles frames name them by, so a frame does not copy them out of the module. */
  private fontBytes = new Map<bigint, Uint8Array>();
  readonly scene: ToolScene;
  readonly content: ContentSource;
  get state() {
    return this.disposed ? ("disposed" as const) : ("ready" as const);
  }

  private constructor(
    private readonly module: EngineModule,
    private readonly handle: number,
  ) {
    const call = <T>(request: Record<string, unknown>) => this.call<T>(request);
    this.scene = {
      get: (address) => call({ call: "get", address }) ?? undefined,
      roots: () => call({ call: "roots" }),
      hitPoint: (point, depth) => call({ call: "hitPoint", point, depth }),
      hitRect: (box, depth, mode) => call({ call: "hitRect", box, depth, mode }),
      snap: (box, exclude, options) => call({ call: "snap", box, exclude, options }),
      toParent: (target, point) => call({ call: "toParent", target, point }),
      toLocal: (address, point) => call({ call: "toLocal", address, point }),
      path: (address) => call({ call: "path", address }),
    };
    this.content = {
      frame: (view) => this.frame(view),
      subscribe: (listener) => this.subscribe(() => listener()),
    };
  }

  static open(module: EngineModule, options: OpenOptions): WasmDocumentEngine {
    const random = options.random ?? random64;
    // A peer below 2^63 and never 0 (Loro reserves the top bit for a document's genesis).
    const peer = random() & 0x7fff_ffff_ffff_ffffn || 1n;
    const request = new TextEncoder().encode(
      JSON.stringify({
        path: options.path,
        peer: peer.toString(),
        seed: random().toString(),
        readOnly: options.readOnly,
      }),
    );
    const upload = (bytes: Uint8Array) => WasmDocumentEngine.upload(module, bytes);
    const handle = module._vellum_engine_open(
      upload(request),
      upload(options.file),
      options.updates ? upload(options.updates) : 0,
    );
    if (!handle)
      throw Error(WasmDocumentEngine.error(module) || "The document could not be opened");
    const engine = new WasmDocumentEngine(module, handle);
    for (const font of options.fonts) engine.registerFont(font);
    return engine;
  }

  private static upload(module: EngineModule, bytes: Uint8Array): number {
    if (bytes.byteLength > 64 * 1024 * 1024) throw Error("Engine input too large");
    const pointer = module._vellum_alloc(bytes.byteLength);
    if (!pointer) throw Error("Engine allocation failed");
    module.HEAPU8.set(bytes, pointer); // Read HEAPU8 again: allocating may grow memory.
    return pointer;
  }

  private static error(module: EngineModule): string {
    const pointer = module._vellum_error();
    if (!pointer) return "";
    try {
      return new TextDecoder().decode(
        module.HEAPU8.slice(pointer, pointer + module._vellum_buffer_len(pointer)),
      );
    } finally {
      module._vellum_free(pointer);
    }
  }

  private live() {
    if (this.disposed) throw Error("Engine disposed");
  }

  private take(pointer: number): Uint8Array {
    if (!pointer) throw Error(WasmDocumentEngine.error(this.module) || "Engine call failed");
    try {
      return this.module.HEAPU8.slice(pointer, pointer + this.module._vellum_buffer_len(pointer));
    } finally {
      this.module._vellum_free(pointer);
    }
  }

  private json(bytes: Record<string, unknown>): number {
    return WasmDocumentEngine.upload(this.module, new TextEncoder().encode(JSON.stringify(bytes)));
  }

  private call<T>(request: Record<string, unknown>): T {
    this.live();
    const bytes = this.take(this.module._vellum_engine_call(this.handle, this.json(request)));
    return JSON.parse(new TextDecoder().decode(bytes)) as T;
  }

  private changed() {
    this.cached = null;
    if (!this.listeners.size) return;
    const snapshot = this.snapshot();
    for (const listener of [...this.listeners]) listener(snapshot);
  }

  snapshot(): EngineSnapshot {
    this.cached ??= this.call<EngineSnapshot>({ call: "snapshot" });
    return this.cached;
  }

  private commit(request: Record<string, unknown>): EngineCommit {
    const result = this.call<EngineCommit>(request);
    if (result.status === "applied") this.changed();
    return result;
  }

  apply(transaction: ToolTransaction): EngineCommit {
    return this.commit({ call: "apply", transaction });
  }

  undo(actor: Actor): EngineCommit {
    return this.commit({ call: "undo", actor });
  }

  redo(actor: Actor): EngineCommit {
    return this.commit({ call: "redo", actor });
  }

  subscribe(listener: (snapshot: EngineSnapshot) => void) {
    this.live();
    this.listeners.add(listener);
    return { dispose: () => void this.listeners.delete(listener) };
  }

  /** The page the person is on; the scene answers for it. */
  setPage(page: string) {
    this.call({ call: "setPage", page });
    this.changed();
  }

  setReadOnly(readOnly: boolean) {
    this.call({ call: "setReadOnly", readOnly });
    this.changed();
  }

  /** A fresh node id from the engine's generator, for the editor's tools. */
  nextId(): string {
    return this.call<string>({ call: "nextId" });
  }

  registerFont(bytes: Uint8Array) {
    this.live();
    const handles = JSON.parse(
      new TextDecoder().decode(
        this.take(
          this.module._vellum_engine_font(
            this.handle,
            WasmDocumentEngine.upload(this.module, bytes),
          ),
        ),
      ),
    ) as string[];
    for (const handle of handles) this.fontBytes.set(BigInt(handle), bytes);
    this.changed();
  }

  /** The canonical `.vellum` bytes of the document as it is now (what Save writes). */
  encode(): Uint8Array {
    this.live();
    return this.take(this.module._vellum_engine_encode(this.handle));
  }

  /** How much of the document this replica has seen. */
  cursor(): Uint8Array {
    this.live();
    return this.take(this.module._vellum_engine_cursor(this.handle));
  }

  /** What a peer that has seen `cursor` lacks: this person's edits, for the daemon. */
  updatesSince(cursor: Uint8Array): Uint8Array {
    this.live();
    return this.take(
      this.module._vellum_engine_updates(
        this.handle,
        WasmDocumentEngine.upload(this.module, cursor),
      ),
    );
  }

  /** Imports the daemon's updates (agents', other editors' and the file's changes). */
  import(updates: Uint8Array): boolean {
    this.live();
    if (!updates.byteLength) return false;
    const status = this.module._vellum_engine_import(
      this.handle,
      WasmDocumentEngine.upload(this.module, updates),
    );
    if (!status) throw Error(WasmDocumentEngine.error(this.module) || "Updates were refused");
    if (status === 2) this.changed();
    return status === 2;
  }

  frame(view: Parameters<ContentSource["frame"]>[0]): Frame | null {
    this.live();
    let bytes: Uint8Array;
    try {
      bytes = this.take(this.module._vellum_engine_frame(this.handle, this.json(view)));
    } catch {
      return null;
    }
    const length = new DataView(bytes.buffer, bytes.byteOffset, 4).getUint32(0, true);
    const header = JSON.parse(new TextDecoder().decode(bytes.subarray(4, 4 + length))) as {
      fonts: { handle: string; index: number }[];
    };
    return {
      zoom: view.viewport.zoom,
      displayList: bytes.subarray(4 + length),
      fonts: header.fonts.flatMap(({ handle, index }) => {
        const id = BigInt(handle);
        const font = this.fontBytes.get(id);
        return font ? [{ handle: id, index, bytes: font }] : [];
      }),
      images: [],
    };
  }

  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.listeners.clear();
    this.module._vellum_engine_close(this.handle);
  }
}
