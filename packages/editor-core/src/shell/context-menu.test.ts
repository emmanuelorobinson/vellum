import { describe, expect, it } from "vitest";
import { contextMenuItems } from "./index";

const all = () => true;
describe("layer context menu", () => {
  it("lists Ask about this… and Redesign this… first, then the design's rows, Delete last", () => {
    const labels = contextMenuItems({ count: 1, enabled: all }).map((i) => i.label);
    expect(labels).toEqual([
      "Ask about this…",
      "Redesign this…",
      "Copy",
      "Paste",
      "Paste here",
      "Group",
      "Frame selection",
      "Add auto-layout",
      "Create component",
      "Bring forward",
      "Copy as CSS",
      "Copy as PNG",
      "Copy node ID",
      "Delete",
    ]);
  });
  it("names the commands the controller runs and the shortcuts they show", () => {
    const items = contextMenuItems({ count: 1, enabled: all });
    const by = (label: string) => items.find((i) => i.label === label);
    expect(by("Copy")?.command).toBe("edit.copy");
    expect(by("Group")?.command).toBe("edit.group");
    expect(by("Add auto-layout")?.command).toBe("edit.autoLayout");
    expect(by("Create component")?.command).toBe("component.toggle");
    expect(by("Bring forward")?.command).toBe("arrange.forward");
    expect(by("Copy as PNG")?.command).toBe("edit.copyPng");
    expect(by("Delete")?.command).toBe("edit.delete");
    expect(by("Copy as CSS")?.command).toBe("edit.copyCss");
    expect(by("Copy node ID")?.command).toBe("edit.copyNodeId");
  });
  it("only Delete is destructive", () => {
    expect(
      contextMenuItems({ count: 2, enabled: all })
        .filter((i) => i.destructive)
        .map((i) => i.label),
    ).toEqual(["Delete"]);
  });
  it("greys out what cannot run, and a node ID needs exactly one layer", () => {
    const items = contextMenuItems({ count: 2, enabled: (c) => c !== "edit.group" });
    expect(items.find((i) => i.label === "Group")?.enabled).toBe(false);
    expect(items.find((i) => i.label === "Copy node ID")?.enabled).toBe(false);
    expect(items.find((i) => i.label === "Copy")?.enabled).toBe(true);
    expect(
      contextMenuItems({ count: 1, enabled: all }).find((i) => i.label === "Copy node ID")?.enabled,
    ).toBe(true);
  });
  it("a read-only file still copies and asks, and refuses every change", () => {
    const items = contextMenuItems({ count: 1, enabled: all, readOnly: true });
    const off = items.filter((i) => !i.enabled).map((i) => i.label);
    expect(off).toEqual([
      "Redesign this…",
      "Paste",
      "Paste here",
      "Group",
      "Frame selection",
      "Add auto-layout",
      "Create component",
      "Bring forward",
      "Delete",
    ]);
  });
  it("with nothing under the pointer nothing can run", () => {
    expect(contextMenuItems({ count: 0, enabled: all }).every((i) => !i.enabled)).toBe(true);
  });
  it("is separated into the design's groups", () => {
    const groups = contextMenuItems({ count: 1, enabled: all }).map((i) => i.group);
    expect(new Set(groups).size).toBe(7);
    expect([...groups].sort((a, b) => a - b)).toEqual(groups);
  });
});
