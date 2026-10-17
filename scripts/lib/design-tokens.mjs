// The rules behind scripts/design-tokens (guide Prompt 43). Pure: no file system, no pen.dev.
//
// buildTokens   the design's variables (from the read-only query) → packages/ui-tokens/source/tokens.json
// generateCss   tokens.json → CSS custom properties, chrome and canvas groups, one block per appearance
// generateTs    tokens.json → the TypeScript theme object
// checkGaps     tokens.json vs DESIGN.md "Visual system" through source/design-md.json; differences must
//               be recorded in DESIGN-GAPS.md (check=token-…) or the check fails
// contrast      WCAG 2.x relative luminance contrast ratio (guide §9 "Colour")
import { parseGaps } from "./design-check.mjs";
import { THEMES } from "./design-frames.mjs";

export const TOKENS_VERSION = 1;

// Which CSS block a token is written to. Chrome follows the host theme ("Match editor colours"); canvas
// meaning (selection, component, agent hues) never does, so it has its own appearance attribute
// (DESIGN.md "Build consistency": chrome and canvas tokens are separate groups).
export const COLOR_GROUPS = { chrome: "chrome", canvas: "canvas", agent: "canvas" };
// Mode-independent groups and their CSS unit. Numbers in the chrome group are sizes.
const UNITS = { space: "px", radius: "px", chrome: "px", motion: "ms", font: null };

export const groupOf = (name) => (name.includes("/") ? name.slice(0, name.indexOf("/")) : null);
export const cssVarName = (name) => `--${name.replace("/", "-")}`;

// One axis must hold every appearance; its values are the token modes (DESIGN-GAPS: `appearance`
// stands in for the guide's `Mode`).
function modeAxis(themes) {
  const entry = Object.entries(themes).find(([, values]) =>
    THEMES.every((t) => values.includes(t)),
  );
  if (!entry)
    throw new Error(`no theme axis holds all of ${THEMES.join(", ")}: ${JSON.stringify(themes)}`);
  return entry[0];
}

function readValue(name, def, axis) {
  if (!Array.isArray(def.value)) return { value: def.value };
  const values = {};
  for (const entry of def.value) {
    const mode = entry.theme?.[axis];
    if (!THEMES.includes(mode) || Object.keys(entry.theme).length !== 1)
      throw new Error(
        `${name}: value for theme ${JSON.stringify(entry.theme)} is not one appearance`,
      );
    values[mode] = def.type === "color" ? entry.value.toUpperCase() : entry.value;
  }
  const missing = THEMES.filter((t) => !(t in values));
  if (missing.length) throw new Error(`${name}: no value for ${missing.join(", ")}`);
  return { values: Object.fromEntries(THEMES.map((t) => [t, values[t]])) };
}

/**
 * raw: parseTokensRaw output. order: inventory.variables.names (the design file's own order, which the
 * Tokens sheet follows). Both must name the same variables, so a stale inventory cannot slip through.
 */
export function buildTokens({ raw, order, sourceSha256, exporter }) {
  const axis = modeAxis(raw.themes);
  const defs = new Map(raw.variables);
  const known = new Set(order);
  const extra = [...defs.keys()].filter((n) => !known.has(n));
  const absent = order.filter((n) => !defs.has(n));
  if (extra.length || absent.length)
    throw new Error(
      `the query and inventory.json name different variables (only in the query: ${extra.join(", ") || "none"}; only in the inventory: ${absent.join(", ") || "none"}). Regenerate the inventory first (pnpm design:snapshot).`,
    );
  const tokens = order.map((name) => {
    const def = defs.get(name);
    if (!["color", "number", "string"].includes(def.type))
      throw new Error(`${name}: unsupported variable type ${def.type}`);
    const value = readValue(name, def, axis);
    if (def.type === "color")
      for (const v of value.values ? Object.values(value.values) : [value.value])
        if (!/^#[0-9A-F]{6}([0-9A-F]{2})?$/i.test(v))
          throw new Error(`${name}: ${v} is not #RRGGBB(AA)`);
    return {
      name,
      group: groupOf(name),
      type: def.type,
      ...(value.values
        ? { values: value.values }
        : { value: def.type === "color" ? value.value.toUpperCase() : value.value }),
    };
  });
  const pairs = raw.pairs
    .map(([kind, foreground, background, count]) => ({
      kind,
      foreground,
      background:
        background === null
          ? null
          : background.startsWith("$")
            ? background.slice(1)
            : background.toUpperCase(),
      count,
    }))
    .sort((a, b) =>
      `${a.kind} ${a.foreground} ${a.background}`.localeCompare(
        `${b.kind} ${b.foreground} ${b.background}`,
      ),
    );
  return {
    version: TOKENS_VERSION,
    source_sha256: sourceSha256,
    exporter,
    axis,
    modes: THEMES,
    tokens,
    pairs,
  };
}

export const serializeTokens = (tokens) => `${JSON.stringify(tokens, null, 2)}\n`;

// Tokens that become CSS and TS: every grouped token. The five legacy names without a group
// (DESIGN-GAPS, `vellum-*`, used only by the landing page frame) stay in tokens.json and nowhere else.
export const generatedTokens = (tokens) => tokens.tokens.filter((t) => t.group !== null);

function scalarCss(token) {
  if (token.type === "string") return JSON.stringify(token.value);
  if (token.type === "color") return token.value;
  const unit = UNITS[token.group];
  if (unit === undefined) throw new Error(`${token.name}: no CSS unit for group ${token.group}`);
  return `${token.value}${unit ?? ""}`;
}

function header(tokens, comment) {
  return comment(
    `GENERATED by scripts/design-tokens from packages/ui-tokens/source/tokens.json (vellum.pen sha256 ${tokens.source_sha256}). Do not edit.`,
  );
}

export function generateCss(tokens) {
  const list = generatedTokens(tokens);
  const themed = list.filter((t) => t.values);
  const scalars = list.filter((t) => !t.values);
  for (const t of themed)
    if (!COLOR_GROUPS[t.group])
      throw new Error(`${t.name}: themed token outside chrome, canvas, agent`);
  const decl = (name, value) => `  ${cssVarName(name)}: ${value};`;
  const out = [header(tokens, (s) => `/* ${s} */`), ""];
  out.push(":root {", ...scalars.map((t) => decl(t.name, scalarCss(t))), "}");
  for (const [group, attribute] of [
    ["chrome", "data-appearance"],
    ["canvas", "data-canvas-appearance"],
  ]) {
    const members = themed.filter((t) => COLOR_GROUPS[t.group] === group);
    for (const mode of tokens.modes) {
      const selector = `[${attribute}="${mode}"]`;
      out.push("", mode === tokens.modes[0] ? `:root,\n${selector} {` : `${selector} {`);
      out.push(...members.map((t) => decl(t.name, t.values[mode])), "}");
    }
  }
  return `${out.join("\n")}\n`;
}

export function generateTs(tokens) {
  const list = generatedTokens(tokens);
  const q = JSON.stringify;
  const colors = list
    .filter((t) => t.values)
    .map(
      (t) =>
        `  ${q(t.name)}: { group: ${q(COLOR_GROUPS[t.group])}, values: { ${tokens.modes.map((m) => `${q(m)}: ${q(t.values[m])}`).join(", ")} } },`,
    );
  const scalars = list
    .filter((t) => !t.values)
    .map((t) => {
      const unit = t.type === "number" ? UNITS[t.group] : null;
      return `  ${q(t.name)}: { type: ${q(t.type)}, value: ${q(t.value)}, unit: ${q(unit ?? null)} },`;
    });
  return [
    header(tokens, (s) => `// ${s}`),
    "",
    `export const SOURCE_SHA256 = ${q(tokens.source_sha256)};`,
    "",
    `export const MODES = [${tokens.modes.map(q).join(", ")}] as const;`,
    "",
    "/** Themed colours in the design file's order. `group` picks the appearance attribute that sets them. */",
    "export const colors = {",
    ...colors,
    "} as const;",
    "",
    "/** Mode-independent tokens: fonts, space, radius, sizes and motion. */",
    "export const scalars = {",
    ...scalars,
    "} as const;",
    "",
  ].join("\n");
}

// --- DESIGN.md vs the design file ---

const section = (md, heading) => {
  const start = md.indexOf(`\n${heading}\n`);
  if (start === -1) throw new Error(`DESIGN.md has no "${heading}" section`);
  // Ends at the next heading of the same or a higher level.
  const level = heading.indexOf(" ");
  const body = start + heading.length + 2;
  const next = md.slice(body).search(new RegExp(`\\n#{1,${level}} `));
  return md.slice(start, next === -1 ? undefined : body + next);
};

// Rows of the first Markdown table in `text` whose header starts with `firstHeader`.
function tableRows(text, firstHeader) {
  const lines = text.split("\n");
  const start = lines.findIndex((l) => l.startsWith(`| ${firstHeader} |`));
  if (start === -1) throw new Error(`DESIGN.md has no table headed "${firstHeader}"`);
  const rows = [];
  for (const line of lines.slice(start + 2)) {
    if (!line.startsWith("|")) break;
    rows.push(
      line
        .slice(1, -1)
        .split("|")
        .map((c) => c.trim().replace(/\*\*/g, "")),
    );
  }
  return rows;
}

const hexes = (cell) => [...cell.matchAll(/#[0-9A-Fa-f]{6}\b/g)].map((m) => m[0].toUpperCase());
const slug = (s) =>
  s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");

/** What DESIGN.md "Visual system" defines, read through the role map in source/design-md.json. */
export function designMdTokens(designMd, map) {
  const expected = []; // { token, light?, dark?, value?, multipleOf?, source }
  const missing = []; // { key, message }
  const visual = section(designMd, "## Visual system");
  for (const [role, light, dark] of tableRows(section(visual, "### Color"), "Role")) {
    const names = map.colors[role];
    if (!names) {
      missing.push({
        key: `token-missing:${slug(role)}`,
        message: `DESIGN.md colour role "${role}" has no token in source/design-md.json`,
      });
      continue;
    }
    const l = hexes(light);
    const d = hexes(dark);
    if (l.length !== names.length || d.length !== names.length)
      throw new Error(
        `design-md.json maps "${role}" to ${names.length} tokens but DESIGN.md gives ${l.length}/${d.length} colours`,
      );
    names.forEach((token, i) => {
      expected.push({ token, light: l[i], dark: d[i], source: `Color "${role}"` });
    });
  }
  const color = section(visual, "### Color");
  for (const [hue, light, dark] of tableRows(
    color.slice(color.indexOf("**Agent colours.**")),
    "Hue",
  ))
    expected.push({
      token: `agent/${slug(hue)}`,
      light: hexes(light)[0],
      dark: hexes(dark)[0],
      source: `Agent colour "${hue}"`,
    });
  for (const [token, value] of tableRows(section(visual, "### Spacing and shape"), "Token"))
    expected.push({
      token: token.replace(/^radius-/, "radius/"),
      value: Number.parseFloat(value),
      source: `"${token}"`,
    });
  for (const [role] of tableRows(section(visual, "### Typography"), "Role")) {
    const names = map.type?.[role];
    if (!names)
      missing.push({
        key: `token-missing:type/${slug(role)}`,
        message: `DESIGN.md type role "${role}" has no variable in vellum.pen`,
      });
  }
  for (const entry of map.prose) {
    if (!designMd.includes(entry.quote))
      throw new Error(
        `source/design-md.json quotes "${entry.quote}" for ${entry.token}, but DESIGN.md no longer says it`,
      );
    expected.push({ ...entry, source: `"${entry.quote}"` });
  }
  return { expected, missing };
}

/** Findings for every difference; a difference recorded in DESIGN-GAPS.md is a warning. */
export function checkGaps({ tokens, designMd, map, gapsText }) {
  const recorded = parseGaps(gapsText);
  const errors = [];
  const warnings = [];
  const report = (message, key) =>
    (recorded.has(key) ? warnings : errors).push({ message: `${message} [check=${key}]`, key });
  const byName = new Map(tokens.tokens.map((t) => [t.name, t]));
  const { expected, missing } = designMdTokens(designMd, map);
  for (const m of missing) report(m.message, m.key);

  const referenced = new Set();
  for (const e of expected) {
    referenced.add(e.token);
    const token = byName.get(e.token);
    if (!token) {
      report(
        `DESIGN.md ${e.source} defines ${e.token}, which vellum.pen does not have`,
        `token-missing:${e.token}`,
      );
      continue;
    }
    const differences = [];
    if (e.light !== undefined)
      for (const mode of ["light", "dark"])
        if (token.values?.[mode] !== e[mode])
          differences.push(
            `${mode}: DESIGN.md ${e[mode]}, vellum.pen ${token.values?.[mode] ?? token.value}`,
          );
    if (e.value !== undefined && token.value !== e.value)
      differences.push(
        `DESIGN.md ${e.value}, vellum.pen ${token.value ?? JSON.stringify(token.values)}`,
      );
    if (e.multipleOf !== undefined && token.value % e.multipleOf !== 0)
      differences.push(`${token.value} is not a multiple of ${e.multipleOf}`);
    if (differences.length)
      report(
        `${e.token} differs from DESIGN.md ${e.source} (${differences.join("; ")})`,
        `token-value:${e.token}`,
      );
  }
  for (const token of tokens.tokens) {
    if (referenced.has(token.name)) continue;
    // Ungrouped legacy names are already recorded as variable-name gaps by design-check.
    const key = token.group === null ? `variable-name:${token.name}` : `token-extra:${token.name}`;
    report(`vellum.pen defines ${token.name}, which DESIGN.md "Visual system" does not`, key);
  }
  return { errors, warnings };
}

// --- Contrast (WCAG 2.x) ---

const channel = (c) => {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
};

export function luminance(hex) {
  const [r, g, b] = [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16));
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

export function contrast(a, b) {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

// Text needs 4.5:1 at every size (nothing in Vellum is large text: the biggest role is 20 px). Icons,
// focus rings and canvas marks need 3:1 (WCAG 1.4.11).
export const MINIMUM = { text: 4.5, icon: 3, ui: 3 };

/**
 * Every pair the design file uses (tokens.pairs, both sides variables) plus the declared pairs of
 * source/contrast-pairs.json (what DESIGN.md's roles allow that no frame happens to show yet), checked in
 * every appearance. Pairs on a literal (unbound) background are listed as skipped: the lint checks tokens.
 */
export function checkContrast({ tokens, declared }) {
  const byName = new Map(tokens.tokens.filter((t) => t.group !== null).map((t) => [t.name, t]));
  const pairs = new Map();
  const skipped = [];
  for (const p of tokens.pairs) {
    if (!byName.has(p.foreground) || !byName.has(p.background ?? "")) {
      skipped.push(p);
      continue;
    }
    pairs.set(`${p.kind}|${p.foreground}|${p.background}`, { ...p, from: "vellum.pen" });
  }
  for (const [kind, list] of Object.entries(declared.pairs))
    for (const [foreground, background] of list) {
      for (const name of [foreground, background])
        if (!byName.has(name))
          throw new Error(`contrast-pairs.json names ${name}, which is not a token`);
      const key = `${kind}|${foreground}|${background}`;
      if (!pairs.has(key))
        pairs.set(key, { kind, foreground, background, from: "contrast-pairs.json" });
    }
  const errors = [];
  const checked = [];
  for (const p of pairs.values()) {
    const minimum = MINIMUM[p.kind];
    if (minimum === undefined) throw new Error(`unknown pair kind ${p.kind}`);
    for (const mode of tokens.modes) {
      const fg = byName.get(p.foreground);
      const bg = byName.get(p.background);
      const a = fg.values?.[mode] ?? fg.value;
      const b = bg.values?.[mode] ?? bg.value;
      if (a.length !== 7 || b.length !== 7)
        throw new Error(`${p.foreground} on ${p.background}: contrast needs opaque colours`);
      const ratio = contrast(a, b);
      checked.push({ ...p, mode, ratio });
      if (ratio < minimum)
        errors.push({
          message: `${p.kind} ${p.foreground} on ${p.background} (${mode}): ${a} on ${b} is ${ratio.toFixed(2)}:1, needs ${minimum}:1 [${p.from}]`,
        });
    }
  }
  return { errors, checked, skipped };
}
