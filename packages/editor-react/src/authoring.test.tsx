// TEST DATA: diagnostics from the host, never JavaScript evaluated in the UI.
import { renderToStaticMarkup as html } from "react-dom/server";
import { expect, it } from "vitest";
import { ScriptDiagnostic } from "./authoring/ScriptSection";

it("script error contains the same message, line and excerpt in the panel and on the node", () => {
  const view = {
    status: "error" as const,
    inputs: [],
    message: "values.map is not a function",
    line: 18,
    excerpt: "18  values.map(drawBar)",
  };
  const panel = html(<ScriptDiagnostic view={view} src="charts/weekly.js" />);
  const node = html(<ScriptDiagnostic view={view} src="charts/weekly.js" canvas />);
  for (const out of [panel, node]) {
    expect(out).toContain("values.map is not a function");
    expect(out).toContain("line 18");
    expect(out).toContain("triangle-alert");
  }
  expect(panel).toContain("18  values.map(drawBar)");
});
