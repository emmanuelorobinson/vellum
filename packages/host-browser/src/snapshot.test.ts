// TEST DATA. A public bundle must not grant editing or arbitrary URL/file access.
import { expect, it } from "vitest";
import { snapshotHost } from "./snapshot";

it("serves exact bundle members and refuses writes and ambient access", async () => {
  const host = snapshotHost(
    {
      schema: 1,
      entry: "app.vellum",
      files: [{ path: "app.vellum", data: "e30=" }],
      missing: [],
      frames: [],
    },
    async () => {},
  );
  expect(new TextDecoder().decode(await host.readFile("app.vellum"))).toBe("{}");
  await expect(host.readFile("https://attacker.test")).rejects.toThrow("missing");
  await expect(host.writeFile("app.vellum", new Uint8Array())).rejects.toThrow("read-only");
  expect(await host.discoverDaemon()).toBeNull();
});
