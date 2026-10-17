/**
 * Design tokens: light, dark, high contrast light and dark; chrome and canvas groups.
 *
 * Generated from the design file's variables (packages/ui-tokens/source/tokens.json, made by
 * scripts/design-tokens). `./tokens.css` sets them as CSS custom properties: chrome tokens follow
 * `data-appearance`, canvas tokens (canvas and agent hues) follow `data-canvas-appearance`, so the chrome
 * can match a host theme without changing what the canvas colours mean.
 */
import { colors, MODES, SOURCE_SHA256, scalars } from "./generated/tokens";

export { colors, MODES, SOURCE_SHA256, scalars };

export const PACKAGE_NAME = "@vellum/ui-tokens";

export type Mode = (typeof MODES)[number];
export type ColorToken = keyof typeof colors;
export type ScalarToken = keyof typeof scalars;
export type Token = ColorToken | ScalarToken;
export type ColorGroup = (typeof colors)[ColorToken]["group"];

/** The attribute that sets each colour group's appearance. */
export const APPEARANCE_ATTRIBUTE: Record<ColorGroup, string> = {
  chrome: "data-appearance",
  canvas: "data-canvas-appearance",
};

export const isMode = (value: string): value is Mode =>
  (MODES as readonly string[]).includes(value);

/** `chrome/panel` → `--chrome-panel`. */
export const cssVarName = (token: Token): string => `--${token.replace("/", "-")}`;

/** `chrome/panel` → `var(--chrome-panel)`, for styles that follow the appearance attributes. */
export const cssVar = (token: Token): string => `var(${cssVarName(token)})`;

/** A colour's value in one appearance, as `#RRGGBB` or `#RRGGBBAA`. */
export const colorValue = (token: ColorToken, mode: Mode): string => colors[token].values[mode];

/** A scalar token as CSS: `4px`, `120ms`, or the font family name. */
export function scalarCss(token: ScalarToken): string {
  const { value, unit } = scalars[token];
  return typeof value === "string" ? JSON.stringify(value) : `${value}${unit ?? ""}`;
}

/** Every colour token in one appearance (for canvases and other non-CSS consumers). */
export function theme(mode: Mode): Record<ColorToken, string> {
  return Object.fromEntries(
    (Object.keys(colors) as ColorToken[]).map((t) => [t, colors[t].values[mode]]),
  ) as Record<ColorToken, string>;
}

/** Agent hue names in the design file's order (`teal`, `green`, …), from the `agent/*` tokens. */
export const agentHues: readonly string[] = (Object.keys(colors) as ColorToken[])
  .filter((t) => t.startsWith("agent/"))
  .map((t) => t.slice("agent/".length));
