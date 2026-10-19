// TEST DATA: recent files and session projection, without a VS Code host.
import { expect, it } from "vitest";
import { RecentDesigns, sessionRows } from "./sidebar-model";

it("deduplicates, bounds and restores recent design files in opening order", () => {
  const recent = new RecentDesigns([
    "file:///one.vellum",
    "https://invalid.vellum",
    "file:///text.ts",
  ]);
  recent.open("file:///two.lib.vellum");
  recent.open("file:///one.vellum");
  expect(recent.items).toEqual(["file:///one.vellum", "file:///two.lib.vellum"]);
  for (let n = 0; n < 40; n++) recent.open(`file:///${n}.vellum`);
  expect(recent.items).toHaveLength(20);
});
it("session rows retain document and identity and use activity ordering", () => {
  const rows = sessionRows([
    { id: "a", path: "/repo/a.vellum", title: "Earlier", client: "Codex", updatedAt: 1 },
    { id: "b", path: "/repo/b.vellum", title: "Latest", client: "Claude Code", updatedAt: 2 },
  ]);
  expect(rows.map((r) => r.id)).toEqual(["b", "a"]);
  expect(rows[0]).toMatchObject({ path: "/repo/b.vellum", client: "Claude Code" });
});
