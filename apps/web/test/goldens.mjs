// TEST DATA. @chrome: unchanged 1%/48-channel gate, W02 dialog regions and W03 full frame.
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";

const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.VELLUM_CHROME,
});
const results = [];
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await mkdir(new URL("../../../target/screen-goldens/", import.meta.url), { recursive: true });
  const boxes = {
    "not-shared": [330, 160, 620, 217],
    checking: [330, 160, 620, 217],
    uploading: [330, 160, 620, 229],
    changed: [330, 160, 620, 195],
    failed: [330, 160, 620, 195],
    shared: [330, 160, 620, 357],
  };
  for (const theme of ["light", "dark"])
    for (const state of [
      "not-shared",
      "checking",
      "uploading",
      "shared",
      "changed",
      "failed",
      "account",
    ]) {
      await page.goto(`http://127.0.0.1:3113/?state=${state}&theme=${theme}`);
      await page.waitForLoadState("networkidle");
      await page.evaluate(() => document.fonts.ready);
      const box = boxes[state],
        id = state === "account" ? "W03" : state === "shared" ? "W02" : `W02-${state}`,
        prefix = new URL(`../../../target/screen-goldens/p62-${id}-${theme}`, import.meta.url)
          .pathname;
      const actual = await page.screenshot({
        path: `${prefix}.png`,
        ...(box ? { clip: { x: box[0], y: box[1], width: box[2], height: box[3] } } : {}),
      });
      let expected;
      try {
        expected = await readFile(
          new URL(`../../../docs/design/snapshots/${id}-${theme}-1280.png`, import.meta.url),
        );
      } catch (e) {
        if (e.code !== "ENOENT") throw e;
        results.push({ id, theme, skipped: "reference absent" });
        continue;
      }
      const result = await page.evaluate(
        async ({ expected, actual, box, moduleURL }) => {
          const { compareComponentPixels } = await import(moduleURL);
          const load = async (data, b) => {
            const image = new Image();
            image.src = `data:image/png;base64,${data}`;
            await image.decode();
            const canvas = document.createElement("canvas");
            canvas.width = b?.[2] ?? image.width;
            canvas.height = b?.[3] ?? image.height;
            const ctx = canvas.getContext("2d");
            ctx.drawImage(image, -(b?.[0] ?? 0), -(b?.[1] ?? 0));
            return {
              canvas,
              pixels: {
                width: canvas.width,
                height: canvas.height,
                data: ctx.getImageData(0, 0, canvas.width, canvas.height).data,
              },
            };
          };
          const e = await load(expected, box),
            a = await load(actual);
          const result = compareComponentPixels(e.pixels, a.pixels);
          let sheet;
          if (!result.pass) {
            const c = document.createElement("canvas");
            c.width = e.canvas.width * 2;
            c.height = e.canvas.height;
            const ctx = c.getContext("2d");
            ctx.drawImage(e.canvas, 0, 0);
            ctx.drawImage(a.canvas, e.canvas.width, 0);
            sheet = c.toDataURL("image/png").split(",")[1];
          }
          return { ...result, sheet };
        },
        {
          expected: expected.toString("base64"),
          actual: actual.toString("base64"),
          box,
          moduleURL:
            process.env.VELLUM_TEST_GOLDEN_MODULE ??
            `/@fs${fileURLToPath(new URL("../../../scripts/lib/component-goldens.mjs", import.meta.url))}`,
        },
      );
      if (result.sheet)
        await writeFile(`${prefix}-comparison.png`, Buffer.from(result.sheet, "base64"));
      delete result.sheet;
      results.push({ id, theme, ...result });
      console.log(
        `${id} ${theme}: ${((result.fraction ?? 1) * 100).toFixed(3)}% ${result.pass ? "pass" : "FAIL"}`,
      );
    }
  await writeFile(
    new URL("../../../target/screen-goldens/p62-results.json", import.meta.url),
    JSON.stringify(results, null, 2),
  );
  if (results.some((r) => r.pass === false)) process.exitCode = 1;
} finally {
  await browser.close();
}
