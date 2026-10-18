import type { FieldSpec } from "@vellum/editor-core";

/** How each kind of number is written in the panel (DESIGN.md: values stay attached to their units). */
export const SPEC = {
  /** Lengths in fields that have their own label: "16 px". */
  px: { suffix: " px" },
  /** Lengths in paired fields where the axis letter says what it is: "320". */
  bare: {},
  degrees: { suffix: "°" },
  /** 0 to 1, shown as "100%". */
  opacity: { suffix: "%", scale: 100, min: 0, max: 1, decimals: 0 },
  length: { suffix: " px", min: 0 },
  bareLength: { min: 0 },
  count: { integer: true, min: 1, max: 100 },
  weight: { integer: true, min: 100, max: 900 },
  size: { min: 1 },
  /** Line height as a percentage of the size. */
  lineHeight: { suffix: "%", min: 1 },
  letter: {},
  position: { decimals: 2 },
} as const satisfies Record<string, FieldSpec>;
