// Chrome + design lane: the editor chrome against the design's own screens (C03, C18, S13) at the sizes and
// appearances the design provides. Needs Playwright's Chromium and the local snapshot PNGs
// (docs/design/snapshots/, made by `pnpm design:snapshot`, gitignored). Without the PNGs these tests skip and say so.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, type Page, test } from "@playwright/test";
import screens from "./screens.json" with { type: "json" };

const repo = fileURLToPath(new URL("../../../", import.meta.url));
const snapshots = `${repo}docs/design/snapshots/`;
const out = `${repo}target/screen-goldens/`;
const { fraction, channel } = screens.tolerance;
const SIZE_SLACK = 1;

type Region = {
  name: string;
  selector: string;
  design: number[];
  clipHeight?: number;
  /** Text-dense regions: how much of the region may differ, with the reason. Never above MAX_FRACTION. */
  fraction?: number;
  why?: string;
};
const MAX_FRACTION = 0.11;

async function compare(page: Page, design: Buffer, mine: Buffer, box: number[]) {
  return page.evaluate(
    async ({ a, b, box, channel }) => {
      const load = async (base64: string) => {
        const image = new Image();
        image.src = `data:image/png;base64,${base64}`;
        await image.decode();
        return image;
      };
      const [d, m] = await Promise.all([load(a), load(b)]);
      const [x, y, w, h] = box as [number, number, number, number];
      if (m.width !== w || m.height !== h)
        return {
          pass: false,
          reason: `size differs: editor ${m.width}×${m.height}, design ${w}×${h}`,
        };
      const read = (img: HTMLImageElement, sx: number, sy: number) => {
        const c = new OffscreenCanvas(w, h).getContext("2d") as OffscreenCanvasRenderingContext2D;
        c.drawImage(img, sx, sy, w, h, 0, 0, w, h);
        return c.getImageData(0, 0, w, h).data;
      };
      const da = read(d, x, y);
      const ma = read(m, 0, 0);
      let different = 0;
      for (let i = 0; i < da.length; i += 4) {
        let delta = 0;
        for (let k = 0; k < 4; k++)
          delta = Math.max(delta, Math.abs((da[i + k] ?? 0) - (ma[i + k] ?? 0)));
        if (delta > channel) different++;
      }
      // Design, editor and difference, enlarged, for whoever has to find out why it failed.
      const scale = Math.max(1, Math.min(6, Math.floor(900 / w)));
      const sheet = new OffscreenCanvas(w * scale, h * scale * 3 + 8);
      const g = sheet.getContext("2d") as OffscreenCanvasRenderingContext2D;
      g.imageSmoothingEnabled = false;
      const put = (data: Uint8ClampedArray, row: number) => {
        const tmp = new OffscreenCanvas(w, h);
        (tmp.getContext("2d") as OffscreenCanvasRenderingContext2D).putImageData(
          new ImageData(data as Uint8ClampedArray<ArrayBuffer>, w, h),
          0,
          0,
        );
        g.drawImage(tmp, 0, row * (h * scale + 4), w * scale, h * scale);
      };
      const delta = new Uint8ClampedArray(da.length);
      for (let i = 0; i < da.length; i += 4) {
        let dd = 0;
        for (let k = 0; k < 3; k++)
          dd = Math.max(dd, Math.abs((da[i + k] ?? 0) - (ma[i + k] ?? 0)));
        delta[i] = dd;
        delta[i + 1] = 0;
        delta[i + 2] = dd;
        delta[i + 3] = 255;
      }
      put(da, 0);
      put(ma, 1);
      put(delta, 2);
      const blob = await sheet.convertToBlob({ type: "image/png" });
      const sheetBase64 = await new Promise<string>((resolve) => {
        const r = new FileReader();
        r.onload = () => resolve(String(r.result).split(",")[1] ?? "");
        r.readAsDataURL(blob);
      });
      return { pass: true, different, fraction: different / (w * h), sheet: sheetBase64 };
    },
    { a: design.toString("base64"), b: mine.toString("base64"), box, channel },
  );
}

for (const screen of screens.screens)
  for (const mode of screen.modes) {
    const file = snapshots + screen.snapshot.replace("{mode}", mode);
    test(`${screen.id} ${mode} at ${screen.size[0]}×${screen.size[1]}`, async ({ page }) => {
      test.skip(
        !existsSync(file),
        `design snapshot ${screen.snapshot.replace("{mode}", mode)} is not exported here`,
      );
      await page.emulateMedia({ reducedMotion: "reduce" });
      await page.setViewportSize({
        width: screen.size[0] as number,
        height: screen.size[1] as number,
      });
      await page.goto(`/?appearance=${mode}&${screen.query}`);
      await page.evaluate(() => document.fonts.ready);
      await expect(page.locator(".vc-docbar")).toBeVisible();
      const hover = (screen as { hover?: string }).hover;
      if (hover)
        await page
          .getByRole("treeitem", { name: hover === "continue" ? "Continue" : hover })
          .hover();
      else await page.mouse.move(0, 0);
      const design = readFileSync(file);
      mkdirSync(out, { recursive: true });
      const failures: string[] = [];
      for (const region of screen.regions as Region[]) {
        const el = page.locator(region.selector).first();
        const bb = (await el.boundingBox()) as {
          x: number;
          y: number;
          width: number;
          height: number;
        };
        // Text is measured by different engines, so an element may be a pixel off the design's size; more than
        // that is a layout difference and fails as one.
        const [, , dw = 0, dh = 0] = region.design;
        const height = region.clipHeight ?? bb.height;
        const near = (a: number, b: number) => Math.abs(a - b) <= SIZE_SLACK;
        const clip = {
          x: bb.x,
          y: bb.y,
          width: near(bb.width, dw) ? dw : bb.width,
          height: near(height, dh) ? dh : height,
        };
        const mine = await page.screenshot({ clip });
        const result = await compare(page, design, mine, region.design);
        const tag = `${screen.id}-${mode}-${region.name.replace(/\W+/g, "-")}`;
        if (process.env.GOLDEN_VERBOSE)
          console.log(
            `${screen.id} ${mode} ${region.name}: ${"reason" in result ? result.reason : ((result.fraction ?? 0) * 100).toFixed(2)}`,
          );
        const limit = Math.min(region.fraction ?? fraction, MAX_FRACTION);
        if (!result.pass || (result.fraction ?? 1) > limit) {
          writeFileSync(`${out}${tag}.editor.png`, mine);
          if ("sheet" in result && result.sheet)
            writeFileSync(`${out}${tag}.compare.png`, Buffer.from(result.sheet, "base64"));
          failures.push(
            `${region.name}: ${"reason" in result ? result.reason : `${((result.fraction ?? 0) * 100).toFixed(2)}% of pixels differ (limit ${limit * 100}%)`}`,
          );
        }
      }
      expect(failures, failures.join("\n")).toEqual([]);
    });
  }
