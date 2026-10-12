// design-check over a fixture inventory: each broken input fails with a precise message
// (guide Prompt 3, "TDD FIRST"). No pen.dev, no design file.
import assert from "node:assert/strict";
import { test } from "node:test";
import { checkDesign, componentFamilies, requiredFrames } from "../lib/design-check.mjs";
import {
  FAMILY_ALIASES,
  fixtureInventory,
  frame,
  MINI_DESIGN_MD,
  SHA,
} from "./design-fixtures.mjs";

const run = (inventory = fixtureInventory(), extra = {}) =>
  checkDesign({
    inventory,
    designMd: MINI_DESIGN_MD,
    sourceSha256: SHA,
    gapsText: "",
    familyAliases: FAMILY_ALIASES,
    snapshotExists: () => true,
    ...extra,
  });

const messages = (result) => result.errors.map((e) => e.message);

test("the fixture inventory passes", () => {
  const result = run();
  assert.deepEqual(messages(result), []);
});

test("requiredFrames follows DESIGN.md: light and dark, hc for C03, 1512 for C03, S13 at 960 and 760", () => {
  const keys = requiredFrames(MINI_DESIGN_MD)
    .filter((r) => !r.optional)
    .map((r) => `${r.id}/${r.theme}/${r.width}`)
    .sort();
  assert.deepEqual(keys, [
    "C01/dark/1280",
    "C01/light/1280",
    "C03/dark/1280",
    "C03/dark/1512",
    "C03/hc-dark/1280",
    "C03/hc-light/1280",
    "C03/light/1280",
    "C03/light/1512",
    "S13/dark/760",
    "S13/dark/960",
    "S13/light/760",
    "S13/light/960",
  ]);
});

test("a v1.1 screen is optional: a gap warning, never an error", () => {
  const optional = requiredFrames(MINI_DESIGN_MD).filter((r) => r.optional);
  assert.ok(optional.length > 0 && optional.every((r) => r.id === "C31"));
  const result = run();
  assert.ok(result.warnings.some((w) => /C31/.test(w.message)));
});

test("a missing screen is listed with its id, theme and width", () => {
  const inv = fixtureInventory();
  inv.frames = inv.frames.filter((f) => f.name !== "C01 · First open · dark");
  assert.deepEqual(messages(run(inv)), [
    "missing frame: C01 · dark · 1280 (DESIGN.md requires it; UI prompts that need it must stop)",
  ]);
});

test("a missing high-contrast frame is reported by its theme name", () => {
  const inv = fixtureInventory();
  inv.frames = inv.frames.filter(
    (f) => !f.name.endsWith("· hc-dark") || f.name.startsWith("Library"),
  );
  assert.ok(
    messages(run(inv)).includes(
      "missing frame: C03 · hc-dark · 1280 (DESIGN.md requires it; UI prompts that need it must stop)",
    ),
  );
});

test("a wrong theme name is reported against the frame", () => {
  const inv = fixtureInventory();
  inv.frames.find((f) => f.name === "C01 · First open · dark").name = "C01 · First open · night";
  const out = messages(run(inv));
  assert.ok(
    out.some((m) => m.startsWith('frame name "C01 · First open · night"') && /theme/.test(m)),
    out.join("\n"),
  );
  assert.ok(out.some((m) => m.startsWith("missing frame: C01 · dark · 1280")));
});

test("a frame whose size disagrees with its name is reported", () => {
  const inv = fixtureInventory();
  const f = inv.frames.find((x) => x.name === "C01 · First open · light");
  f.bounds = { ...f.bounds, width: 1000, height: 800 };
  const out = messages(run(inv));
  assert.ok(
    out.some((m) => m.includes("C01 · First open · light") && m.includes("1000")),
    out.join("\n"),
  );
});

test("a duplicated screen, theme and width is reported", () => {
  const inv = fixtureInventory();
  inv.frames.push(frame("C01 · First open again · light"));
  const out = messages(run(inv));
  assert.ok(
    out.some((m) => m.startsWith("duplicate frame: C01 · light · 1280")),
    out.join("\n"),
  );
});

test("an unrecognised frame name is an error", () => {
  const inv = fixtureInventory();
  inv.frames.push(frame("Frame 12"));
  assert.ok(messages(run(inv)).some((m) => m.startsWith('frame name "Frame 12"')));
});

test("a renamed component fails: the Component library entry has no match", () => {
  const inv = fixtureInventory();
  inv.components[0].name = "Component / Panel Thing / Building";
  assert.deepEqual(messages(run(inv)), [
    'component family missing: DESIGN.md "Component library" lists "Agent Panel" but no reusable component in vellum.pen matches it',
  ]);
});

test("a recorded design gap turns a missing component family into a warning", () => {
  const inv = fixtureInventory();
  inv.components[0].name = "Component / Panel Thing / Building";
  const gaps =
    "- 2026-10-11 · components · DESIGN.md lists Agent Panel · vellum.pen calls it Panel Thing · check=component-family:Agent Panel\n";
  const result = run(inv, { gapsText: gaps });
  assert.deepEqual(messages(result), []);
  assert.ok(
    result.warnings.some((w) => /Agent Panel/.test(w.message) && /DESIGN-GAPS/.test(w.message)),
  );
});

test("a stale checksum fails and names both hashes", () => {
  const out = messages(run(fixtureInventory(), { sourceSha256: "b".repeat(64) }));
  assert.equal(out.length, 1);
  assert.match(
    out[0],
    /^snapshots are stale: inventory.json was taken from vellum\.pen a{64} but the file is b{64}/,
  );
  assert.match(out[0], /pnpm design:snapshot/);
});

test("a gap line cannot excuse a stale checksum or a missing screen", () => {
  const gaps =
    "- 2026-10-11 · x · DESIGN.md says: a · vellum.pen has: b · check=checksum\n" +
    "- 2026-10-11 · y · DESIGN.md says: a · vellum.pen has: b · check=missing-frame:C01\n";
  const inv = fixtureInventory();
  inv.frames = inv.frames.filter((f) => f.name !== "C01 · First open · dark");
  const out = messages(run(inv, { sourceSha256: "b".repeat(64), gapsText: gaps }));
  assert.equal(out.length, 2);
});

test("a screen frame with no snapshot file fails", () => {
  const out = messages(
    run(fixtureInventory(), { snapshotExists: (file) => !file.startsWith("C01-light") }),
  );
  assert.ok(
    out.some((m) => m.startsWith("snapshot missing: C01-light-1280.png")),
    out.join("\n"),
  );
  assert.ok(
    out.some((m) => m.startsWith("snapshot missing: C01-light-1280@2x.png")),
    out.join("\n"),
  );
});

test("variables: a missing token group is reported", () => {
  const inv = fixtureInventory();
  inv.variables.names = inv.variables.names.filter((n) => !n.startsWith("motion/"));
  assert.deepEqual(messages(run(inv)), [
    'variable group missing: no variable named "motion/…" (token groups: chrome, canvas, agent, type or font, space, radius, motion)',
  ]);
});

test("variables: names that are not group/name are reported", () => {
  const inv = fixtureInventory();
  inv.variables.names.push("vellum-ink");
  assert.ok(messages(run(inv)).includes('variable name "vellum-ink" is not "group/name"'));
});

test("variables: a theme axis missing the high-contrast values is reported", () => {
  const inv = fixtureInventory();
  inv.variables.themes = { appearance: ["light", "dark"] };
  const out = messages(run(inv));
  assert.ok(
    out.some((m) => /theme axis/.test(m) && /hc-light/.test(m)),
    out.join("\n"),
  );
});

test("prose that mentions check=<key> outside a dated gap line excuses nothing", () => {
  const inv = fixtureInventory();
  inv.components[0].name = "Component / Panel Thing / Building";
  const gaps = "`check=component-family:Agent Panel` ties a line to a finding\n";
  assert.equal(run(inv, { gapsText: gaps }).errors.length, 1);
});

test("componentFamilies reads the bold entries of the Component library section only", () => {
  assert.deepEqual(componentFamilies(MINI_DESIGN_MD), ["Agent Panel", "Composer"]);
});

test("with no snapshot PNGs in the checkout (they are gitignored), the file check is skipped with a warning", () => {
  const result = run(fixtureInventory(), { snapshotExists: null });
  assert.deepEqual(messages(result), []);
  assert.ok(
    result.warnings.some((w) => w.message.startsWith("snapshot PNGs are not in this checkout")),
    result.warnings.map((w) => w.message).join("\n"),
  );
});
