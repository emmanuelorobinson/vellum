// TEST DATA: a small resolved scene; keyboard navigation must never write to it.
import { describe, expect, it } from "vitest";
import { canvasNavigate, completionChanges } from "./accessibility";
import { createPreferences, FakePreferencesHost } from "./shell/preferences";
import { createWorkspace } from "./workspace";

const items = {
  frame: { children: ["a", "b"], parent: null },
  a: { children: [], parent: "frame" },
  b: { children: ["c"], parent: "frame" },
  c: { children: [], parent: "b" },
  other: { children: [], parent: null },
};
const scene = {
  roots: () => ["frame", "other"],
  get: (id: string) => items[id as keyof typeof items],
};
describe("canvas keyboard navigation", () => {
  it("cycles siblings in both directions, enters and leaves a frame", () => {
    expect(canvasNavigate(scene, [], "Tab")).toEqual(["frame"]);
    expect(canvasNavigate(scene, ["a"], "Tab")).toEqual(["b"]);
    expect(canvasNavigate(scene, ["a"], "Tab", true)).toEqual(["b"]);
    expect(canvasNavigate(scene, ["b"], "Tab")).toEqual(["a"]);
    expect(canvasNavigate(scene, ["frame"], "Enter")).toEqual(["a"]);
    expect(canvasNavigate(scene, ["c"], "Escape")).toEqual(["b"]);
    expect(canvasNavigate(scene, ["frame"], "Escape")).toEqual([]);
    expect(canvasNavigate(scene, ["a"], "ArrowRight")).toBeNull();
  });
  it("handles deletion, empty scenes and a leaf without losing selection", () => {
    expect(canvasNavigate(scene, ["deleted"], "Tab")).toEqual(["frame"]);
    expect(canvasNavigate(scene, ["a"], "Enter")).toEqual(["a"]);
    expect(canvasNavigate({ roots: () => [], get: () => undefined }, [], "Tab")).toEqual([]);
    expect(Object.keys(items)).toHaveLength(5);
  });
});
describe("completion announcements", () => {
  it("ignores history at attachment, announces transitions and new completions once", () => {
    const history = new Map([
      ["old", "done"],
      ["step", "working"],
      ["turn", "running"],
    ]);
    expect(completionChanges(null, history)).toEqual([]);
    const next = new Map([...history, ["step", "done"], ["turn", "done"], ["new", "failed"]]);
    expect(completionChanges(history, next)).toEqual(["step", "turn", "new"]);
    expect(completionChanges(next, next)).toEqual([]);
  });
});

it("swaps physical fit insets in RTL without reflecting document geometry", () => {
  const workspace = createWorkspace();
  workspace.resize({ width: 1280, height: 800 });
  workspace.setSelection(true);
  const before = workspace.layout();
  workspace.setDirection("rtl");
  expect(workspace.layout().canvasInsets).toEqual({
    left: before.canvasInsets.right,
    right: before.canvasInsets.left,
  });
  expect(workspace.layout().agent.rect).toEqual(before.agent.rect);
});

it("treats corrupt host preference storage as defaults", async () => {
  for (const value of [true, 5, "bad", []]) {
    const preferences = createPreferences(new FakePreferencesHost(value as never));
    await expect(preferences.load()).resolves.toBeUndefined();
    expect(preferences.state.language).toBe("en");
  }
});
