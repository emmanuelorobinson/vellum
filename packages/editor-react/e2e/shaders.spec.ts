// TEST DATA. Chrome lane: controls, errors, gallery and C33 design comparison.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

const root = fileURLToPath(new URL("../../../", import.meta.url));
test("uniform edits and variable binding commit attributed transactions; gallery covers every category", async ({
  page,
}) => {
  await page.goto("/shaders.html");
  await page.getByRole("textbox", { name: "Strength", exact: true }).fill("0.25");
  await page.getByRole("textbox", { name: "Strength", exact: true }).press("Enter");
  await page.getByLabel("Bind Strength", { exact: true }).selectOption("v1");
  const tx = await page.evaluate(() => (window as any).shaderTest.engine.transactions);
  expect(tx).toHaveLength(2);
  expect(tx[1].origin.actor.kind).toBe("person");
  expect(tx[1].ops[0].patch.fills[0].uniforms.strength).toEqual({ $var: "v1" });
  await page.getByRole("button", { name: "Examples…", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.getByText("Checker", { exact: true })).toBeVisible();
  await page.getByLabel("Example category").selectOption("Scripts");
  await page.getByRole("button", { name: "Open example" }).click();
  await page.getByLabel("Example category").selectOption("Design systems");
  await page.getByRole("button", { name: "Open example" }).click();
  expect(await page.evaluate(() => (window as any).shaderTest.requests)).toEqual([
    "bars",
    "starter",
  ]);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeHidden();
});
test("compile errors are visible on the node and in the controls", async ({ page }) => {
  await page.goto("/shaders.html?error");
  await expect(page.locator(".vc-shader-controls [role=alert]")).toContainText("unknown value");
  await expect(page.locator(".vc-shader-node-error")).toContainText("unknown value");
});
test("read-only controls preserve the document", async ({ page }) => {
  await page.goto("/shaders.html?readonly");
  await expect(page.getByRole("button", { name: "Choose shader file…" })).toBeDisabled();
  await expect(page.getByRole("textbox", { name: "Strength", exact: true })).toBeDisabled();
  expect(await page.evaluate(() => (window as any).shaderTest.engine.transactions)).toEqual([]);
});
for (const mode of ["light", "dark"])
  test(`C33 shader ${mode} design comparison`, async ({ page }) => {
    const ref = `${root}docs/design/snapshots/C33-shader-${mode}-1280.png`;
    test.skip(!existsSync(ref), "design lane: requires exported C33 snapshots");
    await page.goto(`/shaders.html?mode=${mode}`);
    await page.getByRole("textbox", { name: "Strength", exact: true }).waitFor();
    await page.evaluate(() => document.fonts.ready);
    // C33's first section heading, file and chooser; full uniform layout adds accessible binding selectors.
    const actual = await page.locator(".vc-shader-controls").screenshot();
    const result = await page.evaluate(
      async ({ a, b }) => {
        const load = async (s: string) => {
          const i = new Image();
          i.src = `data:image/png;base64,${s}`;
          await i.decode();
          return i;
        };
        const [left, right] = await Promise.all([load(a), load(b)]);
        const pixels = (i: HTMLImageElement, x: number, y: number) => {
          const c = document.createElement("canvas");
          c.width = 280;
          c.height = 108;
          const ctx = c.getContext("2d")!;
          ctx.drawImage(i, x, y, 280, 108, 0, 0, 280, 108);
          return ctx.getImageData(0, 0, 280, 108).data;
        };
        const p = pixels(left, 992, 56),
          q = pixels(right, 0, 0);
        let different = 0;
        for (let i = 0; i < p.length; i += 4) {
          let max = 0;
          for (let j = 0; j < 4; j++) max = Math.max(max, Math.abs(p[i + j]! - q[i + j]!));
          if (max > 48) different++;
        }
        const sheet = document.createElement("canvas");
        sheet.width = 560;
        sheet.height = 108;
        const context = sheet.getContext("2d")!;
        context.drawImage(left, 992, 56, 280, 108, 0, 0, 280, 108);
        context.drawImage(right, 0, 0, 280, 108, 280, 0, 280, 108);
        return {
          fraction: different / (280 * 108),
          width: right.width,
          sheet: sheet.toDataURL("image/png").split(",")[1]!,
        };
      },
      { a: readFileSync(ref).toString("base64"), b: actual.toString("base64") },
    );
    mkdirSync(`${root}target/screen-goldens`, { recursive: true });
    writeFileSync(`${root}target/screen-goldens/p67-shader-${mode}.png`, actual);
    const { sheet, ...measurement } = result;
    writeFileSync(
      `${root}target/screen-goldens/p67-shader-${mode}.comparison.png`,
      Buffer.from(sheet, "base64"),
    );
    writeFileSync(
      `${root}target/screen-goldens/p67-shader-${mode}.json`,
      JSON.stringify(measurement),
    );
    expect(result.width).toBe(280);
    expect(result.fraction).toBeLessThanOrEqual(0.01);
  });
