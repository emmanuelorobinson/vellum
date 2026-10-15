// Shared comparison, also tested with corrupted output so the golden gate cannot silently pass.
export function comparePixels(expected, actual, tolerance) {
  if (expected.length !== actual.length || expected.length % 4 !== 0)
    return { pass: false, reason: "RGBA length mismatch" };
  const deltas = [],
    diff = new Uint8Array(expected.length);
  let different = 0,
    max = 0;
  for (let i = 0; i < expected.length; i += 4) {
    let d = 0;
    for (let c = 0; c < 4; c++) {
      const a = c === 3 ? expected[i + c] : Math.round((expected[i + c] * expected[i + 3]) / 255);
      const b = c === 3 ? actual[i + c] : Math.round((actual[i + c] * actual[i + 3]) / 255);
      d = Math.max(d, Math.abs(a - b));
    }
    max = Math.max(max, d);
    deltas.push(d);
    if (d > tolerance.channel) different++;
    diff[i] = d;
    diff[i + 1] = 0;
    diff[i + 2] = d;
    diff[i + 3] = 255;
  }
  deltas.sort((a, b) => a - b);
  const p95 = deltas[Math.floor(deltas.length * 0.95)] ?? 0,
    fraction = different / deltas.length;
  return {
    pass: fraction <= tolerance.fraction && p95 <= tolerance.p95,
    max,
    p95,
    different,
    fraction,
    diff,
  };
}
