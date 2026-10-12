// Shared by the page and the Node harness.

export const FIXTURE_SIZE = 256;

/** Nearest-rank percentile of `values` (q in 0..1); 0 for an empty list. */
export function percentile(values, q) {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(q * sorted.length) - 1))];
}
