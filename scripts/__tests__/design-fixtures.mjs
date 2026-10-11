// Shared fixtures for the design-check tests. Not a test file (node --test only runs *.test.mjs).
// Everything here is TEST DATA: a miniature specification and a matching inventory, so the checks can
// be exercised without pen.dev and without the 16 MB design file.

export const MINI_DESIGN_MD = `# Design

## Screen requirements

Editor screens are numbered C01–C38, desktop shell A01–A06, web W01–W03 and system and edge states S01–S16. Design each in light and dark; C03, C05 and C10 also in high contrast.

**C01 — First open.** A new file.
**C03 — Agent workspace (building).** The turn in progress.
**S13 — Narrow editor.** C03 at 960px: agent panel at 320px.
**C31 — Import (v1.1).** Desktop only.

## Component library

- **Agent Panel**: tab bar, session bar.
- **Composer**: field, context chips.

## Deliverables

2. **Editor screens** light and dark (C03, C05, C10 also high contrast) at 1280×800, plus C03 and C05 at 1512×982 and S13 at 960×640 and 760×640.
`;

const sizeFor = (width) => ({ 1280: 800, 1512: 982, 960: 640, 760: 640 })[width];

let counter = 0;
export function frame(name, { width = 1280, id } = {}) {
  counter += 1;
  return {
    frame_id: id ?? `F${String(counter).padStart(4, "0")}`,
    name,
    bounds: { x: 0, y: counter * 1000, width, height: sizeFor(width) ?? 600 },
    components: [],
  };
}

export const SHA = "a".repeat(64);

// Every frame MINI_DESIGN_MD requires, named the way vellum.pen names them.
export function fixtureFrames() {
  const f = [];
  for (const t of ["light", "dark"]) {
    f.push(frame(`C01 · First open · ${t}`));
    f.push(frame(`C03 · Agent workspace · building · ${t}`));
    f.push(frame(`C03 · Agent workspace · building · 1512 · ${t}`, { width: 1512 }));
    f.push(frame(`S13 · Narrow editor · 960 · ${t}`, { width: 960 }));
    f.push(frame(`S13 · IDE split · 760 · ${t}`, { width: 760 }));
  }
  for (const t of ["hc-light", "hc-dark"]) {
    f.push(frame(`C03 · Agent workspace · building · ${t}`));
  }
  f.push(frame("Library · Actor markers", { width: 1600 }));
  return f;
}

export function fixtureInventory(overrides = {}) {
  return {
    version: 1,
    source_sha256: SHA,
    exporter: "TEST DATA",
    frames: fixtureFrames().map((fr) => ({ ...fr })),
    components: [
      { id: "K1", name: "Component / Agent Panel / Building" },
      { id: "K2", name: "Component / Composer / Stop" },
    ],
    variables: {
      themes: { appearance: ["light", "dark", "hc-light", "hc-dark"] },
      names: [
        "chrome/panel",
        "canvas/selection",
        "agent/teal",
        "font/body",
        "space/4",
        "radius/lg",
        "motion/fast",
      ],
    },
    ...overrides,
  };
}

export const FAMILY_ALIASES = { "Agent Panel": ["Agent Panel"], Composer: ["Composer"] };
