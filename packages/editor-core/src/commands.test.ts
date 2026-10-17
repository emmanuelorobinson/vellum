import { describe, expect, it } from "vitest";
import { createCommandController, createRegistries, Keybindings, keyboardBindings } from "./index";

const context = { host: "desktop", platform: "mac", focus: "canvas" } as const;
describe("commands and shortcut dispatch", () => {
  it("releases rebound temporary tools after focus/modifier changes and blur", async () => {
    const actions: string[] = [];
    const controller = createCommandController({
      "tool.holdHand": () => {
        actions.push("hand");
      },
      "tool.releaseHand": () => {
        actions.push("release hand");
      },
      "tool.holdZoom": () => {
        actions.push("zoom");
      },
      "tool.releaseZoom": () => {
        actions.push("release zoom");
      },
    });
    controller.keys.rebind("tool.holdHand", "Alt+KeyQ", context);
    await controller.keyDown({ code: "KeyQ", altKey: true }, context);
    await controller.keyUp({ code: "KeyQ" });
    expect(actions).toEqual(["hand", "release hand"]);
    await controller.keyDown({ code: "KeyQ", altKey: true }, context);
    await controller.keyDown({ code: "KeyZ" }, context);
    await controller.blur();
    await controller.blur();
    expect(actions).toEqual([
      "hand",
      "release hand",
      "hand",
      "zoom",
      "release zoom",
      "release hand",
    ]);
  });
  it("cancels a browser default before an asynchronous command yields", async () => {
    let prevented = false;
    const controller = createCommandController({
      "tool.rect": async () => {
        expect(prevented).toBe(true);
      },
    });
    const result = controller.keyDown(
      {
        code: "KeyR",
        preventDefault: () => {
          prevented = true;
        },
      },
      context,
    );
    expect(prevented).toBe(true);
    await result;
  });
  it("registers, searches, runs and disposes through the same registries", async () => {
    const r = createRegistries();
    let calls = 0;
    const d = r.registerCommand({
      id: "custom",
      title: "Custom action",
      category: "Edit",
      run: () => {
        calls++;
      },
    });
    expect(r.commands.search("ACTION").map((c) => c.id)).toEqual(["custom"]);
    expect(await r.execute("custom")).toEqual({ status: "executed" });
    expect(calls).toBe(1);
    expect(() =>
      r.registerCommand({ id: "custom", title: "Duplicate", category: "Edit", run() {} }),
    ).toThrow();
    d.dispose();
    d.dispose();
    expect(await r.execute("custom")).toEqual({ status: "unavailable" });
    const t = r.registerTool({ id: "custom", title: "Tool" });
    const p = r.registerPanel({ id: "custom", title: "Panel", side: "left" });
    expect(r.tools.list()).toHaveLength(1);
    expect(r.panels.list()).toHaveLength(1);
    t.dispose();
    p.dispose();
    expect(r.tools.list()).toEqual([]);
    expect(r.panels.list()).toEqual([]);
  });
  it("does not claim disabled, absent or throwing handlers succeeded", async () => {
    const r = createRegistries();
    r.registerCommand({
      id: "disabled",
      title: "Disabled",
      category: "Edit",
      enabled: () => false,
      run() {
        throw Error("disabled");
      },
    });
    expect(await r.execute("disabled")).toEqual({ status: "disabled" });
    r.registerCommand({
      id: "bad",
      title: "Bad",
      category: "Edit",
      run() {
        throw Error("failed");
      },
    });
    await expect(r.execute("bad")).rejects.toThrow("failed");
  });
  it("honours IDE variants, reserved chords and focus", () => {
    const k = new Keybindings();
    expect(k.resolve("Mod+KeyK", context)?.command).toBe("agent.inline");
    const ide = { ...context, host: "vscode" } as const;
    expect(k.resolve("Mod+KeyK", ide)).toBeNull();
    expect(k.resolve("Alt+KeyK", ide)?.command).toBe("agent.inline");
    expect(k.resolve("Control+Tab", ide)).toBeNull();
    expect(k.resolve("Alt+BracketRight", ide)?.command).toBe("session.next");
    for (const chord of [
      "Mod+KeyP",
      "Mod+Shift+KeyP",
      "Mod+KeyB",
      "Mod+KeyW",
      "Mod+KeyS",
      "Mod+KeyJ",
    ])
      expect(k.resolve(chord, ide)).toBeNull();
    expect(k.resolve("Mod+KeyJ", { ...ide, focus: "path" })?.command).toBe("path.connect");
    expect(k.resolve("KeyR", { ...ide, focus: "panel" })).toBeNull();
    expect(k.resolve("Mod+KeyL", { ...ide, focus: "outside" })).toBeNull();
    expect(k.resolve("KeyR", { ...ide, focus: "text" })).toBeNull();
    expect(k.resolve("Mod+KeyL", { ...ide, focus: "panel" })?.command).toBe("agent.focus");
  });
  it("gives presentation, path and empty-composer commands precedence", () => {
    const k = new Keybindings();
    expect(k.resolve("Enter", { ...context, focus: "path" })?.command).toBe("path.finish");
    expect(k.resolve("Enter", { ...context, focus: "presentation" })?.command).toBe("present.next");
    expect(k.resolve("Enter", { ...context, focus: "composer" })?.command).toBe("composer.send");
    expect(k.resolve("ArrowUp", { ...context, focus: "composer" })).toBeNull();
    expect(k.resolve("ArrowUp", { ...context, focus: "composerEmpty" })?.command).toBe(
      "composer.previous",
    );
    expect(k.resolve("Mod+KeyA", { ...context, focus: "path" })?.command).toBe("path.selectAll");
    expect(k.resolve("Mod+KeyA", context)?.command).toBe("select.all");
  });
  it("normalizes physical events and platforms without swallowing text or composition", () => {
    const k = new Keybindings();
    expect(
      k.fromEvent({ code: "KeyC", ctrlKey: true }, { ...context, platform: "windows" })?.command,
    ).toBe("edit.copy");
    expect(k.fromEvent({ code: "KeyC", metaKey: true }, context)?.command).toBe("edit.copy");
    expect(k.fromEvent({ code: "KeyC", metaKey: true, isComposing: true }, context)).toBeNull();
    expect(k.fromEvent({ code: "Equal", metaKey: true, shiftKey: true }, context)?.command).toBe(
      "view.zoomIn",
    );
    expect(k.fromEvent({ code: "KeyR", repeat: true }, context)).toBeNull();
    expect(k.fromEvent({ code: "ArrowRight", repeat: true }, context)?.command).toBe(
      "edit.nudgeRight",
    );
    expect(k.fromEvent({ code: "KeyC", ctrlKey: true, metaKey: true }, context)).toBeNull();
    for (const focus of ["composer", "composerEmpty", "text"] as const) {
      expect(k.resolve("Tab", { ...context, focus })).toBeNull();
      expect(k.resolve("Mod+KeyZ", { ...context, focus })).toBeNull();
    }
  });
  it("rebinds transactionally, detects overlapping contexts and marks host conflicts", () => {
    const k = new Keybindings();
    expect(k.rebind("tool.rect", "KeyO", context).ok).toBe(false);
    expect(k.resolve("KeyR", context)?.command).toBe("tool.rect");
    expect(k.rebind("tool.rect", "Mod+KeyP", context).ok).toBe(false);
    expect(k.rebind("tool.rect", "Alt+KeyR", context).ok).toBe(true);
    expect(k.resolve("KeyR", context)).toBeNull();
    expect(k.resolve("Alt+KeyR", context)?.command).toBe("tool.rect");
    expect(k.rebind("tool.rect", null, context).ok).toBe(true);
    expect(k.resolve("Alt+KeyR", context)).toBeNull();
    k.reset("tool.rect", context);
    expect(k.resolve("KeyR", context)?.command).toBe("tool.rect");
    expect(
      k.search("save", { ...context, host: "vscode" }).find((b) => b.command === "file.save")
        ?.hostConflict,
    ).toBe(true);
    expect(k.rebind("tool.rect", "nonsense", context).ok).toBe(false);
    expect(k.rebind("unknown", "KeyQ", context).ok).toBe(false);
    expect(keyboardBindings.length).toBeGreaterThan(100);
  });
});
