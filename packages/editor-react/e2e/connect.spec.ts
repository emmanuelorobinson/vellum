// Chrome lane, TEST DATA: C02 over an injected native-host fixture.

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

for (const appearance of ["light", "dark"]) {
  test(`C02 interaction and accessibility, ${appearance}`, async ({ page }) => {
    await page.goto(`/connect.html?appearance=${appearance}`);
    const dialog = page.getByRole("dialog", { name: "Connect agents" });
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".vc-connect-row")).toHaveCount(10);
    await dialog.getByRole("button", { name: "Connect Codex", exact: true }).click();
    await expect(dialog).toContainText("Configured · restart client");
    await expect(dialog).toContainText("Written to /test-home/client-config.json");
    await dialog.getByRole("button", { name: "Disconnect Codex", exact: true }).click();
    await expect(dialog).toContainText("Removed from /test-home/client-config.json");
    await page.getByLabel("Manual setup client").selectOption("8");
    await page.getByRole("button", { name: "Copy config" }).click();
    await expect(dialog).toContainText("Config copied");
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
  });
  test(`C02 design comparison, ${appearance}`, async ({ page }) => {
    await page.goto(`/connect.html?appearance=${appearance}`);
    await page.evaluate(() => document.fonts.ready);
    const dialog = page.getByRole("dialog", { name: "Connect agents" });
    const box = await dialog.boundingBox();
    expect(box?.x).toBe(270);
    expect(box?.y).toBe(84);
    expect(box?.width).toBe(740);
    const repo = fileURLToPath(new URL("../../../", import.meta.url));
    const reference = readFileSync(
      `${repo}docs/design/snapshots/C02-${appearance}-1280.png`,
    ).toString("base64");
    const actual = (await dialog.screenshot()).toString("base64");
    // The named screen is compared at its exact viewport/theme. Different state symbols, manual selector
    // and the real executable snippet are deliberate functional additions, never hidden by a looser limit.
    const result = await page.evaluate(
      async ({ reference, actual }) => {
        const load = async (s: string) => {
          const i = new Image();
          i.src = `data:image/png;base64,${s}`;
          await i.decode();
          return i;
        };
        const [a, b] = await Promise.all([load(reference), load(actual)]);
        const w = 740,
          h = b.height;
        const canvas = new OffscreenCanvas(w, h * 2);
        const ctx = canvas.getContext("2d")!;
        ctx.drawImage(a, 270, 84, w, h, 0, 0, w, h);
        const da = ctx.getImageData(0, 0, w, h).data;
        ctx.drawImage(b, 0, h);
        const db = ctx.getImageData(0, h, w, h).data;
        let different = 0;
        for (let i = 0; i < da.length; i += 4)
          if (Math.max(...[0, 1, 2].map((k) => Math.abs(da[i + k]! - db[i + k]!))) > 48)
            different++;
        const blob = await canvas.convertToBlob({ type: "image/png" });
        const bytes = new Uint8Array(await blob.arrayBuffer());
        return { fraction: different / (w * h), sheet: btoa(String.fromCharCode(...bytes)) };
      },
      { reference, actual },
    );
    mkdirSync(`${repo}target/screen-goldens`, { recursive: true });
    writeFileSync(
      `${repo}target/screen-goldens/C02-${appearance}-compare.png`,
      Buffer.from(result.sheet, "base64"),
    );
    expect(result.fraction).toBeLessThanOrEqual(0.01);
  });
}
test("C02 small viewport remains scrollable", async ({ page }) => {
  await page.setViewportSize({ width: 500, height: 500 });
  await page.goto("/connect.html");
  const dialog = page.getByRole("dialog", { name: "Connect agents" });
  await expect(dialog).toBeVisible();
  const box = await dialog.boundingBox();
  expect(box?.width).toBeLessThanOrEqual(468);
  await page.getByRole("button", { name: "Copy config" }).scrollIntoViewIfNeeded();
  await expect(page.getByRole("button", { name: "Copy config" })).toBeVisible();
});
