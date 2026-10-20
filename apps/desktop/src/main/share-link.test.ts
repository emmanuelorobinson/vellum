// TEST DATA. @native-fs: isolated temporary download directory; no network, keychain or installed protocol.
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, it } from "vitest";
import { installSnapshot, parseShareLink } from "./share-link";

it("accepts only bounded share links, never credentials, nested origins or alternate schemes", () => {
  const id = "a".repeat(32);
  expect(
    parseShareLink(
      `vellum://share/${id}?origin=${encodeURIComponent("https://account.example.test")}`,
    ),
  ).toEqual({ id, origin: "https://account.example.test/" });
  for (const url of [
    `vellum://share/${id}?origin=https://user:token@host`,
    `vellum://share/${id}?origin=https://host/nested`,
    `https://host/share/${id}`,
    `vellum://share/../x?origin=https://host`,
  ])
    expect(() => parseShareLink(url)).toThrow();
});
it("installs exact files in an exclusive new folder and refuses traversal", async () => {
  const parent = await mkdtemp(join(tmpdir(), "vshare-"));
  const bundle = {
    schema: 1 as const,
    entry: "design/app.vellum",
    files: [
      {
        path: "design/app.vellum",
        data: Buffer.from(
          JSON.stringify({ meta: { formatVersion: 1, name: "Fixture" }, pages: [] }),
        ).toString("base64"),
      },
    ],
    frames: [],
    missing: [],
  };
  try {
    const result = await installSnapshot(parent, bundle);
    expect(await readFile(result, "utf8")).toBe(
      Buffer.from(bundle.files[0]?.data ?? "", "base64").toString(),
    );
    await expect(
      installSnapshot(parent, { ...bundle, entry: "../escape.vellum" }),
    ).rejects.toThrow();
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
});
