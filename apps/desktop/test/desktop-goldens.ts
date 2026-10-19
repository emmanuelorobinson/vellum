// TEST DATA. Full-frame design comparison; missing runtime content is never drawn as live product data.
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { expect } from "@playwright/test";
import type { Page } from "playwright";

export async function compareDesktop(page: Page, screen: string, appearance: string) {
  const reference = await readFile(
    resolve(`../../docs/design/snapshots/${screen}-${appearance}-1280.png`),
  );
  await page.evaluate(() => document.fonts.ready);
  const actual = await page.screenshot({ animations: "disabled", scale: "css" });
  const result = await page.evaluate(
    async ({ actual, reference }) => {
      const load = async (source: string) => {
        const image = new Image();
        image.src = `data:image/png;base64,${source}`;
        await image.decode();
        return image;
      };
      const [a, e] = await Promise.all([load(actual), load(reference)]);
      if (a.width !== e.width || a.height !== e.height) throw Error("different frame dimensions");
      const w = a.width,
        h = a.height;
      const read = (image: HTMLImageElement) => {
        const canvas = new OffscreenCanvas(w, h);
        const g = canvas.getContext("2d");
        if (!g) throw Error("canvas");
        g.drawImage(image, 0, 0);
        return g.getImageData(0, 0, w, h).data;
      };
      const ad = read(a),
        ed = read(e);
      let pixels = 0;
      const diff = new Uint8ClampedArray(ad.length);
      for (let i = 0; i < ad.length; i += 4) {
        const delta = Math.max(
          ...[0, 1, 2, 3].map((c) => Math.abs((ad[i + c] ?? 0) - (ed[i + c] ?? 0))),
        );
        if (delta > 48) pixels++;
        diff[i] = delta;
        diff[i + 2] = delta;
        diff[i + 3] = 255;
      }
      const sheet = new OffscreenCanvas(w * 3, h);
      const g = sheet.getContext("2d");
      if (!g) throw Error("canvas");
      g.drawImage(e, 0, 0);
      g.drawImage(a, w, 0);
      g.putImageData(new ImageData(diff, w, h), w * 2, 0);
      const png = btoa(
        Array.from(new Uint8Array(await (await sheet.convertToBlob()).arrayBuffer()), (b) =>
          String.fromCharCode(b),
        ).join(""),
      );
      return { fraction: pixels / (w * h), png };
    },
    { actual: actual.toString("base64"), reference: reference.toString("base64") },
  );
  const output = resolve("../../target/screen-goldens");
  await mkdir(output, { recursive: true });
  await writeFile(resolve(output, `${screen}-${appearance}-desktop.png`), actual);
  await writeFile(
    resolve(output, `${screen}-${appearance}-desktop-compare.png`),
    Buffer.from(result.png, "base64"),
  );
  expect(result.fraction, `${screen} ${appearance} full-frame fraction`).toBeLessThanOrEqual(0.01);
}
