// TEST DATA. Memory boundaries; no network or account credentials.
import { describe, expect, it } from "vitest";
import {
  type Bundle,
  decodeBundle,
  digest,
  encodeBundle,
  MemoryObjects,
  ShareService,
  zipBundle,
} from "./index";

const document = {
  meta: { formatVersion: 1, name: "Fixture", createdWith: "TEST DATA" },
  pages: [],
};
const bundle: Bundle = {
  schema: 1 as const,
  entry: "design/app.vellum",
  files: [
    { path: "design/app.vellum", data: Buffer.from(JSON.stringify(document)).toString("base64") },
  ],
  missing: [],
  frames: [],
};
function setup(objects = new MemoryObjects()) {
  let n = 0;
  return new ShareService({
    objects,
    clock: () => 1000,
    newId: () => `id_${String(++n).padStart(32, "0")}`,
  });
}
async function upload(s: ShareService, owner = "owner", shareId?: string, value = bundle) {
  const bytes = encodeBundle(value);
  const pending = await s.begin(owner, { size: bytes.length, sha256: digest(bytes), shareId });
  await s.chunk(owner, pending.id, pending.uploadId, 0, bytes);
  return s.publish(owner, pending.id, pending.uploadId);
}
describe("share snapshots", () => {
  it("round-trips exact files through resumable upload, public view and a portable ZIP", async () => {
    const objects = new MemoryObjects(),
      s = setup(objects),
      bytes = encodeBundle(bundle);
    const p = await s.begin("owner", { size: bytes.length, sha256: digest(bytes) });
    await expect(s.publish("owner", p.id, p.uploadId)).rejects.toThrow();
    await s.chunk("owner", p.id, p.uploadId, 0, bytes);
    const restarted = setup(objects);
    expect((await restarted.status("owner", p.id)).received).toEqual([0]);
    await restarted.publish("owner", p.id, p.uploadId);
    const view = await restarted.view(p.id);
    expect(decodeBundle(view.bytes)).toEqual(bundle);
    expect(zipBundle(bundle).subarray(0, 4)).toEqual(Buffer.from([80, 75, 3, 4]));
    expect(
      JSON.parse(Buffer.from(decodeBundle(view.bytes).files[0]?.data ?? "", "base64").toString()),
    ).toEqual(document);
  });
  it("updates the same link atomically; revocation defeats an in-flight update", async () => {
    const s = setup(),
      a = await upload(s),
      old = await s.view(a.id);
    const bytes = encodeBundle({ ...bundle, missing: ["fonts/missing.woff2"] });
    const p = await s.begin("owner", { shareId: a.id, size: bytes.length, sha256: digest(bytes) });
    expect((await s.view(a.id)).bytes).toEqual(old.bytes);
    await s.chunk("owner", a.id, p.uploadId, 0, bytes);
    await s.revoke("owner", a.id);
    await expect(s.publish("owner", a.id, p.uploadId)).rejects.toThrow();
    await expect(s.view(a.id)).rejects.toThrow();
    expect((await s.list("owner"))[0]?.revoked).toBe(true);
  });
  it("marks missing files, hides private owner IDs and refuses cross-owner writes", async () => {
    const s = setup(),
      a = await upload(s, "owner", undefined, { ...bundle, missing: ["assets/missing"] });
    expect((await s.view(a.id)).snapshot.incomplete).toBe(true);
    expect((await s.view(a.id)).snapshot).not.toHaveProperty("owner");
    await expect(s.revoke("other", a.id)).rejects.toThrow();
    await expect(s.status("other", a.id)).rejects.toThrow();
    expect(await s.list("other")).toEqual([]);
  });
  it("checks whole-upload digest, immutable chunks, offsets and safe bundle paths", async () => {
    const s = setup(),
      bytes = encodeBundle(bundle),
      p = await s.begin("owner", { size: bytes.length, sha256: digest(bytes) });
    await expect(s.chunk("owner", p.id, p.uploadId, 1, bytes)).rejects.toThrow();
    await s.chunk("owner", p.id, p.uploadId, 0, bytes);
    await s.chunk("owner", p.id, p.uploadId, 0, bytes);
    await expect(
      s.chunk("owner", p.id, p.uploadId, 0, new Uint8Array(bytes.length)),
    ).rejects.toThrow();
    for (const path of ["../secret", "/root", "a\\b", ".git/config", "A/../../b", "a\u0000b"])
      expect(() => encodeBundle({ ...bundle, files: [{ path, data: "" }] })).toThrow();
    const q = await s.begin("owner", { size: bytes.length, sha256: "a".repeat(64) });
    await s.chunk("owner", q.id, q.uploadId, 0, bytes);
    await expect(s.publish("owner", q.id, q.uploadId)).rejects.toThrow();
  });
  it("publishes idempotently and allows exactly one concurrent update", async () => {
    const s = setup(),
      bytes = encodeBundle(bundle),
      p = await s.begin("owner", { size: bytes.length, sha256: digest(bytes) });
    await s.chunk("owner", p.id, p.uploadId, 0, bytes);
    await s.publish("owner", p.id, p.uploadId);
    await s.publish("owner", p.id, p.uploadId);
    const results = await Promise.allSettled([
      s.begin("owner", { shareId: p.id, size: bytes.length, sha256: digest(bytes) }),
      s.begin("owner", { shareId: p.id, size: bytes.length, sha256: digest(bytes) }),
    ]);
    expect(results.filter((x) => x.status === "fulfilled")).toHaveLength(1);
  });
});

it("replaces a published snapshot at the same link and resumes after pending expiry", async () => {
  let now = 0,
    n = 0;
  const s = new ShareService({
    objects: new MemoryObjects(),
    clock: () => now,
    newId: () => `id_${String(++n).padStart(32, "0")}`,
  });
  const first = await upload(s);
  const before = await s.view(first.id);
  const next: Bundle = {
    ...bundle,
    files: [
      {
        path: bundle.entry,
        data: Buffer.from(
          JSON.stringify({ ...document, meta: { ...document.meta, name: "Changed fixture" } }),
        ).toString("base64"),
      },
    ],
  };
  const updated = await upload(s, "owner", first.id, next);
  expect(updated.id).toBe(first.id);
  expect((await s.view(first.id)).snapshot.name).toBe("Changed fixture");
  expect((await s.view(first.id)).bytes).not.toEqual(before.bytes);
  const bytes = encodeBundle(bundle);
  await s.begin("owner", { shareId: first.id, size: bytes.length, sha256: digest(bytes) });
  now = 86400001;
  expect((await s.status("owner", first.id)).upload).toBeNull();
  await expect(
    s.begin("owner", { shareId: first.id, size: bytes.length, sha256: digest(bytes) }),
  ).resolves.toHaveProperty("uploadId");
});
