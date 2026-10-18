// TEST DATA: font bytes live behind the host; these are supported literal family references and usage only.
import type { Document } from "@vellum/types-ts";
import { expect, it } from "vitest";
import { fontUsage, replaceFontOps } from "./controller";

const doc = (): Document => ({
  meta: { formatVersion: 1, name: "test", createdWith: "test" },
  pages: [
    {
      id: "p1",
      name: "Page",
      children: [
        {
          type: "frame",
          id: "f",
          name: "Frame",
          width: 100,
          height: 100,
          children: [
            {
              type: "text",
              id: "t",
              name: "Text",
              width: 100,
              height: 20,
              content: [
                { text: "test", font: "Missing", size: 12 },
                { text: "other", font: "Inter", size: 14 },
              ],
            },
          ],
        },
      ],
    },
  ],
});
it("counts text sources by family, without counting a shared layer twice", () =>
  expect(fontUsage(doc())).toEqual([
    { family: "Missing", nodes: ["t"], assets: [] },
    { family: "Inter", nodes: ["t"], assets: [] },
  ]));
it("replaces only matching runs and preserves every other run property", () => {
  const d = doc();
  expect(replaceFontOps(d, "Missing", "Inter")).toEqual([
    {
      type: "update",
      id: "t",
      patch: {
        content: [
          { text: "test", font: "Inter", size: 12 },
          { text: "other", font: "Inter", size: 14 },
        ],
      },
    },
  ]);
  expect(d.pages[0]?.children?.[0]).toMatchObject({
    children: [{ content: [{ font: "Missing" }, { font: "Inter" }] }],
  });
});
