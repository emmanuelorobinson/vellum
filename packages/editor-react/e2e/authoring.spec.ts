// Chrome lane. TEST DATA: the real workspace, with scripted engine replies and authorized host ports.
import { expect, test } from "@playwright/test";

test("script errors appear on the node and in the panel with line and excerpt", async ({
  page,
}) => {
  await page.goto("/?authoring=c28-error");
  await expect(page.locator(".vx-script-node")).toContainText("values.map is not a function");
  await expect(page.locator("[data-section=script]")).toContainText("values.map is not a function");
  await expect(page.locator("[data-section=script] pre")).toContainText("18  values.map(drawBar)");
  await page.getByRole("button", { name: "Ask agent to fix" }).click();
  expect((await page.evaluate(() => window.__vellum.authoringRequests())).at(-1)).toMatchObject({
    type: "askScript",
    address: "script",
    intent: "fix",
  });
});
test("script inputs preserve variable bindings; conversion is one person Op", async ({ page }) => {
  await page.goto("/?authoring=c28");
  await expect(page.getByLabel("Bind Bar color")).toHaveValue("primary");
  await page.evaluate(() => window.__vellum.replyDocument(window.__vellum.document()));
  await page.getByLabel("Gap", { exact: true }).fill("24");
  await page.getByLabel("Gap", { exact: true }).press("Enter");
  expect((await page.evaluate(() => window.__vellum.transactions())).at(-1)).toMatchObject({
    origin: { actor: { kind: "person" } },
    ops: [
      { type: "update", id: "script", patch: { inputs: { gap: 24, bar: { $var: "primary" } } } },
    ],
  });
  await page.evaluate(() => window.__vellum.replyDocument(window.__vellum.document()));
  await page.getByRole("button", { name: "Convert to layers", exact: true }).click();
  expect((await page.evaluate(() => window.__vellum.transactions())).at(-1)).toMatchObject({
    origin: { actor: { kind: "person" } },
    ops: [{ type: "convertScript", id: "script" }],
  });
});
test("empty script offers file choice and ask; choosing a file uses an Op", async ({ page }) => {
  await page.goto("/?authoring=c28-empty");
  await expect(page.getByText("Inputs appear here after you choose a script file.")).toBeVisible();
  await page.getByRole("button", { name: "Ask the agent", exact: true }).click();
  expect((await page.evaluate(() => window.__vellum.authoringRequests())).at(-1)).toMatchObject({
    type: "askScript",
    intent: "write",
  });
  await page.evaluate(() => window.__vellum.replyDocument(window.__vellum.document()));
  await page.getByRole("button", { name: "Choose a script file", exact: true }).click();
  await expect
    .poll(() => page.evaluate(() => window.__vellum.transactions()))
    .toMatchObject([
      {
        ops: [
          { type: "update", id: "script", patch: { src: "design/charts/weekly.js", inputs: {} } },
        ],
      },
    ]);
});
test("slot suggestions insert an instance and retain both existing rows", async ({ page }) => {
  await page.goto("/?authoring=c29-filled");
  await page.evaluate(() => window.__vellum.replyDocument(window.__vellum.document()));
  await page.getByRole("button", { name: "Insert HabitRow", exact: true }).click();
  const tx = (await page.evaluate(() => window.__vellum.transactions())).at(-1);
  expect(tx).toMatchObject({
    origin: { actor: { kind: "person" } },
    ops: [
      {
        type: "update",
        id: "instance",
        patch: {
          slotContent: {
            slot: [
              { id: "habit1" },
              { id: "habit2" },
              { type: "instance", componentId: "habit", id: "n1" },
            ],
          },
        },
      },
    ],
  });
});
test("component slot marker has a label and can add a suggestion", async ({ page }) => {
  await page.goto("/?authoring=c29");
  await expect(page.locator(".vx-slot-marker")).toContainText("Slot · Content");
  await page.getByRole("button", { name: "Add suggestion", exact: true }).click();
  await page.evaluate(() => window.__vellum.replyDocument(window.__vellum.document()));
  await page.getByLabel("Suggested component").selectOption("card");
  expect((await page.evaluate(() => window.__vellum.transactions())).at(-1)).toMatchObject({
    ops: [{ type: "makeSlot", id: "slot", suggested: ["habit", "stat", "card"] }],
  });
});
test("Guidelines opens from Assets, saves with a version and stays editable", async ({ page }) => {
  await page.goto("/?authoring=c12&tab=assets");
  await page.getByRole("button", { name: "Guidelines", exact: true }).click();
  await expect(page.getByLabel("Design-system guidelines")).toHaveValue(/# Starter Minimal/);
  await page.getByLabel("Design-system guidelines").fill("# Rules\nKeep copy short.");
  await page.getByRole("button", { name: "Save guidelines", exact: true }).click();
  await expect
    .poll(() => page.evaluate(() => window.__vellum.authoringRequests()))
    .toContainEqual({
      type: "saveGuidelines",
      path: "design-systems/starter/guidelines.md",
      text: "# Rules\nKeep copy short.",
      version: "1",
    });
  expect(await page.evaluate(() => window.__vellum.transactions())).toEqual([]);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeHidden();
});
test("notes update a frame through a person transaction", async ({ page }) => {
  await page.goto("/?authoring=c12");
  const field = page.getByRole("textbox", { name: "Notes for agents", exact: true });
  await field.fill("Use the existing illustration.");
  await page.evaluate(() => window.__vellum.replyDocument(window.__vellum.document()));
  await field.blur();
  expect((await page.evaluate(() => window.__vellum.transactions())).at(-1)).toMatchObject({
    origin: { actor: { kind: "person" } },
    ops: [{ type: "update", id: "frame", patch: { agentNotes: "Use the existing illustration." } }],
  });
});
test("Upload imports an asset; Generate remains disabled for v1.1", async ({ page }) => {
  await page.goto("/?authoring=c30-upload");
  await expect(page.getByRole("tab", { name: "Generate · v1.1" })).toBeDisabled();
  await page.evaluate(() => window.__vellum.replyDocument(window.__vellum.document()));
  await page.getByRole("button", { name: "Choose a file", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
  expect((await page.evaluate(() => window.__vellum.transactions())).at(-1)).toMatchObject({
    ops: [
      {
        type: "update",
        id: "photo",
        patch: { fills: [{ type: "image", asset: "b".repeat(64), fit: "fill" }] },
      },
    ],
  });
});
test("Stock uses the selected photo, stores attribution and shows real source links", async ({
  page,
}) => {
  await page.goto("/?authoring=c30");
  await page.getByRole("tab", { name: "Stock", exact: true }).click();
  await page.getByLabel("Search stock images").fill("plants");
  await page.getByLabel("Search stock images").press("Enter");
  await expect(page.getByRole("link", { name: "Pixabay", exact: true }).first()).toHaveAttribute(
    "href",
    "https://pixabay.com/photos/example/",
  );
  await page.getByRole("button", { name: "Select foliage", exact: true }).click();
  await page.evaluate(() => window.__vellum.replyDocument(window.__vellum.document()));
  await page.getByRole("button", { name: "Use selected photo", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
  expect((await page.evaluate(() => window.__vellum.transactions())).at(-1)).toMatchObject({
    ops: [
      {
        type: "update",
        id: "photo",
        patch: {
          fills: [{ type: "image", asset: "d".repeat(64) }],
          metadata: { attribution: { provider: "Pixabay", author: "Example photographer" } },
        },
      },
    ],
  });
});
test("read-only script, slot, guidelines and image actions cannot write", async ({ page }) => {
  for (const [name, button] of [
    ["c28", "Convert to layers"],
    ["c29-filled", "Insert HabitRow"],
    ["c30-upload", "Choose a file"],
  ]) {
    await page.goto(`/?authoring=${name}&readonly=1`);
    await expect(page.getByRole("button", { name: button!, exact: true })).toBeDisabled();
  }
  await page.goto("/?authoring=c12-guidelines&readonly=1");
  await expect(page.getByLabel("Design-system guidelines")).toHaveAttribute("readonly", "");
});

test("the paint editor opens the built-in image picker and file drop uses AssetStore", async ({
  page,
}) => {
  await page.goto("/?authoring=c30-upload");
  await page.getByRole("button", { name: "Close dialog", exact: true }).click();
  await page.getByRole("button", { name: "Edit fill", exact: true }).click();
  await page.getByRole("button", { name: "Replace…", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Image fill" })).toBeVisible();
  await page.evaluate(() => window.__vellum.replyDocument(window.__vellum.document()));
  await page.locator(".vx-upload").evaluate((el) => {
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(new File(["TEST DATA"], "picture.png", { type: "image/png" }));
    el.dispatchEvent(new DragEvent("drop", { bubbles: true, cancelable: true, dataTransfer }));
  });
  await expect(page.getByRole("dialog")).toBeHidden();
  expect((await page.evaluate(() => window.__vellum.transactions())).at(-1)).toMatchObject({
    ops: [{ type: "update", id: "photo", patch: { fills: [{ asset: "c".repeat(64) }] } }],
  });
});

test("header controls cover booleans, enums, multiline strings and references", async ({
  page,
}) => {
  await page.goto("/?authoring=c28-inputs");
  await expect(page.getByLabel("Show legend", { exact: true })).not.toBeChecked();
  await page.evaluate(() => {
    const d = window.__vellum.document();
    const n = d.pages[0]?.children?.[0];
    if (n?.type === "script") n.inputs = { ...n.inputs, showLegend: true };
    window.__vellum.replyDocument(d);
  });
  await page.getByLabel("Show legend", { exact: true }).check();
  expect((await page.evaluate(() => window.__vellum.transactions())).at(-1)).toMatchObject({
    ops: [{ patch: { inputs: { showLegend: true } } }],
  });
  await page.evaluate(() => window.__vellum.replyDocument(window.__vellum.document()));
  await page.getByLabel("Mode", { exact: true }).selectOption("lines");
  expect((await page.evaluate(() => window.__vellum.transactions())).at(-1)).toMatchObject({
    ops: [{ patch: { inputs: { mode: "lines" } } }],
  });
  await expect(page.getByLabel("Label", { exact: true })).toHaveJSProperty("tagName", "TEXTAREA");
  await expect(page.getByLabel("Source", { exact: true })).toHaveValue("script");
  await expect(page.locator("datalist option").first()).toHaveAttribute("value", "script");
});

test("live diagnostics update the panel and canvas at the same document revision", async ({
  page,
}) => {
  await page.goto("/?authoring=c28");
  await page.evaluate(() =>
    window.__vellum.authoringEmit({
      status: "error",
      inputs: [],
      message: "New runtime failure",
      line: 9,
      excerpt: "9 throw Error()",
    }),
  );
  await expect(page.locator(".vx-script-node")).toContainText("New runtime failure");
  await expect(page.locator("[data-section=script]")).toContainText("New runtime failure");
});
