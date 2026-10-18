// Chrome lane. TEST DATA: the Juniper document, a scripted engine and a fake daemon. Palette (C23), context menu
// (C24), Editor settings (C25) and the edge states S01–S16, then the global state matrix.

import { expect, type Page, test } from "@playwright/test";
import { copy } from "@vellum/config";
import { EDGE_BACKDROP } from "../fixtures/shell-scenarios";

const BASE = "agent=c11&shell=1";
const actions = (page: Page) =>
  page.evaluate(() => window.__vellum.hostActions().map((a) => a.command));
const ready = async (page: Page) => {
  await page.waitForFunction(() => !!window.__vellum);
  await page.evaluate(() => window.__vellum.agentReady());
};

test.describe("command palette (C23)", () => {
  test("⌘/ opens it; Ask the agent is always first and the highlight starts on the first match", async ({
    page,
  }) => {
    await page.goto(`/?${BASE}`);
    await ready(page);
    await page.locator(".vc-canvas").click({ position: { x: 700, y: 650 } });
    await page.keyboard.press("ControlOrMeta+Slash");
    const dialog = page.getByRole("dialog", { name: "Command palette" });
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText(
      "Search actions, layers, pages, components, variables and sessions.",
    );
    await page.getByRole("combobox").fill("pause");
    const options = dialog.getByRole("option");
    await expect(options.first()).toHaveText(/Ask the agent: pause/);
    await expect(options.nth(1)).toContainText("Pause all agents");
    await expect(options.nth(1)).toHaveAttribute("aria-selected", "true");
    await expect(dialog).toContainText("opens this palette · Enter to run · Esc to close");
    await page.keyboard.press("Enter");
    await expect(dialog).toHaveCount(0);
    expect(await actions(page)).toContain("agents.pauseAll");
  });

  test("a search can become a prompt: Ask puts the words in the composer and sends nothing", async ({
    page,
  }) => {
    await page.goto(`/?${BASE}`);
    await ready(page);
    await page.locator(".vc-canvas").click({ position: { x: 700, y: 650 } });
    await page.keyboard.press("ControlOrMeta+Slash");
    await page.getByRole("combobox").fill("make the cards calmer");
    await page.getByRole("option").first().click();
    await expect(page.getByLabel("Request")).toHaveValue("make the cards calmer");
    expect(await page.evaluate(() => window.__vellum.composerSent())).toHaveLength(0);
  });

  test("finds layers, pages, components, variables and sessions, and opens them", async ({
    page,
  }) => {
    await page.goto(`/?${BASE}`);
    await ready(page);
    await page.locator(".vc-canvas").click({ position: { x: 700, y: 650 } });
    await page.keyboard.press("ControlOrMeta+Slash");
    await page.getByRole("combobox").fill("step 2");
    await page
      .getByRole("option", { name: /Step 2/ })
      .first()
      .click();
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["step2"]);
    await page.keyboard.press("ControlOrMeta+Slash");
    await page.getByRole("combobox").fill("settings");
    await expect(page.getByRole("option", { name: /Settings/ })).toContainText("Page");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "Command palette" })).toHaveCount(0);
  });

  test("arrow keys move the highlight and Escape runs nothing", async ({ page }) => {
    await page.goto(`/?${BASE}`);
    await ready(page);
    await page.locator(".vc-canvas").click({ position: { x: 700, y: 650 } });
    await page.keyboard.press("ControlOrMeta+Slash");
    await page.getByRole("combobox").fill("a");
    const before = await page.evaluate(
      () => (window.__vellum.palette() as { active: number }).active,
    );
    await page.keyboard.press("ArrowDown");
    const after = await page.evaluate(
      () => (window.__vellum.palette() as { active: number }).active,
    );
    expect(after).not.toBe(before);
    await page.keyboard.press("Escape");
    expect(await actions(page)).toEqual([]);
  });
});

test("⇧⌘E opens the Export dialog and the palette lists Export selection with that chord", async ({
  page,
}) => {
  await page.goto("/?ledger=c10&shell=1&agent=c11&select=step2");
  await ready(page);
  await page.locator(".vc-canvas").click({ position: { x: 700, y: 650 } });
  await page.keyboard.press("ControlOrMeta+Slash");
  await page.getByRole("combobox").fill("export");
  const option = page.getByRole("option", { name: /Export selection/ });
  await expect(option).toContainText(/⇧⌘E|Ctrl\+Shift\+E/);
  await page.keyboard.press("Escape");
  await page.locator(".vc-canvas").click({ button: "right", position: { x: 950, y: 350 } });
  await page.keyboard.press("Escape");
  await page.locator(".vc-canvas").focus();
  await page.keyboard.press("ControlOrMeta+Shift+KeyE");
  await expect(page.getByRole("dialog")).toContainText("Export");
});

test.describe("layer context menu (C24)", () => {
  const open = async (page: Page) => {
    await page.goto(`/?${BASE}&select=step2`);
    await ready(page);
    await page.locator(".vc-canvas").click({ button: "right", position: { x: 950, y: 350 } });
    return page.getByRole("menu", { name: "Layer actions" });
  };
  test("Ask about this… and Redesign this… come first, Delete last", async ({ page }) => {
    const menu = await open(page);
    await expect(menu).toBeVisible();
    const labels = await menu.getByRole("menuitem").allTextContents();
    expect(labels.map((l) => l.replace(/[⌘⌥⇧⌃⌫].*$/, "").trim())[0]).toBe("Ask about this…");
    expect(labels[1]).toMatch(/^Redesign this…/);
    expect(labels.at(-1)).toMatch(/^Delete/);
    await expect(menu.getByRole("menuitem", { name: /^Delete/ })).toHaveCSS("color", /.+/);
  });
  test("a row runs its command through the controller and closes the menu", async ({ page }) => {
    const menu = await open(page);
    await menu.getByRole("menuitem", { name: /Add auto-layout/ }).click();
    await expect(menu).toHaveCount(0);
    expect(await actions(page)).toContain("edit.autoLayout");
  });
  test("right-clicking a layer that is not selected selects it first", async ({ page }) => {
    await page.goto(`/?${BASE}`);
    await ready(page);
    await page.locator(".vc-canvas").click({ button: "right", position: { x: 950, y: 350 } });
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["step2"]);
    await expect(page.getByRole("menu", { name: "Layer actions" })).toBeVisible();
  });
  test("right-clicking empty canvas shows no menu; Escape closes it", async ({ page }) => {
    await page.goto(`/?${BASE}`);
    await ready(page);
    await page.locator(".vc-canvas").click({ button: "right", position: { x: 700, y: 650 } });
    await expect(page.getByRole("menu", { name: "Layer actions" })).toHaveCount(0);
    await page.locator(".vc-canvas").click({ button: "right", position: { x: 950, y: 350 } });
    await page.keyboard.press("Escape");
    await expect(page.getByRole("menu", { name: "Layer actions" })).toHaveCount(0);
  });
  test("Ask about this… takes the selection to the composer; Redesign this… starts the request", async ({
    page,
  }) => {
    const menu = await open(page);
    await menu.getByRole("menuitem", { name: /Redesign this…/ }).click();
    await expect(page.getByLabel("Request")).toHaveValue("Redesign ");
    await expect(page.locator(".vk-chip").first()).toContainText("Step 2");
  });
  test("works from the Layers panel too", async ({ page }) => {
    await page.goto(`/?${BASE}&tab=layers`);
    await ready(page);
    await page.getByRole("treeitem", { name: "Step 1" }).click({ button: "right" });
    await expect(page.getByRole("menu", { name: "Layer actions" })).toBeVisible();
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["step1"]);
  });
  test("a read-only file refuses every change but still copies and asks", async ({ page }) => {
    await page.goto(`/?${BASE}&readonly=1&select=step2`);
    await ready(page);
    await page.locator(".vc-canvas").click({ button: "right", position: { x: 950, y: 350 } });
    const menu = page.getByRole("menu", { name: "Layer actions" });
    await expect(menu.getByRole("menuitem", { name: /^Delete/ })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await expect(menu.getByRole("menuitem", { name: /^Copy$|^Copy ⌘/ })).not.toHaveAttribute(
      "aria-disabled",
      "true",
    );
  });
});

test.describe("editor settings (C25)", () => {
  const open = async (page: Page) => {
    await page.goto(`/?${BASE}`);
    await ready(page);
    await page.getByRole("button", { name: "Editor settings" }).click();
    return page.getByRole("dialog", { name: "Editor settings" });
  };
  test("has every section DESIGN.md lists, and Telemetry is off with its exact disclosure", async ({
    page,
  }) => {
    const dialog = await open(page);
    for (const name of [
      "Appearance",
      "Agents",
      "Permissions",
      "Skills",
      "Assets",
      "Canvas",
      "Renderer",
      "Language",
      "Telemetry",
      "Keyboard shortcuts",
    ])
      await expect(dialog.getByRole("region", { name })).toBeVisible();
    const telemetry = dialog.getByRole("region", { name: "Telemetry" });
    await expect(telemetry.getByRole("switch")).toHaveAttribute("aria-checked", "false");
    await expect(telemetry).toContainText("Off by default");
    await expect(telemetry).toContainText(
      "If on: version, OS, renderer failures, anonymous feature counts.",
    );
    await expect(telemetry).toContainText("Never files, prompts, keys or repo paths.");
    await expect(dialog.getByRole("region", { name: "Assets" })).toContainText(copy.keyStorage);
    expect(await page.evaluate(() => window.__vellum.preferencesSaved())).toHaveLength(0);
  });
  test("a changed value is saved once, validated, and a bad one is refused beside the field", async ({
    page,
  }) => {
    const dialog = await open(page);
    const big = dialog.getByRole("textbox", { name: "Big nudge", exact: true });
    await big.fill("20");
    await big.blur();
    await expect(big).toHaveValue("20");
    const saved = await page.evaluate(() => window.__vellum.preferencesSaved());
    expect((saved.at(-1) as Record<string, unknown>)["canvas.nudgeBig"]).toBe(20);
    await big.fill("0");
    await big.blur();
    await expect(dialog.getByRole("alert")).toContainText("Enter a whole number from 1 to 100");
    await expect(big).toHaveValue("20");
  });
  test("Skip all prompts goes through the feed, and the composer then warns", async ({ page }) => {
    const dialog = await open(page);
    await dialog.getByRole("switch", { name: "Skip all prompts" }).click();
    expect(await page.evaluate(() => window.__vellum.agentRequests())).toContainEqual({
      type: "setSkipAll",
      on: true,
    });
    await dialog.getByRole("button", { name: "Done" }).click();
    await expect(page.locator(".vk-skipall")).toBeVisible();
  });
  test("a shortcut can be rebound; a taken one says who has it; the host's stay with the host", async ({
    page,
  }) => {
    const dialog = await open(page);
    await dialog.getByLabel("Search shortcuts").fill("Stop current session");
    const row = dialog.getByRole("button", { name: "Change shortcut for Stop current session" });
    await row.click();
    await page.keyboard.press("ControlOrMeta+KeyL");
    await expect(dialog.getByRole("alert")).toContainText("Already used by Focus composer");
    await row.click();
    await page.keyboard.press("ControlOrMeta+Alt+KeyY");
    await expect(row).toContainText("Y");
    await expect(dialog).toContainText("stay with the host");
    await dialog.getByLabel("Search shortcuts").fill("Save through host");
    await expect(
      dialog.getByRole("button", { name: "Change shortcut for Save through host" }),
    ).toBeDisabled();
  });
  test("Renderer and Language are choices the person makes, and Esc or Done closes", async ({
    page,
  }) => {
    const dialog = await open(page);
    await dialog.getByRole("radio", { name: "Basic" }).click();
    expect(
      ((await page.evaluate(() => window.__vellum.preferences())) as Record<string, unknown>)
        .renderer,
    ).toBe("basic");
    await expect(dialog.getByText("Arabic UI mirrors chrome only")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
  });
  test("animations off and a backdrop colour reach the workspace", async ({ page }) => {
    const dialog = await open(page);
    await dialog.getByRole("switch", { name: "Animations and effects" }).click();
    // Until the person picks one, the field shows the appearance's own backdrop.
    await expect(dialog.getByLabel("Backdrop")).toHaveValue(/^#[0-9A-F]{6}$/);
    await dialog.getByLabel("Backdrop").fill(EDGE_BACKDROP);
    await dialog.getByLabel("Backdrop").blur();
    const root = page.locator(".vc-workspace");
    await expect(root).toHaveAttribute("data-animations", "off");
    await expect(root).toHaveCSS("--canvas-backdrop", EDGE_BACKDROP);
  });
});

test("S12 a document with no sessions says so in the switcher, with New session as the next step", async ({
  page,
}) => {
  await page.goto("/?agent=none&shell=1");
  await page.waitForFunction(() => !!window.__vellum);
  await page.evaluate(() => window.__vellum.agentReady());
  await expect(page.locator(".va-bar__title")).toHaveText("No session yet");
  await page.getByRole("button", { name: "Switch session" }).click();
  const switcher = page.getByRole("menu", { name: "Sessions" });
  await expect(switcher).toContainText("No sessions yet");
  await expect(switcher.getByRole("menuitem", { name: "New session" })).toBeVisible();
});

const EDGE = (name: string, extra = "") =>
  `/?scenario=juniper&zoom=0.5&pan=484,197&edge=${name}${extra}`;
test.describe("edge states", () => {
  test("S01 daemon offline: the notice, the toast, the strip and the disabled composer", async ({
    page,
  }) => {
    await page.goto(EDGE("s01"));
    await ready(page);
    const notice = page.locator(".ve-notice");
    await expect(notice).toContainText(copy.daemonOffline);
    await expect(notice).toContainText(
      "Agents need the Vellum daemon. Hand editing and saving still work.",
    );
    await expect(page.locator(".vc-strip")).toContainText(copy.daemonOffline);
    await expect(page.getByLabel("Request")).toBeDisabled();
    await expect(page.locator(".vk-offline")).toContainText("Agents need the Vellum daemon");
    const toast = page.getByRole("status", { name: copy.daemonOffline });
    await expect(toast).toContainText("Editing and saving still work, but agents cannot connect.");
    await toast.getByRole("button", { name: "Restart daemon" }).click();
    expect(await actions(page)).toContain("edge.restartDaemon");
    await toast.getByRole("button", { name: "Dismiss" }).click();
    await expect(toast).toHaveCount(0);
    await notice.getByRole("button", { name: "Restart" }).click();
    expect((await actions(page)).filter((a) => a === "edge.restartDaemon")).toHaveLength(2);
  });
  test("S02 basic renderer: a dismissible banner and the note in the strip", async ({ page }) => {
    await page.goto(EDGE("s02"));
    await ready(page);
    const banner = page.locator(".ve-banner");
    await expect(banner).toContainText("Effects are simplified on screen. Exports are unaffected.");
    await expect(page.locator(".ve-notice")).toContainText(copy.basicRenderer);
    await expect(page.locator(".vc-strip")).toContainText(copy.basicRenderer);
    await banner.getByRole("button", { name: "Dismiss" }).click();
    await expect(banner).toHaveCount(0);
    await expect(page.locator(".vc-strip")).toContainText(copy.basicRenderer);
  });
  test("S03 missing fonts: counts, lists, jumps to the layers and offers a replacement", async ({
    page,
  }) => {
    await page.goto(EDGE("s03"));
    await ready(page);
    const dialog = page.getByRole("dialog", { name: "Missing fonts" });
    await expect(dialog).toContainText(
      "Inter substitutes missing fonts so you can continue editing.",
    );
    await expect(dialog.getByRole("region", { name: "Lora" })).toContainText(
      "4 layers · substitute: Inter",
    );
    await expect(dialog.getByRole("region", { name: "IBM Plex Sans Arabic" })).toContainText(
      "2 layers · substitute: System Arabic",
    );
    await dialog
      .getByRole("region", { name: "Lora" })
      .getByRole("button", { name: "Jump to layers" })
      .click();
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["step2"]);
    await expect(dialog).toHaveCount(0);
    await expect(page.locator(".ve-notice")).toContainText(
      "2 fonts are unavailable. Choose a replacement or install them.",
    );
    await page.locator(".ve-notice").getByRole("button", { name: "Review fonts" }).click();
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: "Install from Google Fonts" }).first().click();
    expect(
      await page.evaluate(() => window.__vellum.hostActions().map((a) => a.command)),
    ).toContain("font.install");
  });
  test("S04 newer format: read-only banner, view-only pill, disabled composer", async ({
    page,
  }) => {
    await page.goto(EDGE("s04"));
    await ready(page);
    await expect(page.locator(".ve-banner")).toContainText(
      "Made with a newer version of Vellum (format 3) · Update to edit",
    );
    await expect(page.locator(".ve-viewonly")).toContainText("View only · file is not downgraded");
    await expect(page.locator(".ve-notice")).toContainText(
      "Made with a newer version of Vellum (format 3). Update to edit.",
    );
    await expect(page.getByLabel("Request")).toBeDisabled();
    await expect(page.locator(".vk-offline")).toContainText("Update Vellum to edit format 3");
    await page.locator(".ve-banner").getByRole("button", { name: "Update" }).click();
    expect(await actions(page)).toContain("edge.updateVellum");
  });
  test("S05 unreadable file: plain words, line and column, no partial rendering", async ({
    page,
  }) => {
    await page.goto(EDGE("s05"));
    await ready(page);
    const dialog = page.getByRole("dialog", { name: "juniper.vellum can’t be read" });
    await expect(dialog).toContainText(
      "The file contains invalid JSON. No partial design has been rendered.",
    );
    await expect(dialog).toContainText("Line 128 · column 17");
    await expect(dialog).toContainText('Expected a closing brace after "children".');
    await dialog.getByRole("button", { name: "Open as text" }).click();
    await dialog.getByRole("button", { name: "Format specification" }).click();
    expect(await actions(page)).toEqual(
      expect.arrayContaining(["edge.openAsText", "edge.formatSpec"]),
    );
    await expect(page.locator(".ve-notice")).toContainText("File can't be read");
    await expect(page.getByLabel("Request")).toBeDisabled();
  });
  test("S06 large file: a quiet suggestion that never blocks", async ({ page }) => {
    await page.goto(EDGE("s06"));
    await ready(page);
    await expect(page.locator(".ve-banner")).toContainText(
      "This file has 18,420 layers. The multi-file format can make changes easier to review.",
    );
    await expect(page.locator(".ve-notice")).toContainText("Large file");
    await expect(page.getByLabel("Request")).toBeEnabled();
  });
  test("S07 a refused write names who holds the lease and whom you are following", async ({
    page,
  }) => {
    await page.goto(EDGE("s07"));
    await ready(page);
    const toast = page.getByRole("status", { name: "Write rejected" });
    await expect(toast).toContainText("Saffron’s change rejected");
    await expect(toast).toContainText("Frame leased by Teal. You are following Cursor.");
    await toast.getByRole("button", { name: "View in ledger" }).click();
    await expect(page.getByRole("tab", { name: "History" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });
  test("S08 key failed: names the provider, never the key, and says other sessions are fine", async ({
    page,
  }) => {
    await page.goto(EDGE("s08"));
    await ready(page);
    const notice = page.locator(".ve-notice");
    await expect(notice).toContainText("Engine failed");
    await expect(notice).toContainText(
      "OpenRouter rejected the key. Other sessions are unaffected.",
    );
    await notice.getByRole("button", { name: "Update key" }).click();
    await expect(page.getByRole("dialog")).toContainText("OpenRouter");
  });
  test("S09 usage limit: when it resets, and Switch engine", async ({ page }) => {
    await page.goto(EDGE("s09"));
    await ready(page);
    const notice = page.locator(".ve-notice");
    await expect(notice).toContainText("Claude Code reached its usage limit");
    await expect(notice).toContainText("Resets at 18:00 · October 11");
    await expect(notice).toContainText("Other sessions are unaffected.");
    await notice.getByRole("button", { name: "Switch engine" }).click();
    await expect(page.getByRole("dialog")).toBeVisible();
  });
  test("S10 a step over two minutes says so, with Stop and Retry step", async ({ page }) => {
    await page.goto(EDGE("s10"));
    await ready(page);
    const block = page.locator(".va-stuck");
    await expect(block).toContainText(copy.agentStuck);
    await expect(block).toContainText(
      "You can stop this step or retry it. The screens already built are kept.",
    );
    await block.getByRole("button", { name: "Stop" }).click();
    expect(await page.evaluate(() => window.__vellum.agentRequests())).toContainEqual(
      expect.objectContaining({ type: "stop" }),
    );
    await block.getByRole("button", { name: "Retry step" }).click();
    expect(await page.evaluate(() => window.__vellum.agentRequests())).toContainEqual(
      expect.objectContaining({ type: "retry" }),
    );
  });
  test("S11 unsaved while agents work: the dialog names the file and the panel the agents", async ({
    page,
  }) => {
    await page.goto(EDGE("s11"));
    await ready(page);
    const dialog = page.getByRole("dialog", { name: "Save juniper.vellum?" });
    await expect(dialog).toContainText("Your changes have not been written to disk.");
    await expect(page.locator(".ve-notice")).toContainText(
      "3 agents are still working. Stop them and close?",
    );
    await dialog.getByRole("button", { name: "Don’t save" }).click();
    expect(await page.evaluate(() => window.__vellum.hostActions())).toContainEqual({
      command: "edge.close",
      detail: { choice: "discard" },
    });
  });
  test("S16 not signed in: the command to run, Open terminal, Re-check, Switch engine", async ({
    page,
  }) => {
    await page.goto(EDGE("s16"));
    await ready(page);
    const dialog = page.getByRole("dialog", { name: "Claude Code isn't signed in" });
    await expect(dialog).toContainText(
      "This session is paused. Your other sessions are unaffected.",
    );
    await expect(dialog).toContainText("Run claude in a terminal to sign in.");
    await dialog.getByRole("button", { name: "Open terminal" }).click();
    await dialog.getByRole("button", { name: "Re-check" }).click();
    expect(await actions(page)).toEqual(
      expect.arrayContaining(["edge.openTerminal", "edge.recheck"]),
    );
    await dialog.getByRole("button", { name: "Close dialog" }).click();
    await expect(dialog).toHaveCount(0);
    await expect(page.locator(".ve-notice")).toContainText("Claude Code isn't signed in");
  });
});

/**
 * The global state matrix: every panel × {loading, empty, offline daemon, read-only, basic renderer, missing fonts,
 * missing library} shows its exact copy, or still works.
 */
const MATRIX: [string, string, string][] = [];
for (const tab of ["agent", "layers", "assets", "history"])
  for (const state of ["loading", "empty", "offline", "readonly", "basic", "fonts", "library"])
    MATRIX.push([
      tab,
      state,
      `/?tab=${tab}&edge=m-${state}${tab === "history" ? "&ledger=c10" : ""}`,
    ]);
const EXPECT: Record<string, Record<string, string | RegExp>> = {
  agent: {
    loading: "Loading sessions",
    empty: /What are we designing\?|Describe what to design or change/,
    offline: copy.daemonOffline,
    readonly: "Read-only",
    basic: copy.basicRenderer,
    fonts: "Missing fonts",
    library: /What are we designing\?|Describe what to design or change/,
  },
  layers: {
    loading: "Loading layers",
    empty: copy.layersEmpty,
    offline: "Step 1",
    readonly: "Step 1",
    basic: "Step 1",
    fonts: "Step 1",
    library: "Step 1",
  },
  assets: {
    loading: "Loading assets",
    empty: "No components or libraries yet",
    offline: "Search assets",
    readonly: "Search assets",
    basic: "Search assets",
    fonts: "Search assets",
    library: "Search assets",
  },
  history: {
    loading: "Loading history",
    empty: copy.historyEmpty,
    offline: copy.historyFooter,
    readonly: copy.historyFooter,
    basic: copy.historyFooter,
    fonts: copy.historyFooter,
    library: copy.historyFooter,
  },
};
for (const [tab, state, url] of MATRIX)
  test(`matrix · ${tab} · ${state}`, async ({ page }) => {
    await page.goto(url);
    await page.waitForFunction(() => !!window.__vellum);
    await page.evaluate(() => window.__vellum.agentReady());
    const panel = page.locator(".vc-panel");
    const want = EXPECT[tab]?.[state] as string | RegExp;
    if (tab === "assets" && state !== "loading" && state !== "empty")
      await expect(panel.getByLabel(want as string)).toBeVisible();
    else if (tab === "layers" && !["loading", "empty"].includes(state))
      await expect(panel.getByRole("treeitem").first()).toBeVisible();
    else await expect(panel).toContainText(want);
    const text = (await panel.innerText()) as string;
    expect(text).not.toMatch(/!|\p{Extended_Pictographic}/u);
  });
