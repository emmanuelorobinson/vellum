// Chrome lane: requires Playwright Chromium. Real workspace; core replies are scripted, never a JS document engine.
import { expect, test } from "@playwright/test";
import type { Document } from "@vellum/types-ts";

const open = async (page: import("@playwright/test").Page, resource: string) => {
  await page.goto(`/?resource=${resource}&tab=assets`);
  await expect(page.locator(".vc-canvas")).toBeVisible();
};
const reply = async (page: import("@playwright/test").Page, change: (d: Document) => Document) => {
  const doc = await page.evaluate(() => window.__vellum.document());
  await page.evaluate(
    (text) => window.__vellum.replyDocument(JSON.parse(text) as Document),
    JSON.stringify(change(doc)),
  );
};
const ops = async (page: import("@playwright/test").Page) =>
  page.evaluate(() => window.__vellum.transactions());
test("two axes group every combination and cell edits preserve fallback entries", async ({
  page,
}) => {
  await open(page, "themes");
  for (const name of ["Light / Phone", "Dark / Phone", "Light / Tablet", "Dark / Tablet"])
    await expect(page.getByRole("columnheader", { name, exact: true })).toBeVisible();
  await expect(page.getByLabel("space/4 · Light / Tablet")).toHaveValue("24");
  await reply(page, (d) => d);
  await page.getByLabel("space/4 · Dark / Tablet").fill("32");
  await page.getByLabel("space/4 · Dark / Tablet").press("Enter");
  expect(await ops(page)).toMatchObject([
    {
      ops: [
        {
          type: "setVariables",
          set: {
            space: {
              values: [
                { value: 16 },
                { value: 24, when: { Device: "Tablet" } },
                { value: 32, when: { Mode: "Dark", Device: "Tablet" } },
              ],
            },
          },
        },
      ],
    },
  ]);
});
test("frame themes change only that frame and reset to inheritance", async ({ page }) => {
  await open(page, "frame");
  await expect(page.getByLabel("Theme Mode", { exact: true })).toHaveText(/Dark/);
  await page.getByLabel("Theme Device", { exact: true }).click();
  await reply(page, (d) => d);
  await page.getByRole("option", { name: "Tablet", exact: true }).click();
  expect(await ops(page)).toMatchObject([
    { ops: [{ type: "update", id: "card", patch: { theme: { Mode: "Dark", Device: "Tablet" } } }] },
  ]);
});
test("missing library locate changes its path then save offers the host save", async ({ page }) => {
  await open(page, "missingLibrary");
  await expect(page.getByText("⚠ Missing · using cached copy")).toBeVisible();
  await reply(page, (d) => ({
    ...d,
    imports: { ...d.imports, core: "../libraries/located.lib.vellum" },
  }));
  await page.getByRole("button", { name: "Locate…" }).click();
  await expect(page.getByRole("button", { name: "Save document" })).toBeVisible();
  expect(await ops(page)).toMatchObject([
    {
      ops: [{ type: "setShell", shell: { imports: { core: "../libraries/located.lib.vellum" } } }],
    },
  ]);
  await page.getByRole("button", { name: "Save document" }).click();
  await expect(page.getByRole("button", { name: "Save document" })).toHaveCount(0);
  expect(await page.evaluate(() => window.__vellum.hostActions())).toContainEqual({
    command: "document.save",
  });
});
test("library conversion asks before a write and cancel writes nothing", async ({ page }) => {
  await open(page, "libraries");
  await page.getByRole("button", { name: "Turn this file into a library", exact: true }).click();
  expect(await ops(page)).toHaveLength(0);
  await expect(page.getByText("This action cannot be undone.", { exact: false })).toBeVisible();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  expect(await ops(page)).toHaveLength(0);
  await page.getByRole("button", { name: "Turn this file into a library", exact: true }).click();
  await reply(page, (d) => ({ ...d, meta: { ...d.meta, kind: "library" } }));
  await page.getByRole("button", { name: "Turn into library", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await ops(page)).toMatchObject([
    { ops: [{ type: "setShell", shell: { meta: { kind: "library" } } }] },
  ]);
});
test("font search filters the sections and import preserves shell data", async ({ page }) => {
  await open(page, "fonts");
  await page.getByLabel("Search fonts").fill("Noto");
  await expect(page.getByRole("button", { name: "Noto Sans", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Helvetica Neue", exact: true })).toHaveCount(0);
  await reply(page, (d) => d);
  await page.getByRole("button", { name: "Add font…", exact: true }).click();
  expect(await ops(page)).toMatchObject([
    {
      ops: [
        { type: "setShell", shell: { fonts: { ["b".repeat(64)]: { family: "Imported Font" } } } },
      ],
    },
  ]);
});
test("paste decisions go to the injected core planner, not the document", async ({ page }) => {
  await open(page, "conflicts");
  await expect(page.getByRole("button", { name: "Paste", exact: true })).toBeDisabled();
  await page.getByRole("button", { name: "Use existing", exact: true }).first().click();
  await page.getByRole("button", { name: "Add renamed", exact: true }).nth(1).click();
  await reply(page, (d) => d);
  await page.getByRole("button", { name: "Paste", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await page.evaluate(() => window.__vellum.hostActions())).toContainEqual({
    command: "paste.plan",
    detail: { link: false, variables: { primary: "existing", space: "renamed" } },
  });
});
test("scale shows result sizes and sends Scale for the core to resolve bound sizes and overrides", async ({
  page,
}) => {
  await open(page, "scale");
  await expect(page.getByText("416 px", { exact: true })).toBeVisible();
  await expect(page.getByText("960 px", { exact: true })).toBeVisible();
  await page.getByRole("radio", { name: "Anchor 2, 2" }).click();
  await reply(page, (d) => d);
  await page.getByLabel("Scale factor").fill("150%");
  await page.getByLabel("Scale factor").press("Enter");
  expect(await ops(page)).toMatchObject([
    { ops: [{ type: "scale", id: "card", factor: 1.5, anchor: { x: 1, y: 1 } }] },
  ]);
  expect(await page.evaluate(() => window.__vellum.tool())).toBe("move");
});
test("vector inspector selects a mapped anchor, edits coordinates and finishes", async ({
  page,
}) => {
  await open(page, "vector");
  await page.mouse.click(770, 410);
  await expect(page.getByLabel("Point X", { exact: true })).toHaveValue("250");
  await reply(page, (d) => d);
  await page.getByLabel("Point X", { exact: true }).fill("180");
  await page.getByLabel("Point X", { exact: true }).press("Enter");
  expect(JSON.stringify(await ops(page))).toContain("180 120");
  await page.getByRole("button", { name: "Done · Enter" }).click();
  expect(await page.evaluate(() => window.__vellum.tool())).toBe("move");
});
test("read-only dialogs disable file mutations and Escape restores the workspace", async ({
  page,
}) => {
  await page.goto("/?resource=libraries&readonly");
  await expect(page.getByRole("button", { name: "Import library…" })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Turn this file into a library" })).toBeDisabled();
  await page.getByRole("dialog").press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await ops(page)).toHaveLength(0);
});

test("a plain size can bind to a variable through the sizing menu", async ({ page }) => {
  await open(page, "frame");
  await page.getByRole("button", { name: "Width sizing", exact: true }).click();
  await reply(page, (d) => d);
  await page.getByRole("menuitem", { name: "space/4", exact: true }).click();
  expect(await ops(page)).toMatchObject([
    { ops: [{ type: "update", id: "card", patch: { width: { $var: "space" } } }] },
  ]);
});
test("invalid scale text never applies the previous valid factor", async ({ page }) => {
  await open(page, "scale");
  await page.getByLabel("Scale factor").fill("invalid");
  await page.getByRole("button", { name: "Apply scale", exact: true }).click();
  await expect(
    page.getByText("Enter a positive factor or percentage", { exact: true }),
  ).toBeVisible();
  expect(await ops(page)).toHaveLength(0);
});
test("editing an alias by name writes its stable id", async ({ page }) => {
  await open(page, "variables");
  await reply(page, (d) => d);
  await page.getByLabel("color/button · Dark", { exact: true }).fill("$color/primary");
  await page.getByLabel("color/button · Dark", { exact: true }).press("Enter");
  expect(await ops(page)).toMatchObject([
    {
      ops: [
        {
          type: "setVariables",
          set: {
            button: {
              values: [{ $alias: "primary" }, { $alias: "primary", when: { Mode: "Dark" } }],
            },
          },
        },
      ],
    },
  ]);
});

test("a bound width is left for the core Scale operation to convert", async ({ page }) => {
  await open(page, "frame");
  const d = await page.evaluate(() => window.__vellum.document());
  const node = d.pages[0]?.children?.[0];
  if (!node) throw Error("Missing TEST DATA");
  node.width = { $var: "space" };
  node.metadata = { testBounds: { width: 208, height: 480 } };
  await page.evaluate(
    (text) => window.__vellum.publish(JSON.parse(text) as Document),
    JSON.stringify(d),
  );
  await page.locator(".vc-canvas").focus();
  await page.keyboard.press("k");
  await page.getByLabel("Scale factor").fill("200%");
  await reply(page, (d) => d);
  await page.getByLabel("Scale factor").press("Enter");
  expect(await ops(page)).toMatchObject([{ ops: [{ type: "scale", id: "card", factor: 2 }] }]);
});
test("scaling a component instance sends Scale and never edits its definition", async ({
  page,
}) => {
  await open(page, "frame");
  const d = await page.evaluate(() => window.__vellum.document());
  d.components = {
    c: { name: "Test", root: { type: "frame", id: "root", name: "Main", width: 208, height: 480 } },
  };
  const firstPage = d.pages[0];
  if (!firstPage) throw Error("Missing TEST DATA page");
  firstPage.children = [
    { type: "instance", id: "card", name: "Instance", componentId: "c", width: 208, height: 480 },
  ];
  await page.evaluate(
    (text) => window.__vellum.publish(JSON.parse(text) as Document),
    JSON.stringify(d),
  );
  await page.locator(".vc-canvas").focus();
  await page.keyboard.press("k");
  await reply(page, (d) => d);
  await page.getByLabel("Scale factor").fill("200%");
  await page.getByLabel("Scale factor").press("Enter");
  expect(await ops(page)).toMatchObject([{ ops: [{ type: "scale", id: "card", factor: 2 }] }]);
  expect(await page.evaluate(() => window.__vellum.document().components?.c?.root)).toEqual(
    d.components.c?.root,
  );
});

test("an engine-supplied segment hit lets Alt-click add a point without changing its silhouette", async ({
  page,
}) => {
  await open(page, "vector");
  await reply(page, (d) => d);
  await page.keyboard.down("Alt");
  await page.mouse.click(615, 355);
  await page.keyboard.up("Alt");
  const transactions = await ops(page);
  expect(transactions).toHaveLength(1);
  expect(transactions).toMatchObject([{ ops: [{ type: "update", id: "vector" }] }]);
  expect(JSON.stringify(transactions)).toContain("95 65");
});

test("P and V switch pen and point selection while a path is open", async ({ page }) => {
  await open(page, "vector");
  await page.locator(".vc-canvas").focus();
  await page.keyboard.press("p");
  expect(await page.evaluate(() => window.__vellum.tool())).toBe("pen");
  await page.keyboard.press("v");
  expect(await page.evaluate(() => window.__vellum.tool())).toBe("path");
  await expect(page.getByRole("button", { name: "Done · Enter" })).toBeVisible();
});
