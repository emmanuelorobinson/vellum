/** Portable, inert snapshot files. Scripts are downloaded as bytes and never evaluated by the viewer. */
import { createHash } from "node:crypto";
import { brand } from "@vellum/config";
export const MAX_BUNDLE = 64 * 1024 * 1024;
export interface BundleFrame {
  id: string;
  name: string;
  width: number;
  height: number;
  displayList: string;
  css?: string;
  spec?: Record<string, unknown> | null;
  fonts: { handle: string; index: number; path: string }[];
  images: { id: string; path: string }[];
}
export interface Bundle {
  schema: 1;
  entry: string;
  files: { path: string; data: string }[];
  missing: string[];
  frames: BundleFrame[];
}
export const digest = (bytes: Uint8Array | string) =>
  createHash("sha256").update(bytes).digest("hex");
export function safePath(path: unknown): path is string {
  return (
    typeof path === "string" &&
    path.length > 0 &&
    path.length <= 1024 &&
    !/[\\:]/.test(path) &&
    ![...path].some((c) => c.charCodeAt(0) < 32 || c.charCodeAt(0) === 127) &&
    path
      .split("/")
      .every(
        (p) =>
          p.length > 0 &&
          p !== "." &&
          p !== ".." &&
          !p.startsWith(".") &&
          !/[. ]$/.test(p) &&
          !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(p),
      )
  );
}
export function decodeBundle(bytes: Uint8Array): Bundle {
  if (bytes.length > MAX_BUNDLE) throw Error("Snapshot too large");
  const b = JSON.parse(Buffer.from(bytes).toString("utf8")) as Bundle;
  if (
    b?.schema !== 1 ||
    !safePath(b.entry) ||
    !b.entry.endsWith(".vellum") ||
    !Array.isArray(b.files) ||
    b.files.length > 2048 ||
    !Array.isArray(b.missing) ||
    b.missing.length > 2048 ||
    !Array.isArray(b.frames) ||
    b.frames.length > 100
  )
    throw Error("Invalid snapshot");
  const paths = new Set<string>();
  let size = 0;
  for (const f of b.files) {
    if (
      !safePath(f.path) ||
      paths.has(f.path.toLowerCase()) ||
      typeof f.data !== "string" ||
      Buffer.from(f.data, "base64").toString("base64") !== f.data
    )
      throw Error("Invalid snapshot file");
    size += Buffer.byteLength(f.data, "base64");
    paths.add(f.path.toLowerCase());
  }
  if (size > MAX_BUNDLE || !b.files.some((f) => f.path === b.entry) || !b.missing.every(safePath))
    throw Error("Invalid snapshot files");
  const frameIds = new Set<string>();
  for (const f of b.frames) {
    if (
      frameIds.has(f.id) ||
      (f.css !== undefined && typeof f.css !== "string") ||
      (f.spec !== undefined &&
        f.spec !== null &&
        (typeof f.spec !== "object" || Array.isArray(f.spec)))
    )
      throw Error("Invalid snapshot inspection");
    frameIds.add(f.id);
    if (
      typeof f.id !== "string" ||
      f.id.length > 1024 ||
      typeof f.name !== "string" ||
      f.name.length > 1024 ||
      !Number.isFinite(f.width) ||
      !Number.isFinite(f.height) ||
      f.width <= 0 ||
      f.height <= 0 ||
      f.width > 8192 ||
      f.height > 8192 ||
      f.width * f.height > 16_777_216 ||
      !Array.isArray(f.fonts) ||
      !Array.isArray(f.images) ||
      !paths.has(f.displayList?.toLowerCase())
    )
      throw Error("Invalid snapshot frame");
    for (const r of [...f.fonts, ...f.images])
      if (!safePath(r.path) || !paths.has(r.path.toLowerCase()))
        throw Error("Invalid snapshot resource");
    for (const r of f.fonts)
      if (
        !/^\d{1,20}$/.test(r.handle) ||
        BigInt(r.handle) > 0xffffffffffffffffn ||
        !Number.isInteger(r.index) ||
        r.index < 0 ||
        r.index > 0xffffffff
      )
        throw Error("Invalid font");
    for (const r of f.images)
      if (typeof r.id !== "string" || !/^[a-zA-Z0-9_-]{1,128}$/.test(r.id))
        throw Error("Invalid image");
  }
  const entry = b.files.find((f) => f.path === b.entry);
  if (!entry) throw Error("Snapshot entry unavailable");
  const doc = JSON.parse(Buffer.from(entry.data, "base64").toString("utf8"));
  if (
    !Number.isInteger(doc?.meta?.formatVersion) ||
    !Array.isArray(doc.pages) ||
    typeof doc.meta.name !== "string" ||
    doc.meta.name.length > 1024
  )
    throw Error("Invalid document");
  return b;
}
export function encodeBundle(bundle: Bundle): Buffer {
  const bytes = Buffer.from(JSON.stringify(bundle));
  decodeBundle(bytes);
  return bytes;
}
// ZIP method 0: exact file bytes, fixed DOS timestamp, UTF-8 paths, no decompression or code execution.
function crc32(bytes: Uint8Array) {
  let crc = 0xffffffff;
  for (const b of bytes) {
    crc ^= b;
    for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
export function zipBundle(bundle: Bundle): Buffer {
  decodeBundle(Buffer.from(JSON.stringify(bundle)));
  const local: Buffer[] = [],
    central: Buffer[] = [];
  let offset = 0;
  for (const f of [...bundle.files, { path: brand.shareRootMarker, data: "" }]) {
    const name = Buffer.from(f.path),
      data = Buffer.from(f.data, "base64"),
      crc = crc32(data);
    const h = Buffer.alloc(30);
    h.writeUInt32LE(0x04034b50);
    h.writeUInt16LE(20, 4);
    h.writeUInt16LE(0x800, 6);
    h.writeUInt16LE(33, 12);
    h.writeUInt32LE(crc, 14);
    h.writeUInt32LE(data.length, 18);
    h.writeUInt32LE(data.length, 22);
    h.writeUInt16LE(name.length, 26);
    const c = Buffer.alloc(46);
    c.writeUInt32LE(0x02014b50);
    c.writeUInt16LE(20, 4);
    c.writeUInt16LE(20, 6);
    c.writeUInt16LE(0x800, 8);
    c.writeUInt16LE(33, 14);
    c.writeUInt32LE(crc, 16);
    c.writeUInt32LE(data.length, 20);
    c.writeUInt32LE(data.length, 24);
    c.writeUInt16LE(name.length, 28);
    c.writeUInt32LE(offset, 42);
    local.push(h, name, data);
    central.push(c, name);
    offset += h.length + name.length + data.length;
  }
  const directory = Buffer.concat(central),
    end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50);
  end.writeUInt16LE(bundle.files.length + 1, 8);
  end.writeUInt16LE(bundle.files.length + 1, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...local, directory, end]);
}
