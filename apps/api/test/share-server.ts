// TEST DATA. @chrome acceptance server, memory cloud boundaries, actual native snapshot bytes.
import { readFile } from "node:fs/promises";
import type { Auth } from "@vellum/auth";
import { MemoryJobQueue } from "@vellum/jobs";
import { CHUNK_SIZE, digest, MemoryObjects, ShareService } from "@vellum/storage";
import { createApi } from "../src/index";

let seq = 0;
const shares = new ShareService({
  objects: new MemoryObjects(),
  clock: () => 1791988320000,
  newId: () => `test_${String(++seq).padStart(32, "0")}`,
});
const bytes = await readFile(new URL("../../../target/share-acceptance.json", import.meta.url));
const pending = await shares.begin("owner", {
  size: bytes.length,
  sha256: digest(bytes),
  ownerName: "Fixture owner",
});
for (let i = 0; i < Math.ceil(bytes.length / CHUNK_SIZE); i++)
  await shares.chunk(
    "owner",
    pending.id,
    pending.uploadId,
    i,
    bytes.subarray(i * CHUNK_SIZE, (i + 1) * CHUNK_SIZE),
  );
await shares.publish("owner", pending.id, pending.uploadId);
const auth = {
  getSession: async (h: Headers) =>
    h.get("cookie")?.includes("fixture=owner")
      ? {
          user: { id: "owner", name: "Fixture owner", email: "fixture@example.test" },
          id: "fixture",
        }
      : null,
  handler: async () => new Response(null, { status: 404 }),
  sessions: async () => [{ id: "fixture", current: true, expiresAt: 1791988320000 }],
  revoke: async () => {},
} as unknown as Auth;
const app = createApi({
  shares,
  auth,
  billing: {
    getEntitlements: async () => ["share_links"],
    createCheckout: async () => "https://checkout.example.test",
    createPortal: async () => "https://billing.example.test",
    acceptWebhook: async () => {},
  },
  jobs: new MemoryJobQueue(),
  newId: () => "fixture",
  accountOrigin: "http://127.0.0.1:3110",
  authorizeInput: async () => false,
});
console.log(`Share fixture: ${pending.id}`);
export default { hostname: "127.0.0.1", port: 3112, fetch: app.fetch };
