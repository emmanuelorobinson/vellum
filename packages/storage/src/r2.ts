/** R2 via its S3 API. Credentials stay in the server composition and are never returned to clients. */
import {
  DeleteObjectCommand,
  GetObjectCommand,
  ListObjectsV2Command,
  PutObjectCommand,
  S3Client,
} from "@aws-sdk/client-s3";
import { MAX_BUNDLE } from "./bundle";
import { type ObjectStore, StorageConflict } from "./index";
export function r2Objects(options: {
  endpoint: string;
  bucket: string;
  accessKeyId: string;
  secretAccessKey: string;
}): ObjectStore {
  const endpoint = new URL(options.endpoint);
  if (
    endpoint.protocol !== "https:" ||
    !/^[a-z0-9]+\.r2\.cloudflarestorage\.com$/.test(endpoint.hostname) ||
    endpoint.username ||
    endpoint.password ||
    endpoint.pathname !== "/" ||
    endpoint.search ||
    endpoint.hash
  )
    throw Error("Invalid R2 endpoint");
  const client = new S3Client({
    region: "auto",
    endpoint: options.endpoint,
    credentials: { accessKeyId: options.accessKeyId, secretAccessKey: options.secretAccessKey },
    maxAttempts: 3,
  });
  const key = (value: string) => {
    if (!/^[a-zA-Z0-9_/-]+$/.test(value) || value.includes(".."))
      throw Error("Invalid storage key");
    return value;
  };
  return {
    async get(value) {
      try {
        const r = await client.send(
          new GetObjectCommand({ Bucket: options.bucket, Key: key(value) }),
        );
        if (!r.Body || !r.ETag || r.ContentLength === undefined || r.ContentLength > MAX_BUNDLE)
          throw Error("Object unavailable");
        const chunks: Uint8Array[] = [];
        let size = 0;
        for await (const chunk of r.Body as AsyncIterable<Uint8Array>) {
          size += chunk.length;
          if (size > MAX_BUNDLE) {
            if ("destroy" in r.Body) r.Body.destroy();
            throw Error("Object too large");
          }
          chunks.push(chunk);
        }
        return { bytes: Buffer.concat(chunks), etag: r.ETag };
      } catch (e) {
        if ((e as { $metadata?: { httpStatusCode?: number } }).$metadata?.httpStatusCode === 404)
          return null;
        throw Error("Object storage unavailable");
      }
    },
    async put(value, bytes, match) {
      try {
        await client.send(
          new PutObjectCommand({
            Bucket: options.bucket,
            Key: key(value),
            Body: bytes,
            ContentType: "application/octet-stream",
            CacheControl: "no-store",
            ...(match === null ? { IfNoneMatch: "*" } : { IfMatch: match }),
          }),
        );
      } catch (e) {
        if (
          [409, 412].includes(
            (e as { $metadata?: { httpStatusCode?: number } }).$metadata?.httpStatusCode ?? 0,
          )
        )
          throw new StorageConflict();
        throw Error("Object storage unavailable");
      }
    },
    async list(prefix) {
      const out: string[] = [];
      let cursor: string | undefined;
      do {
        const r = await client.send(
          new ListObjectsV2Command({
            Bucket: options.bucket,
            Prefix: key(prefix),
            ContinuationToken: cursor,
            MaxKeys: 1000,
          }),
        );
        for (const o of r.Contents ?? []) if (o.Key) out.push(o.Key);
        if (out.length > 2000) throw Error("Storage list limit reached");
        cursor = r.IsTruncated ? r.NextContinuationToken : undefined;
        if (r.IsTruncated && !cursor) throw Error("Storage list incomplete");
      } while (cursor);
      return out;
    },
    async delete(value) {
      await client.send(new DeleteObjectCommand({ Bucket: options.bucket, Key: key(value) }));
    },
  };
}
