// The rules behind scripts/design-check: is the committed inventory current, does it hold every frame
// DESIGN.md "Deliverables" asks for, and do frame, component and variable names follow the convention.
// Pure: it takes the inventory, the DESIGN.md text and the recorded gaps, and returns findings.
import {
  DEFAULT_WIDTH,
  HEIGHT_FOR_WIDTH,
  parseFrameName,
  SNAPSHOT_KINDS,
  snapshotFiles,
  THEMES,
} from "./design-frames.mjs";

// DESIGN.md, "Screen requirements": "Design each in light and dark; C03, C05 and C10 also in high
// contrast." "Deliverables": "plus C03 and C05 at 1512×982 and S13 at 960×640 and 760×640".
// scripts/__tests__/design-spec-rules.test.mjs fails if DESIGN.md stops saying so.
export const HIGH_CONTRAST_IDS = ["C03", "C05", "C10"];
export const WIDE_IDS = ["C03", "C05"];
export const NARROW_IDS = { S13: [960, 760] };
const BASE_THEMES = ["light", "dark"];
const HC_THEMES = ["hc-light", "hc-dark"];
export const WIDE_WIDTH = 1512;

export const TOKEN_GROUPS = [
  { label: "chrome", names: ["chrome"] },
  { label: "canvas", names: ["canvas"] },
  { label: "agent", names: ["agent"] },
  { label: "type or font", names: ["type", "font"] },
  { label: "space", names: ["space"] },
  { label: "radius", names: ["radius"] },
  { label: "motion", names: ["motion"] },
];

// "**C03 — Agent workspace (building).** ..." → one entry per numbered screen. (v1.1) screens are optional:
// DESIGN.md says they "can follow later".
export function requiredFrames(designMd) {
  const required = [];
  const heading = /^\*\*([CAWS]\d{2}) — ([^*]*)\*\*/gm;
  for (const match of designMd.matchAll(heading)) {
    const [, id, title] = match;
    const optional = /v1\.1/.test(title);
    const themes = HIGH_CONTRAST_IDS.includes(id) ? [...BASE_THEMES, ...HC_THEMES] : BASE_THEMES;
    const widths = NARROW_IDS[id] ?? [DEFAULT_WIDTH];
    for (const width of widths)
      for (const theme of themes) required.push({ id, theme, width, optional });
    if (WIDE_IDS.includes(id))
      for (const theme of BASE_THEMES) required.push({ id, theme, width: WIDE_WIDTH, optional });
  }
  return required;
}

// The bold entries of the "Component library" section.
export function componentFamilies(designMd) {
  const start = designMd.indexOf("\n## Component library");
  if (start === -1) return [];
  const end = designMd.indexOf("\n## ", start + 1);
  const section = designMd.slice(start, end === -1 ? undefined : end);
  return [...section.matchAll(/\*\*([^*]+)\*\*/g)].map((m) => m[1]);
}

// Recorded lines of DESIGN-GAPS.md ("- <date> · …") may carry "check=<key>"; a recorded gap turns the
// matching finding into a warning. Prose in the file's header is not a recorded gap.
export function parseGaps(gapsText) {
  const keys = new Set();
  for (const line of gapsText.split("\n")) {
    if (!/^- \d{4}-\d{2}-\d{2} /.test(line)) continue;
    for (const m of line.matchAll(/check=([^·\n]+?)\s*(?:·|$)/g)) keys.add(m[1].trim());
  }
  return keys;
}

const label = (r) => `${r.id} · ${r.theme} · ${r.width}`;

export function checkDesign({
  inventory,
  designMd,
  sourceSha256,
  gapsText = "",
  familyAliases = {},
  snapshotExists,
}) {
  const errors = [];
  const warnings = [];
  const gaps = parseGaps(gapsText);
  // Findings with a key can be excused by a recorded gap; the rest can never be.
  const fail = (message, key) => {
    if (key && gaps.has(key))
      warnings.push({ key, message: `${message} (recorded in DESIGN-GAPS.md)` });
    else errors.push({ key: key ?? null, message });
  };

  if (inventory.version !== 1) fail(`inventory.json has version ${inventory.version}, expected 1`);
  if (inventory.source_sha256 !== sourceSha256) {
    fail(
      `snapshots are stale: inventory.json was taken from vellum.pen ${inventory.source_sha256} but the file is ${sourceSha256}; run pnpm design:snapshot (needs pen login or PEN_CLI_KEY)`,
    );
  }

  // Frame names, sizes, duplicates.
  const seen = new Map();
  for (const frame of inventory.frames) {
    const parsed = parseFrameName(frame.name);
    if (!parsed.ok) {
      fail(`frame name "${frame.name}" is not valid: ${parsed.reason}`, `frame-name:${frame.name}`);
      continue;
    }
    if (!SNAPSHOT_KINDS.includes(parsed.kind)) continue;
    const { width, height } = frame.bounds;
    const expectedHeight = HEIGHT_FOR_WIDTH[parsed.width];
    if (width !== parsed.width || height !== expectedHeight) {
      fail(
        `frame "${frame.name}" is ${width}×${height} but its name says ${parsed.width}×${expectedHeight}`,
        `frame-size:${frame.name}`,
      );
    }
    const key = `${parsed.id}|${parsed.theme}|${parsed.width}`;
    seen.set(key, [...(seen.get(key) ?? []), frame.name]);
    for (const file of Object.values(snapshotFiles(parsed))) {
      if (snapshotExists && !snapshotExists(file))
        fail(`snapshot missing: ${file} for "${frame.name}" (run pnpm design:snapshot)`);
    }
  }
  for (const [key, names] of seen) {
    if (names.length > 1) {
      const [id, theme, width] = key.split("|");
      fail(
        `duplicate frame: ${id} · ${theme} · ${width} appears ${names.length} times (${names.join("; ")})`,
      );
    }
  }

  // Required screens × themes × widths.
  for (const r of requiredFrames(designMd)) {
    if (seen.has(`${r.id}|${r.theme}|${r.width}`)) continue;
    if (r.optional) {
      warnings.push({
        key: null,
        message: `missing frame (v1.1, DESIGN.md allows it to follow later): ${label(r)}`,
      });
    } else {
      fail(`missing frame: ${label(r)} (DESIGN.md requires it; UI prompts that need it must stop)`);
    }
  }

  // Components named in DESIGN.md's Component library.
  const componentNames = (inventory.components ?? []).map((c) => c.name.toLowerCase());
  for (const family of componentFamilies(designMd)) {
    const needles = (familyAliases[family] ?? [family]).map((a) => a.toLowerCase());
    if (!componentNames.some((name) => needles.some((n) => name.includes(n)))) {
      fail(
        `component family missing: DESIGN.md "Component library" lists "${family}" but no reusable component in vellum.pen matches it`,
        `component-family:${family}`,
      );
    }
  }

  // Variables: token groups, group/name names, a theme axis with every appearance.
  const names = inventory.variables?.names ?? [];
  for (const name of names) {
    if (!/^[a-z][a-z0-9-]*\/.+$/.test(name))
      fail(`variable name "${name}" is not "group/name"`, `variable-name:${name}`);
  }
  for (const group of TOKEN_GROUPS) {
    if (!names.some((n) => group.names.includes(n.split("/")[0]))) {
      fail(
        `variable group missing: no variable named "${group.names[0]}/…" (token groups: ${TOKEN_GROUPS.map((g) => g.label).join(", ")})`,
        `variable-group:${group.label}`,
      );
    }
  }
  const axes = inventory.variables?.themes ?? {};
  if (!Object.values(axes).some((values) => THEMES.every((t) => values.includes(t)))) {
    const found =
      Object.entries(axes)
        .map(([axis, values]) => `${axis} = ${values.join(", ")}`)
        .join("; ") || "none";
    fail(
      `theme axis incomplete: no axis in vellum.pen has all of ${THEMES.join(", ")} (found: ${found})`,
      "variable-theme-axis",
    );
  }

  if (!snapshotExists) {
    warnings.push({
      message:
        "snapshot PNGs are not in this checkout (they are gitignored); only inventory.json was checked. Run pnpm design:snapshot to make them",
    });
  }
  return { errors, warnings };
}
