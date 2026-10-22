// The editor over the real Rust engine (ADR-033): needs the editor WASM module (pnpm wasm:build) and Chrome
// with WebGL2. The canvas draws the engine's display list; tools and undo go through the engine's write path.
import { expect, type Page, test } from "@playwright/test";

declare global {
  interface Window {
    __engine: {
      snapshot(): {
        revision: number;
        readOnly: boolean;
        document: { pages: { children: { id: string; type: string }[] }[] };
      };
      get(
        address: string,
      ): { bounds: { x: number; y: number; width: number; height: number } } | undefined;
      encode(): string;
      insertEveryKind(): string[];
    };
  }
}

async function open(page: Page, query = "") {
  await page.goto(`/engine.html${query}`);
  await expect(page.locator("[data-content]").first()).toHaveAttribute("data-content", "ready", {
    timeout: 30_000,
  });
}

const layers = (page: Page) =>
  page.evaluate(() => window.__engine.snapshot().document.pages[0]?.children.map((c) => c.type));

test("the canvas draws the engine's document", async ({ page }) => {
  await open(page);
  const label = await page.evaluate(() => window.__engine.get("frame00001/text000001"));
  expect(label?.bounds.width).toBeGreaterThan(50);
  const canvas = page.locator("canvas").first();
  const shot = await canvas.screenshot();
  // The white frame and black text are drawn: the picture is not a single flat colour.
  expect(new Set(shot.subarray(shot.length - 4000)).size).toBeGreaterThan(2);
});

test("every creation tool's default layer is a valid node for the engine", async ({ page }) => {
  await open(page);
  const results = await page.evaluate(() => window.__engine.insertEveryKind());
  expect(results).toEqual(Array(9).fill("applied"));
});

test("drawing a rectangle commits through the engine, and undo takes it back", async ({ page }) => {
  await open(page);
  expect(await layers(page)).toEqual(["frame"]);
  await page
    .locator(".vc-canvas")
    .first()
    .click({ position: { x: 700, y: 700 } });
  await page.keyboard.press("r");
  await page.mouse.move(500, 560);
  await page.mouse.down();
  await page.mouse.move(560, 600, { steps: 4 });
  await page.mouse.move(620, 640, { steps: 4 });
  await page.mouse.up();
  await expect.poll(() => layers(page)).toEqual(["frame", "rect"]);
  const bytes = await page.evaluate(() => window.__engine.encode());
  expect(bytes).toContain('"type": "rect"');
  await page.keyboard.press("ControlOrMeta+z");
  await expect.poll(() => layers(page)).toEqual(["frame"]);
  await page.keyboard.press("ControlOrMeta+Shift+z");
  await expect.poll(() => layers(page)).toEqual(["frame", "rect"]);
});

test("a read-only file refuses edits", async ({ page }) => {
  await open(page, "?readonly");
  expect(await page.evaluate(() => window.__engine.snapshot().readOnly)).toBe(true);
  expect(await page.evaluate(() => window.__engine.insertEveryKind()[0])).toBe("rejected");
});
