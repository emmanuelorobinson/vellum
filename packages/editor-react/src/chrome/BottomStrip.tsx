import { copy } from "@vellum/config";
import { Icon } from "../icons/Icon";

export type Connection =
  | { mode: "live"; agents: number }
  | { mode: "headless" }
  | { mode: "offline"; note?: string };

export function connectionLabel(c: Connection): string {
  switch (c.mode) {
    case "live":
      return `Live · ${c.agents} ${c.agents === 1 ? "agent" : "agents"}`;
    case "headless":
      return "Headless only";
    case "offline":
      return copy.daemonOffline;
  }
}

/** Git branch, connection mode and, when it matters, the renderer note: quiet, centred, floating. */
export function BottomStrip({
  centerX,
  y,
  branch,
  dirty,
  connection,
  basicRenderer,
}: {
  centerX: number;
  y: number;
  branch: string | null;
  dirty: boolean;
  connection: Connection;
  basicRenderer: boolean;
}) {
  return (
    <section
      className="vc-bar vc-strip"
      style={{ left: centerX, top: y }}
      aria-label="Status"
      data-connection={connection.mode}
    >
      {branch ? (
        <span className="vc-strip__branch">
          <Icon name="git-branch" size={14} />
          <span className="vc-mono">{branch}</span>
          {dirty ? (
            <span
              className="vc-strip__dirty"
              role="img"
              aria-label="Uncommitted changes"
              title="Uncommitted changes"
            />
          ) : null}
        </span>
      ) : null}
      <span
        className="vc-strip__mode"
        data-attention={connection.mode === "offline" ? "" : undefined}
      >
        {connectionLabel(connection)}
      </span>
      {connection.mode === "offline" && connection.note ? (
        <span className="vc-strip__mode" data-attention="">
          {connection.note}
        </span>
      ) : null}
      {basicRenderer ? <span className="vc-strip__mode">{copy.basicRenderer}</span> : null}
    </section>
  );
}
