import type { Frame, Renderer } from "@vellum/renderer-api";
export interface RendererHarness {
  renderer: Renderer;
  /** A valid scene with nonzero ink, already in 256×256 device pixels. */
  frame: Frame;
}
function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(`Renderer contract: ${message}`);
}
/** Run against the real browser adapter and test fakes; no DOM/vendor/test-runner dependency. */
export function rendererContract({ renderer, frame }: RendererHarness) {
  assert(renderer.state === "ready", "starts ready");
  renderer.resize({ width: 256, height: 256, dpr: 1 });
  renderer.draw(frame);
  const first = renderer.readPixels();
  assert(first.length === 256 * 256 * 4, "RGBA length matches device viewport");
  assert(
    first.some((v, i) => i % 4 === 3 && v > 0),
    "scene has ink",
  );
  let refused = false;
  try {
    renderer.draw({ ...frame, displayList: new Uint8Array([1, 2, 3]) });
  } catch {
    refused = true;
  }
  assert(refused, "malformed display list is refused");
  const after = renderer.readPixels();
  assert(
    after.length === first.length && after.every((v, i) => v === first[i]),
    "failed preparation preserves the last frame",
  );
  refused = false;
  try {
    renderer.resize({ width: 0, height: 1, dpr: 1 });
  } catch {
    refused = true;
  }
  assert(refused, "invalid viewport is refused");
  renderer.resize({ width: 128, height: 128, dpr: 1 });
  renderer.draw(frame);
  assert(renderer.readPixels().length === 128 * 128 * 4, "resize recreates the target surface");
  renderer.resize({ width: 128, height: 128, dpr: 2 });
  renderer.draw(frame);
  const resized = renderer.readPixels();
  assert(resized.length === first.length, "DPR affects device size once");
  assert(
    resized.every((v, i) => v === first[i]),
    "resize and DPR preserve the same device-space frame",
  );
  renderer.dispose();
  renderer.dispose();
  assert((renderer.state as string) === "disposed", "dispose is idempotent");
  refused = false;
  try {
    renderer.draw(frame);
  } catch {
    refused = true;
  }
  assert(refused, "drawing after dispose is refused");
}
