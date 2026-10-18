import { describe, expect, it } from "vitest";
import {
  clampField,
  type FieldSpec,
  formatField,
  mergeValues,
  parseField,
  scrubField,
  stepField,
} from "./values";

const px: FieldSpec = { suffix: " px" };
const percent: FieldSpec = { suffix: "%", scale: 100, min: 0, max: 1 };
const degrees: FieldSpec = { suffix: "°" };

describe("parseField", () => {
  it("reads plain numbers, with or without the unit", () => {
    expect(parseField("16", px)).toEqual({ ok: true, value: 16 });
    expect(parseField(" 16 px ", px)).toEqual({ ok: true, value: 16 });
    expect(parseField("-4.5", px)).toEqual({ ok: true, value: -4.5 });
    expect(parseField("90°", degrees)).toEqual({ ok: true, value: 90 });
  });
  it("evaluates expressions with the usual precedence", () => {
    expect(parseField("16*2", px)).toEqual({ ok: true, value: 32 });
    expect(parseField("8 + 4 * 2", px)).toEqual({ ok: true, value: 16 });
    expect(parseField("(8 + 4) * 2", px)).toEqual({ ok: true, value: 24 });
    expect(parseField("100/3", px)).toEqual({ ok: true, value: 33.3333 });
    expect(parseField("-(2+3)", px)).toEqual({ ok: true, value: -5 });
    expect(parseField("2 × 8 ÷ 4 − 1", px)).toEqual({ ok: true, value: 3 });
  });
  it("does not accumulate floating point noise", () => {
    expect(parseField("0.1+0.2", px)).toEqual({ ok: true, value: 0.3 });
  });
  it("converts typed percentages to the stored unit", () => {
    expect(parseField("50", percent)).toEqual({ ok: true, value: 0.5 });
    expect(parseField("50%", percent)).toEqual({ ok: true, value: 0.5 });
    expect(parseField("200/4", percent)).toEqual({ ok: true, value: 0.5 });
  });
  it("refuses a percentage where the field has none", () => {
    expect(parseField("10%", px)).toEqual({ ok: false, message: "Percent does not apply here" });
  });
  it("clamps to the field's range and says so", () => {
    expect(parseField("250", percent)).toEqual({ ok: true, value: 1, clamped: true });
    expect(parseField("-5", { min: 0 })).toEqual({ ok: true, value: 0, clamped: true });
  });
  it("rounds to whole numbers when the field wants integers", () => {
    expect(parseField("2.6", { integer: true })).toEqual({ ok: true, value: 3 });
  });
  it("refuses what is not a number, with a message to show beside the field", () => {
    const bad = { ok: false, message: "Enter a number or a sum such as 16*2" };
    expect(parseField("abc", px)).toEqual(bad);
    expect(parseField("", px)).toEqual(bad);
    expect(parseField("4 +", px)).toEqual(bad);
    expect(parseField("(4", px)).toEqual(bad);
    expect(parseField("1e5", px)).toEqual(bad);
    expect(parseField("alert(1)", px)).toEqual(bad);
  });
  it("refuses division by zero and values too large to store", () => {
    expect(parseField("1/0", px)).toEqual({ ok: false, message: "Cannot divide by zero" });
    expect(parseField("99999999999*99999999999", px)).toEqual({
      ok: false,
      message: "That number is too large",
    });
  });
  it("is not a general evaluator: names, properties and calls are never read", () => {
    for (const text of ["constructor", "this", "1;2", "1,2", "Math.PI", "`1`", "1_0"])
      expect(parseField(text, px).ok).toBe(false);
  });
});

describe("formatField", () => {
  it("keeps the unit attached to the value", () => {
    expect(formatField(16, px)).toBe("16 px");
    expect(formatField(0, degrees)).toBe("0°");
    expect(formatField(1, percent)).toBe("100%");
    expect(formatField(0.5, percent)).toBe("50%");
  });
  it("drops trailing zeros and caps the decimals", () => {
    expect(formatField(1.5, {})).toBe("1.5");
    expect(formatField(1 / 3, {})).toBe("0.33");
    expect(formatField(1 / 3, { decimals: 4 })).toBe("0.3333");
    expect(formatField(-0, {})).toBe("0");
  });
});

describe("stepField", () => {
  it("steps by one, ten with Shift and a tenth with Alt", () => {
    expect(stepField(16, 1, {}, px)).toBe(17);
    expect(stepField(16, -1, {}, px)).toBe(15);
    expect(stepField(16, 1, { shift: true }, px)).toBe(26);
    expect(stepField(16, -1, { shift: true }, px)).toBe(6);
    expect(stepField(16, 1, { alt: true }, px)).toBe(16.1);
  });
  it("steps in the displayed unit and stays inside the range", () => {
    expect(stepField(0.5, 1, {}, percent)).toBe(0.51);
    expect(stepField(0.995, 1, {}, percent)).toBe(1);
    expect(stepField(0, -1, {}, percent)).toBe(0);
  });
});

describe("scrubField", () => {
  it("changes by one unit per pixel dragged, faster with Shift, finer with Alt", () => {
    expect(scrubField(10, 5, {}, px)).toBe(15);
    expect(scrubField(10, -3, {}, px)).toBe(7);
    expect(scrubField(10, 5, { shift: true }, px)).toBe(60);
    expect(scrubField(10, 5, { alt: true }, px)).toBe(10.5);
  });
  it("scrubs a percentage in percent", () => {
    expect(scrubField(0.5, 10, {}, percent)).toBe(0.6);
    expect(scrubField(0.95, 10, {}, percent)).toBe(1);
  });
});

describe("clampField", () => {
  it("passes values without a range through", () => {
    expect(clampField(-1e6, {})).toBe(-1e6);
  });
});

describe("mergeValues", () => {
  it("is the value when every selected layer agrees", () => {
    expect(mergeValues([16, 16, 16])).toEqual({ kind: "value", value: 16 });
  });
  it("is mixed when they differ", () => {
    expect(mergeValues([16, 8])).toEqual({ kind: "mixed" });
  });
  it("treats an absent value as the default, not as agreement with a number", () => {
    expect(mergeValues([undefined, undefined])).toEqual({ kind: "value", value: undefined });
    expect(mergeValues([undefined, 1])).toEqual({ kind: "mixed" });
  });
  it("compares objects by content", () => {
    expect(mergeValues([{ $var: "a" }, { $var: "a" }])).toEqual({
      kind: "value",
      value: { $var: "a" },
    });
    expect(mergeValues([{ $var: "a" }, { $var: "b" }])).toEqual({ kind: "mixed" });
  });
  it("has no value for an empty selection", () => {
    expect(mergeValues([])).toEqual({ kind: "none" });
  });
});
