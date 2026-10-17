// Writes the TEST DATA fixtures in this folder with the reference Kiwi implementation, so the Rust decoder is
// checked against code it was not written with.
//
//   npm pack kiwi-schema@0.5.0 && tar xzf kiwi-schema-0.5.0.tgz     (MIT; not a dependency of this repository)
//   node generate.mjs package/kiwi.js
//
// The schema below is written from memory of the names and shapes Figma uses; it is not Figma's own file. The
// importer reads the schema a paste carries and goes by field and enum names, so a real paste with more fields
// reads the same way. Nothing here has been checked against a live paste (see reports.md).
import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";
import { deflateRawSync } from "node:zlib";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const kiwi = createRequire(import.meta.url)(resolve(process.argv[2] ?? "package/kiwi.js"));

const text = `
enum MessageType { JOIN_START = 0; NODE_CHANGES = 1; }
enum NodeType { NONE = 0; DOCUMENT = 1; CANVAS = 2; GROUP = 3; FRAME = 4; BOOLEAN_OPERATION = 5; VECTOR = 6; STAR = 7;
  LINE = 8; ELLIPSE = 9; RECTANGLE = 10; REGULAR_POLYGON = 11; ROUNDED_RECTANGLE = 12; TEXT = 13; SLICE = 14; SYMBOL = 15;
  INSTANCE = 16; }
enum NodePhase { CREATED = 0; REMOVED = 1; }
enum PaintType { SOLID = 0; GRADIENT_LINEAR = 1; GRADIENT_RADIAL = 2; IMAGE = 5; }
enum EffectType { INNER_SHADOW = 0; DROP_SHADOW = 1; FOREGROUND_BLUR = 2; BACKGROUND_BLUR = 3; }
enum StrokeAlign { CENTER = 0; INSIDE = 1; OUTSIDE = 2; }
enum StrokeCap { NONE = 0; ROUND = 1; SQUARE = 2; }
enum StrokeJoin { MITER = 0; BEVEL = 1; ROUND = 2; }
enum WindingRule { NONZERO = 0; ODD = 1; }
enum TextAlignHorizontal { LEFT = 0; CENTER = 1; RIGHT = 2; JUSTIFIED = 3; }
enum TextAlignVertical { TOP = 0; CENTER = 1; BOTTOM = 2; }
enum TextAutoResize { NONE = 0; WIDTH_AND_HEIGHT = 1; HEIGHT = 2; }
enum NumberUnits { RAW = 0; PIXELS = 1; PERCENT = 2; }
enum StackMode { NONE = 0; HORIZONTAL = 1; VERTICAL = 2; }
struct GUID { uint sessionID; uint localID; }
struct Color { float r; float g; float b; float a; }
struct Vector { float x; float y; }
struct Matrix { float m00; float m01; float m02; float m10; float m11; float m12; }
struct ParentIndex { GUID guid; string position; }
struct ColorStop { Color color; float position; }
struct Number { float value; NumberUnits units; }
struct FontName { string family; string style; string postscript; }
message Paint { PaintType type = 1; Color color = 2; float opacity = 3; bool visible = 4; ColorStop[] stops = 5; }
message Effect { EffectType type = 1; Color color = 2; Vector offset = 3; float radius = 4; float spread = 5; bool visible = 6; }
message Geometry { WindingRule windingRule = 1; uint commandsBlob = 2; }
message TextData { string characters = 1; }
message NodeChange {
  GUID guid = 1; NodePhase phase = 2; ParentIndex parentIndex = 3; NodeType type = 4; string name = 5; bool visible = 6;
  float opacity = 7; Vector size = 8; Matrix transform = 9; Paint[] fillPaints = 10; Paint[] strokePaints = 11;
  float strokeWeight = 12; StrokeAlign strokeAlign = 13; StrokeCap strokeCap = 14; StrokeJoin strokeJoin = 15;
  float cornerRadius = 16; Effect[] effects = 17; Geometry[] fillGeometry = 18; Geometry[] strokeGeometry = 19;
  TextData textData = 20; float fontSize = 21; FontName fontName = 22; TextAlignHorizontal textAlignHorizontal = 23;
  TextAlignVertical textAlignVertical = 24; Number lineHeight = 25; Number letterSpacing = 26;
  TextAutoResize textAutoResize = 27; StackMode stackMode = 28; bool frameMaskDisabled = 29; float[] dashPattern = 30;
  bool rectangleCornerRadiiIndependent = 31; float rectangleTopLeftCornerRadius = 32;
  float rectangleTopRightCornerRadius = 33; float rectangleBottomLeftCornerRadius = 34;
  float rectangleBottomRightCornerRadius = 35;
}
message Blob { byte[] bytes = 1; }
message Message { MessageType type = 1; NodeChange[] nodeChanges = 2; Blob[] blobs = 3; }
`;

const schema = kiwi.parseSchema(text);
const compiled = kiwi.compileSchema(schema);
const binarySchema = kiwi.encodeBinarySchema(schema);

const guid = (s, l) => ({ sessionID: s, localID: l });
const color = (r, g, b, a = 1) => ({ r, g, b, a });
const solid = (r, g, b, a = 1, opacity) => ({
  type: "SOLID",
  color: color(r, g, b, a),
  ...(opacity === undefined ? {} : { opacity }),
});
const at = (m02, m12) => ({ m00: 1, m01: 0, m02, m10: 0, m11: 1, m12 });
const node = (id, parent, position, type, name, extra) => ({
  guid: guid(...id),
  parentIndex: { guid: guid(...parent), position },
  type,
  name,
  ...extra,
});

// Path commands: 0 close, 1 move, 2 line, 3 cubic, 4 quadratic; f32 little-endian coordinates.
const f32 = (...n) => {
  const b = Buffer.alloc(4 * n.length);
  n.forEach((v, i) => {
    b.writeFloatLE(v, 4 * i);
  });
  return b;
};
const blob = (...parts) => new Uint8Array(Buffer.concat(parts));
const arrow = blob(
  Buffer.from([1]),
  f32(0, 0),
  Buffer.from([2]),
  f32(24, 0),
  Buffer.from([3]),
  f32(24, 8, 16, 24, 8, 24),
  Buffer.from([4]),
  f32(0, 24, 0, 12),
  Buffer.from([0]),
);
const stroked = blob(
  Buffer.from([1]),
  f32(0, 0),
  Buffer.from([2]),
  f32(10, 10),
  Buffer.from([2]),
  f32(10, 12),
  Buffer.from([2]),
  f32(0, 2),
  Buffer.from([0]),
);

const card = [
  node([1, 1], [0, 99], "!", "FRAME", "Card", {
    size: { x: 320, y: 200 },
    transform: at(100, 50),
    fillPaints: [solid(1, 1, 1)],
    strokePaints: [solid(0, 0, 0)],
    strokeWeight: 2,
    strokeAlign: "INSIDE",
    cornerRadius: 12,
    stackMode: "VERTICAL",
    effects: [
      {
        type: "DROP_SHADOW",
        color: color(0, 0, 0, 0.25),
        offset: { x: 0, y: 4 },
        radius: 8,
        spread: 0,
      },
    ],
  }),
  // Out of order on purpose: children go by their position string, back to front.
  node([1, 5], [1, 1], "d", "VECTOR", "Arrow", {
    size: { x: 24, y: 24 },
    transform: at(16, 120),
    fillPaints: [solid(0, 0, 0)],
    fillGeometry: [{ windingRule: "ODD", commandsBlob: 0 }],
  }),
  node([1, 3], [1, 1], "b", "RECTANGLE", "Swatch", {
    size: { x: 40, y: 20 },
    transform: { m00: 0, m01: -1, m02: 70, m10: 1, m11: 0, m12: 80 },
    fillPaints: [
      {
        type: "GRADIENT_LINEAR",
        opacity: 1,
        stops: [
          { color: color(1, 0, 0), position: 0 },
          { color: color(0, 0, 1), position: 1 },
        ],
      },
      { type: "IMAGE", opacity: 1 },
    ],
  }),
  node([1, 2], [1, 1], "a", "TEXT", "Title", {
    size: { x: 200, y: 24 },
    transform: at(16, 16),
    fillPaints: [solid(0, 0.2, 0.4)],
    textData: { characters: "Hello Figma" },
    fontSize: 20,
    fontName: { family: "Inter", style: "Bold", postscript: "Inter-Bold" },
    textAlignHorizontal: "CENTER",
    lineHeight: { value: 24, units: "PIXELS" },
    letterSpacing: { value: -5, units: "PERCENT" },
    textAutoResize: "WIDTH_AND_HEIGHT",
  }),
  node([1, 4], [1, 1], "c", "ELLIPSE", "Dot", {
    size: { x: 30, y: 30 },
    transform: at(250, 20),
    fillPaints: [solid(0, 1, 0, 1, 0.5)],
  }),
  node([1, 6], [1, 1], "e", "GROUP", "Pair", { size: { x: 60, y: 40 }, transform: at(200, 120) }),
  node([1, 8], [1, 6], "b", "LINE", "Rule", {
    size: { x: 60, y: 0 },
    transform: at(0, 30),
    strokePaints: [solid(0.4, 0.4, 0.4)],
    strokeWeight: 1,
    strokeCap: "ROUND",
    dashPattern: [4, 2],
  }),
  node([1, 7], [1, 6], "a", "RECTANGLE", "Chip", {
    size: { x: 20, y: 20 },
    transform: at(0, 0),
    fillPaints: [solid(0.4, 0, 0.4)],
    rectangleCornerRadiiIndependent: true,
    rectangleTopLeftCornerRadius: 4,
    rectangleTopRightCornerRadius: 0,
    rectangleBottomLeftCornerRadius: 0,
    rectangleBottomRightCornerRadius: 8,
  }),
  node([1, 9], [1, 1], "f", "SLICE", "Export", { size: { x: 10, y: 10 }, transform: at(0, 0) }),
  node([1, 10], [1, 1], "g", "RECTANGLE", "Hidden", {
    size: { x: 5, y: 5 },
    transform: at(0, 0),
    visible: false,
    opacity: 0.5,
    fillPaints: [solid(1, 0, 0)],
  }),
  node([1, 11], [1, 1], "h", "VECTOR", "Open path", {
    size: { x: 10, y: 12 },
    transform: at(0, 0),
    strokePaints: [solid(0, 0, 1)],
    strokeWeight: 2,
    strokeGeometry: [{ windingRule: "NONZERO", commandsBlob: 1 }],
  }),
  node([1, 12], [1, 1], "i", "INSTANCE", "Button", {
    size: { x: 50, y: 20 },
    transform: at(100, 150),
    fillPaints: [solid(0.2, 0.2, 0.2)],
    frameMaskDisabled: true,
  }),
  node([2, 1], [0, 99], '"', "ELLIPSE", "Loose", {
    size: { x: 10, y: 10 },
    transform: at(20, 400),
    fillPaints: [solid(0, 0, 0)],
  }),
];

const page = [
  { guid: guid(0, 0), type: "DOCUMENT", name: "Document" },
  node([0, 1], [0, 0], "!", "CANVAS", "Page 1", {}),
  ...card.map((n) =>
    n.parentIndex.guid.localID === 99
      ? { ...n, parentIndex: { ...n.parentIndex, guid: guid(0, 1) } }
      : n,
  ),
];

function html(nodeChanges) {
  const message = compiled.encodeMessage({
    type: "NODE_CHANGES",
    nodeChanges,
    blobs: [{ bytes: arrow }, { bytes: stroked }],
  });
  const u32 = (n) => {
    const b = Buffer.alloc(4);
    b.writeUInt32LE(n);
    return b;
  };
  const chunk = (data) => {
    const z = deflateRawSync(Buffer.from(data));
    return Buffer.concat([u32(z.length), z]);
  };
  const file = Buffer.concat([
    Buffer.from("fig-kiwi"),
    u32(15),
    chunk(binarySchema),
    chunk(message),
  ]);
  const meta = Buffer.from(
    JSON.stringify({ fileKey: "TESTDATA", pasteID: 1, dataType: "scene" }),
  ).toString("base64");
  return `<meta charset="utf-8"><div><span data-metadata="<!--(figmeta)${meta}(/figmeta)-->"></span><span data-buffer="<!--(figma)${file.toString("base64")}(/figma)-->"></span></div><span style="white-space:pre-wrap;"></span>`;
}

writeFileSync(resolve(here, "selection.html"), html(card));
writeFileSync(resolve(here, "page.html"), html(page));
console.log("wrote selection.html and page.html");
