export type EditorHost = "vscode" | "electron" | "desktop" | "browser";
export type Platform = "mac" | "windows" | "linux";
export type Focus =
  | "canvas"
  | "path"
  | "panel"
  | "composer"
  | "composerEmpty"
  | "text"
  | "presentation"
  | "outside";
type Scope = "editor" | "global" | Exclude<Focus, "outside" | "text" | "panel">;
export interface BindingContext {
  host: EditorHost;
  platform: Platform;
  focus: Focus;
}
export interface Binding {
  command: string;
  title: string;
  category: string;
  chord: string;
  scope: Scope;
  ide?: string | null;
  desktopOnly?: boolean;
  repeat?: boolean;
}
const bindings: Binding[] = [];
function add(
  category: string,
  scope: Scope,
  rows: readonly (readonly [string, string, string, (string | null)?])[],
) {
  for (const [command, title, chord, ide] of rows)
    bindings.push({
      command,
      title,
      category,
      chord,
      scope,
      ...(ide === undefined ? {} : { ide }),
    });
}
add("Agents", "global", [
  ["agent.focus", "Focus composer", "Mod+KeyL"],
  ["agent.inline", "Inline prompt on selection", "Mod+KeyK", "Alt+KeyK"],
  ["agent.stop", "Stop current session", "Mod+Period"],
  ["agent.pauseAll", "Pause all agents", "Mod+Shift+Period"],
  ["session.new", "New session", "Mod+Alt+KeyN"],
  ["session.next", "Next session", "Control+Tab", "Alt+BracketRight"],
  ["session.previous", "Previous session", "Control+Shift+Tab", "Alt+BracketLeft"],
  ["panel.agent", "Agent panel", "Alt+Digit1"],
  ["panel.layers", "Layers panel", "Alt+Digit2"],
  ["panel.assets", "Assets panel", "Alt+Digit3"],
  ["panel.history", "History panel", "Alt+Digit4"],
  ["palette.open", "Command palette", "Mod+Slash"],
]);
add("Composer", "composer", [
  ["composer.send", "Send prompt", "Enter"],
  ["composer.newline", "New line", "Shift+Enter"],
]);
add("Composer", "composerEmpty", [
  ["composer.previous", "Previous prompt", "ArrowUp"],
  ["composer.next", "Next prompt", "ArrowDown"],
]);
add("Tools", "canvas", [
  ["tool.move", "Move", "KeyV"],
  ["tool.scale", "Scale", "KeyK"],
  ["tool.frame", "Frame", "KeyF"],
  ["tool.rect", "Rectangle", "KeyR"],
  ["tool.ellipse", "Ellipse", "KeyO"],
  ["tool.line", "Line", "KeyL"],
  ["tool.pen", "Pen", "KeyP"],
  ["tool.text", "Text", "KeyT"],
  ["tool.note", "Note", "KeyN"],
  ["tool.hand", "Hand", "KeyH"],
  ["tool.holdHand", "Hold hand", "Space"],
  ["tool.holdZoom", "Hold zoom", "KeyZ"],
]);
add("Edit", "canvas", [
  ["edit.copy", "Copy", "Mod+KeyC"],
  ["edit.cut", "Cut", "Mod+KeyX"],
  ["edit.paste", "Paste", "Mod+KeyV"],
  ["edit.pasteOver", "Paste over selection", "Mod+Shift+KeyV"],
  ["edit.copyPng", "Copy as PNG", "Mod+Shift+KeyC"],
  ["edit.copyHtml", "Copy as HTML", "Mod+Alt+KeyC"],
  ["edit.duplicate", "Duplicate", "Mod+KeyD"],
  ["edit.delete", "Delete", "Delete"],
  ["edit.delete", "Delete", "Backspace"],
  ["edit.group", "Group", "Mod+KeyG"],
  ["edit.ungroup", "Ungroup", "Mod+Shift+KeyG"],
  ["edit.wrap", "Wrap in frame", "Mod+Alt+KeyG"],
  ["edit.autoLayout", "Add auto-layout", "Shift+KeyA"],
  ["edit.removeAutoLayout", "Remove auto-layout", "Alt+Shift+KeyA"],
  ["edit.flipHorizontal", "Flip horizontally", "Shift+KeyH"],
  ["edit.flipVertical", "Flip vertically", "Shift+KeyV"],
]);
add("Edit", "editor", [
  ["edit.undo", "Undo your changes", "Mod+KeyZ"],
  ["edit.redo", "Redo your changes", "Mod+Shift+KeyZ"],
]);
for (const direction of ["Left", "Right", "Up", "Down"]) {
  add("Edit", "canvas", [
    [
      `edit.nudge${direction}`,
      `Nudge ${direction.toLowerCase()} or reorder in auto-layout`,
      `Arrow${direction}`,
    ],
    [
      `edit.bigNudge${direction}`,
      `Big nudge ${direction.toLowerCase()}`,
      `Shift+Arrow${direction}`,
    ],
  ]);
  add("Path", "path", [
    [`path.nudge${direction}`, `Nudge points ${direction.toLowerCase()}`, `Arrow${direction}`],
    [
      `path.bigNudge${direction}`,
      `Big nudge points ${direction.toLowerCase()}`,
      `Shift+Arrow${direction}`,
    ],
  ]);
}
add("Arrange", "canvas", [
  ["arrange.left", "Align left", "Alt+KeyA"],
  ["arrange.right", "Align right", "Alt+KeyD"],
  ["arrange.top", "Align top", "Alt+KeyW"],
  ["arrange.bottom", "Align bottom", "Alt+KeyS"],
  ["arrange.horizontal", "Align horizontal centres", "Alt+KeyH"],
  ["arrange.vertical", "Align vertical centres", "Alt+KeyV"],
  ["arrange.distributeHorizontal", "Distribute horizontally", "Control+Alt+KeyH"],
  ["arrange.distributeVertical", "Distribute vertically", "Control+Alt+KeyV"],
  ["arrange.backward", "Send backward", "Mod+BracketLeft"],
  ["arrange.forward", "Bring forward", "Mod+BracketRight"],
  ["arrange.back", "Send to back", "BracketLeft"],
  ["arrange.front", "Bring to front", "BracketRight"],
]);
add("Select", "canvas", [
  ["select.all", "Select all", "Mod+KeyA"],
  ["select.children", "Select children or edit shape path", "Enter"],
  ["select.parent", "Select parent", "Shift+Enter"],
  ["select.clear", "Clear selection", "Escape"],
  ["select.click", "Select layer", "Click"],
  ["select.deepest", "Select deepest layer", "Mod+Click"],
  ["select.marquee", "Marquee inside frames", "Mod+Drag"],
  ["select.add", "Add to selection", "Shift+Click"],
]);
add("Components", "canvas", [
  ["component.toggle", "Make component or turn back into a layer", "Mod+Alt+KeyK"],
  ["component.detach", "Detach instance", "Mod+Alt+KeyX"],
  ["component.replace", "Replace inside instance", "Alt+Shift+KeyR"],
]);
add("Path", "path", [
  ["path.pen", "Pen", "KeyP"],
  ["path.select", "Select points", "KeyV"],
  ["path.cut", "Cut segment", "KeyX"],
  ["path.selectAll", "Select all points", "Mod+KeyA"],
  ["path.connect", "Connect points", "Mod+KeyJ"],
  ["path.toggleSmooth", "Toggle smooth or corner", "DoubleClickPoint"],
  ["path.symmetric", "Symmetric handles", "Mod+DragPoint"],
  ["path.breakSymmetry", "Break handle symmetry", "Alt+DragHandle"],
  ["path.bend", "Bend segment", "Mod+DragSegment"],
  ["path.bendFixed", "Bend keeping neighbours fixed", "Mod+Alt+DragSegment"],
  ["path.straighten", "Straighten segment", "Mod+DoubleClickSegment"],
  ["path.addPoint", "Add point", "Alt+ClickSegment"],
  ["path.delete", "Delete points, segments or handles", "Delete"],
  ["path.delete", "Delete points, segments or handles", "Backspace"],
  ["path.finish", "Finish path editing", "Enter"],
  ["path.finish", "Finish path editing", "Escape"],
]);
add("View", "canvas", [
  ["view.zoom", "Zoom", "Mod+Scroll"],
  ["view.zoom", "Pinch zoom", "Pinch"],
  ["view.panHorizontal", "Pan sideways", "Shift+Scroll"],
  ["view.pan", "Pan", "MiddleDrag"],
  ["view.zoomIn", "Zoom in", "Mod+Equal"],
  ["view.zoomIn", "Zoom in", "Mod+Shift+Equal"],
  ["view.zoomOut", "Zoom out", "Mod+Minus"],
  ["view.actualSize", "100%", "Mod+Digit0"],
  ["view.fitAll", "Fit all", "Shift+Digit1"],
  ["view.fitSelection", "Fit selection", "Shift+Digit2"],
  ["view.pixelGrid", "Pixel grid", "Mod+Quote"],
  ["view.snapPixelGrid", "Snap to pixel grid", "Mod+Shift+Quote"],
  ["view.zoomClick", "Held zoom: click in", "ZoomClick"],
  ["view.zoomClickOut", "Held zoom: click out", "Alt+ZoomClick"],
  ["view.zoomRegion", "Held zoom: region", "ZoomDrag"],
]);
add("View", "editor", [
  ["view.hidePanels", "Hide all panels", "Tab"],
  ["view.togglePanels", "Toggle panels", "Mod+Backslash"],
]);
add("Present", "canvas", [["present.start", "Present slides page", "Mod+Enter"]]);
add("Present", "presentation", [
  ["present.previous", "Previous slide", "ArrowLeft"],
  ["present.next", "Next slide", "ArrowRight"],
  ["present.next", "Next slide", "Space"],
  ["present.next", "Next slide", "Enter"],
  ["present.stop", "Stop presenting", "Escape"],
]);
add("Files", "editor", [
  ["file.save", "Save through host", "Mod+KeyS"],
  ["file.saveAs", "Save as", "Mod+Shift+KeyS"],
  ["file.open", "Open file", "Mod+KeyO"],
  ["file.new", "New file", "Mod+Shift+KeyN"],
  ["file.home", "Home", "Mod+KeyN"],
]);
for (const b of bindings) {
  if (b.category === "Files") b.desktopOnly = true;
  if (/^(edit|path)\.(bigNudge|nudge)|^view\.zoom(In|Out)$/.test(b.command)) b.repeat = true;
}
export const keyboardBindings: readonly Readonly<Binding>[] = Object.freeze(
  bindings.map((b) => Object.freeze(b)),
);

const modifiers = ["Mod", "Control", "Alt", "Shift", "Meta"];
const keys =
  /^(Key[A-Z]|Digit[0-9]|Arrow(Left|Right|Up|Down)|Enter|Tab|Space|Escape|Delete|Backspace|BracketLeft|BracketRight|Backslash|Slash|Period|Quote|Equal|Minus|Click|Drag|DoubleClickPoint|DragPoint|DragHandle|DragSegment|DoubleClickSegment|ClickSegment|Scroll|Pinch|MiddleDrag|ZoomClick|ZoomDrag)$/;
export function canonicalChord(chord: string, platform: Platform): string | null {
  const pieces = chord.split("+");
  const key = pieces.pop();
  if (
    !key ||
    !keys.test(key) ||
    pieces.some((m) => !modifiers.includes(m)) ||
    new Set(pieces).size !== pieces.length
  )
    return null;
  const physical = pieces.map((m) => (m === "Mod" ? (platform === "mac" ? "Meta" : "Control") : m));
  return [...new Set(physical)].sort().concat(key).join("+");
}
const focuses: Record<Scope, readonly Focus[]> = {
  editor: ["canvas", "path", "panel"],
  global: ["canvas", "path", "panel", "composer", "composerEmpty", "text"],
  canvas: ["canvas"],
  path: ["path"],
  composer: ["composer", "composerEmpty"],
  composerEmpty: ["composerEmpty"],
  presentation: ["presentation"],
};
function reserved(chord: string, context: BindingContext) {
  const chords = ["Mod+KeyP", "Mod+Shift+KeyP", "Mod+KeyB", "Mod+KeyW", "Mod+KeyS"];
  if (context.focus !== "path") chords.push("Mod+KeyJ");
  if (context.host === "vscode") chords.push("Mod+KeyK", "Control+Tab", "Control+Shift+Tab");
  return chords.some((c) => canonicalChord(c, context.platform) === chord);
}
export interface KeyEvent {
  code: string;
  metaKey?: boolean;
  ctrlKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
  repeat?: boolean;
  isComposing?: boolean;
}
export class Keybindings {
  private overrides = new Map<string, string | null>();
  private key(command: string, c: BindingContext) {
    return `${c.host}/${c.platform}/${command}`;
  }
  list(c: BindingContext): (Binding & { hostConflict: boolean })[] {
    const seen = new Set<string>();
    return keyboardBindings.flatMap((b) => {
      const key = this.key(b.command, c);
      const override = this.overrides.get(key);
      if (this.overrides.has(key)) {
        if (seen.has(key) || override === null) return [];
        seen.add(key);
      }
      const chord = override ?? (c.host === "vscode" && b.ide !== undefined ? b.ide : b.chord);
      if (chord === null) return [];
      return [
        {
          ...b,
          chord,
          hostConflict:
            reserved(canonicalChord(chord, c.platform) ?? "", c) ||
            !!(b.desktopOnly && c.host !== "desktop" && c.host !== "electron"),
        },
      ];
    });
  }
  resolve(chord: string, c: BindingContext): Binding | null {
    if (c.focus === "outside") return null;
    const normalized = canonicalChord(chord, c.platform);
    if (!normalized || reserved(normalized, c)) return null;
    return (
      this.list(c)
        .filter(
          (b) =>
            !b.hostConflict &&
            focuses[b.scope].includes(c.focus) &&
            canonicalChord(b.chord, c.platform) === normalized,
        )
        .sort((a, b) => focuses[a.scope].length - focuses[b.scope].length)[0] ?? null
    );
  }
  fromEvent(e: KeyEvent, c: BindingContext): Binding | null {
    if (e.isComposing) return null;
    const chord = [
      e.metaKey && "Meta",
      e.ctrlKey && "Control",
      e.altKey && "Alt",
      e.shiftKey && "Shift",
      e.code,
    ]
      .filter(Boolean)
      .join("+");
    const binding = this.resolve(chord, c);
    return e.repeat && !binding?.repeat ? null : binding;
  }
  search(query: string, c: BindingContext) {
    const words = query.toLocaleLowerCase().trim().split(/\s+/);
    return this.list(c).filter((b) =>
      words.every((w) =>
        `${b.command} ${b.title} ${b.category} ${b.chord}`.toLocaleLowerCase().includes(w),
      ),
    );
  }
  rebind(
    command: string,
    chord: string | null,
    c: BindingContext,
  ): { ok: boolean; conflicts: string[] } {
    const source = keyboardBindings.find((b) => b.command === command);
    const normalized = chord === null ? null : canonicalChord(chord, c.platform);
    if (!source || (chord !== null && !normalized)) return { ok: false, conflicts: ["invalid"] };
    const conflicts =
      normalized === null
        ? []
        : this.list(c)
            .filter(
              (b) =>
                b.command !== command &&
                canonicalChord(b.chord, c.platform) === normalized &&
                focuses[source.scope].some((f) => focuses[b.scope].includes(f)),
            )
            .map((b) => b.command);
    // Reserved host keys stay delegated even if the rebind is made from a different focus.
    if (
      normalized &&
      reserved(normalized, { ...c, focus: source.scope === "path" ? "path" : "canvas" })
    )
      conflicts.push("host");
    if (source.desktopOnly && c.host !== "desktop" && c.host !== "electron") conflicts.push("host");
    if (conflicts.length) return { ok: false, conflicts };
    this.overrides.set(this.key(command, c), chord);
    return { ok: true, conflicts: [] };
  }
  reset(command: string, c: BindingContext): void {
    this.overrides.delete(this.key(command, c));
  }
}
