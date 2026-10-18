// Browser tests for the properties panel: the real workspace over a scripted engine. TEST DATA throughout.
// The engine answers what each test scripts, then the test checks the Person transaction the panel sent and what
// the panel shows afterwards. No document engine is simulated here.
import { expect, type Page, test } from "@playwright/test";
import { COLOURS } from "../fixtures/design-scenarios";

// biome-ignore lint/suspicious/noExplicitAny: the tests read nested patches by path
type Tx = { origin: { actor: { kind: string; device: string } }; ops: Record<string, any>[] };
const tx = (page: Page) => page.evaluate(() => window.__vellum.transactions() as Tx[]);
const open = async (page: Page, scenario: string, more = "") => {
  await page.goto(`/?design=${scenario}&zoom=0.5&tab=agent${more}`);
  await expect(page.locator(".vc-design")).toBeVisible();
};
/** Lets the next write succeed, as the engine would. */
const answer = (page: Page) =>
  page.evaluate(() => window.__vellum.replyDocument(window.__vellum.document()));
/** The engine now holds the document with this change, and the panel shows it. */
const publish = (page: Page, change: string) =>
  page.evaluate((body) => {
    const doc = window.__vellum.document();
    new Function("doc", body)(doc);
    window.__vellum.publish(doc);
  }, change);
const panel = (page: Page) => page.locator(".vc-right");
const field = (page: Page, label: string) => panel(page).getByLabel(label, { exact: true });

test.describe("value fields", () => {
  test("a sum is worked out and written as one Person transaction", async ({ page }) => {
    await open(page, "c13");
    await answer(page);
    const gap = field(page, "Gap");
    await gap.fill("16*2");
    await gap.press("Enter");
    const sent = await tx(page);
    expect(sent).toHaveLength(1);
    expect(sent[0]?.origin.actor).toEqual({ kind: "person", device: "device-test" });
    expect(sent[0]?.ops).toEqual([{ type: "update", id: "step2", patch: { gap: 32 } }]);
  });
  test("Escape puts the old value back and writes nothing", async ({ page }) => {
    await open(page, "c13");
    const x = field(page, "X");
    await x.fill("999");
    await x.press("Escape");
    await expect(x).toHaveValue("320");
    expect(await tx(page)).toHaveLength(0);
  });
  test("something that is not a number reverts, with a brief message beside the field", async ({
    page,
  }) => {
    await open(page, "c13");
    const x = field(page, "X");
    await x.fill("abc");
    await x.press("Enter");
    await expect(panel(page).getByRole("alert")).toHaveText("Enter a number or a sum such as 16*2");
    await expect(x).toHaveValue("320");
    expect(await tx(page)).toHaveLength(0);
    await expect(panel(page).getByRole("alert")).toHaveCount(0, { timeout: 5000 });
  });
  test("arrow keys step by one, ten with Shift", async ({ page }) => {
    await open(page, "c13");
    await answer(page);
    const x = field(page, "X");
    await x.focus();
    await x.press("ArrowUp");
    await answer(page);
    await x.press("Shift+ArrowUp");
    const sent = await tx(page);
    expect(sent.map((t) => t.ops[0]?.patch.position.x)).toEqual([321, 330]);
  });
  test("dragging the label scrubs the value and writes once on release", async ({ page }) => {
    await open(page, "c13");
    await answer(page);
    const label = panel(page).locator(".vc-field__axis", { hasText: /^X$/ });
    const box = (await label.boundingBox()) as {
      x: number;
      y: number;
      width: number;
      height: number;
    };
    await page.mouse.move(box.x + 4, box.y + 6);
    await page.mouse.down();
    await page.mouse.move(box.x + 24, box.y + 6, { steps: 5 });
    expect(await tx(page)).toHaveLength(0);
    await page.mouse.up();
    const sent = await tx(page);
    expect(sent).toHaveLength(1);
    expect(sent[0]?.ops[0]?.patch.position).toEqual({ type: "absolute", x: 340, y: 120 });
  });
  test("a value beyond its range is kept inside it, and says so", async ({ page }) => {
    await open(page, "c13");
    await answer(page);
    const opacity = field(page, "Opacity");
    await opacity.fill("-20");
    await opacity.press("Enter");
    await expect(panel(page).getByRole("alert")).toHaveText("Kept within the allowed range");
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({ opacity: 0 });
  });
  test("a selection that disagrees reads Mixed, and typing sets every layer", async ({ page }) => {
    await open(page, "c13", "&select=step2,card");
    const opacity = field(page, "Opacity");
    await expect(opacity).toHaveAttribute("placeholder", "Mixed");
    await expect(opacity).toHaveValue("");
    await answer(page);
    await opacity.fill("30");
    await opacity.press("Enter");
    const sent = await tx(page);
    expect(sent).toHaveLength(1);
    expect(sent[0]?.ops).toEqual([
      { type: "update", id: "step2", patch: { opacity: 0.3 } },
      { type: "update", id: "card", patch: { opacity: 0.3 } },
    ]);
  });
  test("is disabled in a read-only file, with the reason", async ({ page }) => {
    await open(page, "c13", "&readonly=1");
    await expect(field(page, "X")).toBeDisabled();
    await expect(field(page, "Gap")).toBeDisabled();
    await expect(page.getByRole("radio", { name: "Row" })).toBeDisabled();
    await expect(panel(page).locator('[title="This file is read-only"]').first()).toBeVisible();
  });
});

test.describe("variables", () => {
  test("a bound value shows its chip, and Detach writes the number it has now", async ({
    page,
  }) => {
    await open(page, "c13");
    await expect(panel(page).getByRole("button", { name: "Variable space/4" })).toContainText(
      "{space/4} 16",
    );
    await answer(page);
    await panel(page).getByRole("button", { name: "Variable space/4" }).click();
    await page.getByRole("menuitem", { name: "Detach variable" }).click();
    expect((await tx(page))[0]?.ops).toEqual([{ type: "update", id: "step2", patch: { gap: 16 } }]);
  });
  test("a bound value can be switched to another variable of the same type", async ({ page }) => {
    await open(page, "c13");
    await answer(page);
    await panel(page).getByRole("button", { name: "Variable space/4" }).click();
    await page.getByRole("menuitem", { name: /space\/8/ }).click();
    expect((await tx(page))[0]?.ops).toEqual([
      { type: "update", id: "step2", patch: { gap: { $var: "v4" } } },
    ]);
  });
  test("typing over a bound value writes the number and leaves the variable", async ({ page }) => {
    await open(page, "c13");
    await answer(page);
    const gap = field(page, "Gap");
    await gap.fill("20");
    await gap.press("Enter");
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({ gap: 20 });
  });
});

test.describe("layout", () => {
  test("Row, Column, Free and Grid write the layout, with Free as none", async ({ page }) => {
    await open(page, "c13");
    await answer(page);
    await page.getByRole("radio", { name: "Row" }).click();
    await answer(page);
    await page.getByRole("radio", { name: "Free" }).click();
    expect((await tx(page)).map((t) => t.ops[0]?.patch)).toEqual([
      { layout: "row" },
      { layout: null },
    ]);
  });
  test("unlinking padding keeps the value on every side", async ({ page }) => {
    await open(page, "c13");
    await answer(page);
    await panel(page).getByRole("button", { name: "Padding linked" }).click();
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({ padding: [24, 24, 24, 24] });
  });
  test("unlinked padding is four fields, and each side is written alone", async ({ page }) => {
    await open(page, "c13");
    await publish(page, "doc.pages[0].children[0].padding = [8, 16, 8, 16];");
    await expect(field(page, "Padding start")).toHaveValue("16");
    await expect(field(page, "Padding top")).toHaveValue("8");
    await answer(page);
    await field(page, "Padding top").fill("4");
    await field(page, "Padding top").press("Enter");
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({ padding: [4, 16, 8, 16] });
  });
  test("padding written as two values is read as four sides", async ({ page }) => {
    await open(page, "c13");
    await publish(page, "doc.pages[0].children[0].padding = [8, 16];");
    await expect(field(page, "Padding end")).toHaveValue("16");
    await expect(field(page, "Padding bottom")).toHaveValue("8");
  });
  test("linking again takes the top value for all four sides", async ({ page }) => {
    await open(page, "c13");
    await publish(page, "doc.pages[0].children[0].padding = [8, 16, 8, 16];");
    await answer(page);
    await panel(page).getByRole("button", { name: "Padding unlinked" }).click();
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({ padding: 8 });
  });
  test("Hug, Fill and Fixed set both sizes; Fixed takes the size the layer has", async ({
    page,
  }) => {
    await open(page, "c13");
    await answer(page);
    await page.getByRole("radio", { name: "Fixed" }).click();
    const sent = await tx(page);
    expect(sent).toHaveLength(1);
    expect(sent[0]?.ops).toEqual([
      { type: "update", id: "step2", patch: { width: 240, height: 440 } },
    ]);
  });
  test("a width typed is fixed, and the sizing menu sets one axis", async ({ page }) => {
    await open(page, "c13");
    await answer(page);
    await field(page, "Width").fill("300");
    await field(page, "Width").press("Enter");
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({ width: 300 });
    await answer(page);
    await panel(page).getByRole("button", { name: "Height sizing" }).click({ force: true });
    await page.getByRole("menuitem", { name: "Fill container" }).click();
    expect((await tx(page)).at(-1)?.ops[0]?.patch).toEqual({ height: "fill" });
  });
  test("alignment icons write the cross-axis alignment", async ({ page }) => {
    await open(page, "c13");
    await answer(page);
    await panel(page).getByRole("button", { name: "Align centre" }).click();
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({ align: "center" });
  });
});

test.describe("appearance", () => {
  test("clip content toggles from the heading", async ({ page }) => {
    await open(page, "c13");
    await answer(page);
    await panel(page).getByRole("button", { name: "Clip content" }).click({ force: true });
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({ clip: true });
  });
  test("Notes for agents opens, takes text, and writes it when the field is left", async ({
    page,
  }) => {
    await open(page, "c13");
    await panel(page)
      .getByRole("button", { name: /Notes for agents/ })
      .click();
    await answer(page);
    const notes = panel(page).getByLabel("Notes for agents", { exact: true }).last();
    await notes.fill("Copy under 10 words.");
    await notes.blur();
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({ agentNotes: "Copy under 10 words." });
  });
});

test.describe("fills and strokes", () => {
  test("the hex of a fill is edited in place, keeping its opacity", async ({ page }) => {
    await open(page, "c13");
    await answer(page);
    const hex = field(page, "Surface hex");
    await hex.fill(COLOURS.typed);
    await hex.press("Enter");
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({
      fills: [{ type: "solid", color: COLOURS.typed }],
    });
  });
  test("a hex that is not a colour reverts with a message", async ({ page }) => {
    await open(page, "c13");
    const hex = field(page, "Surface hex");
    await hex.fill("nope");
    await hex.press("Enter");
    await expect(hex).toHaveValue(COLOURS.surface);
    expect(await tx(page)).toHaveLength(0);
  });
  test("the swatch opens the fill editor on the Solid tab with Color and Variables", async ({
    page,
  }) => {
    await open(page, "c13");
    await panel(page).getByRole("button", { name: "Edit surface" }).click();
    const editor = page.getByTestId("paint-editor");
    await expect(editor.getByRole("heading")).toHaveText("Fill · Solid");
    await expect(editor.getByRole("tab", { name: "Solid" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await editor.getByRole("radio", { name: "Variables" }).click();
    await answer(page);
    await editor.getByRole("button", { name: "color/primary" }).click();
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({
      fills: [{ type: "solid", color: { $var: "v3" } }],
    });
  });
  test("the Gradient tab turns the colour into a gradient from it to clear", async ({ page }) => {
    await open(page, "c13");
    await panel(page).getByRole("button", { name: "Edit surface" }).click();
    await answer(page);
    await page.getByTestId("paint-editor").getByRole("tab", { name: "Gradient" }).click();
    const fills = (await tx(page))[0]?.ops[0]?.patch.fills;
    expect(fills[0]).toMatchObject({ type: "gradient", kind: "linear" });
    // The colour was bound to a variable, so both stops stay bound to it.
    expect(fills[0].stops.map((st: { color: unknown }) => st.color)).toEqual([
      { $var: "v2" },
      { $var: "v2" },
    ]);
  });
  test("Add fill appends the default colour, and a fill can be removed and hidden", async ({
    page,
  }) => {
    await open(page, "c13");
    await answer(page);
    await panel(page).getByRole("button", { name: "Add fill" }).click({ force: true });
    expect((await tx(page))[0]?.ops[0]?.patch.fills).toHaveLength(2);
    await answer(page);
    await panel(page).getByRole("button", { name: "Hide paint" }).click({ force: true });
    expect((await tx(page)).at(-1)?.ops[0]?.patch.fills[0]).toMatchObject({ visible: false });
    await answer(page);
    await panel(page).getByRole("button", { name: "Remove paint" }).click({ force: true });
    expect((await tx(page)).at(-1)?.ops[0]?.patch).toEqual({ fills: null });
  });
  test("a stroke is added with the default colour and per-side widths in the sides popover", async ({
    page,
  }) => {
    await open(page, "c13");
    await answer(page);
    await panel(page).getByRole("button", { name: "Add stroke" }).click({ force: true });
    expect((await tx(page))[0]?.ops[0]?.patch.stroke).toMatchObject({ width: 1 });
  });
  test("stroke align, caps, joins and dashes are written; bad dashes revert", async ({ page }) => {
    await open(page, "c33e");
    await answer(page);
    await field(page, "Dashes").fill("6 3");
    await field(page, "Dashes").press("Enter");
    expect((await tx(page))[0]?.ops[0]?.patch.stroke.dash).toEqual([6, 3]);
    await field(page, "Dashes").fill("x");
    await field(page, "Dashes").press("Enter");
    await expect(panel(page).getByRole("alert")).toHaveText("Enter lengths such as 4, 4");
    await answer(page);
    await field(page, "Stroke alignment").click();
    await page.getByRole("option", { name: "Outside" }).click();
    expect((await tx(page)).at(-1)?.ops[0]?.patch.stroke.align).toBe("outside");
  });
  test("per-side widths: one side changes and the others stay", async ({ page }) => {
    await open(page, "c33e");
    await panel(page).getByRole("button", { name: "Stroke sides" }).click();
    await answer(page);
    await page.getByLabel("Top width", { exact: true }).fill("3");
    await page.getByLabel("Top width", { exact: true }).press("Enter");
    expect((await tx(page))[0]?.ops[0]?.patch.stroke.width).toEqual([3, 1, 2, 1]);
  });
});

test.describe("effects", () => {
  test("the add menu offers the effects, with the shader marked v1.1 and unavailable", async ({
    page,
  }) => {
    await open(page, "c33e");
    await panel(page).getByRole("button", { name: "Add effect", exact: true }).last().click();
    await expect(page.getByRole("menuitem", { name: "Drop shadow" })).toBeEnabled();
    await expect(page.getByRole("menuitem", { name: /Shader/ })).toBeDisabled();
    await answer(page);
    await page.getByRole("menuitem", { name: "Layer blur" }).click();
    const effects = (await tx(page))[0]?.ops[0]?.patch.effects;
    expect(effects.at(-1)).toEqual({ type: "layer-blur", radius: 8 });
  });
  test("an effect's numbers are edited in its popover, and its eye hides it", async ({ page }) => {
    await open(page, "c33e");
    await panel(page).getByRole("button", { name: "Edit drop shadow" }).click();
    await answer(page);
    await page.getByLabel("Shadow blur", { exact: true }).fill("24");
    await page.getByLabel("Shadow blur", { exact: true }).press("Enter");
    expect((await tx(page))[0]?.ops[0]?.patch.effects[0]).toMatchObject({
      type: "drop-shadow",
      blur: 24,
    });
  });
});

test.describe("fill editor and mesh gradients", () => {
  test("opens on the Gradient tab for a mesh, with its type, grid and point", async ({ page }) => {
    await open(page, "c33");
    await panel(page).getByRole("button", { name: "Edit fill" }).click();
    const editor = page.getByTestId("paint-editor");
    await expect(editor.getByRole("heading")).toHaveText("Fill · Mesh gradient");
    await expect(editor.getByRole("tab", { name: "Gradient" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(editor.getByLabel("Grid size")).toHaveValue("2 × 2");
    await expect(editor.getByLabel("Top left colour")).toHaveValue(COLOURS.primary);
  });
  test("dragging a point on the canvas moves it, one transaction on release", async ({ page }) => {
    await open(page, "c33", "&pan=280,100");
    await panel(page).getByRole("button", { name: "Edit fill" }).click();
    await answer(page);
    // Step 2 is at (320, 120), 240 × 440, at half zoom and panned (280, 100): its top left corner is at (440, 160),
    // clear of the agent panel and tool rail on the left and of the fill editor on the right.
    await page.mouse.move(440, 160);
    await page.mouse.down();
    await page.mouse.move(500, 270, { steps: 6 });
    expect(await tx(page)).toHaveLength(0);
    await page.mouse.up();
    const sent = await tx(page);
    expect(sent).toHaveLength(1);
    expect(sent[0]?.ops[0]?.patch.fills[0].points[0]).toMatchObject({
      x: 0.5,
      y: 0.5,
      color: COLOURS.primary,
    });
    expect(sent[0]?.ops[0]?.patch.fills[0].points[1]).toMatchObject({ x: 1, y: 0 });
  });
  test("Escape while dragging puts the point back and writes nothing", async ({ page }) => {
    await open(page, "c33", "&pan=280,100");
    await panel(page).getByRole("button", { name: "Edit fill" }).click();
    await page.mouse.move(440, 160);
    await page.mouse.down();
    await page.mouse.move(500, 270, { steps: 4 });
    await page.keyboard.press("Escape");
    await page.mouse.up();
    expect(await tx(page)).toHaveLength(0);
  });
  test("selecting a point changes the colour the Point field edits", async ({ page }) => {
    await open(page, "c33");
    await panel(page).getByRole("button", { name: "Edit fill" }).click();
    const editor = page.getByTestId("paint-editor");
    await editor.getByRole("button", { name: "Bottom right point" }).click();
    await expect(editor.getByLabel("Bottom right colour")).toHaveValue(COLOURS.bottomRight);
    await answer(page);
    await editor.getByLabel("Bottom right colour").fill(COLOURS.white);
    await editor.getByLabel("Bottom right colour").press("Enter");
    expect((await tx(page))[0]?.ops[0]?.patch.fills[0].points[3].color).toBe(COLOURS.white);
  });
  test("a bigger grid is written with the colours resampled", async ({ page }) => {
    await open(page, "c33");
    await panel(page).getByRole("button", { name: "Edit fill" }).click();
    await answer(page);
    const grid = page.getByTestId("paint-editor").getByLabel("Grid size");
    await grid.fill("3 × 3");
    await grid.press("Enter");
    const fill = (await tx(page))[0]?.ops[0]?.patch.fills[0];
    expect([fill.columns, fill.rows, fill.points.length]).toEqual([3, 3, 9]);
  });
  test("a grid size that is not a size reverts with a message", async ({ page }) => {
    await open(page, "c33");
    await panel(page).getByRole("button", { name: "Edit fill" }).click();
    const grid = page.getByTestId("paint-editor").getByLabel("Grid size");
    await grid.fill("big");
    await grid.press("Enter");
    await expect(page.getByTestId("paint-editor").getByRole("alert")).toContainText("3 × 3");
    expect(await tx(page)).toHaveLength(0);
  });
  test("the type menu turns the mesh into a radial gradient, keeping its colours", async ({
    page,
  }) => {
    await open(page, "c33");
    await panel(page).getByRole("button", { name: "Edit fill" }).click();
    await answer(page);
    await page.getByTestId("paint-editor").getByLabel("Gradient type").click();
    await page.getByRole("option", { name: "Radial" }).click();
    expect((await tx(page))[0]?.ops[0]?.patch.fills[0]).toMatchObject({
      type: "gradient",
      kind: "radial",
    });
  });
  test("an image's fit is set, and its corners are cropped on the canvas", async ({ page }) => {
    await open(page, "c33i", "&pan=280,100");
    await panel(page).getByRole("button", { name: "Edit fill" }).click();
    const editor = page.getByTestId("paint-editor");
    await expect(editor.getByRole("tab", { name: "Image" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await answer(page);
    await editor.getByRole("radio", { name: "Stretch" }).click();
    expect((await tx(page))[0]?.ops[0]?.patch.fills[0]).toMatchObject({
      type: "image",
      fit: "stretch",
    });
    await answer(page);
    await editor.getByRole("button", { name: "Crop on canvas" }).click();
    // Step 2 is at (440, 160), 120 × 220 on screen: drag the bottom right corner from (560, 380) to (620, 490).
    await page.mouse.move(560, 380);
    await page.mouse.down();
    await page.mouse.move(620, 490, { steps: 5 });
    expect(await tx(page)).toHaveLength(1);
    await page.mouse.up();
    const sent = await tx(page);
    expect(sent).toHaveLength(2);
    expect(sent[1]?.ops[0]?.patch.fills[0].crop).toEqual([1.5, 0, 0, 1.5, 0, 0]);
  });
  test("the Shader tab says it arrives in v1.1", async ({ page }) => {
    await open(page, "c33");
    await panel(page).getByRole("button", { name: "Edit fill" }).click();
    await page.getByTestId("paint-editor").getByRole("tab", { name: "Shader" }).click();
    await expect(page.getByTestId("paint-editor")).toContainText("Shader fills arrive in v1.1.");
  });
  test("the Image tab asks the host to choose a file; the panel has no file access itself", async ({
    page,
  }) => {
    await open(page, "c33");
    await panel(page).getByRole("button", { name: "Edit fill" }).click();
    await page.getByTestId("paint-editor").getByRole("tab", { name: "Image" }).click();
    await page.getByRole("button", { name: "Choose image…" }).click();
    expect(await page.evaluate(() => window.__vellum.hostActions())).toEqual([
      { command: "image.choose", detail: { scope: "fill", index: 0 } },
    ]);
  });
  test("Done closes the editor and the canvas drops the points", async ({ page }) => {
    await open(page, "c33", "&pan=280,100");
    await panel(page).getByRole("button", { name: "Edit fill" }).click();
    await page.getByTestId("paint-editor").getByRole("button", { name: "Done" }).click();
    await expect(page.getByTestId("paint-editor")).toHaveCount(0);
    await page.mouse.move(440, 160);
    await page.mouse.down();
    await page.mouse.move(500, 270, { steps: 4 });
    await page.mouse.up();
    // With the editor closed the press is the canvas's: it moves the layer, never a mesh point.
    const sent = await tx(page);
    expect(JSON.stringify(sent)).not.toContain('"points"');
  });
});

test.describe("text", () => {
  test("the Text section shows the missing font in amber and the font in use", async ({ page }) => {
    await open(page, "c14");
    await expect(panel(page).getByRole("status")).toHaveText("Font missing · using Inter");
    await expect(page.getByLabel("Font", { exact: true })).toContainText("Inter");
  });
  test("size changes keep the line height in proportion", async ({ page }) => {
    await open(page, "c14");
    await answer(page);
    await field(page, "Size").fill("48");
    await field(page, "Size").press("Enter");
    const run = (await tx(page))[0]?.ops[0]?.patch.content[0];
    expect(run).toMatchObject({ size: 48, lineHeight: 57.6 });
  });
  test("line height is a percentage, and the word auto puts the font's own back", async ({
    page,
  }) => {
    await open(page, "c14");
    await answer(page);
    await field(page, "Line height").fill("150");
    await field(page, "Line height").press("Enter");
    expect((await tx(page))[0]?.ops[0]?.patch.content[0].lineHeight).toBe(36);
    await answer(page);
    await field(page, "Line height").fill("auto");
    await field(page, "Line height").press("Enter");
    const written = (await tx(page)).at(-1)?.ops[0]?.patch.content[0] ?? {};
    expect("lineHeight" in written).toBe(false);
  });
  test("alignment and resizing are segmented", async ({ page }) => {
    await open(page, "c14");
    await answer(page);
    await page.getByRole("radio", { name: "Center" }).click();
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({ align: "center" });
    await answer(page);
    await page.getByRole("radio", { name: "Auto height" }).click();
    expect((await tx(page)).at(-1)?.ops[0]?.patch).toEqual({ resize: "auto-height" });
  });
  test("a text style is applied to the runs and remembered on the layer", async ({ page }) => {
    await open(page, "c14");
    await answer(page);
    await field(page, "Text style").click();
    await page.getByRole("option", { name: "None" }).click();
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({ textStyle: null });
  });
  test("double-clicking text opens it for typing: the panel shows Text, Fill and Content", async ({
    page,
  }) => {
    await open(page, "c14", "&pan=280,100");
    await page.mouse.dblclick(444, 204);
    await expect(page.locator(".vc-textedit")).toBeFocused();
    expect(
      await panel(page)
        .locator(".vc-section")
        .evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.section)),
    ).toEqual(["text", "fill", "content"]);
  });
  test("typing on the canvas is a preview; leaving the field writes one change", async ({
    page,
  }) => {
    await open(page, "c14", "&edit=1");
    await expect(page.locator(".vc-textedit")).toBeFocused();
    await page.keyboard.type("Grow");
    expect(await tx(page)).toHaveLength(0);
    await answer(page);
    await page.locator(".vc-textedit").press("Meta+Enter");
    const sent = await tx(page);
    expect(sent).toHaveLength(1);
    expect(sent[0]?.ops[0]).toMatchObject({
      type: "update",
      id: "title",
      patch: { content: [{ text: "Grow", font: "Fraunces" }] },
    });
  });
  test("Escape puts the old text back and writes nothing", async ({ page }) => {
    await open(page, "c14", "&edit=1");
    await page.keyboard.type("Grow");
    await page.keyboard.press("Escape");
    await expect(page.locator(".vc-textedit")).toHaveCount(0);
    expect(await tx(page)).toHaveLength(0);
  });
  test("the Content section shows what is typed on the canvas, and typing there is typing on the canvas", async ({
    page,
  }) => {
    await open(page, "c14", "&edit=1&pan=280,100");
    await page.keyboard.type("Grow");
    await expect(panel(page).getByLabel("Text content")).toHaveValue("Grow");
    await panel(page).getByLabel("Text content").fill("Plant something");
    await expect(page.locator(".vc-textedit")).toHaveValue("Plant something");
    expect(await tx(page)).toHaveLength(0);
  });
  test("moving into the panel keeps the text open, and a change there restyles the draft: one change at the end", async ({
    page,
  }) => {
    await open(page, "c14", "&edit=1&pan=280,100");
    await page.keyboard.type("Grow");
    await field(page, "Size").fill("32");
    await field(page, "Size").press("Enter");
    expect(await tx(page)).toHaveLength(0);
    await expect(page.locator(".vc-textedit")).toHaveCount(1);
    await answer(page);
    // A press on the canvas ends the text.
    await page.mouse.click(600, 500);
    const sent = await tx(page);
    expect(sent).toHaveLength(1);
    expect(sent[0]?.ops[0]?.patch.content[0]).toMatchObject({
      text: "Grow",
      size: 32,
      lineHeight: 38.4,
    });
  });
  test("alignment is the layer's own, so it is written at once even while typing", async ({
    page,
  }) => {
    await open(page, "c14", "&edit=1&pan=280,100");
    await answer(page);
    await page.getByRole("radio", { name: "Center" }).click();
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({ align: "center" });
    await expect(page.locator(".vc-textedit")).toHaveCount(1);
  });
  test("Content belongs to the typing view: it is not shown otherwise", async ({ page }) => {
    await open(page, "c14", "&pan=280,100");
    await expect(panel(page).getByLabel("Text content")).toHaveCount(0);
  });
});

test.describe("components", () => {
  test("a main component shows its name and prop defaults; changes reach every instance", async ({
    page,
  }) => {
    await open(page, "c16");
    await expect(field(page, "Component name")).toHaveValue("Button / Juniper");
    await expect(panel(page)).toContainText("Changes update every instance.");
    await answer(page);
    await field(page, "Label").fill("Next");
    await field(page, "Label").press("Enter");
    const op = (await tx(page))[0]?.ops[0];
    expect(op).toMatchObject({ type: "updateComponent", id: "c1", name: "Button / Juniper" });
    expect(op?.props.Label).toEqual({ type: "text", default: "Next" });
  });
  test("renaming a component writes the name and keeps its props and variants", async ({
    page,
  }) => {
    await open(page, "c16");
    await answer(page);
    await field(page, "Component name").fill("Button / Juniper 2");
    await field(page, "Component name").press("Enter");
    const op = (await tx(page))[0]?.ops[0];
    expect(op).toMatchObject({ type: "updateComponent", name: "Button / Juniper 2" });
    expect(op?.variants).toHaveLength(9);
  });
  test("an instance lists what it changed, each with a Reset", async ({ page }) => {
    await open(page, "c16i");
    for (const reset of ["Reset variant", "Reset size", "Reset label"])
      await expect(panel(page).getByRole("button", { name: reset })).toBeVisible();
  });
  test("choosing a variant keeps the other choices; Reset removes one", async ({ page }) => {
    await open(page, "c16i");
    await answer(page);
    await field(page, "Variant").click();
    await page.getByRole("option", { name: "Primary" }).click();
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({
      variant: { Variant: "Primary", Size: "Medium" },
    });
    await answer(page);
    await panel(page).getByRole("button", { name: "Reset variant" }).click();
    expect((await tx(page)).at(-1)?.ops[0]?.patch).toEqual({ variant: { Size: "Medium" } });
  });
  test("a prop is set and reset: Reset label leaves the other prop alone", async ({ page }) => {
    await open(page, "c16i");
    await answer(page);
    await panel(page).getByRole("button", { name: "Reset label" }).click();
    expect((await tx(page))[0]?.ops[0]?.patch).toEqual({ props: { "Show icon": false } });
    await answer(page);
    await panel(page).getByRole("switch", { name: "Show icon" }).click();
    expect((await tx(page)).at(-1)?.ops[0]?.patch.props).toMatchObject({ "Show icon": true });
  });
  test("Detach writes the core's Detach for the instance", async ({ page }) => {
    await open(page, "c16i");
    await answer(page);
    await panel(page).getByRole("button", { name: "Detach", exact: true }).click();
    expect((await tx(page))[0]?.ops).toEqual([{ type: "detach", id: "cont" }]);
  });
  test("Detach has a keyboard shortcut", async ({ page }) => {
    await open(page, "c16i");
    await answer(page);
    await page.locator(".vc-canvas").focus();
    await page.keyboard.press("Alt+Meta+x");
    await page.keyboard.press("Alt+Control+x");
    expect((await tx(page)).some((t) => t.ops[0]?.type === "detach")).toBe(true);
  });
  test("Replace inside instance lists the file's components and points the instance at the one chosen", async ({
    page,
  }) => {
    await open(page, "c16i");
    await answer(page);
    await panel(page).getByRole("button", { name: "Replace inside instance" }).click();
    await page.getByRole("menuitem", { name: "Card" }).click();
    expect((await tx(page))[0]?.ops[0]).toEqual({
      type: "update",
      id: "cont",
      patch: {
        componentId: "c2",
        library: null,
        variant: null,
        props: null,
        overrides: null,
        slotContent: null,
      },
    });
  });
  test("the Replace shortcut opens the same picker", async ({ page }) => {
    await open(page, "c16i");
    await page.locator(".vc-canvas").focus();
    await page.keyboard.press("Alt+Shift+KeyR");
    await expect(page.getByRole("menuitem", { name: "Card" })).toBeVisible();
  });
  test("Go to main component asks the host to show it, when it is not on the canvas", async ({
    page,
  }) => {
    await open(page, "c16i");
    await panel(page).getByRole("button", { name: "Go to main component" }).click();
    expect(
      (await page.evaluate(() => window.__vellum.hostActions())).map((a) => a.command),
    ).toEqual(["component.goToMain"]);
  });
  test("an instance of a component that is gone says so", async ({ page }) => {
    await open(page, "c16i");
    await publish(page, 'doc.pages[0].children[0].componentId = "gone";');
    await expect(panel(page).getByRole("status")).toHaveText(
      "This component is missing from the file",
    );
  });
  test("an instance from a library says Open library, and asks the host", async ({ page }) => {
    await open(page, "c16i");
    await publish(
      page,
      'doc.pages[0].children[0].library = "Core"; doc.pages[0].children[0].componentId = "k9";',
    );
    await panel(page).getByRole("button", { name: "Open library" }).click();
    expect(await page.evaluate(() => window.__vellum.hostActions())).toEqual([
      { command: "component.goToMain", detail: { componentId: "k9", library: "Core" } },
    ]);
  });
});

test.describe("export", () => {
  test("Add export asks the host: the panel does not export", async ({ page }) => {
    await open(page, "c13");
    await panel(page).getByRole("button", { name: "Add export" }).click();
    expect(
      (await page.evaluate(() => window.__vellum.hostActions())).map((a) => a.command),
    ).toEqual(["export.add"]);
  });
});

test.describe("the right panel", () => {
  test("has Design, Inspect and Activity; the last two are the host's", async ({ page }) => {
    await open(page, "c13");
    await expect(page.getByRole("tab", { name: "Design" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await page.getByRole("tab", { name: "Inspect" }).click();
    await expect(panel(page)).toContainText("Inspect is not available yet.");
    await page.getByRole("tab", { name: "Design" }).click();
    await expect(page.locator(".vc-design")).toBeVisible();
  });
  test("appears only when something is selected", async ({ page }) => {
    await page.goto("/?design=c13&zoom=0.5&tab=agent&select=nothing");
    await expect(page.locator(".vc-right")).toHaveCount(0);
  });
  test("menus and popovers follow the appearance of the workspace", async ({ page }) => {
    await page.goto("/?design=c13&zoom=0.5&tab=agent&appearance=dark");
    await panel(page).getByRole("button", { name: "Edit surface" }).click();
    // The popover paints with the workspace's own raised colour in the dark appearance, not the page's light default.
    const colours = await page.evaluate(() => {
      const workspace = document.querySelector(".vc-workspace") as HTMLElement;
      const probe = document.createElement("div");
      probe.style.background = "var(--chrome-raised)";
      workspace.append(probe);
      const inWorkspace = getComputedStyle(probe).backgroundColor;
      probe.remove();
      const popup = getComputedStyle(
        document.querySelector('[data-testid="paint-editor"]') as HTMLElement,
      ).backgroundColor;
      const onPage = document.createElement("div");
      onPage.style.background = "var(--chrome-raised)";
      document.body.append(onPage);
      const light = getComputedStyle(onPage).backgroundColor;
      onPage.remove();
      return { inWorkspace, popup, light };
    });
    expect(colours.popup).toBe(colours.inWorkspace);
    expect(colours.popup).not.toBe(colours.light);
  });
});
