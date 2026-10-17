/**
 * The token page: every colour token in all four appearances, the type scale and the spacing, shape and
 * motion notes. It reproduces the design file's Tokens sheet ("Foundations · Tokens · all appearances",
 * 1600 px wide) so `scripts/token-page-compare.mjs` can compare the two. Geometry is the sheet's; every
 * colour and number on it comes from the tokens. Needs `./generated/tokens.css` and the Inter and
 * JetBrains Mono faces on the page.
 */
import { brand } from "@vellum/config";
import {
  type ColorToken,
  colors,
  colorValue,
  cssVar,
  MODES,
  type Mode,
  type ScalarToken,
  scalars,
} from "./index";

export const TOKEN_PAGE_WIDTH = 1600;

// The fonts' own line heights ((ascender + descender) / units per em, from their hhea tables). The sheet
// keeps them for the samples without a line height, even when a fallback face (Arabic) is taller.
const INTER_LINE_HEIGHT = (1984 + 494) / 2048;
const MONO_LINE_HEIGHT = (1020 + 300) / 1000;

const MODE_LABELS: Record<Mode, string> = {
  light: "Light",
  dark: "Dark",
  "hc-light": "High contrast light",
  "hc-dark": "High contrast dark",
};

const html = (s: string) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const n = (token: ScalarToken) => scalars[token].value;

// Strokes are centred on the edge, as in the design file (1 px: half inside, half outside).

// Opaque colours only: the elevation colour is a shadow tint, which the sheet does not list.
export const swatchTokens = (): ColorToken[] =>
  (Object.keys(colors) as ColorToken[]).filter((t) =>
    MODES.every((m) => colorValue(t, m).length === 7),
  );

const STYLE = `
.tp{box-sizing:border-box;width:${TOKEN_PAGE_WIDTH}px;padding:40px;display:flex;flex-direction:column;gap:28px;background:${cssVar("chrome/panel")};color:${cssVar("chrome/text")};font-family:${cssVar("font/ui")};-webkit-font-smoothing:antialiased}
.tp *{box-sizing:border-box;margin:0}
.tp-title{font-size:28px;font-weight:650;line-height:normal}
.tp-lede{font-size:16px;font-weight:450;line-height:1.5;color:${cssVar("chrome/secondary")}}
.tp-row{display:flex;gap:8px;align-items:center;height:36px}
.tp-head{height:40px}
.tp-head span{width:280px;font-size:12px;font-weight:600;line-height:normal;color:${cssVar("chrome/secondary")}}
.tp-head span:first-child,.tp-name{width:320px}
.tp-token{box-shadow:0 0.5px 0 ${cssVar("chrome/separator")},inset 0 -0.5px 0 ${cssVar("chrome/separator")}}
.tp-name,.tp-hex{font-family:${cssVar("font/mono")};font-size:12px;font-weight:450;line-height:normal}
.tp-cell{width:280px;display:flex;gap:10px;align-items:center}
.tp-swatch{width:24px;height:24px;border-radius:${n("radius/xs")}px;box-shadow:0 0 0 0.5px ${cssVar("chrome/tertiary")},inset 0 0 0 0.5px ${cssVar("chrome/tertiary")}}
.tp-foot{display:flex;gap:40px;padding:12px 0}
.tp-col{width:740px;display:flex;flex-direction:column}
.tp-type{gap:14px}.tp-geo{gap:12px}
.tp-col h2{font-size:20px;font-weight:600;line-height:normal}
.tp-geo p{font-size:12px;font-weight:450;line-height:normal;color:${cssVar("chrome/secondary")}}
.tp-note{font-size:12px;font-weight:450;line-height:1.5;color:${cssVar("chrome/secondary")}}
`;

// The type samples the sheet shows: role, size, weight, line height.
const TYPE_SAMPLES: [string, number, number, number][] = [
  ["Window title", 20, 650, 1.25],
  ["Dialog title", 15, 600, 1.3],
  ["Panel body", 12, 450, 1.4],
  ["Panel caption", 11, 500, 1.35],
  ["Prose", 14, 400, 1.5],
];

// One line of text. Its box is rounded up to whole pixels, as the design file sizes text boxes.
const line = (text: string, size: number, lineHeight: number, style: string) =>
  `<p style="font-size:${size}px;line-height:${lineHeight};height:${Math.ceil(size * lineHeight)}px;${style}" dir="auto">${html(text)}</p>`;

function typeColumn(): string {
  const samples = TYPE_SAMPLES.map(([role, size, weight, lineHeight]) =>
    line(
      `${role}  /  ${size} px · ${weight} · ${lineHeight}`,
      size,
      lineHeight,
      `font-weight:${weight}`,
    ),
  );
  const mono = line(
    `${n("font/mono")} / exact paths, values and code`,
    12.5,
    MONO_LINE_HEIGHT,
    `font-family:${cssVar("font/mono")};font-weight:450`,
  );
  const arabic = line(
    `${n("font/arabic")} · إعدادات التصميم`,
    14,
    INTER_LINE_HEIGHT,
    "font-weight:450",
  );
  return `<div class="tp-col tp-type"><h2>Typography</h2>${samples.join("")}${mono}${arabic}</div>`;
}

function geometryColumn(): string {
  const radii = (["radius/xs", "radius/sm", "radius/md", "radius/lg"] as const).map(n).join(" / ");
  const lines = [
    `${n("space/1")}-point grid · panel padding ${n("space/3")} px`,
    `Toolbar ${n("chrome/toolbar-height")} · status ${n("chrome/status-height")} · values/layers ${n("chrome/layer-height")} · ledger ${n("chrome/ledger-height")}`,
    `Radii ${radii} / full`,
    "Focus: 2 px ring + 1 px offset",
    "Direct manipulation: next frame, no easing",
    `Insert: ${n("motion/insert-fade")} ms fade + ${n("motion/insert-tint")} ms tint`,
    `Cursor: ${n("motion/cursor")} ms · panels: ${n("motion/panel")} ms`,
    `Dialogs/toasts: ${n("motion/dialog")} ms · reduced motion: static`,
  ];
  return `<div class="tp-col tp-geo"><h2>Spacing, shape, focus &amp; motion</h2>${lines.map((l) => `<p>${html(l)}</p>`).join("")}</div>`;
}

/** The page as an HTML fragment with its own style element, in the chrome's light appearance. */
export function renderTokenPage(): string {
  const head = `<div class="tp-row tp-head"><span>Token / role</span>${MODES.map((m) => `<span>${MODE_LABELS[m]}</span>`).join("")}</div>`;
  const rows = swatchTokens().map((token) => {
    const cells = MODES.map((mode) => {
      const value = colorValue(token, mode);
      return `<div class="tp-cell"><div class="tp-swatch" style="background:${value}"></div><span class="tp-hex">${value}</span></div>`;
    });
    return `<div class="tp-row tp-token"><span class="tp-name">${token}</span>${cells.join("")}</div>`;
  });
  return `<style>${STYLE}</style>
<div class="tp" data-appearance="light" data-canvas-appearance="light">
<h1 class="tp-title">${html(brand.name)} / Foundations</h1>
<p class="tp-lede">Quiet chrome. A visible canvas. Selection blue, component violet, and attributed agent hues. High contrast light and dark are first-class appearances.</p>
${head}
${rows.join("\n")}
<div class="tp-foot">${typeColumn()}${geometryColumn()}</div>
<p class="tp-note">Secondary and tertiary text remain at least 4.5:1. Agent hue is an outline/marker; name tags use neutral text on panel fill. No essential meaning depends on color. Numeric fields stay LTR in Arabic. Properties do not inherit.</p>
</div>`;
}
