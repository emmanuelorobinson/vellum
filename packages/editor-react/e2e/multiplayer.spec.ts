// @chrome. TEST DATA: scripted host boundary; Rust owns the wire/convergence tests.
import { expect, test } from "@playwright/test";

test("C arms an unanchored comment; Escape cancels and clicking focuses the composer", async ({
  page,
}) => {
  await page.goto("/?scenario=juniper&live=presence");
  await page.locator(".vc-canvas").click({ position: { x: 700, y: 250 } });
  await page.keyboard.press("c");
  await expect(
    page.getByRole("status").filter({ hasText: "Click the canvas to place a comment" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByText("Click the canvas to place a comment · Esc to cancel")).toHaveCount(
    0,
  );
  await page.keyboard.press("c");
  await page.locator(".vc-canvas").click({ position: { x: 700, y: 250 } });
  await expect(page.getByRole("textbox", { name: "New comment", exact: true })).toBeFocused();
});

test("two people and two agents have distinct markers; following restores the local camera", async ({
  page,
}) => {
  await page.goto("/?scenario=juniper&live=presence");
  await expect(page.locator(".vm-cursor")).toHaveCount(4);
  await page.getByRole("button", { name: "In this document", exact: true }).click();
  await expect(page.getByRole("region", { name: "Participants" })).toContainText(
    "2 people · 2 agents",
  );
  const before = await page.evaluate(() => window.__vellum.viewport());
  await page
    .locator(".vm-participant")
    .filter({ hasText: "Alex Rivera" })
    .getByRole("button", { name: "Follow" })
    .click();
  await expect(page.getByText("Following Alex Rivera · Esc to stop")).toBeVisible();
  await page.keyboard.press("Escape");
  expect(await page.evaluate(() => window.__vellum.viewport())).toEqual(before);
  await page.evaluate(() => window.__vellum.liveConnection("offline"));
  await expect(page.locator(".vm-cursor")).toHaveCount(0);
  await expect(page.getByRole("status").filter({ hasText: "Connection lost" })).toBeVisible();
});
test("role loss preserves a draft and disables posting", async ({ page }) => {
  await page.goto("/?scenario=juniper&live=comments");
  await page
    .getByRole("textbox", { name: "Reply to this thread", exact: true })
    .fill("TEST DATA: keep my draft");
  await page.evaluate(() => window.__vellum.liveRole("viewer"));
  await expect(
    page.getByRole("textbox", { name: "Reply to this thread", exact: true }),
  ).toHaveValue("TEST DATA: keep my draft");
  await expect(page.getByRole("button", { name: "Post reply", exact: true })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Copy draft", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Close comments", exact: true }).click();
  await expect(page.locator(".vm-comments-entry")).toBeFocused();
});
test("failed posts retain text and retry the same idempotent request", async ({ page }) => {
  await page.goto("/?scenario=juniper&live=failed");
  await page
    .getByRole("textbox", { name: "Reply to this thread", exact: true })
    .fill("TEST DATA: failed reply");
  await page.getByRole("button", { name: "Post reply", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Couldn’t post your reply");
  await expect(
    page.getByRole("textbox", { name: "Reply to this thread", exact: true }),
  ).toHaveValue("TEST DATA: failed reply");
  await page.getByRole("button", { name: "Try again", exact: true }).click();
  const requests = (await page.evaluate(() => window.__vellum.liveRequests())).filter(
    (r) => !!r && typeof r === "object" && "type" in r && r.type === "comment",
  );
  expect(requests).toHaveLength(2);
  expect(requests[0]).toEqual(requests[1]);
});
