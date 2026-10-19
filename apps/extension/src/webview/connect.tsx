import { ConnectAgentsDialog, type ConnectAgentsProps } from "@vellum/editor-react";
import { createRoot } from "react-dom/client";
import "@vellum/ui-tokens/tokens.css";
import "./connect-host.css";

declare function acquireVsCodeApi(): { postMessage(value: unknown): void };
const api = acquireVsCodeApi();
const element = document.getElementById("root");
if (!element) throw Error("missing root");
const root = createRoot(element);
type State = Pick<ConnectAgentsProps, "clients" | "pending" | "result" | "status" | "error"> & {
  type: "connections";
  appearance: string;
  connection: string;
};
window.addEventListener("message", (event: MessageEvent<State>) => {
  const state = event.data;
  if (
    state?.type !== "connections" ||
    !Array.isArray(state.clients) ||
    !["light", "dark", "hc-light", "hc-dark"].includes(state.appearance)
  )
    return;
  document.documentElement.dataset.appearance = state.appearance;
  root.render(
    <ConnectAgentsDialog
      {...state}
      status={[state.status, state.connection].filter(Boolean).join(" · ")}
      onClose={() => api.postMessage({ type: "close" })}
      onAction={(id, action) => api.postMessage({ type: "change", id, action })}
      onCopy={(text) => {
        const client = state.clients.find((c) => c.entry === text);
        if (client) api.postMessage({ type: "copy", id: client.id });
      }}
    />,
  );
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      if (document.querySelector('[role="dialog"]')) api.postMessage({ type: "rendered" });
    }),
  );
});
api.postMessage({ type: "ready" });
