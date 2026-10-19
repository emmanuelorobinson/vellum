// TEST DATA. @chrome: a running account app and headless Chrome; routes are intercepted, no mail sent.

import assert from "node:assert/strict";
import { mkdir, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import { compareComponentPixels } from "../../../scripts/lib/component-goldens.mjs";

const browser = await chromium.launch({
  executablePath: process.env.VELLUM_CHROME,
  headless: true,
});
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page.goto(
    new URL("/sign-in", process.env.VELLUM_TEST_ACCOUNT_URL ?? "http://127.0.0.1:3100").href,
  );
  await page.evaluate(() => document.fonts.ready);
  const dir = fileURLToPath(new URL("../../../target/screen-goldens", import.meta.url));
  await mkdir(dir, { recursive: true });
  const decode = (bytes) =>
    page.evaluate(async (base64) => {
      const img = new Image();
      img.src = `data:image/png;base64,${base64}`;
      await img.decode();
      const canvas = document.createElement("canvas");
      canvas.width = img.width;
      canvas.height = img.height;
      const ctx = canvas.getContext("2d");
      ctx.drawImage(img, 0, 0);
      return {
        width: img.width,
        height: img.height,
        data: Array.from(ctx.getImageData(0, 0, img.width, img.height).data),
      };
    }, bytes.toString("base64"));
  for (const theme of ["light", "dark"]) {
    await page.evaluate(
      (theme) => document.documentElement.setAttribute("data-appearance", theme),
      theme,
    );
    const actual = await page.screenshot({ path: `${dir}/p61-A05-${theme}.png` });
    let expected;
    try {
      expected = await readFile(
        new URL(`../../../docs/design/snapshots/A05-${theme}-1280.png`, import.meta.url),
      );
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      console.log(`A05 ${theme}: design PNG absent, comparison skipped`);
      continue;
    }
    const comparison = compareComponentPixels(await decode(expected), await decode(actual));
    assert.ok(comparison.pass, `A05 ${theme} differs by ${comparison.fraction * 100}%`);
    console.log(`A05 ${theme}: ${(comparison.fraction * 100).toFixed(3)}%, within 1%`);
  }
  await page.route("**/api/auth/sign-in/magic-link", (r) =>
    r.fulfill({ status: 200, contentType: "application/json", body: '{"success":true}' }),
  );
  await page.getByRole("textbox", { name: "Email" }).fill("person@example.test");
  await page.getByRole("button", { name: "Continue with email" }).click();
  await page.getByRole("status").filter({ hasText: "Check your email" }).waitFor();
  await page.unroute("**/api/auth/sign-in/magic-link");
  await page.route("**/api/auth/sign-in/magic-link", (r) =>
    r.fulfill({ status: 503, body: "private fixture detail" }),
  );
  await page.getByRole("button", { name: "Continue with email" }).click();
  await page.getByRole("status").filter({ hasText: "Sign-in unavailable" }).waitFor();
  if ((await page.textContent("body")).includes("private fixture detail"))
    throw new Error("Raw error exposed");
  console.log(
    "Account browser: mailbox confirmation and sanitized failure passed; light/dark screenshots saved.",
  );
} finally {
  await browser.close();
}
