/** Keyboard selection is local view state. Arrow keys still use the existing Person nudge commands. */
export interface AccessibleScene {
  roots(): readonly string[];
  get(id: string): { children: readonly string[]; parent: string | null } | null | undefined;
}

export function canvasNavigate(
  scene: AccessibleScene,
  selection: readonly string[],
  key: string,
  backwards = false,
): string[] | null {
  const id = selection[0];
  const item = id ? scene.get(id) : null;
  if (key === "Escape") return item?.parent ? [item.parent] : [];
  if (key === "Enter") return item?.children[0] ? [item.children[0]] : [...selection];
  if (key !== "Tab") return null;
  const siblings = item?.parent ? (scene.get(item.parent)?.children ?? []) : scene.roots();
  if (!siblings.length) return [];
  const at = id ? siblings.indexOf(id) : -1;
  const next =
    at < 0
      ? backwards
        ? siblings.length - 1
        : 0
      : (at + (backwards ? -1 : 1) + siblings.length) % siblings.length;
  return [siblings[next] as string];
}

/** Baseline stored history quietly; announce terminal transitions and newly arriving completed work. */
export function completionChanges(
  previous: ReadonlyMap<string, string> | null,
  next: ReadonlyMap<string, string>,
): string[] {
  if (!previous) return [];
  return [...next].flatMap(([key, state]) =>
    ["done", "failed", "stopped", "warning"].includes(state) && previous.get(key) !== state
      ? [key]
      : [],
  );
}
