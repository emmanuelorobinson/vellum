/** Object storage and atomic snapshot publication. No public bucket or signed object URLs. */
import { decodeBundle, digest, MAX_BUNDLE } from "./bundle";

export * from "./bundle";
export const PACKAGE_NAME = "@vellum/storage";
export const CHUNK_SIZE = 512 * 1024;
export interface ObjectValue {
  bytes: Uint8Array;
  etag: string;
}
export interface ObjectStore {
  get(key: string): Promise<ObjectValue | null>;
  /** Conditional writes MUST be atomic. null means absent; an ETag means replace exactly that version. */
  put(key: string, bytes: Uint8Array, match: null | string): Promise<void>;
  list(prefix: string): Promise<string[]>;
  delete(key: string): Promise<void>;
}
export class ShareUnavailable extends Error {
  constructor() {
    super("Share unavailable");
  }
}
export class StorageConflict extends Error {
  constructor() {
    super("Snapshot changed; refresh and try again");
  }
}
export class MemoryObjects implements ObjectStore {
  private data = new Map<string, ObjectValue>();
  private version = 0;
  async get(key: string) {
    const v = this.data.get(key);
    return v ? { bytes: v.bytes.slice(), etag: v.etag } : null;
  }
  async put(key: string, bytes: Uint8Array, match: null | string) {
    if ((this.data.get(key)?.etag ?? null) !== match) throw new StorageConflict();
    this.data.set(key, { bytes: bytes.slice(), etag: String(++this.version) });
  }
  async list(prefix: string) {
    return [...this.data.keys()].filter((k) => k.startsWith(prefix)).sort();
  }
  async delete(key: string) {
    this.data.delete(key);
  }
}
interface Upload {
  id: string;
  size: number;
  sha256: string;
  started: number;
}
interface Snapshot {
  ownerName?: string;
  uploadId: string;
  at: number;
  name: string;
  incomplete: boolean;
  missing: string[];
  sha256: string;
  size: number;
}
interface RecordValue {
  id: string;
  owner: string;
  revoked: boolean;
  ownerName?: string;
  pending?: Upload;
  snapshot?: Snapshot;
}
export class ShareService {
  constructor(private ports: { objects: ObjectStore; clock: () => number; newId: () => string }) {}
  private id(value: string) {
    if (!/^[A-Za-z0-9_-]{24,128}$/.test(value)) throw new ShareUnavailable();
    return value;
  }
  private key(id: string) {
    return `shares/${this.id(id)}`;
  }
  private ownerKey(owner: string) {
    if (!owner || owner.length > 256) throw Error("Sign in required");
    return `owners/${digest(owner)}/`;
  }
  private async read(id: string, owner?: string) {
    const o = await this.ports.objects.get(this.key(id));
    if (!o) throw new ShareUnavailable();
    const r = JSON.parse(Buffer.from(o.bytes).toString()) as RecordValue;
    if (owner !== undefined && r.owner !== owner) throw new ShareUnavailable();
    return { r, etag: o.etag };
  }
  private write(r: RecordValue, etag: string | null) {
    return this.ports.objects.put(this.key(r.id), Buffer.from(JSON.stringify(r)), etag);
  }
  async begin(
    owner: string,
    input: { size: number; sha256: string; shareId?: string; ownerName?: string },
  ) {
    if (
      !Number.isSafeInteger(input.size) ||
      input.size <= 0 ||
      input.size > MAX_BUNDLE ||
      !/^[a-f0-9]{64}$/.test(input.sha256)
    )
      throw Error("Invalid upload");
    const prefix = this.ownerKey(owner),
      id = this.id(input.shareId ?? this.ports.newId()),
      uploadId = this.id(this.ports.newId());
    const old = input.shareId ? await this.read(id, owner) : null;
    if (old?.r.revoked || (old?.r.pending && old.r.pending.started + 86400000 > this.ports.clock()))
      throw new StorageConflict();
    if (!old && (await this.ports.objects.list(prefix)).length >= 1000)
      throw Error("Share limit reached");
    const r: RecordValue = old?.r ?? {
      id,
      owner,
      revoked: false,
      ownerName: typeof input.ownerName === "string" ? input.ownerName.slice(0, 256) : "",
    };
    r.pending = {
      id: uploadId,
      size: input.size,
      sha256: input.sha256,
      started: this.ports.clock(),
    };
    // Index first: a failed creation is invisible to list; repeat uses a fresh id.
    if (!old) await this.ports.objects.put(`${prefix}${id}`, Buffer.from(id), null);
    await this.write(r, old?.etag ?? null);
    return { id, uploadId, chunkSize: CHUNK_SIZE };
  }
  async status(owner: string, id: string) {
    const { r } = await this.read(id, owner),
      received: number[] = [];
    if (r.pending && !r.revoked) {
      const prefix = `chunks/${r.pending.id}/`,
        keys = new Set(await this.ports.objects.list(prefix));
      for (let i = 0; i < Math.ceil(r.pending.size / CHUNK_SIZE); i++)
        if (keys.has(`${prefix}${i}`)) received.push(i);
    }
    return {
      id: r.id,
      revoked: r.revoked,
      snapshot: r.snapshot ?? null,
      upload:
        r.revoked || (r.pending && r.pending.started + 86400000 <= this.ports.clock())
          ? null
          : (r.pending ?? null),
      received,
    };
  }
  async chunk(owner: string, id: string, uploadId: string, index: number, bytes: Uint8Array) {
    this.id(uploadId);
    const { r } = await this.read(id, owner),
      p = r.pending;
    if (
      r.revoked ||
      !p ||
      p.id !== uploadId ||
      p.started + 86400000 <= this.ports.clock() ||
      !Number.isInteger(index) ||
      index < 0 ||
      index >= Math.ceil(p.size / CHUNK_SIZE) ||
      bytes.length !== Math.min(CHUNK_SIZE, p.size - index * CHUNK_SIZE)
    )
      throw Error("Invalid upload chunk");
    const key = `chunks/${uploadId}/${index}`,
      old = await this.ports.objects.get(key);
    if (old) {
      if (digest(old.bytes) !== digest(bytes)) throw new StorageConflict();
      return;
    }
    try {
      await this.ports.objects.put(key, bytes, null);
    } catch (e) {
      const current = await this.ports.objects.get(key);
      if (!current || digest(current.bytes) !== digest(bytes)) throw e;
    }
  }
  async publish(owner: string, id: string, uploadId: string) {
    this.id(uploadId);
    const { r, etag } = await this.read(id, owner);
    if (r.revoked) throw new ShareUnavailable();
    if (r.snapshot?.uploadId === uploadId) return { id, snapshot: r.snapshot };
    const p = r.pending;
    if (!p || p.id !== uploadId || p.started + 86400000 <= this.ports.clock())
      throw Error("Upload unavailable");
    const parts: Uint8Array[] = [];
    for (let i = 0; i < Math.ceil(p.size / CHUNK_SIZE); i++) {
      const o = await this.ports.objects.get(`chunks/${uploadId}/${i}`);
      if (!o) throw Error("Upload incomplete");
      parts.push(o.bytes);
    }
    const bytes = Buffer.concat(parts);
    if (bytes.length !== p.size || digest(bytes) !== p.sha256)
      throw Error("Snapshot checksum mismatch");
    const bundle = decodeBundle(bytes),
      doc = JSON.parse(
        Buffer.from(
          bundle.files.find((f) => f.path === bundle.entry)?.data ?? "",
          "base64",
        ).toString(),
      );
    const key = `snapshots/${uploadId}`,
      old = await this.ports.objects.get(key);
    if (!old) await this.ports.objects.put(key, bytes, null);
    else if (digest(old.bytes) !== p.sha256) throw new StorageConflict();
    r.snapshot = {
      ownerName: r.ownerName,
      uploadId,
      at: this.ports.clock(),
      name: doc.meta.name,
      incomplete: bundle.missing.length > 0,
      missing: bundle.missing,
      sha256: p.sha256,
      size: p.size,
    };
    delete r.pending;
    await this.write(r, etag);
    return { id, snapshot: r.snapshot };
  }
  async revoke(owner: string, id: string) {
    const { r, etag } = await this.read(id, owner);
    if (r.revoked) return;
    r.revoked = true;
    delete r.pending;
    await this.write(r, etag);
  }
  async list(owner: string) {
    const out = [];
    for (const key of await this.ports.objects.list(this.ownerKey(owner))) {
      const id = key.split("/").at(-1);
      if (!id) throw Error("Invalid share index");
      try {
        const { r } = await this.read(id, owner);
        out.push({ id: r.id, revoked: r.revoked, snapshot: r.snapshot ?? null });
      } catch (e) {
        if (!(e instanceof ShareUnavailable)) throw e;
      }
    }
    return out;
  }
  async view(id: string) {
    const { r } = await this.read(id);
    if (r.revoked || !r.snapshot) throw new ShareUnavailable();
    const object = await this.ports.objects.get(`snapshots/${r.snapshot.uploadId}`);
    if (!object || digest(object.bytes) !== r.snapshot.sha256) throw Error("Snapshot unavailable");
    return { snapshot: r.snapshot, bytes: object.bytes };
  }
}
