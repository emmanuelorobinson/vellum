import { describe, expect, it } from "vitest";
import { Keybindings } from "../keybindings";
import {
  createPreferences,
  createSettingsController,
  DEFAULT_PREFERENCES,
  FakePreferencesHost,
  TELEMETRY_DISCLOSURE,
} from "./index";

const ctx = { host: "desktop", platform: "mac", focus: "canvas" } as const;
describe("preferences", () => {
  it("start at the documented defaults", () => {
    const p = createPreferences(new FakePreferencesHost());
    expect(p.state).toEqual(DEFAULT_PREFERENCES);
    expect(p.get("telemetry")).toBe(false);
    expect(p.get("renderer")).toBe("automatic");
    expect(p.get("appearance.theme")).toBe("follow");
    expect(p.get("canvas.nudgeSmall")).toBe(1);
    expect(p.get("canvas.nudgeBig")).toBe(10);
    expect(p.get("agents.insertHighlight")).toBe(true);
    expect(p.get("permissions.repoReads")).toBe("ask");
  });
  it("persist every change through the host and read back what was saved", async () => {
    const host = new FakePreferencesHost();
    const p = createPreferences(host);
    p.set("canvas.pixelGrid", true);
    p.set("canvas.nudgeBig", 20);
    expect(host.saved.at(-1)).toMatchObject({ "canvas.pixelGrid": true, "canvas.nudgeBig": 20 });
    const again = createPreferences(host);
    await again.load();
    expect(again.get("canvas.pixelGrid")).toBe(true);
    expect(again.get("canvas.nudgeBig")).toBe(20);
  });
  it("refuse a value of the wrong kind or out of range and say so", () => {
    const p = createPreferences(new FakePreferencesHost());
    expect(p.set("canvas.nudgeSmall", 0)).toEqual({
      ok: false,
      reason: "Enter a whole number from 1 to 100",
    });
    expect(p.set("canvas.nudgeBig", 1.5).ok).toBe(false);
    expect(p.set("renderer", "fast" as never).ok).toBe(false);
    expect(p.set("canvas.backdrop", "red").ok).toBe(false);
    expect(p.get("canvas.nudgeSmall")).toBe(1);
    expect(p.set("canvas.backdrop", "#ededeb")).toEqual({ ok: true });
  });
  it("drop saved values it does not know instead of trusting them", async () => {
    const host = new FakePreferencesHost({
      "canvas.nudgeBig": "lots",
      renderer: "basic",
      extra: 1,
    } as never);
    const p = createPreferences(host);
    await p.load();
    expect(p.get("canvas.nudgeBig")).toBe(10);
    expect(p.get("renderer")).toBe("basic");
  });
  it("reset one key to its default and notify subscribers", () => {
    const p = createPreferences(new FakePreferencesHost());
    let n = 0;
    p.subscribe(() => n++);
    p.set("renderer", "basic");
    p.reset("renderer");
    expect(p.get("renderer")).toBe("automatic");
    expect(n).toBe(2);
  });
});

describe("telemetry", () => {
  it("is off until the person turns it on and discloses exactly what is sent", () => {
    const p = createPreferences(new FakePreferencesHost());
    expect(p.get("telemetry")).toBe(false);
    expect(TELEMETRY_DISCLOSURE).toEqual([
      "Off by default",
      "If on: version, OS, renderer failures, anonymous feature counts.",
      "Never files, prompts, keys or repo paths.",
    ]);
  });
});

describe("keyboard shortcuts in settings", () => {
  const settings = () =>
    createSettingsController({
      preferences: createPreferences(new FakePreferencesHost()),
      keys: new Keybindings(),
      context: ctx,
    });
  it("lists every shortcut once, searchable, with host conflicts marked", () => {
    const s = settings();
    const rows = s.shortcuts("");
    expect(rows.length).toBeGreaterThan(80);
    const palette = rows.find((r) => r.command === "palette.open");
    expect(palette?.chord).toBe("Mod+Slash");
    expect(rows.find((r) => r.command === "file.save")?.hostConflict).toBe(true);
    expect(s.shortcuts("palette").every((r) => /palette/i.test(`${r.title} ${r.command}`))).toBe(
      true,
    );
  });
  it("a rebind onto a taken chord is refused and names the other command", () => {
    const s = settings();
    const result = s.rebind("agent.stop", "Mod+KeyL");
    expect(result).toEqual({ ok: false, conflicts: ["agent.focus"] });
    expect(s.shortcuts("Stop current session")[0]?.chord).toBe("Mod+Period");
  });
  it("names the command that holds a taken chord, even when the list is filtered", () => {
    const s = settings();
    expect(s.titleOf("agent.focus")).toBe("Focus composer");
    s.shortcuts("Stop current session");
    expect(s.titleOf("agent.focus")).toBe("Focus composer");
    expect(s.titleOf("not.a.command")).toBe("not.a.command");
  });
  it("a rebind onto a free chord sticks, and reset puts it back", () => {
    const s = settings();
    expect(s.rebind("agent.stop", "Mod+Alt+KeyY").ok).toBe(true);
    expect(s.shortcuts("Stop current session")[0]?.chord).toBe("Mod+Alt+KeyY");
    s.resetShortcut("agent.stop");
    expect(s.shortcuts("Stop current session")[0]?.chord).toBe("Mod+Period");
  });
  it("refuses a chord the host keeps", () => {
    expect(settings().rebind("agent.stop", "Mod+KeyP")).toEqual({ ok: false, conflicts: ["host"] });
  });
});

it("image provider settings persist a configured provider and can return to unconfigured", async () => {
  const host = new FakePreferencesHost();
  const p = createPreferences(host);
  expect(p.set("assets.imageProvider", "configured")).toEqual({ ok: true });
  expect(p.set("assets.imageModel", "fixture")).toEqual({ ok: true });
  const again = createPreferences(host);
  await again.load();
  expect(again.get("assets.imageProvider")).toBe("configured");
  again.reset("assets.imageProvider");
  expect(again.get("assets.imageProvider")).toBe("none");
});
