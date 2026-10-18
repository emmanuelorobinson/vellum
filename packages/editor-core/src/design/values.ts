/**
 * The rules every value field in the properties panel shares (DESIGN.md "Controls"): numbers with their unit,
 * expressions such as "16*2", arrow-key steps, scrubbing, and "Mixed" for a multi-selection. No DOM here.
 */

export interface FieldSpec {
  /** Shown after the number and accepted when typed: " px", "%", "°". */
  suffix?: string;
  /** Stored value × scale is what the person sees (opacity is stored 0 to 1 and shown 0 to 100). */
  scale?: number;
  /** Bounds in the stored unit. */
  min?: number;
  max?: number;
  /** Most decimals shown, in the displayed unit; 2 when absent. */
  decimals?: number;
  integer?: boolean;
}

export type ParseResult =
  | { ok: true; value: number; clamped?: true }
  | { ok: false; message: string };

const NOT_A_NUMBER = "Enter a number or a sum such as 16*2";
/** Anything past this cannot be stored as a layer length (the format quantizes to f32). */
const LIMIT = 1e9;
const round = (n: number, places = 4) => {
  const f = 10 ** places;
  return Math.round(n * f) / f;
};

type Token = { t: "num"; v: number } | { t: "op"; v: string } | { t: "pct" };
function tokens(text: string): Token[] | null {
  const out: Token[] = [];
  let i = 0;
  while (i < text.length) {
    const c = text[i] as string;
    if (c === " " || c === "\t") {
      i++;
      continue;
    }
    if (/[0-9.]/.test(c)) {
      const m = /^(?:\d+\.?\d*|\.\d+)/.exec(text.slice(i));
      if (!m) return null;
      out.push({ t: "num", v: Number(m[0]) });
      i += m[0].length;
      // A unit written after the number is read and ignored: "16 px", "90°", "4deg".
      const unit = /^\s*(px|deg|°)/.exec(text.slice(i));
      if (unit) i += unit[0].length;
      continue;
    }
    if (c === "%") {
      out.push({ t: "pct" });
      i++;
      continue;
    }
    if ("+-*/()".includes(c)) {
      out.push({ t: "op", v: c });
      i++;
      continue;
    }
    return null;
  }
  return out;
}

class DivideByZero extends Error {}
/** Recursive descent over + - * / and parentheses; there is no other syntax, so nothing can be called. */
function evaluate(list: Token[]): number | null {
  let at = 0;
  const peek = () => list[at];
  const op = (v: string) => {
    const t = peek();
    if (t?.t === "op" && t.v === v) {
      at++;
      return true;
    }
    return false;
  };
  const primary = (): number => {
    if (op("(")) {
      const v = sum();
      if (!op(")")) throw new SyntaxError("paren");
      return v;
    }
    const t = peek();
    if (t?.t === "num") {
      at++;
      return t.v;
    }
    throw new SyntaxError("number");
  };
  const unary = (): number => {
    if (op("-")) return -unary();
    if (op("+")) return unary();
    return primary();
  };
  const product = (): number => {
    let v = unary();
    for (;;) {
      if (op("*")) v *= unary();
      else if (op("/")) {
        const d = unary();
        if (d === 0) throw new DivideByZero();
        v /= d;
      } else return v;
    }
  };
  const sum = (): number => {
    let v = product();
    for (;;) {
      if (op("+")) v += product();
      else if (op("-")) v -= product();
      else return v;
    }
  };
  const v = sum();
  if (at !== list.length) throw new SyntaxError("trailing");
  return v;
}

export function clampField(value: number, spec: FieldSpec): number {
  const lo = spec.min ?? Number.NEGATIVE_INFINITY;
  const hi = spec.max ?? Number.POSITIVE_INFINITY;
  return Math.min(hi, Math.max(lo, value));
}

export function parseField(input: string, spec: FieldSpec = {}): ParseResult {
  const text = input.replace(/[−–]/g, "-").replace(/×/g, "*").replace(/÷/g, "/").trim();
  if (!text) return { ok: false, message: NOT_A_NUMBER };
  let list = tokens(text);
  if (!list) return { ok: false, message: NOT_A_NUMBER };
  const percents = list.filter((t) => t.t === "pct").length;
  if (percents) {
    // A percent sign is the unit of a field shown in percent, written once at the end.
    const last = list.at(-1);
    if (percents > 1 || last?.t !== "pct") return { ok: false, message: NOT_A_NUMBER };
    if (spec.suffix !== "%") return { ok: false, message: "Percent does not apply here" };
    list = list.slice(0, -1);
  }
  let shown: number | null;
  try {
    shown = evaluate(list);
  } catch (e) {
    return {
      ok: false,
      message: e instanceof DivideByZero ? "Cannot divide by zero" : NOT_A_NUMBER,
    };
  }
  if (shown === null || Number.isNaN(shown)) return { ok: false, message: NOT_A_NUMBER };
  if (!Number.isFinite(shown) || Math.abs(shown) > LIMIT)
    return { ok: false, message: "That number is too large" };
  let value = round(round(shown) / (spec.scale ?? 1), 6);
  if (spec.integer) value = Math.round(value);
  const clamped = clampField(value, spec);
  return clamped === value ? { ok: true, value } : { ok: true, value: clamped, clamped: true };
}

export function formatField(value: number, spec: FieldSpec = {}): string {
  const shown = round(value * (spec.scale ?? 1), spec.decimals ?? 2);
  // Never print "-0".
  const text = String(shown === 0 ? 0 : shown);
  return `${text}${spec.suffix ?? ""}`;
}

export interface StepModifiers {
  shift?: boolean;
  alt?: boolean;
}
const factor = (mods: StepModifiers) => (mods.shift ? 10 : mods.alt ? 0.1 : 1);

/** Arrow keys: one unit, ten with Shift, a tenth with Alt, in the unit the field shows. */
export function stepField(
  value: number,
  direction: 1 | -1,
  mods: StepModifiers,
  spec: FieldSpec = {},
): number {
  const scale = spec.scale ?? 1;
  const next = round((value * scale + direction * factor(mods)) / scale, 6);
  return clampField(spec.integer ? Math.round(next) : next, spec);
}

/** Dragging a field's label: one unit per pixel, ten with Shift, a tenth with Alt. */
export function scrubField(
  start: number,
  dx: number,
  mods: StepModifiers,
  spec: FieldSpec = {},
): number {
  const scale = spec.scale ?? 1;
  const next = round((start * scale + dx * factor(mods)) / scale, 6);
  return clampField(spec.integer ? Math.round(next) : next, spec);
}

export type Merged<T> = { kind: "value"; value: T } | { kind: "mixed" } | { kind: "none" };

const same = (a: unknown, b: unknown) =>
  a === b ||
  (typeof a === "object" && typeof b === "object" && JSON.stringify(a) === JSON.stringify(b));

/** What a field shows for several layers: their shared value, or Mixed. An absent value is the default. */
export function mergeValues<T>(values: readonly (T | undefined)[]): Merged<T | undefined> {
  const [first, ...rest] = values;
  if (!values.length) return { kind: "none" };
  return rest.every((v) => same(v, first)) ? { kind: "value", value: first } : { kind: "mixed" };
}
