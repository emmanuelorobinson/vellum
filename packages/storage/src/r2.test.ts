// TEST DATA. SDK command boundary, no R2 account/network.
import { expect, it, vi } from "vitest";

const send = vi.hoisted(() => vi.fn());
vi.mock("@aws-sdk/client-s3", () => ({
  S3Client: class {
    send = send;
  },
  GetObjectCommand: class {
    constructor(public input: unknown) {}
  },
  PutObjectCommand: class {
    constructor(public input: unknown) {}
  },
  ListObjectsV2Command: class {
    constructor(public input: unknown) {}
  },
  DeleteObjectCommand: class {
    constructor(public input: unknown) {}
  },
}));

import { r2Objects } from "./r2";

it("requires conditional R2 writes, refuses arbitrary endpoints, and maps conflicts", async () => {
  expect(() =>
    r2Objects({
      endpoint: "http://attacker.test",
      bucket: "fixture",
      accessKeyId: "fixture",
      secretAccessKey: "fixture",
    }),
  ).toThrow();
  const store = r2Objects({
    endpoint: "https://fixture.r2.cloudflarestorage.com",
    bucket: "fixture",
    accessKeyId: "fixture",
    secretAccessKey: "fixture",
  });
  send.mockResolvedValue({});
  await store.put("shares/fixture", new Uint8Array([1]), null);
  expect(send.mock.lastCall?.[0].input).toMatchObject({
    IfNoneMatch: "*",
    CacheControl: "no-store",
  });
  await store.put("shares/fixture", new Uint8Array([1]), '"etag"');
  expect(send.mock.lastCall?.[0].input).toMatchObject({ IfMatch: '"etag"' });
  send.mockRejectedValue({ $metadata: { httpStatusCode: 412 } });
  await expect(store.put("shares/fixture", new Uint8Array([1]), null)).rejects.toThrow(
    "Snapshot changed",
  );
  send.mockRejectedValue({ $metadata: { httpStatusCode: 404 } });
  expect(await store.get("shares/fixture")).toBeNull();
});
