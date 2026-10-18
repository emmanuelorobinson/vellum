// TEST DATA: paints with Juniper-like colours.
import type { Paint } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import {
  addStop,
  BLEND_MODES,
  blendLabel,
  convertPaint,
  cropCorners,
  dragCropCorner,
  gradientAngle,
  meshPointLabel,
  moveMeshPoint,
  moveStop,
  paintKind,
  paintLabel,
  paintOpacity,
  paintSwatch,
  removeStop,
  renderMesh,
  resizeMesh,
  setGradientAngle,
  setGradientKind,
  setMeshColour,
  setPaintOpacity,
  setStopColour,
  swapPaintOrder,
} from "./paint";

const solid = (color: string, extra: Record<string, unknown> = {}) =>
  ({ type: "solid", color, ...extra }) as unknown as Paint;
const linear = (stops: [string, number][]) =>
  ({
    type: "gradient",
    kind: "linear",
    stops: stops.map(([color, position]) => ({ color, position })),
  }) as unknown as Paint;
const mesh2 = () =>
  ({
    type: "mesh",
    columns: 2,
    rows: 2,
    points: [
      { x: 0, y: 0, color: "#234D3C" },
      { x: 1, y: 0, color: "#A5C5A1" },
      { x: 0, y: 1, color: "#E5EDDE" },
      { x: 1, y: 1, color: "#81976E" },
    ],
  }) as unknown as Paint;
const image = (extra: Record<string, unknown> = {}) =>
  ({ type: "image", asset: "a1", ...extra }) as unknown as Paint;
const identity = (c: unknown) => String(c);

describe("paintKind and paintLabel", () => {
  it("names each kind of paint the way the panel does", () => {
    expect(paintKind(solid("#000000"))).toBe("solid");
    expect(paintKind(linear([["#000000", 0]]))).toBe("linear");
    expect(paintKind(mesh2())).toBe("mesh");
    expect(paintKind(image())).toBe("image");
    expect(paintLabel(mesh2())).toBe("Mesh gradient");
    expect(paintLabel(solid("#000000"))).toBe("Solid");
    expect(paintLabel(image())).toBe("Image");
    expect(paintLabel({ type: "shader", src: "grain.glsl" } as unknown as Paint)).toBe("Shader");
    expect(paintLabel({ type: "gradient", kind: "angular", stops: [] } as unknown as Paint)).toBe(
      "Angular gradient",
    );
  });
});

describe("convertPaint", () => {
  it("keeps the colour when a solid becomes a gradient: from the colour to clear", () => {
    expect(convertPaint(solid("#234D3C"), "linear")).toEqual({
      type: "gradient",
      kind: "linear",
      stops: [
        { color: "#234D3C", position: 0 },
        { color: "#234D3C00", position: 1 },
      ],
    });
  });
  it("keeps the first colour when a gradient becomes a solid", () => {
    expect(
      convertPaint(
        linear([
          ["#234D3C", 0],
          ["#FFFFFF", 1],
        ]),
        "solid",
      ),
    ).toEqual({
      type: "solid",
      color: "#234D3C",
    });
    expect(convertPaint(mesh2(), "solid")).toEqual({ type: "solid", color: "#234D3C" });
  });
  it("changes only the kind between gradients, keeping stops and transform", () => {
    const p = {
      ...linear([
        ["#000000", 0],
        ["#FFFFFF", 1],
      ]),
      transform: [1, 0, 0, 1, 0, 0],
    };
    expect(convertPaint(p as unknown as Paint, "radial")).toEqual({
      ...p,
      kind: "radial",
    });
  });
  it("turns a gradient into a mesh by spreading its stops over a 2 × 2 grid", () => {
    const out = convertPaint(
      linear([
        ["#000000", 0],
        ["#FFFFFF", 1],
      ]),
      "mesh",
    ) as unknown as {
      columns: number;
      rows: number;
      points: { x: number; y: number; color: string }[];
    };
    expect([out.columns, out.rows]).toEqual([2, 2]);
    expect(out.points.map((p) => [p.x, p.y])).toEqual([
      [0, 0],
      [1, 0],
      [0, 1],
      [1, 1],
    ]);
    expect(out.points[0]?.color).toBe("#000000");
    expect(out.points[3]?.color).toBe("#FFFFFF");
  });
  it("keeps a variable-bound colour bound", () => {
    expect(convertPaint(solid({ $var: "c1" } as never), "linear")).toMatchObject({
      stops: [{ color: { $var: "c1" } }, { color: { $var: "c1" } }],
    });
  });
  it("cannot make an image or a shader out of a colour: those need a file", () => {
    expect(convertPaint(solid("#000000"), "image")).toBeNull();
    expect(convertPaint(solid("#000000"), "shader")).toBeNull();
  });
  it("is the same paint when the kind does not change", () => {
    const p = solid("#000000");
    expect(convertPaint(p, "solid")).toBe(p);
  });
});

describe("paint opacity", () => {
  it("is the alpha of a solid colour", () => {
    expect(paintOpacity(solid("#234D3C80"), identity)).toBeCloseTo(128 / 255, 6);
    expect(setPaintOpacity(solid("#234D3C"), 0.5, identity)).toEqual({
      type: "solid",
      color: "#234D3C80",
    });
  });
  it("is an image's own opacity", () => {
    expect(paintOpacity(image(), identity)).toBe(1);
    expect(setPaintOpacity(image(), 0.25, identity)).toMatchObject({ opacity: 0.25 });
    expect(paintOpacity(image({ opacity: 0.4 }), identity)).toBe(0.4);
  });
  it("scales every stop of a gradient so the strongest one is the opacity asked for", () => {
    const p = linear([
      ["#000000", 0],
      ["#00000000", 1],
    ]);
    const out = setPaintOpacity(p, 0.5, identity) as unknown as {
      stops: { color: string }[];
    };
    expect(out.stops.map((s) => s.color)).toEqual(["#00000080", "#00000000"]);
    expect(paintOpacity(out as unknown as Paint, identity)).toBeCloseTo(0.5, 2);
  });
  it("leaves a variable-bound colour alone rather than unbinding it", () => {
    const p = solid({ $var: "c1" } as never);
    expect(setPaintOpacity(p, 0.5, () => "#234D3C")).toBe(p);
  });
});

describe("gradient stops", () => {
  const p = linear([
    ["#000000", 0],
    ["#FFFFFF", 1],
  ]);
  it("adds a stop with the colour the gradient has there", () => {
    const out = addStop(p, 0.5, identity) as unknown as {
      stops: { color: string; position: number }[];
    };
    expect(out.stops).toEqual([
      { color: "#000000", position: 0 },
      { color: "#808080", position: 0.5 },
      { color: "#FFFFFF", position: 1 },
    ]);
  });
  it("removes a stop but never the last two", () => {
    const three = addStop(p, 0.5, identity);
    expect((removeStop(three, 1) as unknown as { stops: unknown[] }).stops).toHaveLength(2);
    expect(removeStop(p, 0)).toBe(p);
  });
  it("moves a stop, keeps the list in order and says where the stop went", () => {
    const three = addStop(p, 0.5, identity);
    const moved = moveStop(three, 1, 0.9);
    expect(moved.index).toBe(1);
    expect(moved.paint).toMatchObject({
      stops: [{ position: 0 }, { position: 0.9 }, { position: 1 }],
    });
    const past = moveStop(three, 0, 0.75);
    expect(past.index).toBe(1);
    expect(
      (past.paint as unknown as { stops: { position: number }[] }).stops.map((s) => s.position),
    ).toEqual([0.5, 0.75, 1]);
  });
  it("keeps positions between 0 and 1", () => {
    expect(moveStop(p, 1, 3).paint).toMatchObject({ stops: [{ position: 0 }, { position: 1 }] });
    expect(moveStop(p, 0, -3).paint).toMatchObject({ stops: [{ position: 0 }, { position: 1 }] });
  });
  it("sets a stop's colour, which may be a variable", () => {
    expect(setStopColour(p, 0, "#FF0000")).toMatchObject({ stops: [{ color: "#FF0000" }, {}] });
    expect(setStopColour(p, 1, { $var: "c1" } as never)).toMatchObject({
      stops: [{}, { color: { $var: "c1" } }],
    });
  });
  it("changes the kind", () => {
    expect(setGradientKind(p, "angular")).toMatchObject({ kind: "angular" });
  });
});

describe("gradient angle", () => {
  it("is 0 with no transform and writes none for 0", () => {
    const p = linear([
      ["#000000", 0],
      ["#FFFFFF", 1],
    ]);
    expect(gradientAngle(p)).toBe(0);
    expect("transform" in setGradientAngle(p, 0)).toBe(false);
  });
  it("round-trips through the transform, about the centre of the shape", () => {
    const p = setGradientAngle(
      linear([
        ["#000000", 0],
        ["#FFFFFF", 1],
      ]),
      90,
    );
    const t = (p as unknown as { transform: number[] }).transform;
    // The centre (0.5, 0.5) maps to itself.
    expect((t[0] ?? 0) * 0.5 + (t[2] ?? 0) * 0.5 + (t[4] ?? 0)).toBeCloseTo(0.5, 6);
    expect((t[1] ?? 0) * 0.5 + (t[3] ?? 0) * 0.5 + (t[5] ?? 0)).toBeCloseTo(0.5, 6);
    expect(gradientAngle(p)).toBeCloseTo(90, 4);
    expect(gradientAngle(setGradientAngle(p, -45))).toBeCloseTo(-45, 4);
  });
});

describe("mesh", () => {
  it("names a point by where it is", () => {
    expect(meshPointLabel(mesh2(), 0)).toBe("Top left");
    expect(meshPointLabel(mesh2(), 1)).toBe("Top right");
    expect(meshPointLabel(mesh2(), 2)).toBe("Bottom left");
    expect(meshPointLabel(mesh2(), 3)).toBe("Bottom right");
    const three = resizeMesh(mesh2(), 3, 3);
    expect(meshPointLabel(three, 1)).toBe("Top");
    expect(meshPointLabel(three, 3)).toBe("Left");
    expect(meshPointLabel(three, 4)).toBe("Row 2, column 2");
    expect(meshPointLabel(three, 5)).toBe("Right");
    expect(meshPointLabel(three, 7)).toBe("Bottom");
  });
  it("grows the grid, laying the points out evenly and sampling the colours it had", () => {
    const out = resizeMesh(mesh2(), 3, 3) as unknown as {
      columns: number;
      rows: number;
      points: { x: number; y: number; color: string }[];
    };
    expect([out.columns, out.rows, out.points.length]).toEqual([3, 3, 9]);
    expect(out.points.map((p) => [p.x, p.y]).slice(0, 3)).toEqual([
      [0, 0],
      [0.5, 0],
      [1, 0],
    ]);
    // Corners keep their colours.
    expect(out.points[0]?.color).toBe("#234D3C");
    expect(out.points[2]?.color).toBe("#A5C5A1");
    expect(out.points[6]?.color).toBe("#E5EDDE");
    expect(out.points[8]?.color).toBe("#81976E");
    // The middle of the top edge is halfway between its ends.
    expect(out.points[1]?.color).toBe("#64896F");
  });
  it("never goes below 2 × 2 or above 8 × 8", () => {
    expect(resizeMesh(mesh2(), 1, 1)).toMatchObject({ columns: 2, rows: 2 });
    expect(resizeMesh(mesh2(), 20, 20)).toMatchObject({ columns: 8, rows: 8 });
  });
  it("is the same paint when the size is the same", () => {
    const p = mesh2();
    expect(resizeMesh(p, 2, 2)).toBe(p);
  });
  it("moves a point across the shape and keeps it inside 0 to 1", () => {
    expect(moveMeshPoint(mesh2(), 0, 0.25, 0.4)).toMatchObject({
      points: [{ x: 0.25, y: 0.4 }, {}, {}, {}],
    });
    expect(moveMeshPoint(mesh2(), 3, 4, -2)).toMatchObject({
      points: [{}, {}, {}, { x: 1, y: 0 }],
    });
  });
  it("sets a point's colour", () => {
    expect(setMeshColour(mesh2(), 1, "#FF0000")).toMatchObject({
      points: [{}, { color: "#FF0000" }, {}, {}],
    });
  });
  it("draws the colours of the points at the points and blends between them", () => {
    const w = 5;
    const h = 5;
    const px = renderMesh(mesh2(), w, h, identity);
    const at = (x: number, y: number) =>
      [...px.slice((y * w + x) * 4, (y * w + x) * 4 + 4)] as [number, number, number, number];
    expect(at(0, 0)).toEqual([0x23, 0x4d, 0x3c, 255]);
    expect(at(4, 0)).toEqual([0xa5, 0xc5, 0xa1, 255]);
    expect(at(0, 4)).toEqual([0xe5, 0xed, 0xde, 255]);
    expect(at(4, 4)).toEqual([0x81, 0x97, 0x6e, 255]);
    const mid = at(2, 2);
    expect(mid[0]).toBeGreaterThan(0x70);
    expect(mid[0]).toBeLessThan(0xa0);
    // No pixel is left unpainted.
    for (let i = 3; i < px.length; i += 4) expect(px[i]).toBe(255);
  });
  it("follows a moved point", () => {
    const moved = moveMeshPoint(mesh2(), 0, 0.5, 0.5);
    const a = renderMesh(mesh2(), 9, 9, identity);
    const b = renderMesh(moved, 9, 9, identity);
    expect([...a]).not.toEqual([...b]);
  });
});

describe("image paints", () => {
  it("covers the shape before it is cropped: the crop is the whole box", () => {
    expect(cropCorners(image())).toEqual([
      { x: 0, y: 0 },
      { x: 1, y: 0 },
      { x: 1, y: 1 },
      { x: 0, y: 1 },
    ]);
  });
  it("shows where a cropped image sits: scale and offset in the shape's own units", () => {
    expect(cropCorners(image({ crop: [2, 0, 0, 2, -0.5, -0.25] }))).toEqual([
      { x: -0.5, y: -0.25 },
      { x: 1.5, y: -0.25 },
      { x: 1.5, y: 1.75 },
      { x: -0.5, y: 1.75 },
    ]);
  });
  it("drags a corner while the opposite one stays, keeping the image's shape", () => {
    const out = dragCropCorner(image(), 2, { x: 1.5, y: 1.5 }) as unknown as { crop: number[] };
    expect(out.crop).toEqual([1.5, 0, 0, 1.5, 0, 0]);
    const other = dragCropCorner(image(), 0, { x: -0.5, y: -0.5 }) as unknown as { crop: number[] };
    expect(other.crop).toEqual([1.5, 0, 0, 1.5, -0.5, -0.5]);
  });
  it("does not let the image shrink to nothing", () => {
    const out = dragCropCorner(image(), 2, { x: -3, y: -3 }) as unknown as { crop: number[] };
    expect(out.crop[0]).toBeGreaterThan(0);
    expect(out.crop[3]).toBeGreaterThan(0);
  });
  it("writes no crop when the box is the whole box again", () => {
    const cropped = dragCropCorner(image(), 2, { x: 2, y: 2 });
    const back = dragCropCorner(cropped, 2, { x: 1, y: 1 });
    expect("crop" in back).toBe(false);
  });
});

describe("paintSwatch", () => {
  it("is the colour for a solid, resolved through variables", () => {
    expect(paintSwatch(solid({ $var: "c1" } as never), () => "#234D3C")).toBe("#234D3C");
  });
  it("is a CSS gradient for a gradient", () => {
    expect(
      paintSwatch(
        linear([
          ["#000000", 0],
          ["#FFFFFF", 1],
        ]),
        identity,
      ),
    ).toBe("linear-gradient(90deg, #000000 0%, #FFFFFF 100%)");
  });
  it("is the first colour for a mesh, an honest small preview, and null for an image", () => {
    expect(paintSwatch(mesh2(), identity)).toBe("#234D3C");
    expect(paintSwatch(image(), identity)).toBeNull();
  });
});

describe("swapPaintOrder", () => {
  it("moves a paint one place", () => {
    const a = solid("#111111");
    const b = solid("#222222");
    expect(swapPaintOrder([a, b], 0, 1)).toEqual([b, a]);
    expect(swapPaintOrder([a, b], 1, 1)).toEqual([a, b]);
    expect(swapPaintOrder([a, b], 0, 5)).toEqual([a, b]);
  });
});

describe("blend modes", () => {
  it("lists the eighteen the format has, Normal first", () => {
    expect(BLEND_MODES).toHaveLength(18);
    expect(BLEND_MODES[0]).toBe("normal");
    expect(new Set(BLEND_MODES).size).toBe(18);
  });
  it("labels them in words", () => {
    expect(blendLabel("normal")).toBe("Normal");
    expect(blendLabel("color-burn")).toBe("Color burn");
    expect(blendLabel("soft-light")).toBe("Soft light");
    expect(blendLabel("luminosity")).toBe("Luminosity");
  });
});
