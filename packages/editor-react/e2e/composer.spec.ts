// Chrome lane: the composer, skills menu, engine picker, usage, Permission card and inline prompt (Prompt 50) over
// the scripted session feed of fixtures/agent-scenarios.ts. TEST DATA throughout.

import { expect, type Page, test } from "@playwright/test";
import { copy } from "@vellum/config";

const S1 = "01J00000000000000000000S01";
const T1 = "01J00000000000000000000T01";
const TEAL = "01J00000000000000000000A01";
const SAFFRON = "01J00000000000000000000A03";
const person = { kind: "person", device: "01J00000000000000000000D01" };

async function open(page: Page, query: string) {
  await page.goto(`/?scenario=juniper&zoom=0.5&tab=agent&${query}`);
  await page.evaluate(() => window.__vellum.agentReady());
}
const field = (page: Page) => page.getByRole("combobox", { name: "Request" });
const send = (page: Page, update: unknown) =>
  page.evaluate((u) => window.__vellum.agentSend(u), update);
const sent = (page: Page) =>
  page.evaluate(() => window.__vellum.composerSent()) as Promise<
    { session: string; request: Record<string, unknown> }[]
  >;
const requests = (page: Page) => page.evaluate(() => window.__vellum.agentRequests());
const event = (seq: number, body: unknown, agent = true) => ({
  type: "event",
  session: S1,
  event: {
    seq,
    at: 1_792_160_000_000 + seq,
    origin: { actor: agent ? { kind: "agent", agent: TEAL } : person, session: S1, turn: T1 },
    body,
  },
});

test.describe("Permission card", () => {
  test("blocks the turn until the person decides", async ({ page }) => {
    await open(page, "agent=c27");
    const card = page.getByRole("region", { name: "Waiting for permission" });
    await expect(card).toContainText("Teal wants to read DESIGN.md");
    await expect(card.locator(".vk-permission__target")).toHaveText("~/code/juniper/DESIGN.md");
    await expect(page.locator(".va-header__state")).toHaveText("Needs you");
    // While it waits, the session is still working: Stop, not Send.
    await expect(page.locator(".vk-composer").getByRole("button", { name: "Stop" })).toBeVisible();

    await card.getByRole("button", { name: "Allow once" }).click();
    expect(await requests(page)).toContainEqual({
      type: "answer",
      session: S1,
      question: "q-design",
      answer: "allow_once",
    });
    // Answered but not yet recorded: the turn stays blocked and the card cannot be answered twice.
    await expect(card).toBeVisible();
    await expect(card.getByRole("button", { name: "Deny" })).toBeDisabled();
    await expect(page.locator(".va-header__state")).toHaveText("Needs you");

    await send(page, { type: "answered", session: S1, question: "q-design" });
    await send(
      page,
      event(
        10,
        {
          type: "permission",
          action: { readFile: { path: "~/code/juniper/DESIGN.md" } },
          kind: "read_repo",
          answer: "allow_once",
          source: { kind: "person" },
          engine: "builtin",
        },
        false,
      ),
    );
    await send(
      page,
      event(11, {
        type: "turn",
        turn: T1,
        from: "awaiting_permission",
        input: "permission_answered",
        to: "running",
      }),
    );
    await expect(card).toHaveCount(0);
    await expect(page.locator(".va-header__state")).toHaveText("Working");
    await expect(page.getByRole("region", { name: "Allowed" })).toContainText(
      "Permission allowed once",
    );
  });

  test("Deny is an answer too, and the skip-all line turns prompts back on", async ({ page }) => {
    await open(page, "agent=c27");
    await page.getByRole("button", { name: "Deny" }).click();
    expect(await requests(page)).toContainEqual({
      type: "answer",
      session: S1,
      question: "q-design",
      answer: "deny",
    });
    await send(page, { type: "permissionSettings", skipAll: true });
    const line = page.locator(".vk-skipall");
    await expect(line).toContainText("Permission prompts skipped");
    await line.getByRole("button", { name: "Turn prompts on" }).click();
    await expect(line).toHaveCount(0);
    expect(await requests(page)).toContainEqual({ type: "setSkipAll", on: false });
  });
});

test.describe("Send and Stop", () => {
  test("Stop shows whenever any agent works, with Stop this agent and Stop all for several", async ({
    page,
  }) => {
    await open(page, "agent=c03");
    const composer = page.locator(".vk-composer");
    await expect(composer.getByRole("button", { name: "Stop" })).toBeVisible();
    await expect(composer.getByRole("button", { name: "Send" })).toHaveCount(0);
    await composer.getByRole("button", { name: "Stop" }).click();
    expect(await requests(page)).toContainEqual({ type: "stop", session: S1 });

    await send(
      page,
      event(50, { type: "parallel", event: "lanes", lanes: [{ agent: TEAL }, { agent: SAFFRON }] }),
    );
    await composer.getByRole("button", { name: "Stop" }).click();
    const menu = page.getByRole("menu");
    await expect(menu).toContainText("Stop this agent");
    await menu.getByRole("menuitem", { name: "Saffron" }).click();
    expect(await requests(page)).toContainEqual({ type: "stopAgent", session: S1, agent: SAFFRON });
    await composer.getByRole("button", { name: "Stop" }).click();
    await page.getByRole("menuitem", { name: "Stop all" }).click();

    await send(
      page,
      event(51, { type: "turn", turn: T1, from: "running", input: "done", to: "done" }),
    );
    await expect(composer.getByRole("button", { name: "Send" })).toBeVisible();
    await expect(composer.getByRole("button", { name: "Stop" })).toHaveCount(0);
  });

  test("sending while an agent works asks Queue after this turn or Stop and send now", async ({
    page,
  }) => {
    await open(page, "agent=c03");
    await field(page).fill("Also make Step 3");
    await field(page).press("Enter");
    const queue = page.getByRole("region", { name: "Send while an agent works" });
    await expect(queue).toBeVisible();
    await expect(queue.getByRole("button", { name: "Queue after this turn" })).toBeFocused();
    await queue.getByRole("button", { name: "Queue after this turn" }).click();
    await expect(queue).toHaveCount(0);
    const last = (await sent(page)).at(-1);
    expect(last?.request).toMatchObject({ text: "Also make Step 3", queue: "after" });
    await expect(field(page)).toHaveValue("");

    await field(page).fill("Start over");
    await field(page).press("Enter");
    await page.getByRole("button", { name: "Stop and send now" }).click();
    expect((await sent(page)).at(-1)?.request).toMatchObject({ queue: "now" });
    expect(await requests(page)).toContainEqual({ type: "stop", session: S1 });
  });

  test("Enter sends with the page chip, Parallel, Explore and engine; Shift+Enter adds a line", async ({
    page,
  }) => {
    await open(page, "agent=c08");
    await field(page).fill("Calmer");
    await field(page).press("Shift+Enter");
    await field(page).pressSequentially("cards");
    await page.locator(".vk-footer-button--parallel").click();
    await page.getByRole("button", { name: "More: Agents" }).click();
    await page.getByRole("radio", { name: "Side by side" }).click();
    await page.keyboard.press("Escape");
    await expect(page.locator(".vk-footer-button--parallel")).toHaveText("2× Side by side");
    await page.locator(".vk-footer-button--explore").click();
    await page.getByRole("button", { name: "More: Variants" }).click();
    await page.getByRole("radio", { name: "Style" }).click();
    await page.keyboard.press("Escape");
    await expect(page.locator(".vk-footer-button--explore")).toHaveText("Explore 2 · Style");
    await field(page).press("Enter");
    expect((await sent(page)).at(-1)?.request).toMatchObject({
      text: "Calmer\ncards",
      context: [{ kind: "page", label: "Onboarding", value: "p1" }],
      parallel: { count: 2, mode: "side_by_side" },
      explore: { count: 2, vary: "style" },
      engine: { engine: "builtin", provider: "openrouter" },
    });
  });

  test("↑ and ↓ walk prompt history in an empty composer; each session keeps its draft", async ({
    page,
  }) => {
    await open(page, "agent=c06");
    await send(
      page,
      event(60, { type: "turn", turn: T1, from: "running", input: "done", to: "done" }),
    );
    await field(page).click();
    await field(page).press("ArrowUp");
    await expect(field(page)).toHaveValue(
      "Design the onboarding screens in @DESIGN.md using Starter Minimal.",
    );
    await field(page).press("ArrowDown");
    await expect(field(page)).toHaveValue("");

    await field(page).fill("Draft for onboarding");
    await page.getByRole("button", { name: "Switch session" }).click();
    await page.getByRole("menuitem", { name: /Token cleanup/ }).click();
    await expect(field(page)).toHaveValue("");
    await field(page).fill("Draft for tokens");
    await page.getByRole("button", { name: "Switch session" }).click();
    await page.getByRole("menuitem", { name: /Onboarding screens/ }).click();
    await expect(field(page)).toHaveValue("Draft for onboarding");
  });

  test("⌘L focuses the composer", async ({ page }) => {
    await open(page, "agent=c08");
    await page.locator(".vc-canvas").click({ position: { x: 1200, y: 700 } });
    await page.keyboard.press("ControlOrMeta+l");
    await expect(field(page)).toBeFocused();
  });

  test("the composer is disabled with a Restart line when the daemon is offline", async ({
    page,
  }) => {
    await open(page, "agent=c08&connection=offline");
    await expect(field(page)).toBeDisabled();
    const [line, restart] = copy.composerWhenOffline.split(" · ");
    await expect(page.locator(".vk-offline")).toContainText(line as string);
    await page
      .locator(".vk-offline")
      .getByRole("button", { name: restart as string })
      .click();
    expect(await page.evaluate(() => window.__vellum.hostActions())).toContainEqual({
      command: "daemon.restart",
    });
  });
});

test.describe("chips, mentions and skills", () => {
  test("/ opens the skills menu, a pick is a chip sent with the request, and Add skill… adds one", async ({
    page,
  }) => {
    await open(page, "agent=c26");
    await field(page).fill("/");
    const menu = page.getByRole("listbox", { name: "Skills" });
    await expect(menu.getByRole("group")).toHaveCount(3);
    await expect(menu.getByRole("group", { name: "Built-in" }).getByRole("option")).toHaveCount(4);
    await expect(menu.getByRole("group", { name: "Yours" })).toContainText(
      "Juniper voice · ~/skills/juniper-voice/SKILL.md",
    );
    await field(page).fill("/juni");
    await expect(menu.getByRole("option")).toHaveCount(1);
    await field(page).press("Enter");
    await expect(menu).toHaveCount(0);
    await expect(page.locator(".vk-chips")).toContainText("Skill · Juniper voice");
    await field(page).fill("Design onboarding from @DESIGN.md.");
    await page.keyboard.press("Escape");
    await page.locator(".vk-composer").getByRole("button", { name: "Send" }).click();
    expect((await sent(page)).at(-1)?.request).toMatchObject({
      skills: ["~/skills/juniper-voice/SKILL.md"],
    });
  });

  test("@ lists files, frames and components; a pick becomes @text and a removable chip", async ({
    page,
  }) => {
    await open(page, "agent=c26");
    await field(page).fill("Design onboarding from @DES");
    const menu = page.getByRole("listbox", { name: "Mentions" });
    await expect(menu.getByRole("option")).toHaveText(["DESIGN.md · File"]);
    await field(page).press("Enter");
    await expect(field(page)).toHaveValue("Design onboarding from @DESIGN.md ");
    await expect(page.locator(".vk-chips")).toContainText("DESIGN.md");
    await field(page).fill("Use @Step");
    await expect(menu.getByRole("option")).toHaveText([
      "Step 1 · Frame",
      "Step 2 · Frame",
      "Step 3 · Frame",
    ]);
    await menu.getByRole("option", { name: "Step 2 · Frame" }).click();
    await expect(page.locator(".vk-chips")).toContainText("Step 2");
    await page.getByRole("button", { name: "Remove Step 2" }).click();
    await expect(page.locator(".vk-chips")).not.toContainText("Step 2");
  });

  test("the selection is a chip and changes the placeholder", async ({ page }) => {
    await open(page, "agent=c08&select=habit,continue");
    await expect(page.locator(".vk-chips")).toHaveText(/2 layers in Step 2/);
    await expect(field(page)).toHaveAttribute("placeholder", copy.selectionPlaceholder);
  });

  test("attach adds the host's file as a chip", async ({ page }) => {
    await open(page, "agent=c08");
    await page.getByRole("button", { name: "Attach" }).click();
    await page.getByRole("menuitem", { name: "Attach a file…" }).click();
    await expect(page.locator(".vk-chips")).toContainText("hero.png");
  });
});

test.describe("first open", () => {
  test("asks what we are designing, and each suggestion fills the composer", async ({ page }) => {
    await open(page, "agent=c01&empty=1");
    const first = page.getByRole("region", { name: copy.firstOpenHeading });
    await expect(first).toContainText(copy.drawByHandHint);
    await first.getByRole("button", { name: "Design from a spec" }).click();
    await expect(field(page)).toBeFocused();
    await expect(page.getByRole("listbox", { name: "Mentions" }).getByRole("option")).toHaveText([
      "DESIGN.md · File",
      "README.md · File",
    ]);
    await page.keyboard.press("Escape");
    await first.getByRole("button", { name: "Pick a visual style" }).click();
    const dialog = page.getByRole("dialog", { name: "Pick a visual style" });
    await dialog.getByRole("button", { name: "Use Artisan Editorial" }).click();
    await expect(dialog).toHaveCount(0);
    await expect(field(page)).toHaveValue("Use the Artisan Editorial visual style. ");
    await page.getByRole("button", { name: "Connect your coding agent" }).click();
    expect(await page.evaluate(() => window.__vellum.hostActions())).toContainEqual({
      command: "agents.connect",
    });
  });
});

test.describe("engine, model and usage", () => {
  test("the picker lists providers with capabilities in words; choosing one changes the engine line", async ({
    page,
  }) => {
    await open(page, "agent=c11");
    await page.getByRole("button", { name: /^Engine and model:/ }).click();
    const picker = page.getByRole("dialog", { name: "Engine and model" });
    await expect(picker).toContainText("Vision · Fast · Large context");
    await expect(picker).toContainText("On your computer");
    await expect(picker).toContainText("Installed · signed in");
    await expect(picker).toContainText("A new provider won't see earlier turns in this session.");
    await picker.getByRole("button", { name: /^Anthropic/ }).click();
    await expect(page.locator(".vk-model")).toHaveText("Anthropic · Claude Sonnet");
    await picker.getByRole("button", { name: "OpenAI model: Reasoning model" }).click();
    await page.getByRole("menuitemradio", { name: "Reasoning model" }).click();
    await picker.getByRole("button", { name: /Reasoning effort/ }).click();
    await page.getByRole("menuitemradio", { name: "High" }).click();
    await page.keyboard.press("Escape");
    await expect(page.locator(".vk-model")).toHaveText("OpenAI · Reasoning model");
    await field(page).fill("Go");
    await field(page).press("Enter");
    expect((await sent(page)).at(-1)?.request).toMatchObject({
      engine: { engine: "builtin", provider: "openai", model: "reasoning", effort: "high" },
    });
  });

  test("an API key is never shown again after it is entered", async ({ page }) => {
    const key = "sk-or-v1-TESTDATA-0000-not-a-real-key";
    await open(page, "agent=c11&composer=providers");
    await page
      .getByRole("dialog", { name: "Add provider" })
      .getByRole("button", { name: /OpenRouter/ })
      .click();
    const setup = page.getByRole("dialog", { name: "Add provider" });
    await expect(setup).toContainText(copy.keyStorage);
    const input = setup.getByLabel("API key");
    await expect(input).toHaveAttribute("type", "password");
    await input.fill(key);
    await setup.getByRole("button", { name: "Test and continue" }).click();
    await expect(setup.getByRole("status")).toHaveText("Connected");
    await expect(input).toHaveValue("");
    expect(await page.evaluate(() => window.__vellum.keychain())).toEqual({ openrouter: key });
    expect(await page.content()).not.toContain(key);
    expect(await page.evaluate(() => window.__vellum.composerState())).not.toContain(key);
    expect(JSON.stringify(await requests(page))).not.toContain(key);
    await setup.getByRole("button", { name: "Clear key" }).click();
    await expect(setup.getByRole("status")).toHaveText("Configured outside Vellum (not tested)");
  });

  test("the usage ring opens tokens by kind and the estimated cost", async ({ page }) => {
    await open(page, "agent=c27");
    await page.getByRole("img", { name: "Context usage · 18%" }).click();
    const usage = page.getByRole("dialog", { name: "Context usage · 18%" });
    await expect(usage).toContainText("Conversation4.2k tokens");
    await expect(usage).toContainText(`${copy.costLabel} · $0.04`);
    await expect(usage).toContainText(
      "Estimate for this session. Your provider bills actual usage.",
    );
  });
});

test.describe("inline prompt", () => {
  test("Ask in the selection toolbar opens it beside the selection; sending shows progress and Undo", async ({
    page,
  }) => {
    await open(page, "agent=c08&select=habit,continue");
    const toolbar = page.getByRole("toolbar", { name: "Selection" });
    await toolbar.getByRole("button", { name: /Ask/ }).click();
    const inline = page.getByRole("dialog", { name: "Ask about the selection" });
    await expect(inline).toContainText("2 layers in Step 2");
    await expect(toolbar).toHaveCount(0);
    const box = await inline.boundingBox();
    expect(box?.width).toBe(360);
    await inline.getByRole("textbox").fill("Make these feel less crowded");
    await inline.getByRole("textbox").press("Enter");
    await expect(inline.getByRole("status")).toHaveText("Refining selected layers…");
    expect((await sent(page)).at(-1)).toMatchObject({
      session: S1,
      request: {
        text: "Make these feel less crowded",
        context: [{ kind: "selection", label: "2 layers in Step 2", value: "habit,continue" }],
      },
    });
    await inline.getByRole("button", { name: "Undo" }).click();
    await expect(inline).toHaveCount(0);
    expect(await requests(page)).toContainEqual({
      type: "undoTurn",
      session: S1,
      turn: "01J0000000000000000000T001",
    });
  });

  test("⌘K opens it on the selection and Esc closes it", async ({ page }) => {
    await open(page, "agent=c08&select=habit");
    await page.locator(".vc-canvas").focus();
    await page.keyboard.press("ControlOrMeta+k");
    const inline = page.getByRole("dialog", { name: "Ask about the selection" });
    await expect(inline.getByRole("textbox")).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(inline).toHaveCount(0);
  });
});
