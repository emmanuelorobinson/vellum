// Chrome lane. TEST DATA: Juniper ledger entries, scripted engine replies and fake hand-off ports.
import { expect, test } from "@playwright/test";
import { TOKEN_TEXT } from "../fixtures/ledger-scenarios";

const POSITION = "pan=484,197";
const state = (page: import("@playwright/test").Page) =>
  page.evaluate(() => window.__vellum.ledgerState() as Record<string, any>);

test("history lists newest first, expands to ops and filters by actor", async ({ page }) => {
  await page.goto(`/?ledger=c10&${POSITION}`);
  const rows = page.locator(".vl-entry");
  await expect(rows.first()).toContainText("Teal");
  await expect(rows.first()).toContainText("10:43 · 24 changes");
  await expect(page.locator(".vl-op")).toContainText("Update Continue · #cta-02");
  await expect(page.locator(".vl-op")).toContainText(TOKEN_TEXT.opDetail);
  await page.getByLabel("Session").selectOption("all");
  await expect(rows).toHaveCount(6);
  await page.getByLabel("Actor").selectOption("file");
  await expect(rows).toHaveCount(1);
  await expect(rows.first()).toContainText("File changed on disk · git checkout");
  await expect(rows.first()).toContainText("14 changes applied");
  await page.getByLabel("Actor").selectOption("you");
  await expect(rows).toHaveCount(1);
  await expect(page.locator(".vl-footer")).toContainText("Full history is in Git.");
});

test("select layers selects, Copy as JSON copies the recorded ops", async ({ page }) => {
  await page.goto(`/?ledger=c10&${POSITION}`);
  await page.getByRole("button", { name: "Select layers" }).click();
  expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["step2"]);
  await page.getByRole("button", { name: "Copy as JSON" }).click();
  const copied = (await state(page)).copied as string[];
  expect(JSON.parse(copied[0] as string)).toEqual([
    { type: "update", id: "cta-02", patch: { fill: { $var: "surface" } } },
  ]);
});

test("revert asks first, then applies the inverse as a new entry attributed to you", async ({
  page,
}) => {
  await page.goto(`/?ledger=c10&${POSITION}`);
  await page.getByRole("button", { name: "Revert", exact: true }).click();
  await expect(page.getByRole("alertdialog", { name: "Revert this change?" })).toBeVisible();
  expect(await page.evaluate(() => window.__vellum.transactions())).toHaveLength(0);
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await page.evaluate(() => window.__vellum.ledgerReply(window.__vellum.nodes()));
  await page.getByRole("button", { name: "Revert", exact: true }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Revert" }).click();
  const tx = (await page.evaluate(() => window.__vellum.transactions())) as any[];
  expect(tx.at(-1)).toMatchObject({
    origin: { actor: { kind: "person" } },
    ops: [{ type: "update", id: "cta-02", patch: { name: "Continue" } }],
  });
  await expect(page.locator(".vl-toast").filter({ hasText: "Reverted by you" })).toBeVisible();
  await page.getByLabel("Session").selectOption("all");
  await expect(page.locator(".vl-entry").first()).toContainText("Reverted Teal’s batch");
  await expect(page.locator(".vl-entry").first()).toContainText("Sam");
  await expect(page.locator("[data-entry=teal]")).toBeVisible();
});

test("a refused revert says so and records nothing", async ({ page }) => {
  await page.goto(`/?ledger=c10&${POSITION}`);
  await page.getByRole("button", { name: "Revert", exact: true }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Revert" }).click();
  await expect(page.getByRole("alert")).toContainText("Revert refused");
  await expect(page.locator(".vl-toast").filter({ hasText: "Reverted by you" })).toHaveCount(0);
});

test("undo this change goes through the host and names whose batch it was", async ({ page }) => {
  await page.goto(`/?ledger=c10-undoable&${POSITION}`);
  await page.getByRole("button", { name: "Undo this change" }).click();
  await expect(page.locator(".vl-toast")).toContainText("Teal’s batch was undone");
  expect((await state(page)).undone).toEqual(["teal"]);
});

test("inspect shows the spec, copies the prompt and exports HTML with the chosen settings", async ({
  page,
}) => {
  await page.goto(`/?ledger=c19&${POSITION}`);
  await page.getByRole("tab", { name: "Inspect" }).click();
  await expect(page.getByText("Column · Hug height · Fixed 240 px")).toBeVisible();
  await expect(page.locator(".vl-code")).toContainText("display: flex;");
  await page.getByLabel("Code stack").selectOption("tailwind");
  await expect(page.locator(".vl-code")).toContainText("flex flex-col gap-4 p-6");
  await page.getByRole("button", { name: "Copy prompt for your coding agent" }).click();
  await expect(page.locator(".vl-toast")).toContainText("Copied prompt for your agent");
  await expect(page.locator(".vl-toast")).toContainText("using get_spec.");
  expect((await state(page)).clipboard.at(-1)).toContain("using get_spec.");
  await page.getByRole("radio", { name: "HTML + CSS" }).click();
  await page.getByRole("button", { name: /Scaffold: On/ }).click();
  await page.getByLabel("Scaffold").uncheck();
  await page.getByRole("button", { name: "Copy HTML" }).click();
  expect((await state(page)).html.at(-1)).toMatchObject({ flavour: "css", scaffold: false });
  await page.getByRole("button", { name: "Implement with…" }).click();
  await page.getByRole("menuitem", { name: /Claude Code/ }).click();
  expect((await state(page)).implemented).toEqual([["claude-code", "step2"]]);
});

test("token import shows the exact diff and applies it as one person transaction", async ({
  page,
}) => {
  await page.goto(`/?ledger=c20&${POSITION}`);
  const dialog = page.getByRole("dialog", { name: "Import tokens from CSS" });
  await expect(dialog.getByText("3 changes to review")).toBeVisible();
  const rows = dialog.locator(".vl-diff__row");
  await expect(rows).toHaveCount(3);
  await expect(rows.nth(0)).toContainText("Added");
  await expect(rows.nth(0)).toContainText("space/4");
  await expect(rows.nth(0)).toContainText("— → 16 px");
  await expect(rows.nth(1)).toContainText(TOKEN_TEXT.changed);
  await expect(rows.nth(2)).toContainText(TOKEN_TEXT.removed);
  expect(await page.evaluate(() => window.__vellum.transactions())).toHaveLength(0);
  await page.evaluate(() => window.__vellum.ledgerReply(window.__vellum.nodes()));
  await dialog.getByRole("button", { name: "Apply" }).click();
  expect(
    ((await page.evaluate(() => window.__vellum.transactions())) as any[]).at(-1),
  ).toMatchObject({ origin: { actor: { kind: "person" } }, ops: [{ type: "setVariables" }] });
  await expect(page.locator(".vl-toast")).toContainText("Applied 3 token changes");
  await expect(page.locator(".vl-toast")).toContainText("Imported from src/tokens.css.");
});

test("token export previews the file and writes the chosen format", async ({ page }) => {
  await page.goto(`/?ledger=c20-export&${POSITION}`);
  const dialog = page.getByRole("dialog", { name: "Export design tokens" });
  await expect(dialog.locator(".vl-preview")).toContainText(TOKEN_TEXT.exported);
  await dialog.getByRole("radio", { name: "Tailwind v4" }).click();
  await expect(dialog.locator(".vl-preview")).toContainText("@theme");
  await dialog.getByRole("button", { name: "Write file" }).click();
  expect((await state(page)).written[0]).toMatchObject({ format: "tailwind", light: ":root" });
});

test("export lists each frame's file name and writes the request", async ({ page }) => {
  await page.goto(`/?ledger=c21&${POSITION}`);
  const dialog = page.getByRole("dialog", { name: "Export frames" });
  await expect(dialog.getByText("3 frames will be written to your chosen folder.")).toBeVisible();
  await expect(dialog.locator(".vl-thumbs figcaption")).toHaveText([
    "step-1@2x.png",
    "step-2.svg",
    "step-3.pdf",
  ]);
  await dialog.getByLabel("Step 2 format").selectOption("png");
  await expect(dialog.locator(".vl-thumbs figcaption").nth(1)).toHaveText("step-2@2x.png");
  await dialog.getByRole("button", { name: "Export 3 frames" }).click();
  const exported = (await state(page)).exported[0];
  expect(exported.destination).toBe("~/code/juniper/design/exports");
  expect(exported.rows.map((r: { file: string }) => r.file)).toEqual([
    "step-1@2x.png",
    "step-2@2x.png",
    "step-3.pdf",
  ]);
});

test("copy as PNG and HTML use the selection", async ({ page }) => {
  await page.goto(`/?ledger=c19&${POSITION}`);
  await page.mouse.click(808, 400);
  expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["step2"]);
  await page.keyboard.press("ControlOrMeta+Shift+KeyC");
  await page.keyboard.press("ControlOrMeta+Alt+KeyC");
  await expect.poll(async () => (await state(page)).copiedAs).toEqual(["png:step2", "html:step2"]);
  await expect(page.locator(".vl-toast")).toContainText("Copied as HTML");
});

test("a file change from disk shows a banner that opens its ledger entry", async ({ page }) => {
  await page.goto(`/?ledger=c22&${POSITION}`);
  const banner = page.locator(".vl-banner");
  await expect(banner).toContainText(
    "juniper.vellum changed on disk (git checkout) · 14 changes applied",
  );
  await banner.getByRole("button", { name: "View in ledger" }).click();
  await expect(page.locator("[data-entry=file]")).toHaveAttribute("data-open", "");
});

test("a merge conflict is marked on the canvas and resolved by keeping both", async ({ page }) => {
  await page.goto(`/?ledger=c22-conflict&${POSITION}`);
  await expect(page.locator(".vl-banner")).toContainText("has a merge conflict · 2 versions");
  await expect(page.locator(".vl-conflict__tag")).toHaveText("Conflict · 2 versions");
  const dialog = page.getByRole("dialog", { name: "Resolve Step 2" });
  await expect(
    dialog.getByText("Choose a version, or keep both as separate frames."),
  ).toBeVisible();
  await dialog.getByRole("button", { name: "Ask agent to merge" }).click();
  expect((await state(page)).hostActions).toContainEqual({
    command: "conflict.askAgent",
    detail: { node: "step2" },
  });
  await page.evaluate(() => window.__vellum.ledgerReply(window.__vellum.nodes()));
  await dialog.getByRole("button", { name: "Keep both" }).click();
  const tx = ((await page.evaluate(() => window.__vellum.transactions())) as any[]).at(-1);
  expect(tx.origin.actor.kind).toBe("person");
  expect(tx.ops.map((o: { type: string }) => o.type)).toEqual(["replace", "insert"]);
  await expect(dialog).toHaveCount(0);
});
