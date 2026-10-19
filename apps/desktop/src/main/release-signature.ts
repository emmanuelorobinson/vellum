import { createPublicKey, verify } from "node:crypto";
export interface SignedRelease {
  platform: string;
  arch: string;
  version: string;
  files: { url: string; sha512: string; size: number }[];
}
/** Signed bytes are verified before parsing; the installed app, never the server, supplies trust. */
export function verifyRelease(
  envelope: unknown,
  publicKey: string,
  platform: string,
  arch: string,
): SignedRelease {
  if (!envelope || typeof envelope !== "object") throw Error("signature");
  const e = envelope as Record<string, unknown>;
  if (
    typeof e.payload !== "string" ||
    typeof e.signature !== "string" ||
    e.payload.length > 65536 ||
    e.signature.length > 256
  )
    throw Error("signature");
  const key = createPublicKey(publicKey);
  const bytes = Buffer.from(e.payload, "base64");
  if (
    key.asymmetricKeyType !== "ed25519" ||
    !verify(null, bytes, key, Buffer.from(e.signature, "base64"))
  )
    throw Error("signature");
  const data = JSON.parse(bytes.toString("utf8")) as SignedRelease;
  if (
    data.platform !== platform ||
    data.arch !== arch ||
    !/^\d+\.\d+\.\d+$/.test(data.version) ||
    !Array.isArray(data.files) ||
    !data.files.length ||
    data.files.length > 8 ||
    !data.files.every(
      (f) =>
        f &&
        /^[a-zA-Z0-9_.-]+$/.test(f.url) &&
        !f.url.startsWith(".") &&
        typeof f.sha512 === "string" &&
        Buffer.from(f.sha512, "base64").length === 64 &&
        Number.isSafeInteger(f.size) &&
        f.size > 0,
    )
  )
    throw Error("signature");
  return data;
}
