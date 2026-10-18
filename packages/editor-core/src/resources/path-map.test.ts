// TEST DATA: affine mappings supplied by the engine Scene.
import { expect, it } from "vitest";
import { invertPathMapping } from "./path-map";

it("inverts rotation, translation and nonuniform viewBox scaling", () => {
  const inverse = (p: { x: number; y: number }) => ({ x: (p.y - 20) / 2, y: -(p.x - 10) / 3 });
  const map = invertPathMapping(inverse);
  expect(map?.({ x: 4, y: 5 }).x).toBeCloseTo(-5, 12);
  expect(map?.({ x: 4, y: 5 }).y).toBeCloseTo(28, 12);
});
it("rejects singular mappings", () => expect(invertPathMapping(() => ({ x: 0, y: 0 }))).toBeNull());
