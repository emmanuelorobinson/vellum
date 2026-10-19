// TEST DATA. Real Ed25519 verification; the private key exists only in this test's memory.
import { generateKeyPairSync, sign } from "node:crypto";
import { expect, it } from "vitest";
import { verifyRelease } from "./release-signature";

it("rejects tampered manifests, foreign platforms and an untrusted signing key", () => {
  const keys = generateKeyPairSync("ed25519");
  const key = keys.publicKey.export({ type: "spki", format: "pem" }).toString();
  const payload = Buffer.from(
    JSON.stringify({
      platform: "darwin",
      arch: "arm64",
      version: "1.1.0",
      files: [{ url: "app.zip", sha512: Buffer.alloc(64).toString("base64"), size: 123 }],
    }),
  ).toString("base64");
  const signature = sign(null, Buffer.from(payload, "base64"), keys.privateKey).toString("base64");
  const envelope = { payload, signature };
  expect(verifyRelease(envelope, key, "darwin", "arm64").version).toBe("1.1.0");
  expect(() =>
    verifyRelease({ ...envelope, payload: `${payload.slice(0, -4)}AAAA` }, key, "darwin", "arm64"),
  ).toThrow();
  expect(() => verifyRelease(envelope, key, "win32", "arm64")).toThrow();
  const foreign = generateKeyPairSync("ed25519")
    .publicKey.export({ type: "spki", format: "pem" })
    .toString();
  expect(() => verifyRelease(envelope, foreign, "darwin", "arm64")).toThrow();
});
