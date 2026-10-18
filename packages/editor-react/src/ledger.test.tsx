// TEST DATA: Juniper ledger entries (Teal, Sam, the file) from the shared fixtures.
import { copy } from "@vellum/config";
import { createHistoryController, FakeHistoryHost } from "@vellum/editor-core";
import { FakeEngineClient } from "@vellum/engine-client";
import { renderToStaticMarkup as html } from "react-dom/server";
import { expect, it } from "vitest";
import { ledgerEntries } from "../fixtures/ledger-scenarios";
import { HistoryTab } from "./ledger/HistoryTab";

const tab = (entries = ledgerEntries()) =>
  html(
    <HistoryTab
      timeZone="UTC"
      history={createHistoryController({
        engine: new FakeEngineClient(),
        device: "device-test",
        host: new FakeHistoryHost(entries),
      })}
    />,
  );

it("an empty ledger says what appears here, and always ends with the footer", () => {
  const out = tab([]);
  expect(out).toContain(copy.historyEmpty);
  expect(out).toContain(copy.historyFooter);
});
it("each entry names its actor, time and size; agents are rounded squares, people circles", () => {
  const out = tab();
  expect(out).toContain("10:43 · 24 changes");
  expect(out).toContain("10:38 · 14 changes applied");
  expect(out).toMatch(/data-actor="agent"[^>]*data-hue="teal"/);
  expect(out).toMatch(/data-actor="person"/);
  expect(out).toContain('aria-label="File"');
});
it("lists newest first", () => {
  const out = tab();
  expect(out.indexOf("Inserted 24 layers")).toBeLessThan(out.indexOf("Resized 3 layers"));
  expect(out.indexOf("Resized 3 layers")).toBeLessThan(out.indexOf("File changed on disk"));
});
