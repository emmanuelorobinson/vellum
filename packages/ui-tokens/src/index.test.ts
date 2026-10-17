import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import source from "../source/tokens.json";
import {
  APPEARANCE_ATTRIBUTE,
  agentHues,
  type ColorToken,
  colors,
  colorValue,
  cssVar,
  cssVarName,
  isMode,
  MODES,
  PACKAGE_NAME,
  SOURCE_SHA256,
  scalarCss,
  scalars,
  theme,
} from "./index";
import { renderTokenPage, swatchTokens } from "./token-page";

const css = readFileSync(new URL("./generated/tokens.css", import.meta.url), "utf8");
const grouped = source.tokens.filter((t) => t.group !== null);

describe("@vellum/ui-tokens", () => {
  it("identifies itself", () => {
    expect(PACKAGE_NAME).toBe("@vellum/ui-tokens");
  });

  it("is generated from the committed tokens.json", () => {
    expect(SOURCE_SHA256).toBe(source.source_sha256);
    expect(MODES).toEqual(source.modes);
    expect([...Object.keys(colors), ...Object.keys(scalars)].sort()).toEqual(
      grouped.map((t) => t.name).sort(),
    );
    for (const t of grouped) {
      if ("values" in t && t.values) expect(colors[t.name as ColorToken].values).toEqual(t.values);
      else expect(scalars[t.name as keyof typeof scalars].value).toBe(t.value);
    }
  });

  it("keeps the legacy ungrouped names out of the generated tokens", () => {
    const legacy = source.tokens.filter((t) => t.group === null).map((t) => t.name);
    expect(legacy.length).toBeGreaterThan(0);
    for (const name of legacy) expect(css).not.toContain(`--${name}:`);
  });

  it("names CSS variables after the token", () => {
    expect(cssVarName("chrome/panel")).toBe("--chrome-panel");
    expect(cssVar("agent/teal")).toBe("var(--agent-teal)");
    expect(scalarCss("radius/lg")).toBe("14px");
    expect(scalarCss("motion/panel")).toBe("120ms");
    expect(scalarCss("font/ui")).toBe('"Inter"');
  });

  it("sets chrome and canvas colours from separate appearance attributes", () => {
    for (const mode of MODES) {
      expect(css).toContain(`[${APPEARANCE_ATTRIBUTE.chrome}="${mode}"] {`);
      expect(css).toContain(`[${APPEARANCE_ATTRIBUTE.canvas}="${mode}"] {`);
    }
    const block = (attribute: string, mode: string) => {
      const start = css.indexOf(`[${attribute}="${mode}"] {`);
      return css.slice(start, css.indexOf("}", start));
    };
    expect(block("data-appearance", "dark")).toContain(
      `--chrome-panel: ${colorValue("chrome/panel", "dark")};`,
    );
    expect(block("data-appearance", "dark")).not.toContain("--agent-");
    expect(block("data-canvas-appearance", "hc-dark")).toContain(
      `--canvas-selection: ${colorValue("canvas/selection", "hc-dark")};`,
    );
    expect(block("data-canvas-appearance", "hc-dark")).not.toContain("--chrome-");
  });

  it("gives every colour a value in every appearance", () => {
    for (const mode of MODES) {
      const values = theme(mode);
      for (const value of Object.values(values))
        expect(value).toMatch(/^#[0-9A-F]{6}([0-9A-F]{2})?$/);
    }
    expect(isMode("hc-dark")).toBe(true);
    expect(isMode("High contrast")).toBe(false);
  });

  it("lists agent hues from the agent group", () => {
    expect(agentHues).toEqual(
      source.tokens.filter((t) => t.group === "agent").map((t) => t.name.slice("agent/".length)),
    );
  });
});

describe("token page", () => {
  const page = renderTokenPage();

  it("shows every opaque colour token with its four values", () => {
    for (const token of swatchTokens()) {
      expect(page).toContain(`<span class="tp-name">${token}</span>`);
      for (const mode of MODES) expect(page).toContain(colorValue(token, mode));
    }
    expect(swatchTokens()).not.toContain("chrome/elevation-color");
  });

  it("takes every colour on it from the tokens", () => {
    const shown = new Set(page.match(/#[0-9A-F]{6}\b/g));
    const known = new Set<string>(Object.values(colors).flatMap((c) => Object.values(c.values)));
    for (const hex of shown) expect(known.has(hex)).toBe(true);
  });
});
