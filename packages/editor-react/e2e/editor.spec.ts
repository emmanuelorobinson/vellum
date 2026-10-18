// Browser tests for the editor chrome, canvas interaction, Layers and Assets. TEST DATA throughout.
// The engine is scripted: each test says what the next write is answered with, then checks the transaction
// the UI sent and what it shows after the reply. No document engine is simulated here.
import { expect, type Page, test } from "@playwright/test";
import { copy } from "@vellum/config";

type Tx = { origin: { actor: { kind: string; device: string } }; ops: Record<string, unknown>[] };
const tx = (page: Page) => page.evaluate(() => window.__vellum.transactions() as Tx[]);
const nodes = (page: Page): Promise<object[]> =>
  page.evaluate(() => window.__vellum.nodes() as unknown as object[]);
const canvas = (page: Page) => page.locator(".vc-canvas");

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await expect(canvas(page)).toBeVisible();
});

async function showLayers(page: Page) {
  await canvas(page).click({ position: { x: 700, y: 700 } });
  await page.keyboard.press("Alt+Digit2");
  await expect(page.getByRole("tab", { name: "Layers" })).toHaveAttribute("aria-selected", "true");
}

test.describe("draw", () => {
  test("a rectangle is one Person transaction at the dragged bounds, and the layer appears", async ({
    page,
  }) => {
    await canvas(page).click({ position: { x: 700, y: 650 } });
    const before = await nodes(page);
    await page.evaluate((n) => {
      window.__vellum.reply([
        ...(n as never[]),
        {
          type: "rect",
          id: "n1",
          name: "Rectangle",
          width: 120,
          height: 80,
          position: { type: "absolute", x: 500, y: 500 },
        } as never,
      ]);
    }, before);
    await page.keyboard.press("r");
    expect(await page.evaluate(() => window.__vellum.tool())).toBe("rect");
    await page.mouse.move(500, 500);
    await page.mouse.down();
    await page.mouse.move(560, 540, { steps: 4 });
    await expect(page.getByText("60 × 40")).toHaveCount(0); // sizes are drawn on the overlay canvas, not as DOM text
    await page.mouse.move(620, 580, { steps: 4 });
    await page.mouse.up();
    const sent = await tx(page);
    expect(sent).toHaveLength(1);
    expect(sent[0]?.origin.actor).toEqual({ kind: "person", device: "device-test" });
    expect(sent[0]?.ops[0]).toMatchObject({
      type: "insert",
      node: {
        type: "rect",
        position: { type: "absolute", x: 500, y: 500 },
        width: 120,
        height: 80,
      },
    });
    await page.keyboard.press("Alt+Digit2");
    await expect(page.getByRole("treeitem", { name: "Rectangle" })).toBeVisible();
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["n1"]);
  });
  test("a refused write says so and leaves the tool ready", async ({ page }) => {
    await canvas(page).click({ position: { x: 700, y: 650 } });
    await page.keyboard.press("r");
    await page.mouse.move(500, 500);
    await page.mouse.down();
    await page.mouse.move(620, 580, { steps: 4 });
    await page.mouse.up();
    await expect(page.getByRole("status").filter({ hasText: "could not apply" })).toBeVisible();
    expect(await tx(page)).toHaveLength(1);
    expect(await page.evaluate(() => window.__vellum.tool())).toBe("rect");
  });
  test("a read-only file refuses to draw and says so", async ({ page }) => {
    await page.goto("/?readonly=1");
    await canvas(page).click({ position: { x: 700, y: 650 } });
    await page.keyboard.press("r");
    await page.mouse.move(500, 500);
    await page.mouse.down();
    await expect(page.getByRole("status").filter({ hasText: "read-only" })).toBeVisible();
    await page.mouse.up();
    expect(await tx(page)).toHaveLength(0);
  });
});

test.describe("select", () => {
  test("clicking a frame selects it and its layer row shows selected", async ({ page }) => {
    await showLayers(page);
    await canvas(page).click({ position: { x: 780, y: 200 } });
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["step1"]);
    await expect(page.getByRole("treeitem", { name: "Step 1" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });
  test("clicking a layer row selects it on the canvas, and ⇧-click adds", async ({ page }) => {
    await showLayers(page);
    await page.getByRole("treeitem", { name: "Welcome" }).click();
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["welcome"]);
    await page.getByRole("treeitem", { name: "Step 1" }).click({ modifiers: ["Shift"] });
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["welcome", "step1"]);
  });
  test("selecting a nested layer on the canvas opens the layers above it", async ({ page }) => {
    await showLayers(page);
    await expect(page.getByRole("treeitem", { name: "Habit card" })).toHaveCount(0);
    await canvas(page).click({ position: { x: 960, y: 170 }, modifiers: ["ControlOrMeta"] });
    await expect(page.getByRole("treeitem", { name: "Habit card" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });
  test("Escape clears the selection", async ({ page }) => {
    await canvas(page).click({ position: { x: 780, y: 200 } });
    await page.keyboard.press("Escape");
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual([]);
  });
});

test.describe("layers", () => {
  test("dragging a row below another is one reorder op at the final index", async ({ page }) => {
    await showLayers(page);
    const welcome = page.getByRole("treeitem", { name: "Welcome" });
    const step2 = page.getByRole("treeitem", { name: "Step 2" });
    const from = (await welcome.boundingBox()) as {
      x: number;
      y: number;
      width: number;
      height: number;
    };
    const to = (await step2.boundingBox()) as {
      x: number;
      y: number;
      width: number;
      height: number;
    };
    await page.evaluate(() => {
      const [w, s1, s2] = window.__vellum.nodes();
      window.__vellum.reply([s1, s2, w] as never);
    });
    await page.mouse.move(from.x + 80, from.y + from.height / 2);
    await page.mouse.down();
    await page.mouse.move(to.x + 80, to.y + to.height - 3, { steps: 8 });
    await expect(step2).toHaveAttribute("data-drop", "below");
    await page.mouse.up();
    const sent = await tx(page);
    expect(sent).toHaveLength(1);
    expect(sent[0]?.ops).toEqual([{ type: "reorder", id: "welcome", index: 2 }]);
    const order = await page
      .getByRole("treeitem")
      .evaluateAll((els) => els.map((e) => e.getAttribute("aria-label")));
    expect(order).toEqual(["Step 1", "Step 2", "Welcome"]);
  });
  test("Escape during a drag cancels it", async ({ page }) => {
    await showLayers(page);
    const welcome = (await page.getByRole("treeitem", { name: "Welcome" }).boundingBox()) as {
      x: number;
      y: number;
    };
    const step2 = (await page.getByRole("treeitem", { name: "Step 2" }).boundingBox()) as {
      x: number;
      y: number;
    };
    await page.mouse.move(welcome.x + 80, welcome.y + 14);
    await page.mouse.down();
    await page.mouse.move(step2.x + 80, step2.y + 25, { steps: 6 });
    await page.keyboard.press("Escape");
    await page.mouse.up();
    expect(await tx(page)).toHaveLength(0);
  });
  test("Alt+↓ moves the focused layer down among its siblings without the mouse", async ({
    page,
  }) => {
    await showLayers(page);
    await page.getByRole("treeitem", { name: "Welcome" }).focus();
    await page.keyboard.press("Alt+ArrowDown");
    expect((await tx(page))[0]?.ops).toEqual([{ type: "reorder", id: "welcome", index: 1 }]);
  });
  test("renaming: double-click, type, Enter writes one update", async ({ page }) => {
    await showLayers(page);
    await page.getByRole("treeitem", { name: "Welcome" }).dblclick();
    const input = page.getByRole("textbox", { name: "Rename Welcome" });
    await expect(input).toBeFocused();
    await input.fill("  Hero  ");
    await input.press("Enter");
    expect((await tx(page))[0]?.ops).toEqual([
      { type: "update", id: "welcome", patch: { name: "Hero" } },
    ]);
  });
  test("Escape abandons a rename", async ({ page }) => {
    await showLayers(page);
    await page.getByRole("treeitem", { name: "Welcome" }).dblclick();
    await page.getByRole("textbox", { name: "Rename Welcome" }).fill("Nope");
    await page.keyboard.press("Escape");
    expect(await tx(page)).toHaveLength(0);
    await expect(page.getByRole("treeitem", { name: "Welcome" })).toBeVisible();
  });
  test("the eye and the lock write visibility and lock", async ({ page }) => {
    await showLayers(page);
    const row = page.getByRole("treeitem", { name: "Step 1" });
    await row.hover();
    await row.getByRole("button", { name: "Hide Step 1" }).click();
    await row.getByRole("button", { name: "Lock Step 1" }).click();
    const sent = await tx(page);
    expect(sent.flatMap((t) => t.ops)).toEqual([
      { type: "update", id: "step1", patch: { visible: false } },
      { type: "update", id: "step1", patch: { locked: true } },
    ]);
  });
  test("a marker shows beside an agent's new layer for ten seconds and names the agent", async ({
    page,
  }) => {
    await showLayers(page);
    await page.evaluate(() => window.__vellum.mark("step2", "teal", 2000));
    const row = page.getByRole("treeitem", { name: "Step 2" });
    await expect(row.getByRole("img", { name: /Teal/ })).toBeVisible();
    await expect(page.getByText("Teal marker · created in the last 10 seconds")).toBeVisible();
    await page.evaluate(() => window.__vellum.mark("step2", "teal", 20_000));
    await expect(row.getByRole("img")).toHaveCount(0);
  });
  test("arrow keys walk the tree and → opens a frame", async ({ page }) => {
    await showLayers(page);
    await page.getByRole("treeitem", { name: "Welcome" }).focus();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("treeitem", { name: "Step 2" })).toBeFocused();
    await page.keyboard.press("ArrowRight");
    await expect(page.getByRole("treeitem", { name: "Habit card" })).toBeVisible();
  });
  test("pages switch the page and clear the selection", async ({ page }) => {
    await showLayers(page);
    await page.getByRole("treeitem", { name: "Welcome" }).click();
    await page.getByRole("option", { name: "Settings" }).click();
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual([]);
  });
  test("an empty page shows the next step", async ({ page }) => {
    await page.goto("/?empty=1");
    await canvas(page).click({ position: { x: 700, y: 650 } });
    await page.keyboard.press("Alt+Digit2");
    await expect(page.getByText(copy.layersEmpty)).toBeVisible();
    await page.getByRole("button", { name: "Draw a frame" }).click();
    expect(await page.evaluate(() => window.__vellum.tool())).toBe("frame");
  });
});

test.describe("assets", () => {
  test("a refused component drop says so and still made the request", async ({ page }) => {
    await canvas(page).click({ position: { x: 700, y: 650 } });
    await page.keyboard.press("Alt+Digit3");
    const asset = page.locator(".vc-asset", { hasText: "Button / Primary" });
    const a = (await asset.boundingBox()) as { x: number; y: number };
    await page.mouse.move(a.x + 40, a.y + 8);
    await page.mouse.down();
    await page.mouse.move(700, 600, { steps: 8 });
    await page.mouse.up();
    const sent = await tx(page);
    expect(sent).toHaveLength(1); // unscripted: the engine refuses, but the request was made
    await expect(page.getByRole("status").filter({ hasText: "could not apply" })).toBeVisible();
  });
  test("the instance request carries the component, the drop point and the page", async ({
    page,
  }) => {
    await canvas(page).click({ position: { x: 700, y: 650 } });
    await page.evaluate(() => window.__vellum.reply(window.__vellum.nodes()));
    await page.keyboard.press("Alt+Digit3");
    const a = (await page.locator(".vc-asset", { hasText: "Button / Primary" }).boundingBox()) as {
      x: number;
      y: number;
    };
    await page.mouse.move(a.x + 40, a.y + 8);
    await page.mouse.down();
    await page.mouse.move(700, 600, { steps: 8 });
    await page.mouse.up();
    const sent = await tx(page);
    expect(sent[0]?.ops[0]).toMatchObject({
      type: "insert",
      to: { parent: { type: "page", id: "p1" } },
      node: { type: "instance", componentId: "c1", position: { type: "absolute", x: 700, y: 600 } },
    });
  });
  test("search narrows the list and says when nothing matches", async ({ page }) => {
    await canvas(page).click({ position: { x: 700, y: 650 } });
    await page.keyboard.press("Alt+Digit3");
    await page.getByRole("searchbox", { name: "Search assets" }).fill("phosphor");
    await expect(page.getByText("Lucide · Material Symbols · Phosphor · Feather")).toBeVisible();
    await expect(page.getByText("Button / Primary")).toHaveCount(0);
    await page.getByRole("searchbox", { name: "Search assets" }).fill("zzz");
    await expect(page.getByText("Nothing matches “zzz”")).toBeVisible();
  });
  test("typing in the search field does not trigger canvas shortcuts", async ({ page }) => {
    await canvas(page).click({ position: { x: 700, y: 650 } });
    await page.keyboard.press("Alt+Digit3");
    await page.getByRole("searchbox", { name: "Search assets" }).fill("rrr");
    expect(await page.evaluate(() => window.__vellum.tool())).toBe("move");
  });
});

test.describe("panels", () => {
  test("Tab hides all chrome and Tab again brings it back; ⌘\\ keeps the bars", async ({
    page,
  }) => {
    await canvas(page).click({ position: { x: 700, y: 650 } });
    await expect(page.locator(".vc-panel")).toBeVisible();
    await page.keyboard.press("Tab");
    await expect(page.locator(".vc-panel")).toHaveCount(0);
    await expect(page.locator(".vc-docbar")).toHaveCount(0);
    await expect(page.locator(".vc-rail")).toHaveCount(0);
    await page.keyboard.press("Tab");
    await expect(page.locator(".vc-panel")).toBeVisible();
    await page.keyboard.press("ControlOrMeta+Backslash");
    await expect(page.locator(".vc-panel")).toHaveCount(0);
    await expect(page.locator(".vc-docbar")).toBeVisible();
    await expect(page.locator(".vc-rail")).toBeVisible();
    await page.keyboard.press("ControlOrMeta+Backslash");
    await expect(page.locator(".vc-panel")).toBeVisible();
  });
  test("Tab inside a panel still moves focus, so every control stays reachable by keyboard", async ({
    page,
  }) => {
    await page.getByRole("tab", { name: "Agent" }).focus();
    await page.keyboard.press("Tab");
    await expect(page.locator(".vc-panel")).toBeVisible();
    await expect(page.getByRole("tab", { name: "Agent" })).not.toBeFocused();
  });
  test("⌥1–⌥4 switch tabs", async ({ page }) => {
    await canvas(page).click({ position: { x: 700, y: 650 } });
    for (const [key, name] of [
      ["Digit2", "Layers"],
      ["Digit3", "Assets"],
      ["Digit4", "History"],
      ["Digit1", "Agent"],
    ] as const) {
      await page.keyboard.press(`Alt+${key}`);
      await expect(page.getByRole("tab", { name })).toHaveAttribute("aria-selected", "true");
    }
  });
  test("tabs move with the arrow keys", async ({ page }) => {
    await page.getByRole("tab", { name: "Agent" }).focus();
    await page.keyboard.press("ArrowRight");
    await expect(page.getByRole("tab", { name: "Layers" })).toBeFocused();
    await expect(page.getByRole("tab", { name: "Layers" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });
  test("the tool rail selects a tool and the shapes menu holds the others", async ({ page }) => {
    await page.getByRole("button", { name: "Frame" }).click();
    expect(await page.evaluate(() => window.__vellum.tool())).toBe("frame");
    await page.getByRole("button", { name: "Shapes" }).click();
    await page.getByRole("menuitem", { name: /Ellipse/ }).click();
    expect(await page.evaluate(() => window.__vellum.tool())).toBe("ellipse");
    await expect(page.getByRole("button", { name: "Shapes" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });
  test("the zoom pill steps and shows the percentage", async ({ page }) => {
    await expect(page.getByRole("button", { name: /^Zoom 100%/ })).toBeVisible();
    await page.getByRole("button", { name: /^Zoom/ }).click();
    await page.getByRole("menuitem", { name: /Zoom out/ }).click();
    await expect(page.getByRole("button", { name: /^Zoom 50%/ })).toBeVisible();
  });
  test("the main menu lists the registered commands and disables the ones with no handler", async ({
    page,
  }) => {
    await page.getByRole("button", { name: "Main menu" }).click();
    await expect(page.getByRole("menuitem", { name: "Undo your changes" })).toBeEnabled();
    await expect(page.getByRole("menuitem", { name: "Save through host" })).toHaveAttribute(
      "data-disabled",
      "",
    );
  });
});

test.describe("undo and redo are yours", () => {
  test("⌘Z asks the engine for the person's own undo and the agent's layer stays", async ({
    page,
  }) => {
    await canvas(page).click({ position: { x: 700, y: 650 } });
    await page.evaluate(() => window.__vellum.mark("step2", "teal", 2000));
    const kept = await nodes(page);
    await page.evaluate((n) => window.__vellum.reply(n as never, "undo"), kept);
    await page.keyboard.press("ControlOrMeta+KeyZ");
    expect(await page.evaluate(() => window.__vellum.historyActors())).toEqual([
      { kind: "person", device: "device-test" },
    ]);
    await page.keyboard.press("Alt+Digit2");
    await expect(page.getByRole("treeitem", { name: "Step 2" })).toBeVisible();
  });
  test("⇧⌘Z redoes as the person", async ({ page }) => {
    await canvas(page).click({ position: { x: 700, y: 650 } });
    await page.evaluate(() => window.__vellum.reply(window.__vellum.nodes(), "redo"));
    await page.keyboard.press("ControlOrMeta+Shift+KeyZ");
    expect(await page.evaluate(() => window.__vellum.historyActors())).toEqual([
      { kind: "person", device: "device-test" },
    ]);
  });
});

test.describe("responsive rules", () => {
  const box = async (page: Page, sel: string) =>
    (await page.locator(sel).first().boundingBox()) as {
      x: number;
      y: number;
      width: number;
      height: number;
    };
  for (const [width, panel] of [
    [1280, 360],
    [1040, 360],
    [1000, 320],
    [960, 320],
  ] as const)
    test(`${width}px: the agent panel is ${panel}px and the rail sits against it`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height: 800 });
      await expect(page.locator(".vc-panel")).toHaveCSS("width", `${panel}px`);
      expect((await box(page, ".vc-rail")).x).toBe(8 + panel + 8);
      expect((await box(page, ".vc-docbar")).width).toBe(panel);
    });
  test("760px: the panel is a 44px strip and its icons open it over the canvas", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 760, height: 640 });
    await expect(page.locator(".vc-panel--strip")).toBeVisible();
    expect((await box(page, ".vc-panel")).width).toBe(44);
    await page.getByRole("button", { name: "Layers" }).click();
    await expect(page.locator(".vc-panel[data-mode='overlay']")).toBeVisible();
    expect((await box(page, ".vc-panel")).width).toBe(320);
    await page.keyboard.press("Escape");
    await expect(page.locator(".vc-panel--strip")).toBeVisible();
  });
  test("the right panel appears only with a selection, docked from 1200 and over the canvas below", async ({
    page,
  }) => {
    await expect(page.locator("aside.vc-right")).toHaveCount(0);
    await canvas(page).click({ position: { x: 780, y: 200 } });
    const right = page.locator("aside.vc-right");
    await expect(right).toHaveAttribute("data-mode", "docked");
    expect(await right.boundingBox()).toMatchObject({ x: 1280 - 8 - 280, width: 280 });
    await page.keyboard.press("Escape");
    await expect(right).toHaveCount(0);
    await page.setViewportSize({ width: 1100, height: 800 });
    await canvas(page).click({ position: { x: 780, y: 200 } });
    await expect(page.locator("aside.vc-right")).toHaveAttribute("data-mode", "overlay");
  });
});

test.describe("canvas navigation", () => {
  test("⌘-wheel zooms about the pointer and a plain wheel pans", async ({ page }) => {
    await canvas(page).hover({ position: { x: 600, y: 400 } });
    await page.keyboard.down("ControlOrMeta");
    await page.mouse.wheel(0, -100);
    await page.keyboard.up("ControlOrMeta");
    const zoom = await page.evaluate(() => (window.__vellum.viewport() as { zoom: number }).zoom);
    expect(zoom).toBeGreaterThan(1);
    const before = (await page.evaluate(() => window.__vellum.viewport())) as {
      x: number;
      y: number;
    };
    await page.mouse.wheel(0, 120);
    const after = (await page.evaluate(() => window.__vellum.viewport())) as {
      x: number;
      y: number;
    };
    expect(after.y).toBeLessThan(before.y);
  });
  test("holding Space pans with the hand and releasing restores the tool", async ({ page }) => {
    await canvas(page).click({ position: { x: 700, y: 650 } });
    await page.keyboard.press("r");
    await page.keyboard.down("Space");
    expect(await page.evaluate(() => window.__vellum.tool())).toBe("hand");
    await page.keyboard.up("Space");
    expect(await page.evaluate(() => window.__vellum.tool())).toBe("rect");
  });
});
