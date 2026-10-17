// TEST DATA: synthetic cubic and topology, in viewBox coordinates.
import type { Point } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import { add, mul } from "./geometry";
import type { EditablePath, PathPoint } from "./index";
import { anchor, clonePath, dragPath, pathAction, serializePath, splitSegment } from "./path";

const p = (x: number, y: number): Point => ({ x, y });
const cubic = (): EditablePath => ({
  contours: [
    {
      closed: false,
      points: [
        { point: p(0, 0), in: p(-10, 0), out: p(20, 40), smooth: true },
        { point: p(100, 0), in: p(80, 40), out: p(110, 0), smooth: true },
        anchor(p(140, 0)),
      ],
    },
  ],
});
const target = { type: "segment" as const, contour: 0, index: 0 };
const evaluate = (a: PathPoint, b: PathPoint, t: number) =>
  add(
    add(mul(a.point, (1 - t) ** 3), mul(a.out ?? a.point, 3 * (1 - t) ** 2 * t)),
    add(mul(b.in ?? b.point, 3 * (1 - t) * t * t), mul(b.point, t ** 3)),
  );
describe("editable path geometry", () => {
  it("splitting a cubic preserves its curve and both control pairs", () => {
    const original = cubic(),
      path = clonePath(original);
    expect(splitSegment(path, target, 0.3)).toBe(true);
    const [a, m, b] = required(path.contours[0]).points;
    for (const t of [0, 0.1, 0.2, 0.3, 0.5, 0.9, 1]) {
      const expected = evaluate(
        required(required(original.contours[0]).points[0]),
        required(required(original.contours[0]).points[1]),
        t,
      );
      const actual =
        t <= 0.3
          ? evaluate(required(a), required(m), t / 0.3)
          : evaluate(required(m), required(b), (t - 0.3) / 0.7);
      expect(actual.x).toBeCloseTo(expected.x, 10);
      expect(actual.y).toBeCloseTo(expected.y, 10);
    }
    expect(required(original.contours[0]).points).toHaveLength(3);
  });
  it("splits a line at its linear parameter", () => {
    const path = { contours: [{ closed: false, points: [anchor(p(0, 0)), anchor(p(100, 0))] }] };
    splitSegment(path, target, 0.25);
    expect(required(path.contours[0]).points[1]).toEqual(anchor(p(25, 0)));
  });
  it("bend follows the pointer at t and fixed neighbours retain their controls", () => {
    const original = cubic(),
      bent = dragPath(original, { ...target, t: 0.3 }, [], p(0, 0), p(0, 10), false, false, true);
    const before = evaluate(
        required(required(original.contours[0]).points[0]),
        required(required(original.contours[0]).points[1]),
        0.3,
      ),
      after = evaluate(
        required(required(bent.contours[0]).points[0]),
        required(required(bent.contours[0]).points[1]),
        0.3,
      );
    expect(after.x).toBeCloseTo(before.x, 10);
    expect(after.y).toBeCloseTo(before.y + 10, 10);
    expect(required(required(bent.contours[0]).points[0]).in).toEqual(
      required(required(original.contours[0]).points[0]).in,
    );
    expect(required(required(bent.contours[0]).points[1]).out).toEqual(
      required(required(original.contours[0]).points[1]).out,
    );
  });
  it("Alt breaks symmetry; an ordinary smooth handle drag retains the opposite length", () => {
    const original = cubic();
    required(required(original.contours[0]).points[0]).in = p(-30, 0);
    const t = { type: "out" as const, contour: 0, index: 0 };
    const smooth = dragPath(original, t, [], p(20, 40), p(0, 50), false, false, false);
    expect(required(required(smooth.contours[0]).points[0]).in).toEqual(p(0, -30));
    const broken = dragPath(original, t, [], p(20, 40), p(0, 50), false, true, false);
    expect(required(required(broken.contours[0]).points[0]).in).toEqual(p(-30, 0));
    expect(required(required(broken.contours[0]).points[0]).smooth).toBe(false);
  });
  it("cutting a closed contour leaves unrelated incoming controls intact", () => {
    const path = cubic();
    required(path.contours[0]).closed = true;
    path.contours.push({
      closed: false,
      points: [{ ...anchor(p(200, 0)), in: p(190, 0) }, anchor(p(300, 0))],
    });
    const cut = required(pathAction(path, "cut", [target]));
    expect(required(cut.contours[0]).closed).toBe(false);
    expect(required(required(cut.contours[0]).points[0]).in).toBeNull();
    expect(required(required(cut.contours[1]).points[0]).in).toEqual(p(190, 0));
  });
  it("connecting separate contours reverses the handles with orientation", () => {
    const path = cubic();
    path.contours.push({ closed: false, points: [anchor(p(200, 0)), anchor(p(300, 0))] });
    const connected = required(
      pathAction(path, "connect", [{ type: "point", contour: 0, index: 0 }], {
        type: "point",
        contour: 1,
        index: 1,
      }),
    );
    expect(connected.contours).toHaveLength(1);
    expect(required(connected.contours[0]).points.map((x) => x.point)).toEqual([
      p(140, 0),
      p(100, 0),
      p(0, 0),
      p(300, 0),
      p(200, 0),
    ]);
    expect(required(required(connected.contours[0]).points[2]).in).toEqual(p(20, 40));
  });
  it("rejects invalid indices and parameters without changing the path", () => {
    const path = cubic(),
      before = JSON.stringify(path);
    expect(pathAction(path, "addPoint", [target], undefined, Number.NaN)).toBeNull();
    expect(pathAction(path, "delete", [{ type: "point", contour: 0, index: -1 }])).toBeNull();
    expect(
      pathAction(path, "connect", [{ type: "point", contour: 0, index: 1 }], {
        type: "point",
        contour: 0,
        index: 0,
      }),
    ).toBeNull();
    expect(JSON.stringify(path)).toBe(before);
  });
  it("multiple deletion keeps indices stable and resets closure", () => {
    const path = cubic();
    required(path.contours[0]).closed = true;
    const edited = required(
      pathAction(path, "delete", [
        { type: "point", contour: 0, index: 0 },
        { type: "point", contour: 0, index: 2 },
      ]),
    );
    expect(required(edited.contours[0]).points).toEqual([required(path.contours[0]).points[1]]);
    expect(required(edited.contours[0]).closed).toBe(false);
    expect(serializePath(edited)).toBe("M 100 0");
  });
});
describe("multiple segment operations", () => {
  it("cuts every selected segment using its original index", () => {
    const path = {
      contours: [
        {
          closed: false,
          points: [anchor(p(0, 0)), anchor(p(10, 0)), anchor(p(20, 0)), anchor(p(30, 0))],
        },
      ],
    };
    const cut = required(
      pathAction(path, "cut", [
        { ...target, index: 0 },
        { ...target, index: 2 },
      ]),
    );
    expect(cut.contours.map((c) => c.points.map((q) => q.point.x))).toEqual([[0], [10, 20], [30]]);
  });
  it("deletes point and segment selections together without shifting their addresses", () => {
    const path = {
      contours: [
        {
          closed: false,
          points: [anchor(p(0, 0)), anchor(p(10, 0)), anchor(p(20, 0)), anchor(p(30, 0))],
        },
      ],
    };
    const cut = required(
      pathAction(path, "delete", [
        { ...target, index: 2 },
        { type: "point", contour: 0, index: 0 },
      ]),
    );
    expect(cut.contours.map((c) => c.points.map((q) => q.point.x))).toEqual([[10, 20], [30]]);
  });
});

function required<T>(value: T | null | undefined): T {
  if (value == null) throw new Error("Missing fixture value");
  return value;
}
