import type { Appearance } from "@vellum/host-api";
export function appearance(kind: number): Appearance {
  switch (kind) {
    case 1:
      return "light";
    case 3:
      return "hc-dark";
    case 4:
      return "hc-light";
    default:
      return "dark";
  }
}
export type WebviewMessage =
  | { type: "ready" }
  | {
      type: "command";
      command: "save" | "openAsText" | "pauseAll" | "restartDaemon" | "connectAgents";
    };
export function parseMessage(value: unknown): WebviewMessage | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const v = value as Record<string, unknown>;
  const keys = Object.keys(v).sort().join(",");
  if (keys === "type" && v.type === "ready") return { type: "ready" };
  if (
    keys === "command,type" &&
    v.type === "command" &&
    ["save", "openAsText", "pauseAll", "restartDaemon", "connectAgents"].includes(String(v.command))
  )
    return v as WebviewMessage;
  return null;
}
const htmlEscape = (v: string) =>
  v
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
export function webviewHtml(options: {
  source: string;
  script: string;
  style: string;
  nonce: string;
  title: string;
}): string {
  const { source, script, style, nonce, title } = Object.fromEntries(
    Object.entries(options).map(([k, v]) => [k, htmlEscape(v)]),
  ) as typeof options;
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; connect-src 'none'; script-src 'nonce-${nonce}'; style-src ${source}; style-src-attr 'unsafe-inline'; img-src ${source} data: blob:; font-src ${source}; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none';"><meta name="viewport" content="width=device-width, initial-scale=1"><link rel="stylesheet" href="${style}"><title>${title}</title></head><body><div id="root"></div><script nonce="${nonce}" src="${script}"></script></body></html>`;
}
