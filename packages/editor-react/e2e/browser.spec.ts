// TEST DATA. Chrome lane; C31 behavior and snapshots, no external network.
import { expect, test } from "@playwright/test";

test("loads a page, selects measured DOM and imports editable layers", async ({ page }) => {
  await page.goto("/browser.html");
  await expect(page.getByText("HabitCard selected")).toBeVisible();
  await page.getByRole("button", { name: "Import as layers", exact: true }).click();
  await expect(page.getByRole("status")).toHaveText("✓ Layers imported");
  const count = await page.evaluate(
    () =>
      (window as unknown as { browserTest: { imports: unknown[]; requests: unknown[] } })
        .browserTest.imports.length,
  );
  expect(count).toBe(1);
  await page.getByLabel("Page address").fill("http://localhost:3000/offline");
  await page.getByRole("button", { name: "Open page", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("could not be loaded");
});
test("embed stays unloaded until requested and respects the phone device", async ({ page }) => {
  await page.goto("/browser.html?embed&empty");
  await expect(page.getByText("Page not loaded")).toBeVisible();
  await page.getByRole("button", { name: "Open page", exact: true }).first().click();
  await expect(page.getByAltText("Website preview snapshot")).toBeVisible();
  const request = await page.evaluate(() =>
    (
      window as unknown as { browserTest: { imports: unknown[]; requests: unknown[] } }
    ).browserTest.requests.at(-1),
  );
  expect(request).toMatchObject({ width: 375, height: 812, zoom: 0.8 });
});

// Design lane: compare shipped controls to C31 at the recorded size/theme, without replacing references.
for (const mode of ["light", "dark"])
  for (const surface of ["import", "embed"]) {
    test(`C31 ${surface} ${mode} matches the design`, async ({ page }) => {
      const { existsSync, readFileSync, mkdirSync, writeFileSync } = await import("node:fs");
      const { fileURLToPath } = await import("node:url");
      const root = fileURLToPath(new URL("../../../", import.meta.url));
      const reference = `${root}docs/design/snapshots/C31${surface === "embed" ? "-embed" : ""}-${mode}-1280.png`;
      test.skip(!existsSync(reference), "design lane: needs the local C31 snapshot PNGs");
      await page.goto(`/browser.html?mode=${mode}${surface === "embed" ? "&embed" : ""}`);
      await page
        .getByLabel(surface === "embed" ? "Embed properties" : "Import from browser")
        .waitFor();
      const box = surface === "embed" ? [992, 56, 280, 240] : [472, 104, 236, 24];
      const actual =
        surface === "embed"
          ? await page.locator(".vb-properties").screenshot()
          : await page.locator(".vb-import h2").screenshot();
      const design = readFileSync(
        `${root}docs/design/snapshots/C31${surface === "embed" ? "-embed" : ""}-${mode}-1280.png`,
      );
      const result = await page.evaluate(
        async ({ expected, actual, box }) => {
          const load = async (base64: string) => {
            const image = new Image();
            image.src = `data:image/png;base64,${base64}`;
            await image.decode();
            return image;
          };
          const [a, b] = await Promise.all([load(expected), load(actual)]);
          const [x, y, w, h] = box as [number, number, number, number];
          const read = (image: HTMLImageElement, x: number, y: number) => {
            const canvas = document.createElement("canvas");
            canvas.width = w;
            canvas.height = h;
            const ctx = canvas.getContext("2d");
            if (!ctx) throw Error("no canvas");
            ctx.drawImage(image, x, y, w, h, 0, 0, w, h);
            return ctx.getImageData(0, 0, w, h).data;
          };
          const left = read(a, x, y),
            right = read(b, 0, 0);
          let different = 0;
          for (let p = 0; p < left.length; p += 4) {
            let delta = 0;
            for (let c = 0; c < 4; c++)
              delta = Math.max(delta, Math.abs((left[p + c] ?? 0) - (right[p + c] ?? 0)));
            if (delta > 48) different++;
          }
          return {
            fraction: different / (w * h),
            width: b.width,
            height: b.height,
            expectedWidth: w,
            expectedHeight: h,
          };
        },
        { expected: design.toString("base64"), actual: actual.toString("base64"), box },
      );
      mkdirSync(`${root}target/screen-goldens`, { recursive: true });
      writeFileSync(`${root}target/screen-goldens/p65-${surface}-${mode}.png`, actual);
      writeFileSync(
        `${root}target/screen-goldens/p65-${surface}-${mode}.json`,
        JSON.stringify(result),
      );
      expect(result.width).toBe(result.expectedWidth);
      expect(result.height).toBe(result.expectedHeight);
      expect(result.fraction).toBeLessThanOrEqual(0.01);
    });
  }
