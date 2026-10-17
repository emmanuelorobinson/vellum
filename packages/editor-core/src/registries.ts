export interface Disposable {
  dispose(): void;
}
export interface Named {
  id: string;
  title: string;
}
export interface Command extends Named {
  category: string;
  enabled?(): boolean;
  run(): void | Promise<void>;
}
export interface ToolRegistration extends Named {}
export interface PanelRegistration extends Named {
  side: "left" | "right" | "bottom";
}

/** Editor-scoped, ordered registrations. A stale disposer cannot remove a newer registration. */
export class Registry<T extends Named> {
  private entries = new Map<string, T>();
  private listeners = new Set<() => void>();
  register(value: T): Disposable {
    if (!value.id.trim() || this.entries.has(value.id))
      throw Error(`Duplicate or empty registration: ${value.id}`);
    const entry = Object.freeze({ ...value });
    this.entries.set(value.id, entry);
    this.notify();
    return {
      dispose: () => {
        if (this.entries.get(value.id) === entry) {
          this.entries.delete(value.id);
          this.notify();
        }
      },
    };
  }
  get(id: string): T | undefined {
    return this.entries.get(id);
  }
  list(): readonly T[] {
    return [...this.entries.values()];
  }
  search(query: string): readonly T[] {
    const words = query.toLocaleLowerCase().trim().split(/\s+/);
    return this.list().filter((v) =>
      words.every((w) => `${v.id} ${v.title}`.toLocaleLowerCase().includes(w)),
    );
  }
  subscribe(listener: () => void): Disposable {
    this.listeners.add(listener);
    return {
      dispose: () => {
        this.listeners.delete(listener);
      },
    };
  }
  private notify() {
    for (const listener of [...this.listeners]) listener();
  }
}

export function createRegistries() {
  const tools = new Registry<ToolRegistration>();
  const commands = new Registry<Command>();
  const panels = new Registry<PanelRegistration>();
  return {
    tools,
    commands,
    panels,
    registerTool: (tool: ToolRegistration) => tools.register(tool),
    registerCommand: (command: Command) => commands.register(command),
    registerPanel: (panel: PanelRegistration) => panels.register(panel),
    async execute(id: string): Promise<{ status: "executed" | "unavailable" | "disabled" }> {
      const command = commands.get(id);
      if (!command) return { status: "unavailable" };
      if (command.enabled && !command.enabled()) return { status: "disabled" };
      await command.run();
      return { status: "executed" };
    },
  };
}
