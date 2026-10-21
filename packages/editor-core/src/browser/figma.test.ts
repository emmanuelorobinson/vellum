// TEST DATA. Native host owns importing and the single person-origin transaction.
import { expect, it } from "vitest";
import { createFigmaImport, type ImportSignal } from "./figma";

it("reports actual conversion losses, ignores late results after dismissal, and prevents duplicate imports", async () => {
  let finish!: (r: { nodes: number; frames: number; notes: string[] }) => void;
  let count = 0;
  const c = createFigmaImport({
    importFile: () => {
      count++;
      return new Promise((r) => {
        finish = r;
      });
    },
  });
  c.configure("https://www.figma.com/design/test/TEST-DATA");
  const p = c.importFile();
  void c.importFile();
  expect(count).toBe(1);
  finish({ nodes: 8, frames: 2, notes: ["Masks were not converted."] });
  await p;
  expect(c.state.report?.notes).toEqual(["Masks were not converted."]);
  const late = c.importFile();
  c.close();
  finish({ nodes: 9, frames: 3, notes: [] });
  await late;
  expect(c.state.report).toBeNull();
});

it("closing asks the host to cancel before it commits a late REST import", async () => {
  let signal: ImportSignal | undefined;
  const c = createFigmaImport({
    importFile: async (_source, options) => {
      signal = options.signal;
      return new Promise(() => {});
    },
  });
  c.configure("test");
  void c.importFile();
  c.close();
  expect(signal?.aborted).toBe(true);
});
