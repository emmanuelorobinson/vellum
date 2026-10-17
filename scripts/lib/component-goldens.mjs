import { comparePixels } from "../renderer-golden-compare.mjs";
// DESIGN-DECISIONS.md: 1%; Prompt 43's cross-renderer antialiasing channel threshold.
export const COMPONENT_TOLERANCE = { fraction: 0.01, channel: 48, p95: 255 };
export function compareComponentPixels(expected, actual, tolerance = COMPONENT_TOLERANCE) {
  if (expected.width !== actual.width || expected.height !== actual.height)
    return { pass: false, reason: "Component dimensions differ" };
  const { diff: _diff, ...result } = comparePixels(expected.data, actual.data, tolerance);
  return result;
}
/** Pending cases never load an expected image, and can never count as passing goldens. */
export async function componentGoldenCases(cases, capture, expected) {
  const result = { passed: [], pending: [], failed: [] };
  for (const specimen of cases) {
    try {
      const actual = await capture(specimen);
      if (actual.status === "pending") {
        result.pending.push(specimen.id);
        continue;
      }
      if (actual.status !== "ready") throw Error("Missing component render status");
      const comparison = compareComponentPixels(await expected(specimen), actual);
      if (comparison.pass) result.passed.push(specimen.id);
      else result.failed.push({ id: specimen.id, ...comparison });
    } catch (error) {
      result.failed.push({ id: specimen.id, reason: error.message });
    }
  }
  return result;
}
