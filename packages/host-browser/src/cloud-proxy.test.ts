// TEST DATA. Native chunks cross the account origin unchanged; no network.
import { afterEach, expect, it, vi } from "vitest";
import { cloudProxy } from "./cloud-proxy";

afterEach(() => vi.unstubAllGlobals());
it("proxies binary native upload chunks and status only on the account surface", async () => {
  const fetcher = vi.fn(async (_url: URL, init: RequestInit) => {
    expect(init.headers).toBeInstanceOf(Headers);
    return Response.json({ ok: true });
  });
  vi.stubGlobal("fetch", fetcher);
  const parts = ["shares", "s".repeat(32), "uploads", "u".repeat(32), "parts", "0"],
    bytes = new Uint8Array([0, 255, 128, 67]);
  const response = await cloudProxy(
    new Request(`https://account.example.test/v1/${parts.join("/")}`, {
      method: "PUT",
      headers: { authorization: "Bearer fixture", "content-type": "application/octet-stream" },
      body: bytes,
    }),
    parts,
    "https://api.example.test",
  );
  expect(response.status).toBe(200);
  expect(new Uint8Array(fetcher.mock.calls[0]?.[1].body as Uint8Array)).toEqual(bytes);
  expect(
    (
      await cloudProxy(
        new Request("https://viewer.example.test/"),
        parts,
        "https://api.example.test",
        true,
      )
    ).status,
  ).toBe(404);
  const status = await cloudProxy(
    new Request("https://account.example.test/"),
    ["shares", "s".repeat(32), "status"],
    "https://api.example.test",
  );
  expect(status.status).toBe(200);
});
