import { brand } from "@vellum/config";
import { useState } from "react";
import { PortalContainer, usePortalContainer } from "../chrome/portal";
import { ResourceDialog } from "../resources/Dialog";
import "./connect.css";

export interface ConnectClient {
  id: string;
  name: string;
  state: "connected" | "configured" | "ready" | "missing" | "error";
  path: string;
  entry: string;
}
export interface ConnectAgentsProps {
  clients: readonly ConnectClient[];
  pending: boolean;
  result?: { path: string; entry: string; action: "connect" | "disconnect" };
  status?: string;
  error?: string;
  onAction(id: string, action: "connect" | "disconnect"): void;
  onCopy(text: string): void;
}
const states = {
  connected: "✓ Connected",
  configured: "○ Configured · restart client",
  ready: "○ Ready to connect",
  missing: "⊘ Not detected",
  error: "⚠ Check configuration",
};
export function ConnectAgentsBody(
  p: ConnectAgentsProps & { selected: string; onSelect(id: string): void },
) {
  const selected = p.clients.find((c) => c.id === p.selected) ?? p.clients[0];
  const middle = Math.ceil(p.clients.length / 2);
  return (
    <>
      <div className="vc-connect-columns">
        {[p.clients.slice(0, middle), p.clients.slice(middle)].map((column, i) => (
          <div className="vc-connect-column" key={i === 0 ? "first" : "second"}>
            {column.map((client) => (
              <div className="vc-connect-row" key={client.id}>
                <div className="vc-connect-identity">
                  <span>{client.name}</span>
                  <span className="vc-connect-state">{states[client.state]}</span>
                </div>
                <button
                  type="button"
                  className="vc-resource-button"
                  aria-label={`${client.state === "connected" || client.state === "configured" ? "Disconnect" : "Connect"} ${client.name}`}
                  disabled={p.pending || client.state === "error"}
                  onClick={() => {
                    p.onSelect(client.id);
                    p.onAction(
                      client.id,
                      client.state === "connected" || client.state === "configured"
                        ? "disconnect"
                        : "connect",
                    );
                  }}
                >
                  {client.state === "connected" || client.state === "configured"
                    ? "Disconnect"
                    : "Connect"}
                </button>
              </div>
            ))}
          </div>
        ))}
      </div>
      <p className="vc-connect-coverage">Codex covers the desktop app and IDE extension.</p>
      {p.result && (
        <div className="vc-connect-written" role="status">
          <p>
            {p.result.action === "connect" ? "Written to" : "Removed from"}{" "}
            <code>{p.result.path}</code>
          </p>
          <pre>{p.result.entry}</pre>
        </div>
      )}
      <section className="vc-connect-manual" aria-label="Manual setup">
        <label>
          Manual setup{" "}
          <select
            aria-label="Manual setup client"
            value={selected?.id ?? ""}
            onChange={(e) => p.onSelect(e.target.value)}
          >
            {p.clients.map((c) => (
              <option value={c.id} key={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </label>
        {selected && (
          <>
            <code>{selected.path}</code>
            <pre>{selected.entry}</pre>
            <button
              type="button"
              className="vc-resource-button"
              onClick={() => p.onCopy(selected.entry)}
            >
              Copy config
            </button>
          </>
        )}
      </section>
      {p.status && <p role="status">{p.status}</p>}
      {p.error && (
        <p role="alert" className="vc-connect-error">
          ⚠ {p.error}
        </p>
      )}
    </>
  );
}
export function ConnectAgentsDialog(p: ConnectAgentsProps & { onClose(): void }) {
  const [selected, onSelect] = useState(p.clients[0]?.id ?? "");
  const inherited = usePortalContainer();
  const container = inherited ?? (typeof document !== "undefined" ? document.body : null);
  return (
    <PortalContainer.Provider value={container}>
      <ResourceDialog
        title="Connect agents"
        description={`Connect writes only ${brand.name}’s MCP entry. Disconnect removes only that entry.`}
        width={740}
        className="vc-connect-dialog"
        onClose={p.onClose}
      >
        <ConnectAgentsBody {...p} selected={selected} onSelect={onSelect} />
      </ResourceDialog>
    </PortalContainer.Provider>
  );
}
