// Design lane; TEST DATA. Requires the immutable Fixture-RTL PNGs and Chromium. No reference or tolerance updates.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

const root = fileURLToPath(new URL("../../../", import.meta.url));
for (const appearance of ["light", "dark"]) {
  test(`Arabic 1280 ${appearance} matches design`, async ({ page }) => {
    const reference = `${root}docs/design/snapshots/Fixture-RTL-${appearance}-1280.png`;
    test.skip(!existsSync(reference), "design lane: export Fixture-RTL snapshots first");
    await page.goto(`/?language=ar&appearance=${appearance}&tab=layers&scenario=juniper&zoom=0.5`);
    await expect(page.getByRole("tab", { name: "الطبقات", exact: true })).toBeVisible();
    await page.evaluate(() => document.fonts.ready);
    const actual = await page.screenshot();
    const result = await page.evaluate(
      async ({ reference, actual }) => {
        const load = async (data: string) => {
          const image = new Image();
          image.src = `data:image/png;base64,${data}`;
          await image.decode();
          return image;
        };
        const [before, after] = await Promise.all([load(reference), load(actual)]);
        const canvas = document.createElement("canvas");
        canvas.width = 2560;
        canvas.height = 800;
        const ctx = canvas.getContext("2d");
        if (!ctx) throw Error("2D comparison canvas unavailable");
        ctx.drawImage(before, 0, 0);
        ctx.drawImage(after, 1280, 0);
        const a = ctx.getImageData(0, 0, 1280, 800).data,
          b = ctx.getImageData(1280, 0, 1280, 800).data;
        let different = 0;
        for (let i = 0; i < a.length; i += 4) {
          if ([0, 1, 2, 3].some((c) => Math.abs((a[i + c] ?? 0) - (b[i + c] ?? 0)) > 48))
            different++;
        }
        return {
          fraction: different / (1280 * 800),
          width: after.width,
          height: after.height,
          sheet: canvas.toDataURL("image/png").split(",")[1] ?? "",
        };
      },
      { reference: readFileSync(reference).toString("base64"), actual: actual.toString("base64") },
    );
    mkdirSync(`${root}target/screen-goldens`, { recursive: true });
    const out = `${root}target/screen-goldens/p70-rtl-${appearance}`;
    writeFileSync(`${out}.png`, actual);
    const { sheet, ...measurement } = result;
    writeFileSync(`${out}.comparison.png`, Buffer.from(sheet, "base64"));
    writeFileSync(`${out}.json`, JSON.stringify(measurement, null, 2));
    expect(measurement.width).toBe(1280);
    expect(measurement.height).toBe(800);
    expect(measurement.fraction).toBeLessThanOrEqual(0.01);
  });
}
