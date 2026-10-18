// Chrome lane. TEST DATA; real workspace over the session and presence feature fakes.
import { expect, type Page, test } from "@playwright/test";

const open = async (page: Page, name: string) => {
  await page.goto(`/?tab=agent&collaboration=${name}`);
  await page.evaluate(() => window.__vellum.agentReady());
};
test("editable plan stays pending until daemon acknowledgement, including order, intent and mode", async ({
  page,
}) => {
  await open(page, "c04");
  await expect(
    page.getByRole("heading", { name: "Plan · 12 screens from DESIGN.md" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Edit Welcome", exact: true }).click();
  await page.getByLabel("Screen name").fill("Introduction");
  await page.getByLabel("Screen intent").fill("Explain the benefits");
  await page.getByRole("button", { name: "Move down", exact: true }).click();
  await page.getByRole("button", { name: "Done editing", exact: true }).click();
  await page.getByLabel("Plan mode").selectOption("side_by_side");
  await page.getByRole("button", { name: "Build", exact: true }).click();
  const requests = await page.evaluate(() => window.__vellum.agentRequests());
  expect(requests.at(-1)).toMatchObject({
    type: "buildPlan",
    parallel: { count: 4, mode: "side_by_side" },
    plan: {
      items: expect.arrayContaining([
        expect.objectContaining({ name: "Introduction", intent: "Explain the benefits" }),
      ]),
    },
  });
  await expect(page.getByText("Needs you · review before building")).toBeVisible();
});
test("parallel lanes have their own timelines and live progress", async ({ page }) => {
  await open(page, "c05");
  await expect(page.getByText("5 of 12 screens done")).toBeVisible();
  await page.getByRole("button", { name: /Teal · Onboarding/ }).click();
  await expect(
    page
      .locator(".va-lane[data-agent]")
      .first()
      .getByText(/Designed Welcome/),
  ).toBeVisible();
  await expect(page.locator(".vp-placeholder")).toHaveCount(7);
});
test("alternatives can preview, highlight and keep a selected result through the feed", async ({
  page,
}) => {
  await open(page, "c07");
  await page.getByRole("button", { name: "Keep B on canvas", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Keep B, discard others", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Keep B, discard others", exact: true }).click();
  expect((await page.evaluate(() => window.__vellum.agentRequests())).at(-1)).toMatchObject({
    type: "keepOptions",
    options: [2],
  });
  await page.getByRole("button", { name: "Preview A", exact: true }).hover();
  await expect(page.locator(".vp-alternative[data-highlighted]")).toHaveCount(1);
});
test("presence has labels, follow, session, frame stop and pause acknowledgement", async ({
  page,
}) => {
  await open(page, "c05");
  await page.getByRole("button", { name: "Agents", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: /Follow Teal/ })).toBeVisible();
  await page.getByRole("menuitem", { name: /Follow Teal/ }).click();
  await expect(page.getByText("Following Teal · Esc to stop")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByText("Following Teal · Esc to stop")).toBeHidden();
  await page
    .locator(".vp-lease")
    .first()
    .getByRole("button", { name: "Stop", exact: true })
    .click();
  expect((await page.evaluate(() => window.__vellum.presenceRequests())).at(-1)).toMatchObject({
    type: "stopFrame",
  });
  await page.getByRole("button", { name: "Agents", exact: true }).click();
  await page.getByRole("menuitem", { name: "Pause all agents", exact: true }).click();
  await expect(page.getByText("Agents paused · their changes are being rejected")).toBeHidden();
  await page.evaluate(() => window.__vellum.presencePaused(true));
  await expect(page.getByText("Agents paused · their changes are being rejected")).toBeVisible();
  await page.getByRole("button", { name: "Resume", exact: true }).click();
  expect((await page.evaluate(() => window.__vellum.presenceRequests())).at(-1)).toMatchObject({
    type: "resumeAll",
  });
});
test("canvas notes save person Ops; prompt Run sends its engine, model and nearby context", async ({
  page,
}) => {
  await open(page, "c35");
  await expect(page.getByText("Read by agents", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Edit note", exact: true }).click();
  await page.getByLabel("Note content").fill("Review the copy");
  await page.evaluate(() => window.__vellum.replyDocument(window.__vellum.document()));
  await page.getByRole("button", { name: "Save note", exact: true }).click();
  expect((await page.evaluate(() => window.__vellum.transactions())).at(-1)).toMatchObject({
    origin: { actor: { kind: "person" } },
    ops: [{ type: "update", id: "note", patch: { content: "Review the copy" } }],
  });
  await page.getByRole("button", { name: "Run", exact: true }).click();
  const sent = await page.evaluate(() => window.__vellum.composerSent());
  expect(sent.at(-1)?.request).toMatchObject({
    text: "Make a tablet version of Step 2",
    engine: { engine: "builtin", model: "deepseek/deepseek-v4.1-flash" },
    context: expect.arrayContaining([expect.objectContaining({ kind: "context" })]),
  });
  await expect(page.getByText(/Last run/)).toBeVisible();
});

test("person edits a leased frame through the properties panel and the lease tab updates", async ({
  page,
}) => {
  await page.goto("/?tab=agent&collaboration=c05&select=screen2");
  await page.evaluate(() => window.__vellum.agentReady());
  await page.evaluate(() => window.__vellum.replyDocument(window.__vellum.document()));
  const x = page.locator(".vc-right").getByLabel("X", { exact: true });
  await x.fill("660");
  await x.press("Enter");
  await expect(page.locator(".vp-lease").first()).toContainText("You're editing here too");
  const tx = await page.evaluate(() => window.__vellum.transactions());
  expect(tx.at(-1)).toMatchObject({
    origin: { actor: { kind: "person" } },
    ops: [{ type: "update", id: "screen2" }],
  });
});
test("Explore keeps its change labels, and nearby context chips can be removed", async ({
  page,
}) => {
  await open(page, "c07e");
  await expect(page.getByRole("region", { name: "Explore alternatives" })).toContainText(
    "Warmer palette, serif headings",
  );
  await open(page, "c35");
  const chip = page.locator('.vk-chip[data-kind="context"]');
  await expect(chip).toHaveText(/Onboarding rules/);
  await chip.getByRole("button").click();
  await expect(chip).toHaveCount(0);
});
