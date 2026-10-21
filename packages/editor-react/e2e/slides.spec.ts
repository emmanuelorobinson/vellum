// TEST DATA. Chrome lane, real workspace controls over a scripted core/service boundary.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

const root = fileURLToPath(new URL("../../../", import.meta.url));
test("presentation keys preserve document, revision, selection and editor viewport", async ({
  page,
}) => {
  await page.goto("/slides.html");
  const before = await page.evaluate(() => {
    const f = (window as any).slidesTest;
    return { snapshot: f.engine.snapshot(), state: f.session.state };
  });
  await page.locator(".vc-canvas").focus();
  await page.keyboard.press("Meta+Enter");
  await expect(page.getByRole("dialog", { name: "Presentation" })).toBeVisible();
  await expect(page.getByRole("img", { name: "Your weekly progress" })).toBeVisible();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("img", { name: "A daily rhythm" })).toBeVisible();
  await page.keyboard.press("Space");
  await expect(page.getByRole("img", { name: "Next steps" })).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("img", { name: "Next steps" })).toBeVisible();
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(
    await page.evaluate(() => {
      const f = (window as any).slidesTest;
      return { snapshot: f.engine.snapshot(), state: f.session.state };
    }),
  ).toEqual(before);
  expect(await page.evaluate(() => (window as any).slidesTest.engine.transactions)).toEqual([]);
  await expect(page.locator(".vc-canvas")).toBeFocused();
});
test("PDF all and selected use deck order; reorder commits one attributed op", async ({ page }) => {
  await page.goto("/slides.html");
  await page.getByRole("button", { name: "Export PDF · slide order", exact: true }).click();
  await page.getByRole("option", { name: /01/ }).click({ modifiers: ["Meta"] });
  await page.getByRole("button", { name: "Export selected slides", exact: true }).click();
  expect(await page.evaluate(() => (window as any).slidesTest.exports)).toEqual([
    { ids: ["slide00001", "slide00002", "slide00003", "slide00004", "slide00005"], revision: 0 },
    { ids: ["slide00001", "slide00003"], revision: 0 },
  ]);
  await page.evaluate(() => {
    const f = (window as any).slidesTest;
    const next = structuredClone(f.doc);
    next.pages[0].children = [
      next.pages[0].children[1],
      next.pages[0].children[0],
      ...next.pages[0].children.slice(2),
    ];
    f.engine.script({ document: next, revision: 1, readOnly: false });
  });
  await page.getByRole("option", { name: /01/ }).focus();
  await page.keyboard.press("Alt+ArrowDown");
  expect(await page.evaluate(() => (window as any).slidesTest.engine.transactions)).toEqual([
    {
      origin: { actor: { kind: "person", device: "device0001" } },
      ops: [{ type: "reorder", id: "slide00001", index: 1 }],
    },
  ]);
  await expect(page.getByRole("option").first()).toContainText("Small habits");
});
test("read-only presents and exports, disables creation and reorder; missing service is explicit", async ({
  page,
}) => {
  await page.goto("/slides.html?readonly");
  await expect(page.getByRole("button", { name: "New page → Slides" })).toBeDisabled();
  await page.getByRole("option").first().focus();
  await page.keyboard.press("Alt+ArrowDown");
  await page.getByRole("button", { name: "Present", exact: true }).click();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Export PDF · slide order", exact: true }).click();
  expect(await page.evaluate(() => (window as any).slidesTest.engine.transactions)).toEqual([]);
  await page.goto("/slides.html?unavailable");
  await expect(page.getByRole("button", { name: "Present", exact: true })).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Export PDF · slide order", exact: true }),
  ).toBeDisabled();
});
test("slow slide pixels never replace a newer selection", async ({ page }) => {
  await page.goto("/slides.html?slow");
  await page.getByRole("button", { name: "Present", exact: true }).click();
  await page.keyboard.press("ArrowRight");
  await page.evaluate(() => {
    const f = (window as any).slidesTest;
    f.pending.get("slide00004")(f.pixels);
  });
  await expect(page.getByRole("img", { name: "A daily rhythm" })).toBeVisible();
  await page.evaluate(() => {
    const f = (window as any).slidesTest;
    f.pending.get("slide00003")(f.pixels);
  });
  await expect(page.getByRole("img", { name: "A daily rhythm" })).toBeVisible();
  await page.keyboard.press("Escape");
});
for (const mode of ["light", "dark"])
  for (const region of ["strip", "actions"])
    test(`C32 ${mode} ${region} matches design`, async ({ page }) => {
      const ref = `${root}docs/design/snapshots/C32-${mode}-1280.png`;
      test.skip(!existsSync(ref), "design lane: requires exported C32 snapshots");
      await page.goto(`/slides.html?mode=${mode}`);
      await page.evaluate(() => document.fonts.ready);
      const actual = await page
        .locator(region === "strip" ? ".vc-slides" : ".vc-slide-actions-main")
        .screenshot();
      const [w, h, x, y] = region === "strip" ? [360, 556, 8, 100] : [238, 31, 824, 72];
      const result = await page.evaluate(
        async ({ a, b, w, h, x, y }) => {
          const load = async (s: string) => {
            const i = new Image();
            i.src = `data:image/png;base64,${s}`;
            await i.decode();
            return i;
          };
          const [left, right] = await Promise.all([load(a), load(b)]);
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
            for (let j = 0; j < 4; j++) max = Math.max(max, Math.abs(p[i + j]! - q[i + j]!));
            if (max > 48) different++;
          }
          const sheet = document.createElement("canvas");
          sheet.width = w * 2;
          sheet.height = h;
          const ctx = sheet.getContext("2d")!;
          ctx.drawImage(left, x, y, w, h, 0, 0, w, h);
          ctx.drawImage(right, w, 0);
          return {
            fraction: different / (w * h),
            width: right.width,
            height: right.height,
            sheet: sheet.toDataURL("image/png").split(",")[1]!,
          };
        },
        {
          a: readFileSync(ref).toString("base64"),
          b: actual.toString("base64"),
          w: w!,
          h: h!,
          x: x!,
          y: y!,
        },
      );
      mkdirSync(`${root}target/screen-goldens`, { recursive: true });
      writeFileSync(`${root}target/screen-goldens/p68-${region}-${mode}.png`, actual);
      const { sheet, ...measurement } = result;
      writeFileSync(
        `${root}target/screen-goldens/p68-${region}-${mode}.comparison.png`,
        Buffer.from(sheet, "base64"),
      );
      writeFileSync(
        `${root}target/screen-goldens/p68-${region}-${mode}.json`,
        JSON.stringify(measurement),
      );
      expect(result.width).toBe(w);
      expect(result.height).toBe(h);
      expect(result.fraction).toBeLessThanOrEqual(0.01);
    });
