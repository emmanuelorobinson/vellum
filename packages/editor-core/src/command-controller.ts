import { type BindingContext, Keybindings, type KeyEvent, keyboardBindings } from "./keybindings";
import { createRegistries } from "./registries";
import { TOOLS } from "./tool-types";

/** No document writes here. Composition supplies handlers that call EngineClient/Host/DaemonClient. */
export function createCommandController(
  handlers: Readonly<Record<string, (() => void | Promise<void>) | undefined>>,
) {
  const registries = createRegistries();
  const keys = new Keybindings();
  const seen = new Set<string>();
  const held = new Map<string, string>();
  const temporary: Readonly<Record<string, string | undefined>> = {
    "tool.holdHand": "tool.releaseHand",
    "tool.holdZoom": "tool.releaseZoom",
  };
  for (const binding of keyboardBindings) {
    if (seen.has(binding.command)) continue;
    seen.add(binding.command);
    registries.registerCommand({
      id: binding.command,
      title: binding.title,
      category: binding.category,
      enabled: () =>
        !!handlers[binding.command] &&
        (!temporary[binding.command] || !!handlers[temporary[binding.command] ?? ""]),
      run: () => handlers[binding.command]?.(),
    });
  }
  for (const id of ["tool.releaseHand", "tool.releaseZoom"])
    registries.registerCommand({
      id,
      title: id === "tool.releaseHand" ? "Release hand" : "Release zoom",
      category: "Tools",
      enabled: () => !!handlers[id],
      run: () => handlers[id]?.(),
    });
  for (const id of TOOLS) registries.registerTool({ id, title: id });
  for (const title of ["Agent", "Layers", "Assets", "History"])
    registries.registerPanel({ id: title.toLowerCase(), title, side: "left" });
  return {
    ...registries,
    keys,
    async keyDown(event: KeyEvent & { preventDefault?(): void }, context: BindingContext) {
      const binding = keys.fromEvent(event, context);
      if (!binding) return { status: "delegated" as const, preventDefault: false };
      const command = registries.commands.get(binding.command);
      // DOM event defaults must be cancelled synchronously, before awaiting a host or engine.
      if (command && (!command.enabled || command.enabled())) {
        event.preventDefault?.();
        const release = temporary[binding.command];
        if (release) held.set(event.code, release);
      }
      const result = await registries.execute(binding.command);
      return { ...result, preventDefault: result.status === "executed" };
    },
    async keyUp(event: Pick<KeyEvent, "code">) {
      const release = held.get(event.code);
      if (!release) return { status: "delegated" as const };
      held.delete(event.code);
      return registries.execute(release);
    },
    async blur() {
      const releases = [...held.values()].reverse();
      held.clear();
      for (const release of releases) await registries.execute(release);
    },
  };
}
