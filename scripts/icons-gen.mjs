#!/usr/bin/env node
// Builds crates/adapters/icons/data/*.json from the open-source icon packages (guide Prompt 42, ADR-009's sibling
// for icons). The generated files are committed; run this only to update them.
//
//   mkdir sources && cd sources
//   for p in lucide-static@1.59.0 feather-icons@4.29.2 @phosphor-icons/core@2.1.1 @material-symbols/svg-400@0.48.0; do
//     d=$(echo "${p%@*}" | tr '/@' '__'); mkdir -p "$d" && (cd "$d" && npm pack "$p" --silent && tar xzf *.tgz); done
//   curl -sO https://raw.githubusercontent.com/feathericons/feather/main/src/tags.json   # Feather's tags are not in its package
//   curl -s https://fonts.google.com/metadata/icons -o google-icons.json                 # which Material icons people use, and their tags
//   node scripts/icons-gen.mjs sources
//
// Every set is converted to SVG path data (circles, rects, lines and polylines become paths), so the app needs no
// SVG parser to draw an icon. Licences are recorded in each file and in THIRD_PARTY_NOTICES.md.
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(repo, "crates/adapters/icons/data");
const src = resolve(process.argv[2] ?? "sources");
const read = (...p) => readFileSync(join(src, ...p), "utf8");
const version = (dir) => JSON.parse(read(dir, "package", "package.json")).version;

/** How many Material Symbols are shipped: the most used by people (Google's own ranking), not all 3,900. */
const MATERIAL_COUNT = 700;

const n = (v) => String(+(+v).toFixed(3));
const attrs = (tag) =>
  Object.fromEntries([...tag.matchAll(/([\w:-]+)="([^"]*)"/g)].map((m) => [m[1], m[2]]));

const NUMBER = /[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?/y;

/**
 * A path that starts with a relative `m` means "absolute" only when it is the first thing in its own path
 * element. Joined behind another element it would be relative to where that one ended, so the first move is
 * written absolute and the pairs after it, which stay relative, become an explicit `l`.
 */
function absoluteStart(d) {
  const text = d.trim();
  if (!text.startsWith("m")) return text;
  NUMBER.lastIndex = 1;
  const x = NUMBER.exec(
    text.replace(/^m[\s,]*/, (m) => {
      NUMBER.lastIndex = m.length;
      return m;
    }),
  );
  const afterX = NUMBER.lastIndex;
  const skip = text.slice(afterX).match(/^[\s,]*/)[0].length;
  NUMBER.lastIndex = afterX + skip;
  const y = NUMBER.exec(text);
  if (!x || !y) throw new Error(`cannot read the first move of ${d}`);
  const rest = text.slice(NUMBER.lastIndex);
  const more = /^[\s,]*[-+.\d]/.test(rest) ? "l" + rest.replace(/^[\s,]+/, "") : rest;
  return `M${x[0]} ${y[0]}${more}`;
}

function toPath(tag, a) {
  const g = (k, d = 0) => (a[k] === undefined ? d : parseFloat(a[k]));
  switch (tag) {
    case "path":
      return absoluteStart(a.d);
    case "circle": {
      const [cx, cy, r] = [g("cx"), g("cy"), g("r")];
      return `M${n(cx - r)} ${n(cy)}a${n(r)} ${n(r)} 0 1 0 ${n(2 * r)} 0a${n(r)} ${n(r)} 0 1 0 ${n(-2 * r)} 0`;
    }
    case "ellipse": {
      const [cx, cy, rx, ry] = [g("cx"), g("cy"), g("rx"), g("ry")];
      return `M${n(cx - rx)} ${n(cy)}a${n(rx)} ${n(ry)} 0 1 0 ${n(2 * rx)} 0a${n(rx)} ${n(ry)} 0 1 0 ${n(-2 * rx)} 0`;
    }
    case "rect": {
      const [x, y, w, h] = [g("x"), g("y"), g("width"), g("height")];
      let rx = g("rx", g("ry")),
        ry = g("ry", rx);
      rx = Math.min(rx, w / 2);
      ry = Math.min(ry, h / 2);
      if (!rx || !ry) return `M${n(x)} ${n(y)}h${n(w)}v${n(h)}h${n(-w)}z`;
      return (
        `M${n(x + rx)} ${n(y)}h${n(w - 2 * rx)}a${n(rx)} ${n(ry)} 0 0 1 ${n(rx)} ${n(ry)}v${n(h - 2 * ry)}` +
        `a${n(rx)} ${n(ry)} 0 0 1 ${n(-rx)} ${n(ry)}h${n(-(w - 2 * rx))}a${n(rx)} ${n(ry)} 0 0 1 ${n(-rx)} ${n(-ry)}` +
        `v${n(-(h - 2 * ry))}a${n(rx)} ${n(ry)} 0 0 1 ${n(rx)} ${n(-ry)}z`
      );
    }
    case "line":
      return `M${n(g("x1"))} ${n(g("y1"))}L${n(g("x2"))} ${n(g("y2"))}`;
    case "polyline":
    case "polygon": {
      const p = a.points
        .trim()
        .split(/[\s,]+/)
        .map(Number);
      let d = "";
      for (let i = 0; i + 1 < p.length; i += 2) d += `${i ? "L" : "M"}${n(p[i])} ${n(p[i + 1])}`;
      return tag === "polygon" ? d + "z" : d;
    }
    default:
      throw new Error(`unsupported SVG element <${tag}>`);
  }
}

const svgPaths = (svg) =>
  [...svg.matchAll(/<(path|circle|ellipse|rect|line|polyline|polygon)\b([^>]*?)\/?>/g)]
    .map((m) => toPath(m[1], attrs(m[2])))
    .join("");

const tagsOf = (list, name) => [
  ...new Set((list ?? []).map((t) => String(t).toLowerCase()).filter((t) => t !== name)),
];
const byName = (a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0);

function write(file, meta, icons) {
  icons.sort(byName);
  const body = icons.map((i) => `  ${JSON.stringify(i)}`).join(",\n");
  const head = JSON.stringify(meta).slice(0, -1);
  writeFileSync(join(out, file), `${head},"icons":[\n${body}\n]}\n`);
  console.log(file, icons.length, "icons");
}

mkdirSync(out, { recursive: true });

// Lucide: stroke icons. icon-nodes.json already lists each icon's elements.
{
  const dir = "lucide-static";
  const nodes = JSON.parse(read(dir, "package", "icon-nodes.json"));
  const tags = JSON.parse(read(dir, "package", "tags.json"));
  const icons = Object.entries(nodes).map(([name, els]) => [
    name,
    tagsOf(tags[name], name),
    els.map(([t, a]) => toPath(t, a)).join(""),
  ]);
  write(
    "lucide.json",
    {
      id: "lucide",
      label: "Lucide",
      style: "stroke",
      viewBox: [0, 0, 24, 24],
      weights: [100, 200, 300, 400, 500, 600, 700],
      fill: false,
      source: "https://github.com/lucide-icons/lucide",
      package: `lucide-static@${version(dir)}`,
      licence: "ISC",
    },
    icons,
  );
}

// Feather: stroke icons, the same drawing grid.
{
  const dir = "feather-icons";
  const files = readdirSync(join(src, dir, "package", "dist", "icons")).filter((f) =>
    f.endsWith(".svg"),
  );
  const tags = JSON.parse(readFileSync(join(src, "tags.json"), "utf8"));
  const icons = files.map((f) => {
    const name = f.slice(0, -4);
    return [name, tagsOf(tags[name], name), svgPaths(read(dir, "package", "dist", "icons", f))];
  });
  write(
    "feather.json",
    {
      id: "feather",
      label: "Feather",
      style: "stroke",
      viewBox: [0, 0, 24, 24],
      weights: [100, 200, 300, 400, 500, 600, 700],
      fill: false,
      source: "https://github.com/feathericons/feather",
      package: `feather-icons@${version(dir)}`,
      licence: "MIT",
    },
    icons,
  );
}

// Phosphor: filled outlines at four weights, and a filled variant.
{
  const dir = "_phosphor-icons_core";
  const core = await import(pathToFileURL(join(src, dir, "package", "dist", "index.mjs")).href);
  const assets = join(src, dir, "package", "assets");
  const weights = { thin: 100, light: 300, regular: 400, bold: 700 };
  const icons = [];
  for (const meta of core.icons) {
    const name = meta.name;
    const variants = {};
    for (const [w, num] of Object.entries(weights)) {
      const file = join(assets, w, w === "regular" ? `${name}.svg` : `${name}-${w}.svg`);
      if (existsSync(file)) variants[num] = svgPaths(readFileSync(file, "utf8"));
    }
    const fill = join(assets, "fill", `${name}-fill.svg`);
    if (existsSync(fill)) variants.fill = svgPaths(readFileSync(fill, "utf8"));
    if (variants[400]) icons.push([name, tagsOf(meta.tags, name), variants]);
  }
  write(
    "phosphor.json",
    {
      id: "phosphor",
      label: "Phosphor",
      style: "fill",
      viewBox: [0, 0, 256, 256],
      weights: [100, 300, 400, 700],
      fill: true,
      source: "https://github.com/phosphor-icons/core",
      package: `@phosphor-icons/core@${version(dir)}`,
      licence: "MIT",
    },
    icons,
  );
}

// Material Symbols: three styles, weight 400, outlined and filled, for the most used icons.
{
  const dir = "_material-symbols_svg-400";
  const raw = readFileSync(join(src, "google-icons.json"), "utf8").replace(/^\)\]\}'\n/, "");
  const ranked = JSON.parse(raw).icons.sort(
    (a, b) => b.popularity - a.popularity || (a.name < b.name ? -1 : 1),
  );
  for (const style of ["outlined", "rounded", "sharp"]) {
    const folder = join(src, dir, "package", style);
    const icons = [];
    for (const meta of ranked) {
      if (icons.length >= MATERIAL_COUNT) break;
      const file = join(folder, `${meta.name}.svg`);
      if (!existsSync(file)) continue;
      const variants = { 400: svgPaths(readFileSync(file, "utf8")) };
      const fill = join(folder, `${meta.name}-fill.svg`);
      if (existsSync(fill)) variants.fill = svgPaths(readFileSync(fill, "utf8"));
      icons.push([meta.name, tagsOf(meta.tags, meta.name), variants]);
    }
    write(
      `material-symbols-${style}.json`,
      {
        id: `material-symbols-${style}`,
        label: `Material Symbols ${style[0].toUpperCase()}${style.slice(1)}`,
        style: "fill",
        viewBox: [0, -960, 960, 960],
        weights: [400],
        fill: true,
        source: "https://github.com/marella/material-symbols",
        package: `@material-symbols/svg-400@${version(dir)}`,
        licence: "Apache-2.0",
      },
      icons,
    );
  }
}
