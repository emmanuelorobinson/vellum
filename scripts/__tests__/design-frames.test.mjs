// The frame-name grammar of docs/design/vellum.pen, as scripts/lib/design-frames.mjs reads it.
import assert from "node:assert/strict";
import { test } from "node:test";
import { parseFrameName, screenSlug } from "../lib/design-frames.mjs";

test("a screen frame yields id, title, theme and the default width", () => {
  const p = parseFrameName("C12 · Notes for agents · dark");
  assert.deepEqual(
    { ok: p.ok, kind: p.kind, id: p.id, title: p.title, theme: p.theme, width: p.width },
    { ok: true, kind: "screen", id: "C12", title: "Notes for agents", theme: "dark", width: 1280 },
  );
});

test("a width token before the theme is read as the width", () => {
  const p = parseFrameName("C03 · Agent workspace · building · 1512 · hc-light");
  assert.equal(p.id, "C03");
  assert.equal(p.title, "Agent workspace · building");
  assert.equal(p.theme, "hc-light");
  assert.equal(p.width, 1512);
});

test("secondary states keep their suffix in the id", () => {
  assert.equal(parseFrameName("C16-instance · Component instance · light").id, "C16-instance");
  assert.equal(parseFrameName("W02-not-shared · Not shared · dark").id, "W02-not-shared");
});

test("state and fixture frames are screens with an id and a theme", () => {
  const s = parseFrameName("State-loading · File loading · light");
  assert.equal(s.kind, "state");
  assert.equal(s.id, "State-loading");
  const f = parseFrameName("Fixture-RTL · Arabic chrome / RTL · dark");
  assert.equal(f.kind, "fixture");
  assert.equal(f.theme, "dark");
});

test("sheets are recognised without a theme", () => {
  for (const [name, kind] of [
    ["Library · Actor markers", "library"],
    ["Components · Core controls and interaction states · hc-dark", "components"],
    ["Flow · Spec to screens", "flow"],
    ["CLI · Output style", "cli"],
    ["Foundations · Tokens · all appearances", "foundations"],
    ["Vellum · Landing page", "vellum"],
  ]) {
    const p = parseFrameName(name);
    assert.equal(p.ok, true, `${name}: ${p.reason}`);
    assert.equal(p.kind, kind);
  }
});

test("a theme the file does not use is reported, not guessed", () => {
  const p = parseFrameName("C01 · First open · night");
  assert.equal(p.ok, false);
  assert.match(p.reason, /theme/);
  assert.match(p.reason, /light, dark, hc-light or hc-dark/);
});

test("a screen frame with no theme at all is reported", () => {
  const p = parseFrameName("C01 · First open");
  assert.equal(p.ok, false);
  assert.match(p.reason, /theme/);
});

test("an unknown prefix is reported", () => {
  const p = parseFrameName("Frame 12");
  assert.equal(p.ok, false);
  assert.match(p.reason, /not a screen id|unrecognised/i);
});

test("screenSlug names snapshot files from id, theme and width", () => {
  assert.equal(screenSlug({ id: "C03", theme: "hc-dark", width: 1512 }), "C03-hc-dark-1512");
});
