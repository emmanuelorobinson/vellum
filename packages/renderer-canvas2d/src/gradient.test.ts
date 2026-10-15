// TEST DATA: analytic samples independent of DOM or the geometry compiler.
import { expect, it } from "vitest";
import { sampleGradient } from "./gradient";

it("maps affine gradient coordinates and interpolates color and alpha", () => {
  const g = {
    kind: "linear" as const,
    transform: [0.5, 0, 0, 1, 0.25, 0] as [number, number, number, number, number, number],
    stops: [
      { position: 0, color: [255, 0, 0, 255] as [number, number, number, number] },
      { position: 1, color: [0, 0, 255, 0] as [number, number, number, number] },
    ],
  };
  expect(sampleGradient(g, 0.25, 0.5)).toEqual([255, 0, 0, 255]);
  expect(sampleGradient(g, 0.5, 0.5)).toEqual([128, 0, 128, 128]);
  expect(sampleGradient(g, 1, 0.5)).toEqual([0, 0, 255, 0]);
});
