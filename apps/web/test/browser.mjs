// TEST DATA. @chrome. Needs fixture API at 3112 and web app at 3111; no credentials/vendor calls.
import assert from "node:assert/strict";
import { mkdir, readFile } from "node:fs/promises";
import { chromium } from "@playwright/test";

const base = process.env.VELLUM_TEST_WEB_URL ?? "http://127.0.0.1:3111",
  id = "test_00000000000000000000000000000001";
const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.VELLUM_CHROME,
  args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
});
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(`${base}/share/${id}`);
  await page.getByRole("button", { name: "Fixture frame" }).waitFor();
  await page.waitForFunction(() => {
    const canvas = document.querySelector("canvas");
    if (!canvas) return false;
    const gl = canvas.getContext("webgl2");
    if (!gl) return false;
    const p = new Uint8Array(4);
    gl.readPixels(80, 50, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, p);
    return p[0] === 35 && p[1] === 77 && p[2] === 60;
  });
  await mkdir(new URL("../../../target/screen-goldens/", import.meta.url), { recursive: true });
  for (const theme of ["light", "dark"]) {
    await page.evaluate((t) => (document.documentElement.dataset.appearance = t), theme);
    await page.screenshot({
      path: new URL(
        `../../../target/screen-goldens/p62-native-viewer-${theme}.png`,
        import.meta.url,
      ).pathname,
    });
  }
  const response = await page.request.get(`${base}/v1/shares/${id}/download`);
  assert.equal(response.status(), 200);
  assert.equal(response.headers()["content-type"], "application/zip");
  const zip = await response.body();
  const bundle = JSON.parse(
    await readFile(new URL("../../../target/share-acceptance.json", import.meta.url), "utf8"),
  );
  let offset = 0;
  const files = new Map();
  while (zip.readUInt32LE(offset) === 0x04034b50) {
    const size = zip.readUInt32LE(offset + 18),
      n = zip.readUInt16LE(offset + 26),
      extra = zip.readUInt16LE(offset + 28),
      name = zip.subarray(offset + 30, offset + 30 + n).toString(),
      start = offset + 30 + n + extra;
    files.set(name, zip.subarray(start, start + size));
    offset = start + size;
  }
  for (const f of bundle.files) assert.deepEqual(files.get(f.path), Buffer.from(f.data, "base64"));
  assert.equal(files.size, bundle.files.length + 1);
  assert.equal(errors.length, 0, errors.join("\n"));
  const request = await page.request.delete(`http://127.0.0.1:3112/v1/shares/${id}`, {
    headers: { cookie: "fixture=owner", origin: "http://127.0.0.1:3110" },
  });
  assert.equal(request.status(), 200);
  assert.equal((await page.request.get(`${base}/v1/shares/${id}/download`)).status(), 404);
  await page.reload();
  await page.getByText("Share unavailable. The owner may have deleted this link.").waitFor();
  console.log("Native snapshot → API → WASM pixels → ZIP exact bytes → revoke passed.");
} finally {
  await browser.close();
}
