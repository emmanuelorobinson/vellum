// Chrome lane: the Agent tab (Prompt 49) over the scripted session feed of fixtures/agent-scenarios.ts, and the
// installed-engine transcripts of fixtures/agent-panel/ (Prompts 36–37). TEST DATA throughout.

import { expect, type Page, test } from "@playwright/test";
import { copy } from "@vellum/config";

const T1 = "01J00000000000000000000T01";
const T2 = "01J00000000000000000000T02";
const S1 = "01J00000000000000000000S01";
const TEAL = "01J00000000000000000000A01";

async function open(page: Page, query: string) {
  await page.goto(`/?scenario=juniper&zoom=0.5&tab=agent&${query}`);
  await page.evaluate(() => window.__vellum.agentReady());
}
const panel = (page: Page) => page.getByRole("region", { name: "Agent panel" });
const scroller = (page: Page) => page.locator(".va-scroll");
/** A finished step of turn `turn`, at `seq`, as the daemon records it. */
const stepEvent = (seq: number, turn: string, line = "Step") => ({
  type: "event",
  session: S1,
  event: {
    seq,
    at: 1_792_160_000_000 + seq,
    origin: { actor: { kind: "agent", agent: TEAL }, session: S1, turn },
    body: {
      type: "step",
      // Alternating kinds, so a long turn does not fold them into one group.
      tool: seq % 2 ? "update_nodes" : "move_nodes",
      arguments: "{}",
      result: "{}",
      startedAt: 0,
      durationMs: 1000,
      outcome: "done",
      detail: {
        verb: seq % 2 ? "updated" : "moved",
        target: { id: `n${seq}`, name: `${line} ${seq}`, type: "frame" },
        counts: { created: 0, changed: 1, removed: 0, variables: 0 },
      },
    },
  },
});
const live = (text: string) => ({
  type: "live",
  session: S1,
  turn: T1,
  item: { type: "text", text },
});

test.describe("Agent tab", () => {
  test("streaming follows the bottom, and never moves a reader who scrolled up; Latest takes them down", async ({
    page,
  }) => {
    await open(page, "agent=stream");
    for (let seq = 100; seq < 130; seq++)
      await page.evaluate((u) => window.__vellum.agentSend(u), stepEvent(seq, T1));
    const atBottom = () =>
      scroller(page).evaluate((el) => el.scrollHeight - el.clientHeight - el.scrollTop <= 1);
    await expect.poll(atBottom).toBe(true);
    expect(await scroller(page).evaluate((el) => el.scrollHeight > el.clientHeight + 400)).toBe(
      true,
    );
    // Scrolled up: new prose and steps arrive below, and the view stays where the reader is.
    await scroller(page).evaluate((el) => {
      el.scrollTop = 120;
    });
    await page.waitForTimeout(50);
    for (let i = 0; i < 5; i++) {
      await page.evaluate(
        (u) => window.__vellum.agentSend(u),
        live(" More words arrive while you read."),
      );
      await page.evaluate((u) => window.__vellum.agentSend(u), stepEvent(200 + i, T1));
      await page.waitForTimeout(20);
      expect(await scroller(page).evaluate((el) => el.scrollTop)).toBe(120);
    }
    const latest = page.getByRole("button", { name: "Latest" });
    await expect(latest).toBeVisible();
    await latest.click();
    await expect.poll(atBottom).toBe(true);
    await expect(latest).toBeHidden();
    // Back at the bottom, it follows again.
    await page.evaluate((u) => window.__vellum.agentSend(u), stepEvent(300, T1));
    await expect.poll(atBottom).toBe(true);
  });

  test("Undo this turn asks first and undoes only that turn", async ({ page }) => {
    await open(page, "agent=c09");
    // A second, later turn.
    const turn2 = [
      {
        seq: 50,
        body: { type: "prompt", text: "Tighten Step 3", context: [], images: [] },
        person: true,
      },
      {
        seq: 51,
        body: { type: "turn", turn: T2, from: "queued", input: "start", to: "planning" },
        person: true,
      },
      {
        seq: 52,
        body: { type: "turn", turn: T2, from: "planning", input: "no_plan_needed", to: "running" },
      },
      { seq: 54, body: { type: "turn", turn: T2, from: "running", input: "done", to: "done" } },
    ];
    for (const e of turn2)
      await page.evaluate((u) => window.__vellum.agentSend(u), {
        type: "event",
        session: S1,
        event: {
          seq: e.seq,
          at: 1_792_160_100_000 + e.seq,
          origin: {
            actor: e.person
              ? { kind: "person", device: "01J00000000000000000000D01" }
              : { kind: "agent", agent: TEAL },
            session: S1,
            turn: T2,
          },
          body: e.body,
        },
      });
    await page.evaluate((u) => window.__vellum.agentSend(u), stepEvent(53, T2, "Step 3 card"));
    const turns = panel(page).locator("article.va-turn");
    await expect(turns).toHaveCount(2);
    // Retry is offered on the latest turn only.
    await expect(turns.nth(0).getByRole("button", { name: "Retry", exact: true })).toHaveCount(0);
    await expect(turns.nth(1).getByRole("button", { name: "Retry", exact: true })).toBeVisible();

    await turns.nth(0).getByRole("button", { name: "Undo this turn" }).click();
    const confirm = page.locator(".va-confirm");
    await expect(confirm).toContainText(
      copy.undoTurnConfirm.replace("{n}", "47").replace("{agent}", "Teal"),
    );
    await confirm.getByRole("button", { name: "Cancel" }).click();
    expect(await page.evaluate(() => window.__vellum.agentRequests())).toEqual([]);

    await turns.nth(0).getByRole("button", { name: "Undo this turn" }).click();
    await confirm.getByRole("button", { name: "Undo this turn" }).click();
    expect(await page.evaluate(() => window.__vellum.agentRequests())).toEqual([
      { type: "undoTurn", session: S1, turn: T1 },
    ]);
    // The daemon records it; only the first turn reads as undone.
    await page.evaluate((u) => window.__vellum.agentSend(u), {
      type: "event",
      session: S1,
      event: {
        seq: 60,
        at: 1_792_160_200_000,
        origin: {
          actor: { kind: "person", device: "01J00000000000000000000D01" },
          session: S1,
          turn: T1,
        },
        body: { type: "undo", changed: 47, kept: 0 },
      },
    });
    await expect(turns.nth(0)).toContainText("Undone · 47 changes removed");
    await expect(turns.nth(0).getByRole("button", { name: "Undo this turn" })).toHaveCount(0);
    await expect(turns.nth(1).getByRole("button", { name: "Undo this turn" })).toBeVisible();
    await expect(turns.nth(1)).not.toContainText("Undone");
  });

  test("an external agent's session shows its steps and the composer replacement line", async ({
    page,
  }) => {
    await open(page, "agent=c06");
    await expect(page.locator(".vk-composer")).toHaveCount(1);
    await page.getByRole("button", { name: "Switch session" }).click();
    const switcher = page.getByRole("menu", { name: "Sessions" });
    await expect(switcher.getByRole("group")).toHaveCount(3);
    await switcher.getByRole("menuitem", { name: /Claude Code/ }).click();
    await expect(switcher).toBeHidden();
    await expect(panel(page)).toContainText("Claude Code · external");
    await expect(panel(page)).toContainText("Connected · 8 min");
    await expect(panel(page)).toContainText("Designed Step 2 · 28 layers");
    await expect(page.locator(".vk-composer")).toHaveCount(0);
    const line = copy.externalSessionLine.replace("{client}", "Claude Code").split(" · ");
    await expect(page.locator(".va-external")).toContainText(line[0] as string);
    await page
      .locator(".va-external")
      .getByRole("button", { name: line[1] as string })
      .click();
    expect(await page.evaluate(() => window.__vellum.agentRequests())).toEqual([
      { type: "stop", session: "01J00000000000000000000S02" },
    ]);
  });

  test("the switcher is keyboard reachable and makes and deletes sessions", async ({ page }) => {
    await open(page, "agent=c06");
    await page.getByRole("button", { name: "Switch session" }).focus();
    await page.keyboard.press("Enter");
    const switcher = page.getByRole("menu", { name: "Sessions" });
    await expect(switcher).toBeVisible();
    await page.keyboard.press("ArrowDown");
    await expect(switcher.locator("[role=menuitem][data-highlighted]")).toHaveCount(1);
    await page.keyboard.press("Escape");
    await expect(switcher).toBeHidden();
    await page.getByRole("button", { name: "New session" }).click();
    await expect(panel(page)).toContainText("New session");
    await page.getByRole("button", { name: "Switch session" }).click();
    await switcher.getByRole("menuitem", { name: "Delete session" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Delete session" }).click();
    const requests = (await page.evaluate(() => window.__vellum.agentRequests())) as {
      type: string;
    }[];
    expect(requests.map((r) => r.type)).toEqual(["remove"]);
    await expect(panel(page).locator(".va-bar__title")).toHaveText("Onboarding screens");
  });

  test("a designed step's thumbnail zooms to its frame and selects it; steps expand with their details", async ({
    page,
  }) => {
    await open(page, "agent=c03");
    await page.getByRole("button", { name: "Welcome · zoom to frame" }).click();
    expect(await page.evaluate(() => window.__vellum.selection())).toEqual(["welcome"]);
    expect(await page.evaluate(() => window.__vellum.hostActions())).toContainEqual({
      command: "agent.zoomTo",
      detail: { node: "welcome" },
    });
    const row = page.getByRole("button", { name: /Designed Welcome · 28 layers/ });
    await expect(row).toHaveAttribute("aria-expanded", "false");
    await row.click();
    await expect(row).toHaveAttribute("aria-expanded", "true");
    await expect(panel(page)).toContainText("Welcome · welcome");
    await expect(panel(page)).toContainText("28 created");
    // A working step reads Working, with its own duration.
    await expect(page.locator('.va-step[data-state="working"]')).toContainText("Designing Step 1");
    await expect(page.locator('.va-step[data-state="working"]')).toContainText("6 s");
  });

  test("Retry step, Retry and Review changes go to the daemon and History", async ({ page }) => {
    await open(page, "agent=c09");
    await page.getByRole("button", { name: "Retry step" }).click();
    await page.getByRole("button", { name: "Retry", exact: true }).click();
    await page.getByRole("button", { name: "Review changes" }).click();
    const requests = (await page.evaluate(() => window.__vellum.agentRequests())) as {
      type: string;
    }[];
    expect(requests.map((r) => r.type)).toEqual(["retryStep", "retry"]);
    expect(await page.evaluate(() => window.__vellum.hostActions())).toContainEqual({
      command: "history.reviewTurn",
      detail: { session: S1, turn: T1 },
    });
  });

  for (const [engine, prose, tokens] of [
    ["claude-code", "Fixture complete", "26k tokens"],
    [
      "codex",
      "I’ll locate the attached canvas state tool and call it once.Fixture complete",
      "54k tokens",
    ],
  ] as const)
    test(`replays the ${engine} transcript: streaming prose, then the finished turn`, async ({
      page,
    }) => {
      await open(page, `agent=${engine}&hold`);
      const turn = panel(page).locator("article.va-turn");
      await expect(turn).toContainText("Working");
      const remaining = await page.evaluate(() => window.__vellum.agentEmit(1));
      await expect(turn.locator(".va-prose")).toHaveAttribute("aria-busy", "true");
      expect(remaining).toBeGreaterThan(0);
      await page.evaluate(() => window.__vellum.agentEmit());
      await expect(turn).toContainText("Done");
      await expect(turn.locator(".va-prose")).toHaveText(prose);
      await expect(turn.locator(".va-prose")).not.toHaveAttribute("aria-busy", "true");
      await expect(turn).toContainText(tokens);
      await expect(turn.getByRole("button", { name: "Retry", exact: true })).toBeVisible();
    });
});
