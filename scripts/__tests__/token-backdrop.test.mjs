// TEST DATA. A marker's foreground sits over its sibling disc, not over the outer panel.
import assert from "node:assert/strict";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import { TOKENS_QUERY, tokenBackdrop } from "../lib/pen-queries.mjs";

const bounds = { x: 0, y: 0, width: 22, height: 22 };
const panel = { node: { id: "panel", fill: "$chrome/panel" }, bounds };
const marker = { node: { id: "marker" }, bounds, parentCtx: panel };
const ctx = { bounds, parentCtx: marker };
const text = { type: "text", content: "S", fontSize: 11, textAlign: "center" };
const variables = { "chrome/text": { value: "#17181B" } };
test("token query uses preorder context instead of expanding native parent wrappers", () => {
  const rows = [
    { id: "panel", type: "frame", fill: "$chrome/panel" },
    { id: "marker", type: "frame" },
    { id: "disc", type: "ellipse", fill: "$chrome/text" },
    { id: "initial", ...text, fill: "$chrome/inverse" },
  ];
  const output = [];
  runInNewContext(TOKENS_QUERY, {
    GetVariables: () => ({ variables, themes: { appearance: ["light"] } }),
    Print: (...args) => output.push(args.join(" ")),
    Get: (visitor) =>
      rows.forEach((n, i) => {
        visitor(n, {
          depth: [0, 1, 2, 2][i],
          bounds,
          get parentCtx() {
            throw Error("parent wrapper must not be expanded");
          },
          skipChildren() {},
        });
      }),
  });
  assert.ok(output.some((l) => l === '@P ["text","chrome/inverse","$chrome/text",1]'));
});
test("a covering sibling disc supplies the marker contrast background", () => {
  const paints = new Map([["marker", [{ type: "ellipse", fill: "$chrome/text", bounds }]]]);
  assert.equal(tokenBackdrop(text, ctx, paints, variables), "$chrome/text");
});
test("the measured 14px person marker with its 11px centered initial uses the disc", () => {
  const small = { ...bounds, width: 14, height: 14 };
  assert.equal(
    tokenBackdrop(
      text,
      { ...ctx, bounds: small },
      new Map([["marker", [{ type: "ellipse", fill: "$chrome/text", bounds: small }]]]),
      variables,
    ),
    "$chrome/text",
  );
});
test("context identity is determined by node id, including fresh parent wrappers", () => {
  const changing = {
    bounds,
    get parentCtx() {
      return { ...marker };
    },
  };
  assert.equal(
    tokenBackdrop(
      text,
      changing,
      new Map([["marker", [{ type: "ellipse", fill: "$chrome/text", bounds }]]]),
      variables,
    ),
    "$chrome/text",
  );
});
test("unrelated, transparent, rotated or small siblings never mask a failing pair", () => {
  for (const paint of [
    { bounds: { ...bounds, x: 30 } },
    { bounds: { ...bounds, width: 10 } },
    { bounds, rotation: 30 },
    { bounds, fill: "#00000000" },
  ]) {
    assert.equal(
      tokenBackdrop(
        text,
        ctx,
        new Map([["marker", [{ type: "ellipse", fill: "$chrome/text", ...paint }]]]),
        variables,
      ),
      "$chrome/panel",
    );
  }
  assert.equal(
    tokenBackdrop(
      { ...text, content: "long text" },
      ctx,
      new Map([["marker", [{ type: "ellipse", fill: "$chrome/text", bounds }]]]),
      variables,
    ),
    "$chrome/panel",
  );
});
test("an initial drawn left-aligned or offset on its disc still uses the disc", () => {
  // vellum.pen's comment pins and member markers: a fixed-width, left-aligned "2" in a 26x15 box over a
  // 26 px disc, and a centred "2" in a 30 px box over a 30 px disc inset by 4 px. The glyph sits on the disc.
  const disc = (b) => new Map([["marker", [{ type: "ellipse", fill: "$chrome/text", bounds: b }]]]);
  const left = { type: "text", content: "2", fontSize: 11, lineHeight: 1.4 };
  assert.equal(
    tokenBackdrop(
      left,
      { ...ctx, bounds: { x: 0, y: 0, width: 26, height: 15 } },
      disc({ x: 0, y: 0, width: 26, height: 26 }),
      variables,
    ),
    "$chrome/text",
  );
  assert.equal(
    tokenBackdrop(
      { ...text, content: "2" },
      { ...ctx, bounds: { x: 0, y: 0, width: 30, height: 30 } },
      disc({ x: 4, y: 4, width: 30, height: 30 }),
      variables,
    ),
    "$chrome/text",
  );
  // A line whose centre is off the disc keeps the ancestor.
  assert.equal(
    tokenBackdrop(
      left,
      { ...ctx, bounds: { x: 0, y: 0, width: 26, height: 15 } },
      disc({ x: 20, y: 0, width: 26, height: 26 }),
      variables,
    ),
    "$chrome/panel",
  );
});
