// TEST DATA. Chrome lane; production C30/C31 controls, no external services.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

const root = fileURLToPath(new URL("../../../", import.meta.url));
test("generates a marked image and exposes local image actions", async ({ page }) => {
  await page.goto("/images.html");
  await page.getByRole("tab", { name: "Generate", exact: true }).click();
  await page.getByRole("textbox", { name: "Describe the image" }).fill("TEST DATA botanical");
  await page.getByRole("button", { name: "Generate", exact: true }).click();
  const tx = await page.evaluate(() => (window as any).imageTest.engine.transactions);
  expect(tx).toHaveLength(1);
  expect(tx[0].ops[0].patch.metadata.generated).toEqual({
    provider: "configured",
    model: "fixture",
  });
  await page.goto("/images.html?ops");
  await page.getByRole("button", { name: "Remove background", exact: true }).click();
  await page.getByRole("button", { name: "Convert to vectors", exact: true }).click();
  const requests = await page.evaluate(() => (window as any).imageTest.requests);
  expect(requests.map((r: any) => r.action)).toEqual(["remove_background", "trace"]);
});
test("Figma progress ends in the actual report", async ({ page }) => {
  await page.goto("/images.html?figma");
  await page.getByRole("button", { name: "Import as layers", exact: true }).click();
  // The summary line; the same words wait in the collapsed report below it.
  await expect(page.getByText("Imported 12 frames · 284 layers", { exact: true })).toBeVisible();
  await page.getByText("View report", { exact: true }).click();
  await expect(page.locator("pre")).toContainText("Smart Animate");
});
for (const mode of ["light", "dark"]) {
  for (const surface of ["generate", "ops", "figma"]) {
    test(`C30/C31 ${surface} ${mode} design comparison`, async ({ page }) => {
      const screen =
        surface === "figma" ? "C31-figma" : surface === "ops" ? "C30-edit" : "C30-generate";
      const ref = `${root}docs/design/snapshots/${screen}-${mode}-1280.png`;
      test.skip(!existsSync(ref), "design lane: needs exported snapshots");
      await page.goto(
        `/images.html?mode=${mode}${surface === "figma" ? "&figma&report" : surface === "ops" ? "&ops" : ""}`,
      );
      if (surface === "generate") {
        await page.getByRole("tab", { name: "Generate", exact: true }).click();
        await page
          .getByRole("textbox", { name: "Describe the image" })
          .fill("A simple botanical photograph for Juniper’s habit journal, soft morning light.");
      }
      await page.evaluate(() => {
        (document.activeElement as HTMLElement)?.blur();
        return document.fonts.ready;
      });
      const locator = page.locator(
        surface === "generate"
          ? ".vx-image-picker"
          : surface === "ops"
            ? ".vx-image-actions"
            : ".vf-import",
      );
      const actual = await locator.screenshot();
      // Compare only controls present in both sources; dynamic report content uses the title region.
      const box =
        surface === "generate"
          ? [920, 100, 344, 351]
          : surface === "ops"
            ? [968, 168, 272, 18]
            : [504, 204, 512, 24];
      const compared =
        surface === "generate"
          ? actual
          : await locator.locator(surface === "ops" ? "h3" : "h2").screenshot();
      const result = await page.evaluate(
        async ({ a, b, box }) => {
          async function load(s: string) {
            const i = new Image();
            i.src = `data:image/png;base64,${s}`;
            await i.decode();
            return i;
          }
          const [left, right] = await Promise.all([load(a), load(b)]);
          const [x, y, w, h] = box as [number, number, number, number];
          const pixels = (i: HTMLImageElement, x: number, y: number) => {
            const c = document.createElement("canvas");
            c.width = w;
            c.height = h;
            const ctx = c.getContext("2d")!;
            ctx.drawImage(i, x, y, w, h, 0, 0, w, h);
            return ctx.getImageData(0, 0, w, h).data;
          };
          const p = pixels(left, x, y),
            q = pixels(right, 0, 0);
          let different = 0;
          for (let i = 0; i < p.length; i += 4) {
            let max = 0;
            for (let c = 0; c < 4; c++)
              max = Math.max(max, Math.abs((p[i + c] ?? 0) - (q[i + c] ?? 0)));
            if (max > 48) different++;
          }
          return {
            fraction: different / (w * h),
            width: right.width,
            height: right.height,
            expectedWidth: w,
            expectedHeight: h,
          };
        },
        { a: readFileSync(ref).toString("base64"), b: compared.toString("base64"), box },
      );
      mkdirSync(`${root}target/screen-goldens`, { recursive: true });
      writeFileSync(`${root}target/screen-goldens/p66-${surface}-${mode}.png`, actual);
      writeFileSync(
        `${root}target/screen-goldens/p66-${surface}-${mode}.json`,
        JSON.stringify(result),
      );
      expect(result.width).toBe(result.expectedWidth);
      expect(result.height).toBe(result.expectedHeight);
      expect(result.fraction).toBeLessThanOrEqual(0.01);
    });
  }
}
