// TEST DATA. In-process API with memory objects; no vendor network.

import type { Auth } from "@vellum/auth";
import { MemoryJobQueue } from "@vellum/jobs";
import { digest, encodeBundle, MemoryObjects, ShareService } from "@vellum/storage";
import { expect, it, vi } from "vitest";
import { createApi } from "./index";

it("requires Pro to upload, permits public view/download, and revokes after Pro expires", async () => {
  let n = 0,
    pro = false;
  const auth = {
    getSession: async (h: Headers) =>
      h.get("authorization") === "Bearer fixture"
        ? { user: { id: "owner", name: "Owner", email: "owner@example.test" }, id: "session" }
        : null,
    handler: vi.fn(),
  } as unknown as Auth;
  const shares = new ShareService({
    objects: new MemoryObjects(),
    clock: () => 123,
    newId: () => `id_${String(++n).padStart(32, "0")}`,
  });
  const app = createApi({
    auth,
    billing: {
      getEntitlements: async () => (pro ? ["share_links"] : []),
      createCheckout: vi.fn(),
      acceptWebhook: vi.fn(),
    },
    jobs: new MemoryJobQueue(),
    newId: () => "job",
    accountOrigin: "https://account.example.test",
    authorizeInput: async () => false,
    shares,
  });
  const headers = { authorization: "Bearer fixture", "content-type": "application/json" };
  const bytes = encodeBundle({
    schema: 1,
    entry: "app.vellum",
    files: [
      {
        path: "app.vellum",
        data: Buffer.from(
          JSON.stringify({ meta: { formatVersion: 1, name: "Fixture" }, pages: [] }),
        ).toString("base64"),
      },
    ],
    missing: ["missing.png"],
    frames: [],
  });
  const begin = () =>
    app.request("/v1/shares", {
      method: "POST",
      headers,
      body: JSON.stringify({ size: bytes.length, sha256: digest(bytes) }),
    });
  expect((await begin()).status).toBe(403);
  pro = true;
  const p = (await (await begin()).json()) as { id: string; uploadId: string };
  expect((await app.request(`/v1/shares/${p.id}`)).status).toBe(404);
  expect(
    (
      await app.request(`/v1/shares/${p.id}/uploads/${p.uploadId}/parts/0`, {
        method: "PUT",
        headers: { authorization: "Bearer fixture" },
        body: bytes,
      })
    ).status,
  ).toBe(200);
  expect(
    (
      await app.request(`/v1/shares/${p.id}/uploads/${p.uploadId}/publish`, {
        method: "POST",
        headers,
      })
    ).status,
  ).toBe(200);
  const view = await app.request(`/v1/shares/${p.id}`);
  expect(view.status).toBe(200);
  expect(view.headers.get("cache-control")).toBe("no-store");
  expect(((await view.json()) as { snapshot: { incomplete: boolean } }).snapshot.incomplete).toBe(
    true,
  );
  const zip = await app.request(`/v1/shares/${p.id}/download`);
  expect(zip.headers.get("content-type")).toBe("application/zip");
  expect(new Uint8Array(await zip.arrayBuffer()).slice(0, 4)).toEqual(
    new Uint8Array([80, 75, 3, 4]),
  );
  expect((await app.request(`/v1/shares/${p.id}/status`)).status).toBe(401);
  pro = false;
  expect((await app.request(`/v1/shares/${p.id}`, { method: "DELETE", headers })).status).toBe(200);
  expect((await app.request(`/v1/shares/${p.id}/download`)).status).toBe(404);
});
