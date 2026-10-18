/** sRGB colours as the file prints them: `#RRGGBB` when opaque, `#RRGGBBAA` otherwise, upper case. */

export interface Rgba {
  r: number;
  g: number;
  b: number;
  /** 0 to 255. */
  a: number;
}

function read(text: string): Rgba | null {
  const t = text.trim().replace(/^#/, "");
  if (!/^[0-9a-fA-F]+$/.test(t)) return null;
  const full =
    t.length === 3 || t.length === 4
      ? [...t].map((c) => c + c).join("")
      : t.length === 6 || t.length === 8
        ? t
        : null;
  if (!full) return null;
  const byte = (i: number) => Number.parseInt(full.slice(i, i + 2), 16);
  return { r: byte(0), g: byte(2), b: byte(4), a: full.length === 8 ? byte(6) : 255 };
}
const hex2 = (n: number) =>
  Math.round(Math.min(255, Math.max(0, n)))
    .toString(16)
    .padStart(2, "0");
function write(c: Rgba): string {
  const base = `#${hex2(c.r)}${hex2(c.g)}${hex2(c.b)}`;
  return (Math.round(c.a) >= 255 ? base : base + hex2(c.a)).toUpperCase();
}

/** The canonical spelling of what a person typed, or null when it is not a colour. */
export function normalizeHex(text: string): string | null {
  const c = read(text);
  return c ? write(c) : null;
}
/** 0 (clear) to 1 (opaque). */
export function alphaOf(colour: string): number {
  return (read(colour)?.a ?? 255) / 255;
}
export function setAlpha(colour: string, alpha: number): string {
  const c = read(colour);
  if (!c) return colour;
  return write({ ...c, a: Math.min(1, Math.max(0, alpha)) * 255 });
}
export function mixColours(from: string, to: string, t: number): string {
  const a = read(from);
  const b = read(to);
  if (!a || !b) return from;
  const m = (x: number, y: number) => x + (y - x) * t;
  return write({ r: m(a.r, b.r), g: m(a.g, b.g), b: m(a.b, b.b), a: m(a.a, b.a) });
}
