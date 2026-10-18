// TEST DATA: Juniper layers, pages, components, variables and sessions.
import { describe, expect, it, vi } from "vitest";
import {
  ASK_PREFIX,
  createPaletteController,
  type PaletteCommand,
  type PaletteEntry,
} from "./index";

const command = (id: string, title: string, enabled = true): PaletteCommand => ({
  id,
  title,
  category: "Edit",
  enabled,
});
const commands = [
  command("edit.autoLayout", "Add auto-layout"),
  command("component.toggle", "Make component or turn back into a layer"),
  command("agent.pauseAll", "Pause all agents"),
  command("edit.group", "Group", false),
];
const entries: PaletteEntry[] = [
  { kind: "layer", id: "step2", label: "Step 2 · Onboarding" },
  { kind: "page", id: "p2", label: "Settings" },
  { kind: "component", id: "c1", label: "Button / Primary" },
  { kind: "variable", id: "v1", label: "color/primary" },
  { kind: "session", id: "s1", label: "Onboarding screens" },
];
const setup = () => {
  const calls = { run: vi.fn(), open: vi.fn(), ask: vi.fn() };
  const palette = createPaletteController({
    commands: () => commands,
    entries: () => entries,
    ...calls,
  });
  return { palette, ...calls };
};

describe("command palette", () => {
  it("starts closed and opens with the query empty", () => {
    const { palette } = setup();
    expect(palette.state.open).toBe(false);
    palette.open();
    expect(palette.state).toMatchObject({ open: true, query: "" });
  });
  it("with no query offers commands that can run and no Ask row", () => {
    const { palette } = setup();
    palette.open();
    const kinds = palette.results().map((r) => r.kind);
    expect(kinds).not.toContain("ask");
    expect(palette.results().map((r) => r.label)).not.toContain("Group");
  });
  it("the first result is always Ask the agent with the query, whatever else matches", () => {
    const { palette } = setup();
    palette.open();
    palette.setQuery("auto");
    const [first, ...rest] = palette.results();
    expect(first).toMatchObject({ kind: "ask", label: `${ASK_PREFIX}auto` });
    expect(rest[0]).toMatchObject({ kind: "command", command: "edit.autoLayout" });
    palette.setQuery("zzz nothing matches");
    expect(palette.results()).toHaveLength(1);
    expect(palette.results()[0]?.label).toBe(`${ASK_PREFIX}zzz nothing matches`);
  });
  it("searches layers, pages, components, variables and sessions", () => {
    const { palette } = setup();
    palette.open();
    for (const [query, kind] of [
      ["onboarding", "layer"],
      ["settings", "page"],
      ["button", "component"],
      ["color", "variable"],
      ["screens", "session"],
    ] as const) {
      palette.setQuery(query);
      expect(palette.results().map((r) => r.kind)).toContain(kind);
    }
  });
  it("highlights the first real match, not the Ask row, and arrows can reach Ask", () => {
    const { palette } = setup();
    palette.open();
    palette.setQuery("pause");
    expect(palette.results()[palette.state.active]?.command).toBe("agent.pauseAll");
    palette.move(-1);
    expect(palette.results()[palette.state.active]?.kind).toBe("ask");
    palette.move(-1);
    expect(palette.state.active).toBe(palette.results().length - 1);
  });
  it("Enter runs the highlighted result and closes", () => {
    const { palette, run } = setup();
    palette.open();
    palette.setQuery("pause");
    palette.key("Enter");
    expect(run).toHaveBeenCalledWith("agent.pauseAll");
    expect(palette.state.open).toBe(false);
  });
  it("Ask sends the typed text to the composer and does not run a command", () => {
    const { palette, ask, run } = setup();
    palette.open();
    palette.setQuery("  make the cards rounder ");
    palette.choose(0);
    expect(ask).toHaveBeenCalledWith("make the cards rounder");
    expect(run).not.toHaveBeenCalled();
  });
  it("a layer result opens that entry", () => {
    const { palette, open } = setup();
    palette.open();
    palette.setQuery("step 2");
    const at = palette.results().findIndex((r) => r.kind === "layer");
    palette.choose(at);
    expect(open).toHaveBeenCalledWith(entries[0]);
  });
  it("Escape closes without running anything", () => {
    const { palette, run, ask } = setup();
    palette.open();
    palette.setQuery("auto");
    palette.key("Escape");
    expect(palette.state.open).toBe(false);
    expect(run).not.toHaveBeenCalled();
    expect(ask).not.toHaveBeenCalled();
  });
  it("matches every word in any order, case-insensitively, and ranks prefixes first", () => {
    const { palette } = setup();
    palette.open();
    palette.setQuery("LAYOUT ADD");
    expect(palette.results()[1]?.command).toBe("edit.autoLayout");
    palette.setQuery("a");
    const labels = palette.results().map((r) => r.label);
    // "Add auto-layout" starts with it; "Pause all agents" only has words that do.
    expect(labels.indexOf("Add auto-layout")).toBeLessThan(labels.indexOf("Pause all agents"));
    expect(labels.indexOf("Pause all agents")).toBeGreaterThan(0);
  });
  it("opening again starts a fresh search", () => {
    const { palette } = setup();
    palette.open();
    palette.setQuery("auto");
    palette.close();
    palette.open();
    expect(palette.state.query).toBe("");
  });
});
