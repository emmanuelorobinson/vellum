import { type BindingContext, keyboardBindings } from "@vellum/editor-core";

const MAC: Record<string, string> = { Mod: "⌘", Alt: "⌥", Shift: "⇧", Control: "⌃", Meta: "⌘" };
const OTHER: Record<string, string> = {
  Mod: "Ctrl",
  Alt: "Alt",
  Shift: "Shift",
  Control: "Ctrl",
  Meta: "Win",
};
const KEYS: Record<string, string> = {
  Backslash: "\\",
  Slash: "/",
  Period: ".",
  Quote: "'",
  Equal: "=",
  Minus: "−",
  BracketLeft: "[",
  BracketRight: "]",
  Space: "Space",
};

/** "Mod+Alt+KeyK" → "⌥⌘K" on a Mac, "Ctrl+Alt+K" elsewhere. Null when the command has no key. */
export function shortcutLabel(
  command: string,
  context: Pick<BindingContext, "platform" | "host">,
): string | null {
  const binding = keyboardBindings.find((b) => b.command === command);
  if (!binding) return null;
  const chord = context.host === "vscode" && binding.ide ? binding.ide : binding.chord;
  const mac = context.platform === "mac";
  const parts = chord.split("+");
  const key = parts.pop() as string;
  const text = KEYS[key] ?? key.replace(/^(Key|Digit)/, "");
  if (mac) {
    const order = ["Control", "Alt", "Shift", "Mod", "Meta"];
    const mods = parts.sort((a, b) => order.indexOf(a) - order.indexOf(b)).map((m) => MAC[m] ?? m);
    return mods.join("") + text;
  }
  return [...parts.map((m) => OTHER[m] ?? m), text].join("+");
}
