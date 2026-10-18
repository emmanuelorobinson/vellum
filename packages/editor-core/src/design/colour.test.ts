import { describe, expect, it } from "vitest";
import { alphaOf, mixColours, normalizeHex, setAlpha } from "./colour";

describe("normalizeHex", () => {
  it("prints opaque colours as #RRGGBB and the rest as #RRGGBBAA, in upper case", () => {
    expect(normalizeHex("#234d3c")).toBe("#234D3C");
    expect(normalizeHex("234d3c")).toBe("#234D3C");
    expect(normalizeHex("#234d3cff")).toBe("#234D3C");
    expect(normalizeHex("#234d3c80")).toBe("#234D3C80");
  });
  it("expands the short forms", () => {
    expect(normalizeHex("#fa0")).toBe("#FFAA00");
    expect(normalizeHex("#fa08")).toBe("#FFAA0088");
  });
  it("refuses anything else", () => {
    for (const bad of [
      "",
      "#",
      "#12",
      "#12345",
      "#1234567",
      "#12345678a",
      "red",
      "#ggg",
      "rgb(0,0,0)",
    ])
      expect(normalizeHex(bad), bad).toBeNull();
  });
});

describe("alpha", () => {
  it("reads 0 to 1", () => {
    expect(alphaOf("#234D3C")).toBe(1);
    expect(alphaOf("#234D3C00")).toBe(0);
    expect(alphaOf("#234D3C80")).toBeCloseTo(128 / 255, 6);
  });
  it("sets it, dropping the suffix when it is fully opaque", () => {
    expect(setAlpha("#234D3C", 0.5)).toBe("#234D3C80");
    expect(setAlpha("#234D3C80", 1)).toBe("#234D3C");
    expect(setAlpha("#234D3C", 0)).toBe("#234D3C00");
  });
  it("clamps", () => {
    expect(setAlpha("#234D3C", 4)).toBe("#234D3C");
    expect(setAlpha("#234D3C", -1)).toBe("#234D3C00");
  });
});

describe("mixColours", () => {
  it("interpolates each channel, alpha included", () => {
    expect(mixColours("#000000", "#FFFFFF", 0.5)).toBe("#808080");
    expect(mixColours("#000000", "#FFFFFF", 0)).toBe("#000000");
    expect(mixColours("#000000", "#FFFFFF", 1)).toBe("#FFFFFF");
    expect(mixColours("#FF0000", "#FF000000", 0.5)).toBe("#FF000080");
  });
});
