// Draws the brand mark as the icons of the apps: the favicons of the web apps and the docs, the Electron app icon
// and the extension's marketplace icon. The colours come from the chrome tokens in packages/ui-tokens, so the icons
// follow the same palette as the chrome in light and dark appearance. Needs macOS: the rasters are made with sips
// and the .icns file with iconutil. Run `node scripts/brand-icons.mjs` after a change to the mark or the tokens.
import { spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.platform !== "darwin") throw Error("brand-icons needs macOS (sips and iconutil)");

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const tokens = await readFile(join(root, "packages/ui-tokens/src/generated/tokens.css"), "utf8");

// The chrome colours of one appearance, read from the generated token file so no value is typed here.
function chrome(appearance) {
  const block = tokens.split(`[data-appearance="${appearance}"] {`)[1]?.split("}")[0];
  if (!block) throw Error(`tokens.css has no ${appearance} block`);
  const value = (name) => {
    const hex = block.match(new RegExp(`--chrome-${name}:\\s*(#[0-9A-Fa-f]{6});`))?.[1];
    if (!hex) throw Error(`tokens.css has no --chrome-${name} in ${appearance}`);
    return hex;
  };
  return {
    panel: value("panel"),
    border: value("border"),
    text: value("text"),
    secondary: value("secondary"),
  };
}
const light = chrome("light");
const dark = chrome("dark");

// The mark is a rounded square for the agent and a circle for the person (AGENTS.md, visual language). The app icon
// centres it on a tile of 816 units in a 1024 canvas, at 22 units to one unit of the 24-unit mark.
const MARK_TRANSFORM = "translate(512 512) scale(22) translate(-12 -12)";
const TILE = `x="104" y="104" width="816" height="816" rx="184"`;

// Presentation attributes only: sips does not apply a style sheet, so the raster source must carry its colours.
function plainIcon(colours) {
  return [
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">`,
    `<rect ${TILE} fill="${colours.panel}" stroke="${colours.border}" stroke-width="4"/>`,
    `<g transform="${MARK_TRANSFORM}">`,
    `<rect x="2" y="2" width="14" height="14" rx="4" fill="${colours.text}"/>`,
    `<circle cx="16" cy="16" r="6" fill="${colours.secondary}"/>`,
    `</g></svg>`,
  ].join("");
}

// The source file for the web: it switches to the dark colours where the browser prefers dark.
function adaptiveIcon() {
  return [
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">`,
    `<style>`,
    `.tile{fill:${light.panel};stroke:${light.border};stroke-width:4}`,
    `.agent{fill:${light.text}}.person{fill:${light.secondary}}`,
    `@media (prefers-color-scheme: dark){`,
    `.tile{fill:${dark.panel};stroke:${dark.border}}`,
    `.agent{fill:${dark.text}}.person{fill:${dark.secondary}}`,
    `}</style>`,
    `<rect class="tile" ${TILE}/>`,
    `<g transform="${MARK_TRANSFORM}">`,
    `<rect class="agent" x="2" y="2" width="14" height="14" rx="4"/>`,
    `<circle class="person" cx="16" cy="16" r="6"/>`,
    `</g></svg>`,
  ].join("");
}

// The mark alone, without the tile, for the places that draw it on their own.
function adaptiveMark() {
  return [
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">`,
    `<style>`,
    `.agent{fill:${light.text}}.person{fill:${light.secondary}}`,
    `@media (prefers-color-scheme: dark){.agent{fill:${dark.text}}.person{fill:${dark.secondary}}}`,
    `</style>`,
    `<rect class="agent" x="2" y="2" width="14" height="14" rx="4"/>`,
    `<circle class="person" cx="16" cy="16" r="6"/>`,
    `</svg>`,
  ].join("");
}

// An .ico file holding PNG entries, which Windows reads from Windows Vista on. A width of 256 is stored as 0.
function ico(entries) {
  const header = Buffer.alloc(6 + 16 * entries.length);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(entries.length, 4);
  let offset = header.length;
  entries.forEach(({ size, png }, index) => {
    const at = 6 + 16 * index;
    const byte = size === 256 ? 0 : size;
    header[at] = byte;
    header[at + 1] = byte;
    header.writeUInt16LE(1, at + 4);
    header.writeUInt16LE(32, at + 6);
    header.writeUInt32LE(png.length, at + 8);
    header.writeUInt32LE(offset, at + 12);
    offset += png.length;
  });
  return Buffer.concat([header, ...entries.map(({ png }) => png)]);
}

const ICONSET = [
  ["icon_16x16.png", 16],
  ["icon_16x16@2x.png", 32],
  ["icon_32x32.png", 32],
  ["icon_32x32@2x.png", 64],
  ["icon_128x128.png", 128],
  ["icon_128x128@2x.png", 256],
  ["icon_256x256.png", 256],
  ["icon_256x256@2x.png", 512],
  ["icon_512x512.png", 512],
  ["icon_512x512@2x.png", 1024],
];
const SIZES = [16, 32, 48, 64, 128, 180, 256, 512, 1024];

const work = await mkdtemp(join(tmpdir(), "vellum-icons-"));
try {
  const source = join(work, "app-icon-plain.svg");
  await writeFile(source, plainIcon(light));
  const raster = new Map();
  for (const size of SIZES) {
    const output = join(work, `${size}.png`);
    const result = spawnSync(
      "sips",
      ["-s", "format", "png", "-z", `${size}`, `${size}`, source, "--out", output],
      {
        encoding: "utf8",
      },
    );
    if (result.status !== 0) throw Error(`sips failed at ${size}px: ${result.stderr}`);
    raster.set(size, await readFile(output));
  }

  const iconset = join(work, "AppIcon.iconset");
  await mkdir(iconset);
  for (const [name, size] of ICONSET) await writeFile(join(iconset, name), raster.get(size));
  const icns = join(work, "icon.icns");
  const made = spawnSync("iconutil", ["-c", "icns", "-o", icns, iconset], { encoding: "utf8" });
  if (made.status !== 0) throw Error(`iconutil failed: ${made.stderr}`);

  const written = [];
  async function put(path, data) {
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, data);
    written.push(path.slice(root.length + 1));
  }
  const favicon = ico([16, 32, 48, 256].map((size) => ({ size, png: raster.get(size) })));
  const desktopIco = ico(
    [16, 32, 48, 64, 128, 256].map((size) => ({ size, png: raster.get(size) })),
  );

  await put(join(root, "packages/config/assets/mark.svg"), adaptiveMark());
  await put(join(root, "packages/config/assets/app-icon.svg"), adaptiveIcon());
  for (const app of ["apps/docs", "apps/web", "apps/account"]) {
    await put(join(root, app, "app/icon.svg"), adaptiveIcon());
    await put(join(root, app, "app/favicon.ico"), favicon);
    await put(join(root, app, "app/apple-icon.png"), raster.get(180));
  }
  await put(join(root, "apps/desktop/build/icon.icns"), await readFile(icns));
  await put(join(root, "apps/desktop/build/icon.ico"), desktopIco);
  await put(join(root, "apps/desktop/build/icon.png"), raster.get(512));
  await put(join(root, "apps/extension/media/icon.png"), raster.get(128));
  console.log(`brand-icons: wrote ${written.length} files`);
  for (const path of written) console.log(`  ${path}`);
} finally {
  await rm(work, { recursive: true, force: true });
}
