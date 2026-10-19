export interface SessionRow {
  id: string;
  path: string;
  title: string;
  client: string;
  updatedAt: number;
}
export function sessionRows(rows: readonly SessionRow[]): SessionRow[] {
  return [...rows].sort((a, b) => b.updatedAt - a.updatedAt || a.id.localeCompare(b.id));
}
export class RecentDesigns {
  private values: string[] = [];
  constructor(saved: readonly unknown[]) {
    for (const value of [...saved].reverse()) if (typeof value === "string") this.open(value);
  }
  get items(): readonly string[] {
    return this.values;
  }
  open(uri: string) {
    try {
      const u = new URL(uri);
      if (u.protocol !== "file:" || !u.pathname.endsWith(".vellum")) return;
      this.values = [uri, ...this.values.filter((v) => v !== uri)].slice(0, 20);
    } catch {
      /* Ignore stale or unsupported stored resources. */
    }
  }
}
