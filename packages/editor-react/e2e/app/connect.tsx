// TEST DATA: browser-only native-host fixture for C02. No real client configuration is read or written.
import { useState } from "react";
import { createRoot } from "react-dom/client";
import { ConnectAgentsDialog, type ConnectClient } from "../../src/agent/ConnectAgentsDialog";
import "@vellum/ui-tokens/tokens.css";
import "../../src/editor.css";
import "./fonts.css";

const appearance = new URLSearchParams(location.search).get("appearance") ?? "light";
document.documentElement.dataset.appearance = appearance;
const names = [
  "Claude Code",
  "Codex",
  "Cursor",
  "Windsurf",
  "VS Code Copilot",
  "Gemini CLI",
  "Antigravity",
  "OpenCode",
  "Kiro",
  "Claude Desktop",
];
function App() {
  const [clients, setClients] = useState<ConnectClient[]>(
    names.map((name, i) => ({
      id: String(i),
      name,
      state: i === 0 ? "connected" : i === 8 || i === 9 ? "missing" : "ready",
      path: "/test-home/client-config.json",
      entry: '{"mcpServers":{"vellum":{"command":"/test-bundle/vellum","args":["mcp"]}}}',
    })),
  );
  const [result, setResult] = useState<{
    path: string;
    entry: string;
    action: "connect" | "disconnect";
  }>();
  const [status, setStatus] = useState("");
  const [open, setOpen] = useState(true);
  return (
    <div className="vc-workspace">
      {open ? (
        <ConnectAgentsDialog
          clients={clients}
          result={result}
          pending={false}
          status={status}
          onAction={(id, action) => {
            setClients((rows) =>
              rows.map((c) =>
                c.id === id ? { ...c, state: action === "connect" ? "configured" : "ready" } : c,
              ),
            );
            const c = clients.find((c) => c.id === id);
            if (c) setResult({ ...c, action });
          }}
          onCopy={() => setStatus("Config copied")}
          onClose={() => setOpen(false)}
        />
      ) : (
        <button type="button" onClick={() => setOpen(true)}>
          Connect agents
        </button>
      )}
    </div>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
