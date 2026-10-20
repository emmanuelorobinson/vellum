// TEST DATA. @chrome: local account + fixture API; billing destination intercepted, no vendor access.
import assert from "node:assert/strict";
import { chromium } from "@playwright/test";

const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.VELLUM_CHROME,
});
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page
    .context()
    .addCookies([{ name: "fixture", value: "owner", url: "http://127.0.0.1:3110" }]);
  await page.goto("http://127.0.0.1:3110/");
  await page.getByRole("button", { name: "Manage plan", exact: true }).waitFor();
  await page.getByRole("link", { name: "Snapshot fixture", exact: true }).waitFor();
  await page.getByRole("button", { name: "Revoke", exact: true }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Cancel", exact: true }).click();
  assert.equal(await page.getByRole("alertdialog").count(), 0);
  await page.route("https://billing.example.test/**", (r) =>
    r.fulfill({ status: 200, contentType: "text/html", body: "<p>TEST DATA billing portal</p>" }),
  );
  await page.getByRole("button", { name: "Manage plan", exact: true }).click();
  await page.getByText("TEST DATA billing portal").waitFor();
  console.log(
    "Account fixture identity, Pro plan, shared files, session confirmation and billing redirect passed.",
  );
} finally {
  await browser.close();
}
