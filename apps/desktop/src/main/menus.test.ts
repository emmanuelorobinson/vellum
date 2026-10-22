import { MENU_CATEGORIES } from "@vellum/host-electron/bridge";
import { describe, expect, it } from "vitest";
import { menuModel } from "./menus";

describe("desktop menus", () => {
  it("keeps the required groups in order and disables unserved document actions", () => {
    const menu = menuModel({ hasFolder: true, hasDocument: true, daemon: true });
    expect(menu.map((group) => group.label)).toEqual(MENU_CATEGORIES);
    expect(menu[0]?.items.find((item) => item.id === "file.open")).toMatchObject({ enabled: true });
    expect(menu[0]?.items.find((item) => item.id === "file.save")).toMatchObject({
      enabled: false,
    });
    expect(menu[3]?.items.every((item) => !item.enabled)).toBe(true);
    expect(menu[5]?.items.find((item) => item.id === "agent.pauseAll")).toMatchObject({
      enabled: true,
    });
  });
  it("a live document can be saved, and Undo and Redo reach its editor", () => {
    const menu = menuModel({ hasFolder: true, hasDocument: true, daemon: true, editable: true });
    expect(menu[0]?.items.find((item) => item.id === "file.save")).toMatchObject({ enabled: true });
    const edit = menu[1]?.items ?? [];
    expect(edit.find((item) => item.id === "edit.undo")).toMatchObject({
      enabled: true,
      command: "edit.undo",
    });
    expect(edit.find((item) => item.id === "edit.redo")).toMatchObject({
      enabled: true,
      command: "edit.redo",
    });
  });
  it("allows Open a folder without an active document", () => {
    const menu = menuModel({ hasFolder: false, hasDocument: false, daemon: false });
    expect(menu[0]?.items.find((item) => item.id === "folder.open")?.enabled).toBe(true);
    expect(menu[0]?.items.find((item) => item.id === "file.open")?.enabled).toBe(false);
  });
});
