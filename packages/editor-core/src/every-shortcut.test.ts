// Independently transcribed acceptance cases from DESIGN.md Keyboard. TEST DATA, no production table import.
import { describe, expect, it } from "vitest";
import { type BindingContext, createCommandController, type Focus, Keybindings } from "./index";

const cases: [Focus, string, string, string?][] = [];
function rows(focus: Focus, text: string) {
  for (const row of text.trim().split("\n")) {
    const [command, chord, ide] = row.trim().split(" ");
    if (!command || !chord) throw Error("Bad shortcut acceptance row");
    cases.push([focus, command, chord, ide]);
  }
}
rows(
  "canvas",
  `
agent.focus Mod+KeyL
agent.inline Mod+KeyK Alt+KeyK
agent.stop Mod+Period
agent.pauseAll Mod+Shift+Period
session.new Mod+Alt+KeyN
session.next Control+Tab Alt+BracketRight
session.previous Control+Shift+Tab Alt+BracketLeft
panel.agent Alt+Digit1
panel.layers Alt+Digit2
panel.assets Alt+Digit3
panel.history Alt+Digit4
palette.open Mod+Slash
tool.move KeyV
tool.scale KeyK
tool.frame KeyF
tool.rect KeyR
tool.ellipse KeyO
tool.line KeyL
tool.pen KeyP
tool.text KeyT
tool.note KeyN
tool.hand KeyH
tool.holdHand Space
tool.holdZoom KeyZ
edit.copy Mod+KeyC
edit.cut Mod+KeyX
edit.paste Mod+KeyV
edit.pasteOver Mod+Shift+KeyV
edit.copyPng Mod+Shift+KeyC
edit.copyHtml Mod+Alt+KeyC
edit.duplicate Mod+KeyD
edit.delete Delete
edit.nudgeLeft ArrowLeft
edit.nudgeRight ArrowRight
edit.nudgeUp ArrowUp
edit.nudgeDown ArrowDown
edit.bigNudgeLeft Shift+ArrowLeft
edit.bigNudgeRight Shift+ArrowRight
edit.bigNudgeUp Shift+ArrowUp
edit.bigNudgeDown Shift+ArrowDown
edit.group Mod+KeyG
edit.ungroup Mod+Shift+KeyG
edit.wrap Mod+Alt+KeyG
edit.autoLayout Shift+KeyA
edit.removeAutoLayout Alt+Shift+KeyA
edit.flipHorizontal Shift+KeyH
edit.flipVertical Shift+KeyV
edit.undo Mod+KeyZ
edit.redo Mod+Shift+KeyZ
arrange.left Alt+KeyA
arrange.right Alt+KeyD
arrange.top Alt+KeyW
arrange.bottom Alt+KeyS
arrange.horizontal Alt+KeyH
arrange.vertical Alt+KeyV
arrange.distributeHorizontal Control+Alt+KeyH
arrange.distributeVertical Control+Alt+KeyV
arrange.backward Mod+BracketLeft
arrange.forward Mod+BracketRight
arrange.back BracketLeft
arrange.front BracketRight
select.all Mod+KeyA
select.children Enter
select.parent Shift+Enter
select.clear Escape
select.click Click
select.deepest Mod+Click
select.marquee Mod+Drag
select.add Shift+Click
component.toggle Mod+Alt+KeyK
component.detach Mod+Alt+KeyX
component.replace Alt+Shift+KeyR
view.zoom Mod+Scroll
view.zoom Pinch
view.panHorizontal Shift+Scroll
view.pan MiddleDrag
view.zoomIn Mod+Equal
view.zoomOut Mod+Minus
view.actualSize Mod+Digit0
view.fitAll Shift+Digit1
view.fitSelection Shift+Digit2
view.pixelGrid Mod+Quote
view.snapPixelGrid Mod+Shift+Quote
view.hidePanels Tab
view.togglePanels Mod+Backslash
view.zoomClick ZoomClick
view.zoomClickOut Alt+ZoomClick
view.zoomRegion ZoomDrag
present.start Mod+Enter
file.save Mod+KeyS
file.saveAs Mod+Shift+KeyS
file.open Mod+KeyO
file.new Mod+Shift+KeyN
file.home Mod+KeyN
`,
);
rows(
  "composer",
  `composer.send Enter
composer.newline Shift+Enter`,
);
rows(
  "composerEmpty",
  `composer.previous ArrowUp
composer.next ArrowDown`,
);
rows(
  "path",
  `path.pen KeyP
path.select KeyV
path.cut KeyX
path.selectAll Mod+KeyA
path.connect Mod+KeyJ
path.toggleSmooth DoubleClickPoint
path.symmetric Mod+DragPoint
path.breakSymmetry Alt+DragHandle
path.bend Mod+DragSegment
path.bendFixed Mod+Alt+DragSegment
path.straighten Mod+DoubleClickSegment
path.addPoint Alt+ClickSegment
path.delete Delete
path.finish Enter
path.finish Escape`,
);
rows(
  "presentation",
  `present.previous ArrowLeft
present.next ArrowRight
present.next Space
present.next Enter
present.stop Escape`,
);
describe("every DESIGN.md Keyboard shortcut", () => {
  const keys = new Keybindings();
  for (const host of ["desktop", "vscode", "browser", "electron"] as const)
    for (const platform of ["mac", "windows", "linux"] as const) {
      for (const [focus, command, chord, ide] of cases)
        it(`${host} ${platform} ${focus}: ${command} ${chord}`, () => {
          const delegated =
            command === "file.save" ||
            (command.startsWith("file.") && (host === "vscode" || host === "browser"));
          const resolved = keys.resolve(host === "vscode" && ide ? ide : chord, {
            host,
            platform,
            focus,
          });
          expect(resolved?.command ?? null).toBe(delegated ? null : command);
        });
    }
  it("dispatches through registered built-ins and leaves absent features disabled", async () => {
    let calls = 0;
    const c = createCommandController({
      "tool.rect": () => {
        calls++;
      },
    });
    const context: BindingContext = { host: "desktop", platform: "mac", focus: "canvas" };
    expect(await c.keyDown({ code: "KeyR" }, context)).toEqual({
      status: "executed",
      preventDefault: true,
    });
    expect(calls).toBe(1);
    expect(await c.keyDown({ code: "KeyO" }, context)).toEqual({
      status: "disabled",
      preventDefault: false,
    });
    expect(await c.keyDown({ code: "KeyS", metaKey: true }, context)).toEqual({
      status: "delegated",
      preventDefault: false,
    });
  });
});
