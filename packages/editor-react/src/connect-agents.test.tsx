// TEST DATA: C02 view states. Interactive acceptance runs in the Chrome lane.
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { ConnectAgentsBody } from "./agent/ConnectAgentsDialog";

it("names every client and distinguishes live, configured, missing and invalid data", () => {
  const html = renderToStaticMarkup(
    <ConnectAgentsBody
      clients={[
        { id: "a", name: "Claude Code", state: "connected", path: "/config", entry: "entry" },
        { id: "b", name: "Codex", state: "configured", path: "/config", entry: "entry" },
        { id: "c", name: "Kiro", state: "missing", path: "/config", entry: "entry" },
        { id: "d", name: "Cursor", state: "error", path: "/config", entry: "entry" },
      ]}
      selected="a"
      pending={false}
      onSelect={() => {}}
      onAction={() => {}}
      onCopy={() => {}}
    />,
  );
  for (const text of [
    "Claude Code",
    "Codex",
    "Kiro",
    "Cursor",
    "Connected",
    "Configured",
    "Not detected",
    "Check configuration",
    "Disconnect",
    "Manual setup",
  ])
    expect(html).toContain(text);
  expect(html).not.toContain("18 tools");
});
