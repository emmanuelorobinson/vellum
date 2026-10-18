// TEST DATA: the Juniper sessions of fixtures/agent-scenarios.ts.
import { copy } from "@vellum/config";
import { createAgentController } from "@vellum/editor-core";
import { renderToStaticMarkup as html } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  AGENT_ACTORS,
  AGENT_LAYER_NAMES,
  AGENT_NOW,
  type AgentScenario,
  agentScenario,
} from "../fixtures/agent-scenarios";
import { AgentTab } from "./agent/AgentTab";
import { TipProvider } from "./chrome/Tooltip";

async function render(name: AgentScenario, composer = <div data-testid="composer" />) {
  const s = agentScenario(name);
  const agent = createAgentController({
    feed: s.feed,
    now: () => AGENT_NOW,
    timeZone: "UTC",
    nameOf: (id) => AGENT_LAYER_NAMES[id] ?? null,
  });
  await agent.load();
  if (s.current) await agent.select(s.current);
  for (const u of s.updates) s.feed.emit(u);
  for (const key of s.expanded ?? []) agent.toggle(key);
  return {
    agent,
    out: html(
      <TipProvider>
        <AgentTab agent={agent} actors={AGENT_ACTORS} composer={composer} />
      </TipProvider>,
    ),
  };
}
const text = (s: string) =>
  s
    .replace(/<[^>]+>/g, " ")
    .replace(/&#x27;/g, "'")
    .replace(/&amp;/g, "&")
    .replace(/\s+/g, " ");

describe("agent tab", () => {
  it("C03: the running turn reads in the order it happened, with Working and a working ring", async () => {
    const { out } = await render("c03");
    const t = text(out);
    const order = [
      "Onboarding screens",
      "Design the onboarding screens in @DESIGN.md using Starter Minimal.",
      "DESIGN.md · Starter Minimal",
      "Teal",
      "Working",
      "deepseek/deepseek-v4.1-flash",
      "I've built Welcome. I'm building Step 1, then Steps 2 and 3.",
      "Read DESIGN.md",
      "Read design-system guidelines",
      "Claimed frame Welcome",
      "Designed Welcome · 28 layers",
      "Welcome · zoom to frame",
      "Designing Step 1",
    ];
    let at = 0;
    for (const piece of order) {
      const next = t.indexOf(piece, at);
      expect(next, piece).toBeGreaterThanOrEqual(at);
      at = next + piece.length;
    }
    expect(out).toContain('data-state="working"');
    expect(out).toContain('data-testid="composer"');
    // State is a symbol plus words, never colour alone.
    expect(t).toContain("done");
  });
  it("C09: footer, Undo this turn, Retry on the latest turn, the verdict, and a failed step with Retry step", async () => {
    const { out } = await render("c09");
    const t = text(out);
    expect(t).toContain("Made 47 changes · 2 frames");
    expect(t).toContain("1 m 12 s · 38k tokens");
    expect(t).toContain("Fixed: CTA text overflowed at 375 px");
    expect(t).toContain("Image request failed");
    expect(t).toContain("The image service did not answer in time.");
    for (const action of ["Retry step", "Review changes", "Undo this turn", "Retry"])
      expect(out).toMatch(new RegExp(`>${action}<`));
  });
  it("C06 external: the steps and the composer replaced by the external line with Stop", async () => {
    const { out } = await render("c06e");
    const t = text(out);
    expect(t).toContain("Claude Code · external");
    expect(t).toContain("Connected · 8 min");
    expect(t).toContain(
      copy.externalSessionLine.replace("{client}", "Claude Code").split(" · ")[0] as string,
    );
    expect(out).not.toContain('data-testid="composer"');
    expect(t).toContain("Made 28 changes · 1 frame · 18 s");
    expect(out).toMatch(/>Stop</);
    expect(out).not.toMatch(/>Retry</);
  });
  it("no sessions: the tab still shows its composer", async () => {
    const { FakeAgentFeed } = await import("@vellum/editor-core");
    const agent = createAgentController({ feed: new FakeAgentFeed([]), now: () => 0 });
    await agent.load();
    const out = html(
      <TipProvider>
        <AgentTab agent={agent} actors={{}} composer={<div data-testid="composer" />} />
      </TipProvider>,
    );
    expect(out).toContain('data-testid="composer"');
    expect(out).toContain("New session");
  });
});
