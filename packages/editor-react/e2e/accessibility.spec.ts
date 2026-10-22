// Chrome lane; TEST DATA. Needs Playwright Chromium. No network providers or document runtime required.
import { expect, test } from "@playwright/test";

for (const appearance of ["light", "dark", "hc-light", "hc-dark"]) {
  test(`keyboard canvas and accessible Layers in ${appearance}`, async ({ page }) => {
    await page.goto(`/?appearance=${appearance}&tab=layers`);
    const entry = page.getByRole("button", { name: "Keyboard canvas mode", exact: true });
    await entry.focus();
    await entry.press("Enter");
    const canvas = page.locator(".vc-canvas");
    await expect(canvas).toBeFocused();
    await expect(canvas).toHaveAttribute("data-keyboard", "on");
    await canvas.press("Tab");
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["welcome"]);
    await canvas.press("Tab");
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["step1"]);
    await canvas.press("Shift+Tab");
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["welcome"]);
    await canvas.press("Shift+Tab");
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["step2"]);
    await canvas.press("Enter");
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["card"]);
    await expect(page.getByRole("treeitem", { name: "Habit card" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await canvas.press("Escape");
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["step2"]);
    expect(await page.evaluate(() => window.__vellum.transactions())).toEqual([]);
    await canvas.press("F6");
    await expect(entry).toBeFocused();
    await expect(canvas).toHaveAttribute("data-keyboard", "off");
    await entry.press("Tab");
    await expect(entry).not.toBeFocused();
    const tree = page.getByRole("tree", { name: "Layers", exact: true });
    await expect(tree).toBeVisible();
    await expect(tree.getByRole("treeitem", { name: "Step 2", exact: true })).toHaveAttribute(
      "aria-level",
      "1",
    );
  });
}

for (const appearance of ["light", "dark"]) {
  test(`Arabic mirrors chrome without changing canvas coordinates in ${appearance}`, async ({
    page,
  }) => {
    await page.goto(`/?language=ar&appearance=${appearance}&tab=layers`);
    const root = page.locator(".vc-workspace"),
      canvas = page.locator(".vc-canvas");
    await expect(root).toHaveAttribute("dir", "rtl");
    await expect(root).toHaveAttribute("lang", "ar");
    await expect(page.getByRole("tab", { name: "الطبقات", exact: true })).toBeVisible();
    const panel = await page.locator(".vc-panel").boundingBox();
    expect(panel?.x).toBeGreaterThan(900);
    expect(await canvas.evaluate((el) => getComputedStyle(el).direction)).toBe("ltr");
    expect(await canvas.evaluate((el) => el.getBoundingClientRect().x)).toBe(0);
    await expect(page.locator(".vc-docbar__name")).toHaveAttribute("dir", "ltr");
    await page.getByRole("treeitem", { name: "Step 2", exact: true }).focus();
    await page.keyboard.press("ArrowLeft");
    await expect(page.getByRole("treeitem", { name: "Habit card", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.__vellum.transactions())).toEqual([]);
    await page.screenshot({
      path: `../../target/screen-goldens/p70-arabic-${appearance}-1280.png`,
    });
  });
}

test("language preference updates an open workspace, without translating document content", async ({
  page,
}) => {
  await page.goto("/?shell=1&settings=open");
  const language = page.getByRole("combobox", { name: "Language", exact: true });
  await expect(language.locator("option")).toHaveCount(13);
  await language.selectOption("ar");
  await expect(page.locator(".vc-workspace")).toHaveAttribute("dir", "rtl");
  await expect(
    page.getByRole("tab", { name: "الطبقات", exact: true, includeHidden: true }),
  ).toHaveText("الطبقات");
  await page.getByRole("combobox", { name: "اللغة", exact: true }).selectOption("ja");
  await expect(page.locator(".vc-workspace")).toHaveAttribute("dir", "ltr");
  await page.getByRole("radio", { name: "ハイコントラスト", exact: true }).click();
  await expect(page.locator(".vc-workspace")).toHaveAttribute("data-appearance", "hc-light");
  await expect(page.locator(".vc-workspace")).toHaveAttribute("data-canvas-appearance", "hc-light");
  await expect(
    page.getByRole("tab", { name: "レイヤー", exact: true, includeHidden: true }),
  ).toHaveText("レイヤー");
  expect(await page.evaluate(() => window.__vellum.transactions())).toEqual([]);
});

test("polite completion announcements ignore streamed prose and stored history, even with Layers open", async ({
  page,
}) => {
  await page.goto("/?agent=stream&tab=layers");
  await page.evaluate(() => window.__vellum.agentReady());
  const region = page.locator("[data-completions]");
  await expect(region).toHaveAttribute("aria-live", "polite");
  await expect(region).toBeEmpty();
  const session = "01J00000000000000000000S01",
    turn = "01J00000000000000000000T01";
  await page.evaluate(
    ({ session, turn }) =>
      window.__vellum.agentSend({
        type: "live",
        session,
        turn,
        item: { type: "text", text: "Streaming text" },
      }),
    { session, turn },
  );
  await expect(region).toBeEmpty();
  await page.evaluate(
    ({ session, turn }) =>
      window.__vellum.agentSend({
        type: "event",
        session,
        event: {
          seq: 100,
          at: 1792160000000,
          origin: { actor: { kind: "agent", agent: "01J00000000000000000000A01" }, session, turn },
          body: {
            type: "step",
            tool: "update_nodes",
            arguments: "{}",
            result: "{}",
            startedAt: 0,
            durationMs: 1000,
            outcome: "done",
            detail: {
              verb: "updated",
              target: { id: "welcome", name: "Welcome", type: "frame" },
              counts: { created: 0, changed: 1, removed: 0, variables: 0 },
            },
          },
        },
      }),
    { session, turn },
  );
  await expect(region).toContainText("Step complete");
  const afterStep = await region.innerHTML();
  await page.evaluate(
    ({ session, turn }) =>
      window.__vellum.agentSend({
        type: "live",
        session,
        turn,
        item: { type: "text", text: "More prose" },
      }),
    { session, turn },
  );
  await expect(region).toHaveJSProperty("innerHTML", afterStep);
  await page.evaluate(
    ({ session, turn }) =>
      window.__vellum.agentSend({
        type: "event",
        session,
        event: {
          seq: 101,
          at: 1792160000001,
          origin: { actor: { kind: "agent", agent: "01J00000000000000000000A01" }, session, turn },
          body: { type: "turn", turn, from: "running", input: "done", to: "done" },
        },
      }),
    { session, turn },
  );
  await expect(region).toContainText("Turn complete");
});

test("reduced motion and animation preference disable descendant movement", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/?agent=stream&shell=1");
  expect(
    await page
      .locator(".vc-bar")
      .first()
      .evaluate((el) => getComputedStyle(el).animationName),
  ).toBe("none");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.getByRole("button", { name: "Editor settings", exact: true }).click();
  await page.getByRole("switch", { name: "Animations and effects", exact: true }).click();
  await expect(page.locator(".vc-workspace")).toHaveAttribute("data-animations", "off");
  expect(
    await page
      .locator(".vc-bar")
      .first()
      .evaluate((el) => getComputedStyle(el).animationName),
  ).toBe("none");
});
